//! Lowering `dive-sema`'s semantic model to the IR (ADR 0013).
//!
//! The traversal is deterministic: modules in dependency order (each file's
//! inner modules before it), the symbols of each module's context in SANY's
//! insertion order, then its assumptions and theorems. Definitions reached
//! only through references (`LET`, `LAMBDA`, definitions outside every
//! context) get ids when they are first reached. Expression nodes are
//! created children first, so every node's children have smaller ids.
//!
//! Scoping follows TLC's evaluation, which looks names up in its context
//! (`tlc2/util/Context.java`, `SymbolNodeValueLookupProvider.lookup`):
//! - parameters, bound variables and zero-arity `LET` definitions are frame
//!   slots, resolved lexically. TLC's lookup is dynamic, but SANY forbids
//!   operator parameters on recursive operators, so an operator closure can
//!   never be applied inside a newer activation of the definition it closes
//!   over, and the two agree;
//! - instance substitutions are dynamic ([`crate::Expr::SubstRef`]);
//! - a name that is not in scope where it is used (only reachable through a
//!   subexpression selector) falls back as TLC's lookup does: a definition is
//!   evaluated by name, anything else is [`crate::Expr::Unbound`].

use crate::effects::Effects;
use crate::expr::{
    Arg, Bound, BuiltinOp, CaseArm, ExceptUpdate, Expr, Field, JunctionKind, LocalRef, OpRef, Pass,
    QuantKind, SubstBinding, Unevaluable,
};
use crate::ids::{ConstId, DeclId, DefId, ExprId, FileId, ModuleId, Slot, VarId};
use crate::origin::{FileInfo, Origin, Positions};
use crate::std_op::{Phase, StdKind, StdOp, StdSupport, is_module_class};
use crate::{
    Decl, Def, DefKind, DefScope, Feature, Ir, Level, Module, Node, Param, Refusal, SlotInfo,
    SlotKind, Symbol,
};
use dive_sema::Analysis;
use dive_sema::model::{
    DeclKind, DefKind as SemaDefKind, Key, Model, NodeId, NodeKind, OpApplData, Subst,
};
use std::collections::{HashMap, HashSet};

/// Why lowering failed. Lowering an accepted analysis only fails on an
/// internal inconsistency, which is a bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    /// What went wrong.
    pub message: String,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LowerError {}

/// Lowers an accepted analysis.
///
/// # Errors
///
/// The analysis was not accepted (SANY would reject the specification), or
/// an internal inconsistency was found.
pub fn lower(analysis: &Analysis) -> Result<Ir, LowerError> {
    if !analysis.accepted() {
        return Err(LowerError {
            message: "the specification was rejected by semantic analysis".to_owned(),
        });
    }
    let model = analysis.model();
    let Some(root) = model.root() else {
        return Err(LowerError {
            message: "semantic analysis produced no root module".to_owned(),
        });
    };
    let mut l = Lowerer::new(analysis, model);
    l.collect(root);
    l.run_queue();
    l.finish()
}

/// A name in scope while lowering a definition.
#[derive(Debug, Clone, Copy)]
enum Binding {
    /// A frame slot.
    Slot { depth: usize, slot: Slot },
    /// A local operator (a `LET` definition with parameters).
    LocalDef(DefId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindKind {
    Bound,
    FcnName,
}

/// The frame being filled.
struct Frame {
    owner: DefId,
    slots: Vec<SlotInfo>,
}

struct Lowerer<'a> {
    analysis: &'a Analysis,
    m: &'a Model,
    positions: Vec<Option<Positions>>,
    files: Vec<FileInfo>,
    modules: Vec<Module>,
    module_ids: HashMap<NodeId, ModuleId>,
    defs: Vec<Option<Def>>,
    def_nodes: Vec<Option<NodeId>>,
    global_defs: HashMap<NodeId, DefId>,
    top_level: HashSet<NodeId>,
    queue: Vec<(DefId, NodeId)>,
    next_in_queue: usize,
    decls: Vec<Decl>,
    decl_ids: HashMap<NodeId, DeclId>,
    vars: Vec<DeclId>,
    consts: Vec<DeclId>,
    substituted: HashSet<NodeId>,
    std_bodies: HashMap<NodeId, StdOp>,
    exprs: Vec<Node>,
    sema_nodes: Vec<u32>,
    refusals: Vec<Refusal>,
    // Scope while lowering one top-level definition.
    frames: Vec<Frame>,
    bindings: Vec<(NodeId, Binding)>,
    instance: Option<DefId>,
    in_std_body: bool,
    root: ModuleId,
}

impl<'a> Lowerer<'a> {
    fn new(analysis: &'a Analysis, m: &'a Model) -> Self {
        let files: Vec<FileInfo> = analysis
            .files()
            .iter()
            .map(|f| FileInfo {
                display: f.display.clone(),
                module: String::new(),
                standard: f.origin == dive_sema::Origin::Standard,
            })
            .collect();
        Self {
            analysis,
            m,
            positions: vec![None; files.len()],
            files,
            modules: Vec::new(),
            module_ids: HashMap::new(),
            defs: Vec::new(),
            def_nodes: Vec::new(),
            global_defs: HashMap::new(),
            top_level: HashSet::new(),
            queue: Vec::new(),
            next_in_queue: 0,
            decls: Vec::new(),
            decl_ids: HashMap::new(),
            vars: Vec::new(),
            consts: Vec::new(),
            substituted: HashSet::new(),
            std_bodies: HashMap::new(),
            exprs: Vec::new(),
            sema_nodes: Vec::new(),
            refusals: Vec::new(),
            frames: Vec::new(),
            bindings: Vec::new(),
            instance: None,
            in_std_body: false,
            root: ModuleId(0),
        }
    }

    // ---- Collection: modules, declarations, top-level definitions --------

    fn collect(&mut self, root: NodeId) {
        // Substitution targets anywhere in the specification.
        for node in self.m.nodes() {
            match &node.kind {
                NodeKind::SubstIn(d) | NodeKind::ApSubstIn(d) => self.mark_substituted(&d.substs),
                NodeKind::Instance(d) => self.mark_substituted(&d.substs),
                _ => {}
            }
        }
        // Modules: file-level in dependency order, each after its inner
        // modules.
        let file_level: Vec<NodeId> = self.m.modules().to_vec();
        for &mid in &file_level {
            self.collect_module(mid, None);
        }
        self.root = self.module_ids.get(&root).copied().unwrap_or(ModuleId(0));
        // File names: the file-level module each file holds.
        for &mid in &file_level {
            if let (Some(span), NodeKind::Module(d)) =
                (self.m.node(mid).span, &self.m.node(mid).kind)
                && let Some(f) = self.files.get_mut(span.file.index())
                && f.module.is_empty()
            {
                self.m.name(d.name).clone_into(&mut f.module);
            }
        }
        // Root state variables, in the root context's order.
        if let NodeKind::Module(d) = &self.m.node(root).kind {
            for (key, node) in self.m.context(d.ctx).entries() {
                if let (Key::Sym(_), NodeKind::OpDecl(od)) = (key, &self.m.node(node).kind)
                    && od.decl == DeclKind::Variable
                    && let Some(&decl) = self.decl_ids.get(&node)
                    && let Some(dd) = self.decls.get_mut(decl.index())
                    && dd.var.is_none()
                {
                    dd.var = Some(VarId::from_index(self.vars.len()));
                    self.vars.push(decl);
                }
            }
        }
        self.match_overrides(&file_level);
    }

    fn mark_substituted(&mut self, substs: &[Subst]) {
        for s in substs {
            self.substituted.insert(s.op);
        }
    }

    fn collect_module(&mut self, mid: NodeId, parent: Option<ModuleId>) {
        if self.module_ids.contains_key(&mid) {
            return;
        }
        let NodeKind::Module(d) = &self.m.node(mid).kind else {
            return;
        };
        // Inner modules first.
        let inner: Vec<NodeId> = self
            .m
            .context(d.ctx)
            .entries()
            .filter_map(|(k, n)| match (k, &self.m.node(n).kind) {
                (Key::Module(_), NodeKind::Module(md)) if md.nesting > d.nesting => Some(n),
                _ => None,
            })
            .collect();
        let id = ModuleId::from_index(self.modules.len());
        self.module_ids.insert(mid, id);
        self.modules.push(Module {
            name: self.m.name(d.name).to_owned(),
            file: self
                .m
                .node(mid)
                .span
                .map(|s| FileId::from_index(s.file.index())),
            nesting: d.nesting,
            parent,
            standard: d.is_standard,
            instantiated: d.is_instantiated,
            extends: Vec::new(),
            context: Vec::new(),
            assumptions: Vec::new(),
            theorems: Vec::new(),
        });
        for n in inner {
            self.collect_module(n, Some(id));
        }
        let extends: Vec<ModuleId> = d
            .extendees
            .iter()
            .filter_map(|e| self.module_ids.get(e).copied())
            .collect();
        let mut context = Vec::new();
        for (key, node) in self.m.context(d.ctx).entries() {
            let Key::Sym(name) = key else { continue };
            let name = self.m.name(name).to_owned();
            let sym = match &self.m.node(node).kind {
                NodeKind::OpDecl(od) => match od.decl {
                    DeclKind::Constant | DeclKind::Variable => Symbol::Decl(self.decl_id(node)),
                    _ => continue,
                },
                NodeKind::OpDef(od) => match od.kind {
                    SemaDefKind::UserDefined => {
                        self.top_level.insert(node);
                        Symbol::Def(self.global_def(node))
                    }
                    SemaDefKind::ModuleInstance => Symbol::Instance,
                    _ => continue,
                },
                NodeKind::ThmOrAssumpDef(_) => {
                    self.top_level.insert(node);
                    Symbol::Def(self.global_def(node))
                }
                _ => continue,
            };
            context.push((name, sym));
        }
        let assumptions: Vec<DefId> = d.assumptions.iter().map(|&a| self.global_def(a)).collect();
        let theorems: Vec<DefId> = d.theorems.iter().map(|&t| self.global_def(t)).collect();
        if let Some(md) = self.modules.get_mut(id.index()) {
            md.extends = extends;
            md.context = context;
            md.assumptions = assumptions;
            md.theorems = theorems;
        }
    }

    fn decl_id(&mut self, node: NodeId) -> DeclId {
        if let Some(&d) = self.decl_ids.get(&node) {
            return d;
        }
        let NodeKind::OpDecl(od) = &self.m.node(node).kind else {
            return DeclId(u32::MAX);
        };
        let id = DeclId::from_index(self.decls.len());
        let variable = od.decl == DeclKind::Variable;
        let constant = if variable {
            None
        } else {
            let c = ConstId::from_index(self.consts.len());
            self.consts.push(id);
            Some(c)
        };
        let module = od
            .module
            .and_then(|m| self.module_ids.get(&m).copied())
            .unwrap_or(self.root);
        let origin = self.origin_of(node);
        self.decls.push(Decl {
            name: self.m.name(od.name).to_owned(),
            module,
            variable,
            arity: u32::try_from(od.arity).unwrap_or(0),
            var: None,
            constant,
            substituted: self.substituted.contains(&node),
            origin,
            level: self.m.level(node).into(),
        });
        self.decl_ids.insert(node, id);
        id
    }

    /// The id of a definition lowered on its own (not in a `LET` scope),
    /// queueing it for lowering.
    fn global_def(&mut self, node: NodeId) -> DefId {
        if let Some(&d) = self.global_defs.get(&node) {
            return d;
        }
        let id = self.alloc_def(Some(node));
        self.global_defs.insert(node, id);
        self.queue.push((id, node));
        id
    }

    fn alloc_def(&mut self, node: Option<NodeId>) -> DefId {
        let id = DefId::from_index(self.defs.len());
        self.defs.push(None);
        self.def_nodes.push(node);
        id
    }

    /// TLC's `processModuleOverrides`, both phases (see [`crate::std_op`]).
    fn match_overrides(&mut self, file_level: &[NodeId]) {
        for &mid in file_level {
            let NodeKind::Module(d) = &self.m.node(mid).kind else {
                continue;
            };
            let mname = self.m.name(d.name);
            if !is_module_class(mname) {
                continue;
            }
            for (key, node) in self.m.context(d.ctx).entries() {
                let Key::Sym(name) = key else { continue };
                if let NodeKind::OpDef(od) = &self.m.node(node).kind
                    && od.kind == SemaDefKind::UserDefined
                    && let Some(op) = StdOp::lookup(mname, self.m.name(name))
                    && op.phase() == Phase::ModuleClass
                    && let Some(body) = od.body
                {
                    self.std_bodies.insert(body, op);
                }
            }
        }
        for &op in StdOp::ALL {
            if op.phase() != Phase::Annotated {
                continue;
            }
            let Some(mid) = file_level.iter().copied().find(|&m| {
                matches!(&self.m.node(m).kind, NodeKind::Module(d) if self.m.name(d.name) == op.module())
            }) else {
                continue;
            };
            let NodeKind::Module(d) = &self.m.node(mid).kind else {
                continue;
            };
            let Some(name) = self.m.lookup_name(op.name()) else {
                continue;
            };
            if let Some(node) = self.m.context(d.ctx).get(Key::Sym(name))
                && let NodeKind::OpDef(od) = &self.m.node(node).kind
                && od.kind == SemaDefKind::UserDefined
                && let Some(body) = od.body
            {
                self.std_bodies.insert(body, op);
            }
        }
    }

    /// The override of a definition: its body, or the body under its
    /// substitution wrappers, carries one (`SymbolNodeValueLookupProvider.lookup`).
    fn std_of(&self, def: NodeId) -> Option<StdOp> {
        let NodeKind::OpDef(od) = &self.m.node(def).kind else {
            return None;
        };
        let mut body = od.body?;
        loop {
            if let Some(&op) = self.std_bodies.get(&body) {
                return Some(op);
            }
            match &self.m.node(body).kind {
                NodeKind::SubstIn(d) => body = d.body?,
                _ => return None,
            }
        }
    }

    // ---- Definitions -----------------------------------------------------

    fn run_queue(&mut self) {
        while let Some(&(id, node)) = self.queue.get(self.next_in_queue) {
            self.next_in_queue += 1;
            self.lower_global(id, node);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per kind of definition SANY has"
    )]
    fn lower_global(&mut self, id: DefId, node: NodeId) {
        let sema = &self.m.node(node).kind;
        let origin = self.origin_of(node);
        let level: Level = self.m.level(node).into();
        let (name, params, body, kind, module, recursive, local, source) = match sema {
            NodeKind::OpDef(od) => {
                let kind = if self.m.name(od.name) == "LAMBDA" {
                    DefKind::Lambda
                } else if od.body.is_some_and(|b| self.is_fcn_spec(b)) {
                    DefKind::Function
                } else {
                    DefKind::Operator
                };
                let source = od
                    .source
                    .filter(|s| self.top_level.contains(s))
                    .map(|s| self.global_def(s));
                (
                    self.m.name(od.name).to_owned(),
                    od.params.clone(),
                    od.body,
                    kind,
                    od.module,
                    od.in_recursive,
                    od.local,
                    source,
                )
            }
            NodeKind::ThmOrAssumpDef(td) => {
                let source = td
                    .source
                    .filter(|s| self.top_level.contains(s))
                    .map(|s| self.global_def(s));
                (
                    self.m.name(td.name).to_owned(),
                    td.params.clone(),
                    td.body,
                    if td.theorem {
                        DefKind::Theorem
                    } else {
                        DefKind::Assumption
                    },
                    td.module,
                    false,
                    td.local,
                    source,
                )
            }
            NodeKind::Assume(ad) => {
                let name = ad
                    .def
                    .and_then(|d| self.m.symbol_name(d))
                    .unwrap_or("")
                    .to_owned();
                let module = self.owning_module(node);
                (
                    name,
                    Vec::new(),
                    ad.expr,
                    DefKind::Assumption,
                    module,
                    false,
                    false,
                    None,
                )
            }
            NodeKind::Theorem(td) => {
                let name = td
                    .def
                    .and_then(|d| self.m.symbol_name(d))
                    .unwrap_or("")
                    .to_owned();
                let module = self.owning_module(node);
                (
                    name,
                    Vec::new(),
                    td.body,
                    DefKind::Theorem,
                    module,
                    false,
                    false,
                    None,
                )
            }
            _ => (
                self.m.symbol_name(node).unwrap_or("").to_owned(),
                Vec::new(),
                None,
                DefKind::Operator,
                None,
                false,
                false,
                None,
            ),
        };
        let module = module
            .and_then(|m| self.module_ids.get(&m).copied())
            .unwrap_or(self.root);
        let std = self.std_of(node);
        self.instance = source.map(|_| id);
        self.in_std_body = std.is_some();
        self.frames.push(Frame {
            owner: id,
            slots: Vec::new(),
        });
        let mut ps = Vec::new();
        for &p in &params {
            ps.push(self.bind_param(p));
        }
        let body = match body {
            Some(b) => self.expr(b),
            None => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
        };
        let frame = self.frames.pop().map(|f| f.slots).unwrap_or_default();
        self.bindings.clear();
        self.instance = None;
        self.in_std_body = false;
        self.set_def(
            id,
            Def {
                name,
                module,
                kind,
                scope: DefScope::TopLevel,
                depth: 0,
                params: ps,
                frame,
                body,
                recursive,
                local,
                std,
                source,
                origin,
                level,
                effects: Effects::NONE,
                hash: 0,
            },
        );
    }

    /// The module an assumption or theorem node belongs to: the first one
    /// that lists it.
    fn owning_module(&self, node: NodeId) -> Option<NodeId> {
        self.module_ids
            .keys()
            .copied()
            .find(|&m| match &self.m.node(m).kind {
                NodeKind::Module(d) => {
                    (d.assumptions.contains(&node) || d.theorems.contains(&node))
                        && self.m.node(m).span.map(|s| s.file)
                            == self.m.node(node).span.map(|s| s.file)
                }
                _ => false,
            })
    }

    fn set_def(&mut self, id: DefId, def: Def) {
        if let Some(slot) = self.defs.get_mut(id.index()) {
            *slot = Some(def);
        }
    }

    fn is_fcn_spec(&self, body: NodeId) -> bool {
        matches!(&self.m.node(body).kind, NodeKind::OpAppl(d)
            if matches!(self.m.symbol_name(d.operator), Some("$NonRecursiveFcnSpec" | "$RecursiveFcnSpec")))
    }

    fn depth(&self) -> usize {
        self.frames.len().saturating_sub(1)
    }

    fn owner(&self) -> DefId {
        self.frames.last().map_or(DefId(u32::MAX), |f| f.owner)
    }

    fn alloc_slot(&mut self, name: String, kind: SlotKind) -> Slot {
        let Some(frame) = self.frames.last_mut() else {
            return Slot(u32::MAX);
        };
        let slot = Slot::from_index(frame.slots.len());
        frame.slots.push(SlotInfo { name, kind });
        slot
    }

    fn param_name(&self, p: NodeId) -> String {
        self.m.symbol_name(p).unwrap_or("_").to_owned()
    }

    fn bind_param(&mut self, p: NodeId) -> Param {
        let name = self.param_name(p);
        let arity = match &self.m.node(p).kind {
            NodeKind::FormalParam(fp) => u32::try_from(fp.arity).unwrap_or(0),
            _ => 0,
        };
        let slot = self.alloc_slot(name.clone(), SlotKind::Param);
        let depth = self.depth();
        self.bindings.push((p, Binding::Slot { depth, slot }));
        Param { name, arity }
    }

    fn bind_bound(&mut self, p: NodeId, kind: BindKind) -> Slot {
        let name = self.param_name(p);
        let slot_kind = if kind == BindKind::FcnName {
            SlotKind::FcnName
        } else {
            SlotKind::Bound
        };
        let slot = self.alloc_slot(name, slot_kind);
        let depth = self.depth();
        self.bindings.push((p, Binding::Slot { depth, slot }));
        slot
    }

    fn lookup_binding(&self, node: NodeId) -> Option<Binding> {
        self.bindings
            .iter()
            .rev()
            .find(|(n, _)| *n == node)
            .map(|(_, b)| *b)
    }

    fn local_ref(&self, depth: usize, slot: Slot) -> LocalRef {
        LocalRef {
            up: u32::try_from(self.depth().saturating_sub(depth)).unwrap_or(u32::MAX),
            slot,
        }
    }

    /// Lowers a local operator in the current scope, with its own frame: a
    /// `LET` definition with parameters where the `LET` is, or (`inline`) a
    /// `LAMBDA` or out-of-scope `LET` definition where it is used.
    fn lower_local_def(&mut self, id: DefId, node: NodeId, inline: bool) {
        let NodeKind::OpDef(od) = &self.m.node(node).kind else {
            return;
        };
        let parent = self.owner();
        let name = self.m.name(od.name).to_owned();
        let kind = if name == "LAMBDA" {
            DefKind::Lambda
        } else if od.body.is_some_and(|b| self.is_fcn_spec(b)) {
            DefKind::Function
        } else {
            DefKind::Operator
        };
        let params = od.params.clone();
        let body = od.body;
        let recursive = od.in_recursive;
        let module = od
            .module
            .and_then(|m| self.module_ids.get(&m).copied())
            .unwrap_or(self.root);
        let origin = self.origin_of(node);
        let level = self.m.level(node).into();
        let std = self.std_of(node);
        let mark = self.bindings.len();
        if inline {
            // A recursive definition refers to itself.
            self.bindings.push((node, Binding::LocalDef(id)));
        }
        self.frames.push(Frame {
            owner: id,
            slots: Vec::new(),
        });
        let depth = self.depth();
        let mut ps = Vec::new();
        for &p in &params {
            ps.push(self.bind_param(p));
        }
        let saved_std = self.in_std_body;
        self.in_std_body = saved_std || std.is_some();
        let body = match body {
            Some(b) => self.expr(b),
            None => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
        };
        self.in_std_body = saved_std;
        let frame = self.frames.pop().map(|f| f.slots).unwrap_or_default();
        self.bindings.truncate(mark);
        self.set_def(
            id,
            Def {
                name,
                module,
                kind,
                scope: if inline {
                    DefScope::Inline { parent }
                } else {
                    DefScope::Local { parent }
                },
                depth: u32::try_from(depth).unwrap_or(u32::MAX),
                params: ps,
                frame,
                body,
                recursive,
                local: false,
                std,
                source: None,
                origin,
                level,
                effects: Effects::NONE,
                hash: 0,
            },
        );
    }

    // ---- Expressions -----------------------------------------------------

    fn origin_of(&mut self, node: NodeId) -> Origin {
        let Some(span) = self.m.node(node).span else {
            return Origin {
                instance: self.instance,
                ..Origin::NONE
            };
        };
        let fi = span.file.index();
        let Some(file) = self.analysis.files().get(fi) else {
            return Origin::NONE;
        };
        if self.positions.get(fi).is_some_and(Option::is_none)
            && let Some(p) = self.positions.get_mut(fi)
        {
            *p = Some(Positions::new(&file.text));
        }
        let start = usize::from(span.range.start());
        let end = usize::from(span.range.end());
        let (begin, endp) = match self.positions.get(fi).and_then(Option::as_ref) {
            Some(p) => (
                p.position(&file.text, start, false),
                p.position(&file.text, end.saturating_sub(1).max(start), true),
            ),
            None => ((0, 0), (0, 0)),
        };
        Origin {
            file: Some(FileId::from_index(fi)),
            range: (
                u32::try_from(start).unwrap_or(u32::MAX),
                u32::try_from(end).unwrap_or(u32::MAX),
            ),
            begin,
            end: endp,
            instance: self.instance,
        }
    }

    fn push(&mut self, expr: Expr, node: NodeId) -> ExprId {
        let origin = self.origin_of(node);
        let level = self.m.level(node).into();
        let id = ExprId::from_index(self.exprs.len());
        self.exprs.push(Node {
            expr,
            origin,
            level,
            effects: Effects::NONE,
        });
        self.sema_nodes
            .push(u32::try_from(node.index()).unwrap_or(u32::MAX));
        id
    }

    fn expr_of(&self, id: ExprId) -> Option<&Expr> {
        self.exprs.get(id.index()).map(|n| &n.expr)
    }

    fn refuse(&mut self, op: StdOp, expr: ExprId) {
        if self.in_std_body || op.support() == StdSupport::Core {
            return;
        }
        let def = self.owner();
        self.refusals.push(Refusal {
            feature: Feature::StdOp(op),
            expr,
            def,
        });
    }

    #[allow(clippy::too_many_lines, reason = "one arm per kind of semantic node")]
    fn expr(&mut self, node: NodeId) -> ExprId {
        let kind = &self.m.node(node).kind;
        match kind {
            NodeKind::OpAppl(d) => self.appl(node, d),
            NodeKind::Numeral(t) => self.push(Expr::Num(t.as_str().into()), node),
            NodeKind::Decimal(a, b) => self.push(
                Expr::Decimal {
                    int: a.as_str().into(),
                    frac: b.as_str().into(),
                },
                node,
            ),
            NodeKind::Str(s) => self.push(Expr::Str(s.as_str().into()), node),
            NodeKind::At { .. } => self.push(Expr::ExceptAt, node),
            NodeKind::Label(d) => {
                let name = self.m.name(d.name).to_owned();
                let body = match d.body {
                    Some(b) => self.expr(b),
                    None => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
                };
                self.push(
                    Expr::Label {
                        name: name.into(),
                        body,
                    },
                    node,
                )
            }
            NodeKind::LetIn(d) => {
                let defs = d.defs.clone();
                let body = d.body;
                self.let_in(node, &defs, body)
            }
            NodeKind::SubstIn(d) | NodeKind::ApSubstIn(d) => {
                let substs = d.substs.clone();
                let body = d.body;
                let mut bindings = Vec::new();
                for s in &substs {
                    let decl = self.decl_id(s.op);
                    let value = self.arg(s.expr, true);
                    bindings.push(SubstBinding { decl, value });
                }
                let body = match body {
                    Some(b) => self.expr(b),
                    None => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
                };
                self.push(
                    Expr::Subst {
                        bindings: bindings.into(),
                        body,
                    },
                    node,
                )
            }
            NodeKind::OpArg(d) => {
                // An operator where an expression is expected; SANY only
                // produces these as arguments, which [`Self::arg`] handles.
                let name = self.m.symbol_name(d.op).unwrap_or("?").to_owned();
                self.push(Expr::Unbound(name.into()), node)
            }
            NodeKind::AssumeProve(_) => {
                self.push(Expr::Unevaluable(Unevaluable::AssumeProve), node)
            }
            _ => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
        }
    }

    fn opt_expr(&mut self, op: Option<&Option<NodeId>>, at: NodeId) -> ExprId {
        match op.copied().flatten() {
            Some(n) => self.expr(n),
            None => self.push(Expr::Unevaluable(Unevaluable::Proof), at),
        }
    }

    fn let_in(&mut self, node: NodeId, defs: &[NodeId], body: Option<NodeId>) -> ExprId {
        let mark = self.bindings.len();
        // Operators with parameters are visible in every definition of the
        // LET (RECURSIVE ones even in their own), so they get ids first.
        let mut local_ids = HashMap::new();
        for &d in defs {
            if let NodeKind::OpDef(od) = &self.m.node(d).kind
                && od.kind == SemaDefKind::UserDefined
                && !od.params.is_empty()
            {
                let id = self.alloc_def(Some(d));
                local_ids.insert(d, id);
                self.bindings.push((d, Binding::LocalDef(id)));
            }
        }
        let mut out = Vec::new();
        for &d in defs {
            let NodeKind::OpDef(od) = &self.m.node(d).kind else {
                continue;
            };
            if od.kind != SemaDefKind::UserDefined {
                continue;
            }
            if let Some(&id) = local_ids.get(&d) {
                self.lower_local_def(id, d, false);
                out.push(id);
                continue;
            }
            // A zero-arity definition: a thunk in a slot of this frame.
            let id = self.alloc_def(Some(d));
            let name = self.m.name(od.name).to_owned();
            let body_node = od.body;
            let recursive = od.in_recursive;
            let kind = if body_node.is_some_and(|b| self.is_fcn_spec(b)) {
                DefKind::Function
            } else {
                DefKind::Operator
            };
            let module = od
                .module
                .and_then(|m| self.module_ids.get(&m).copied())
                .unwrap_or(self.root);
            let std = self.std_of(d);
            let slot = self.alloc_slot(name.clone(), SlotKind::LetValue(id));
            let saved_std = self.in_std_body;
            self.in_std_body = saved_std || std.is_some();
            let body = match body_node {
                Some(b) => self.expr(b),
                None => self.push(Expr::Unevaluable(Unevaluable::Proof), d),
            };
            self.in_std_body = saved_std;
            let pass = if self.is_strict_shape(body) {
                Pass::Strict
            } else {
                Pass::Lazy
            };
            let depth = self.depth();
            self.bindings.push((d, Binding::Slot { depth, slot }));
            let origin = self.origin_of(d);
            let level = self.m.level(d).into();
            self.set_def(
                id,
                Def {
                    name,
                    module,
                    kind,
                    scope: DefScope::LetValue {
                        parent: self.owner(),
                        slot,
                        pass,
                    },
                    depth: u32::try_from(depth).unwrap_or(u32::MAX),
                    params: Vec::new(),
                    frame: Vec::new(),
                    body,
                    recursive,
                    local: false,
                    std,
                    source: None,
                    origin,
                    level,
                    effects: Effects::NONE,
                    hash: 0,
                },
            );
            out.push(id);
        }
        let body = match body {
            Some(b) => self.expr(b),
            None => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
        };
        self.bindings.truncate(mark);
        self.push(
            Expr::Let {
                defs: out.into(),
                body,
            },
            node,
        )
    }

    /// Whether an expression is pure and cannot fail by its shape alone: a
    /// literal, a bound variable, or a tuple or record of these. Such a
    /// laziness point may be evaluated eagerly (ADR 0013 §3).
    fn is_strict_shape(&self, id: ExprId) -> bool {
        match self.expr_of(id) {
            Some(Expr::Bool(_) | Expr::Num(_) | Expr::Str(_)) => true,
            Some(Expr::Local(r)) => self.slot_is_value(*r),
            Some(Expr::Tuple(items)) => items.iter().all(|&i| self.is_strict_shape(i)),
            Some(Expr::Record(fields)) => fields.iter().all(|f| self.is_strict_shape(f.value)),
            _ => false,
        }
    }

    /// Whether a slot holds a value (a bound variable or a recursive
    /// function's name) rather than a thunk.
    fn slot_is_value(&self, r: LocalRef) -> bool {
        let up = r.up as usize;
        let Some(frame) = self
            .frames
            .len()
            .checked_sub(1 + up)
            .and_then(|i| self.frames.get(i))
        else {
            return false;
        };
        matches!(
            frame.slots.get(r.slot.index()).map(|s| &s.kind),
            Some(SlotKind::Bound | SlotKind::FcnName)
        )
    }

    fn arg(&mut self, node: NodeId, lazy: bool) -> Arg {
        if let NodeKind::OpArg(d) = &self.m.node(node).kind {
            let op = d.op;
            let r = self.op_ref(op, node);
            if let OpRef::Std(s) = r {
                let e = self.push(Expr::Unbound(s.qualified_name().into()), node);
                // Record the refusal against a node at the argument's place.
                self.refuse(s, e);
                self.exprs.pop();
                self.sema_nodes.pop();
                if let Some(r) = self.refusals.last_mut()
                    && r.expr == e
                {
                    r.expr = ExprId(u32::MAX);
                }
            }
            let level = self.m.level(node).into();
            return Arg::Op { op: r, level };
        }
        let expr = self.expr(node);
        let pass = if !lazy {
            Pass::Direct
        } else if self.is_strict_shape(expr) {
            Pass::Strict
        } else {
            Pass::Lazy
        };
        Arg::Expr { expr, pass }
    }

    /// The operator an `OpArg` or a substitution passes.
    fn op_ref(&mut self, op: NodeId, at: NodeId) -> OpRef {
        match &self.m.node(op).kind {
            NodeKind::OpDef(od) => match od.kind {
                SemaDefKind::BuiltIn => {
                    let name = self.m.name(od.name);
                    BuiltinOp::from_sany_name(name)
                        .map_or_else(|| OpRef::Unbound(name.into()), OpRef::Builtin)
                }
                SemaDefKind::UserDefined => self.user_op_ref(op, at),
                _ => OpRef::Unbound(self.m.name(od.name).into()),
            },
            NodeKind::ThmOrAssumpDef(_) => OpRef::Def(self.global_def(op)),
            NodeKind::OpDecl(_) => self.decl_op_ref(op),
            NodeKind::FormalParam(_) => match self.lookup_binding(op) {
                Some(Binding::Slot { depth, slot, .. }) => {
                    OpRef::Param(self.local_ref(depth, slot))
                }
                Some(Binding::LocalDef(id)) => OpRef::Def(id),
                None => OpRef::Unbound(self.param_name(op).into()),
            },
            _ => OpRef::Unbound(self.m.symbol_name(op).unwrap_or("?").into()),
        }
    }

    fn user_op_ref(&mut self, op: NodeId, at: NodeId) -> OpRef {
        match self.lookup_binding(op) {
            Some(Binding::LocalDef(id)) => return OpRef::Def(id),
            Some(Binding::Slot { .. }) => {
                // A zero-arity definition passed as an operator: impossible
                // in SANY's arity rules.
                return OpRef::Unbound(self.m.symbol_name(op).unwrap_or("?").into());
            }
            None => {}
        }
        if let Some(s) = self.std_of(op) {
            return OpRef::Std(s);
        }
        if self.is_inline(op) {
            let id = self.alloc_def(Some(op));
            self.lower_local_def(id, op, true);
            let _ = at;
            return OpRef::Def(id);
        }
        OpRef::Def(self.global_def(op))
    }

    /// A user definition that belongs to no module context and is not in
    /// scope: a `LAMBDA` (SANY creates one afresh at each occurrence,
    /// including for subexpression selectors with parameters), or a `LET`
    /// definition reached through a selector from outside its `LET`. TLC
    /// finds neither in its context and evaluates the definition's body in
    /// the context of the use (`Tool.evalAppl`, `getOpContext`), so it is
    /// lowered there ([`crate::DefScope::Inline`]).
    fn is_inline(&self, op: NodeId) -> bool {
        !self.top_level.contains(&op)
            && matches!(&self.m.node(op).kind, NodeKind::OpDef(od) if od.kind == SemaDefKind::UserDefined)
    }

    fn decl_op_ref(&mut self, op: NodeId) -> OpRef {
        let Some(&decl) = self.decl_ids.get(&op) else {
            return OpRef::Unbound(self.m.symbol_name(op).unwrap_or("?").into());
        };
        let d = &self.decls[decl.index()];
        if d.substituted {
            return OpRef::Subst(decl);
        }
        match d.constant {
            Some(c) => OpRef::Const(c),
            None => OpRef::Subst(decl),
        }
    }

    fn args(&mut self, operands: &[Option<NodeId>], lazy: bool, at: NodeId) -> Vec<Arg> {
        operands
            .iter()
            .map(|o| match o {
                Some(n) => self.arg(*n, lazy),
                None => Arg::Expr {
                    expr: self.push(Expr::Unevaluable(Unevaluable::Proof), at),
                    pass: Pass::Direct,
                },
            })
            .collect()
    }

    fn appl(&mut self, node: NodeId, d: &OpApplData) -> ExprId {
        let op = d.operator;
        match &self.m.node(op).kind {
            NodeKind::OpDef(od) => match od.kind {
                SemaDefKind::BuiltIn => {
                    let name = self.m.name(od.name).to_owned();
                    self.builtin(node, &name, d)
                }
                SemaDefKind::UserDefined => self.user_appl(node, op, &d.operands),
                _ => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
            },
            NodeKind::ThmOrAssumpDef(_) => {
                let id = self.global_def(op);
                if d.operands.is_empty() {
                    self.push(Expr::Def(id), node)
                } else {
                    let args = self.args(&d.operands, true, node);
                    self.push(
                        Expr::Apply {
                            callee: OpRef::Def(id),
                            args: args.into(),
                        },
                        node,
                    )
                }
            }
            NodeKind::OpDecl(_) => self.decl_appl(node, op, &d.operands),
            NodeKind::FormalParam(_) => self.param_appl(node, op, &d.operands),
            _ => self.push(Expr::Unevaluable(Unevaluable::Proof), node),
        }
    }

    fn user_appl(&mut self, node: NodeId, op: NodeId, operands: &[Option<NodeId>]) -> ExprId {
        match self.lookup_binding(op) {
            Some(Binding::Slot { depth, slot, .. }) => {
                let r = self.local_ref(depth, slot);
                return self.push(Expr::Local(r), node);
            }
            Some(Binding::LocalDef(id)) => {
                let args = self.args(operands, true, node);
                return self.push(
                    Expr::Apply {
                        callee: OpRef::Def(id),
                        args: args.into(),
                    },
                    node,
                );
            }
            None => {}
        }
        if let Some(s) = self.std_of(op) {
            let args = self.args(operands, s.kind() == StdKind::Evaluating, node);
            let args: Vec<Arg> = args
                .into_iter()
                .map(|a| match a {
                    Arg::Expr { expr, .. } => Arg::Expr {
                        expr,
                        pass: Pass::Direct,
                    },
                    other @ Arg::Op { .. } => other,
                })
                .collect();
            let e = self.push(
                Expr::Apply {
                    callee: OpRef::Std(s),
                    args: args.into(),
                },
                node,
            );
            self.refuse(s, e);
            return e;
        }
        let callee = if self.is_inline(op) {
            let id = self.alloc_def(Some(op));
            self.lower_local_def(id, op, true);
            id
        } else {
            self.global_def(op)
        };
        if operands.is_empty() {
            return self.push(Expr::Def(callee), node);
        }
        let args = self.args(operands, true, node);
        self.push(
            Expr::Apply {
                callee: OpRef::Def(callee),
                args: args.into(),
            },
            node,
        )
    }

    fn decl_appl(&mut self, node: NodeId, op: NodeId, operands: &[Option<NodeId>]) -> ExprId {
        let Some(&decl) = self.decl_ids.get(&op) else {
            let name = self.m.symbol_name(op).unwrap_or("?").to_owned();
            return self.push(Expr::Unbound(name.into()), node);
        };
        let (substituted, var, constant) = {
            let d = &self.decls[decl.index()];
            (d.substituted, d.var, d.constant)
        };
        if operands.is_empty() {
            let e = if substituted {
                Expr::SubstRef {
                    decl,
                    primed: false,
                }
            } else if let Some(v) = var {
                Expr::Var {
                    var: v,
                    primed: false,
                }
            } else if let Some(c) = constant {
                Expr::Const(c)
            } else {
                Expr::SubstRef {
                    decl,
                    primed: false,
                }
            };
            return self.push(e, node);
        }
        let callee = self.decl_op_ref(op);
        let args = self.args(operands, true, node);
        self.push(
            Expr::Apply {
                callee,
                args: args.into(),
            },
            node,
        )
    }

    fn param_appl(&mut self, node: NodeId, op: NodeId, operands: &[Option<NodeId>]) -> ExprId {
        let callee = match self.lookup_binding(op) {
            Some(Binding::Slot { depth, slot, .. }) => {
                let r = self.local_ref(depth, slot);
                if operands.is_empty() {
                    return self.push(Expr::Local(r), node);
                }
                OpRef::Param(r)
            }
            Some(Binding::LocalDef(id)) => OpRef::Def(id),
            None => {
                let name = self.param_name(op);
                if operands.is_empty() {
                    return self.push(Expr::Unbound(name.into()), node);
                }
                OpRef::Unbound(name.into())
            }
        };
        let args = self.args(operands, true, node);
        self.push(
            Expr::Apply {
                callee,
                args: args.into(),
            },
            node,
        )
    }

    fn operand(&mut self, d: &OpApplData, i: usize, at: NodeId) -> ExprId {
        self.opt_expr(d.operands.get(i), at)
    }

    fn operands(&mut self, d: &OpApplData, at: NodeId) -> Box<[ExprId]> {
        let ops = d.operands.clone();
        ops.iter().map(|o| self.opt_expr(Some(o), at)).collect()
    }

    /// Lowers bound groups: domains first, outside the binders' scope, then
    /// the variables (`Generator.processQuantBounds`).
    fn bounds(&mut self, d: &OpApplData) -> Vec<Bound> {
        let groups = d.bounded.clone().unwrap_or_default();
        let tuples = d.tuples.clone().unwrap_or_default();
        let ranges = d.ranges.clone();
        let domains: Vec<Option<ExprId>> = ranges.iter().map(|&r| Some(self.expr(r))).collect();
        groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let vars: Box<[Slot]> = g
                    .iter()
                    .map(|&p| self.bind_bound(p, BindKind::Bound))
                    .collect();
                Bound {
                    vars,
                    tuple: tuples.get(i).copied().unwrap_or(false),
                    domain: domains.get(i).copied().flatten(),
                }
            })
            .collect()
    }

    fn unbounded(&mut self, d: &OpApplData) -> Box<[Slot]> {
        d.unbounded
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|&p| self.bind_bound(p, BindKind::Bound))
            .collect()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per SANY built-in, as in Tool.evalAppl"
    )]
    fn builtin(&mut self, node: NodeId, name: &str, d: &OpApplData) -> ExprId {
        let mark = self.bindings.len();
        let e = match name {
            "TRUE" => Expr::Bool(true),
            "FALSE" => Expr::Bool(false),
            "\\lnot" => Expr::Not(self.operand(d, 0, node)),
            "\\land" | "\\lor" => {
                let items = self.operands(d, node);
                Expr::Junction {
                    kind: if name == "\\land" {
                        JunctionKind::And
                    } else {
                        JunctionKind::Or
                    },
                    bulleted: false,
                    items,
                }
            }
            "$ConjList" | "$DisjList" => {
                let items = self.operands(d, node);
                Expr::Junction {
                    kind: if name == "$ConjList" {
                        JunctionKind::And
                    } else {
                        JunctionKind::Or
                    },
                    bulleted: true,
                    items,
                }
            }
            "=>" => {
                let a = self.operand(d, 0, node);
                let b = self.operand(d, 1, node);
                Expr::Implies(a, b)
            }
            "\\equiv" => {
                let a = self.operand(d, 0, node);
                let b = self.operand(d, 1, node);
                Expr::Equiv(a, b)
            }
            "'" => {
                let inner = self.operand(d, 0, node);
                let primed = match self.expr_of(inner) {
                    Some(Expr::Var { var, primed: false }) => Some(Expr::Var {
                        var: *var,
                        primed: true,
                    }),
                    Some(Expr::SubstRef {
                        decl,
                        primed: false,
                    }) => Some(Expr::SubstRef {
                        decl: *decl,
                        primed: true,
                    }),
                    _ => None,
                };
                match primed {
                    Some(p) => {
                        // Replace the operand with its primed form, keeping
                        // the `x'` node's origin and level.
                        let origin = self.origin_of(node);
                        let level = self.m.level(node).into();
                        if let Some(n) = self.exprs.get_mut(inner.index()) {
                            n.expr = p;
                            n.origin = origin;
                            n.level = level;
                        }
                        if let Some(s) = self.sema_nodes.get_mut(inner.index()) {
                            *s = u32::try_from(node.index()).unwrap_or(u32::MAX);
                        }
                        return inner;
                    }
                    None => Expr::Prime(inner),
                }
            }
            "ENABLED" => Expr::Enabled(self.operand(d, 0, node)),
            "UNCHANGED" => Expr::Unchanged(self.operand(d, 0, node)),
            "\\cdot" => {
                let a = self.operand(d, 0, node);
                let b = self.operand(d, 1, node);
                Expr::Cdot(a, b)
            }
            "[]" => Expr::Always(self.operand(d, 0, node)),
            "<>" => Expr::Eventually(self.operand(d, 0, node)),
            "~>" => {
                let a = self.operand(d, 0, node);
                let b = self.operand(d, 1, node);
                Expr::LeadsTo(a, b)
            }
            "-+->" | "$TemporalWhile" => {
                let a = self.operand(d, 0, node);
                let b = self.operand(d, 1, node);
                Expr::WhilePlus(a, b)
            }
            "$AngleAct" | "$SquareAct" => {
                let action = self.operand(d, 0, node);
                let sub = self.operand(d, 1, node);
                if name == "$AngleAct" {
                    Expr::StepAngle { action, sub }
                } else {
                    Expr::StepBox { action, sub }
                }
            }
            "$WF" | "$SF" => {
                let sub = self.operand(d, 0, node);
                let action = self.operand(d, 1, node);
                if name == "$WF" {
                    Expr::Wf { sub, action }
                } else {
                    Expr::Sf { sub, action }
                }
            }
            "$BoundedChoose" | "$BoundedExists" | "$BoundedForall" => {
                let bounds = self.bounds(d);
                let body = self.operand(d, 0, node);
                Expr::Quant {
                    kind: match name {
                        "$BoundedChoose" => QuantKind::Choose,
                        "$BoundedExists" => QuantKind::Exists,
                        _ => QuantKind::Forall,
                    },
                    bounds: bounds.into(),
                    body,
                }
            }
            "$UnboundedChoose" | "$UnboundedExists" | "$UnboundedForall" => {
                let vars = self.unbounded(d);
                let body = self.operand(d, 0, node);
                Expr::Quant {
                    kind: match name {
                        "$UnboundedChoose" => QuantKind::Choose,
                        "$UnboundedExists" => QuantKind::Exists,
                        _ => QuantKind::Forall,
                    },
                    bounds: vec![Bound {
                        vars,
                        tuple: false,
                        domain: None,
                    }]
                    .into(),
                    body,
                }
            }
            "$TemporalExists" | "$TemporalForall" => {
                let vars = self.unbounded(d);
                let body = self.operand(d, 0, node);
                Expr::TemporalQuant {
                    exists: name == "$TemporalExists",
                    vars,
                    body,
                }
            }
            "$CartesianProd" => Expr::Cartesian(self.operands(d, node)),
            "$SetEnumerate" => Expr::SetEnum(self.operands(d, node)),
            "$Tuple" => Expr::Tuple(self.operands(d, node)),
            "$Case" => {
                let mut arms = Vec::new();
                let mut other = None;
                for o in d.operands.clone().iter().flatten() {
                    let NodeKind::OpAppl(p) = &self.m.node(*o).kind else {
                        continue;
                    };
                    let guard = p.operands.first().copied().flatten();
                    let value = self.opt_expr(p.operands.get(1), *o);
                    match guard {
                        Some(g) => {
                            let guard = self.expr(g);
                            arms.push(CaseArm { guard, value });
                        }
                        None => other = Some(value),
                    }
                }
                Expr::Case {
                    arms: arms.into(),
                    other,
                }
            }
            "$Except" => {
                let base = self.operand(d, 0, node);
                let mut updates = Vec::new();
                for o in d.operands.clone().iter().skip(1).flatten() {
                    let NodeKind::OpAppl(p) = &self.m.node(*o).kind else {
                        continue;
                    };
                    let mut path = Vec::new();
                    if let Some(Some(seq)) = p.operands.first()
                        && let NodeKind::OpAppl(s) = &self.m.node(*seq).kind
                    {
                        for c in s.operands.clone().iter().flatten() {
                            path.push(self.expr(*c));
                        }
                    }
                    let value = self.opt_expr(p.operands.get(1), *o);
                    updates.push(ExceptUpdate {
                        path: path.into(),
                        value,
                    });
                }
                Expr::Except {
                    base,
                    updates: updates.into(),
                }
            }
            "$FcnApply" => {
                let fcn = self.operand(d, 0, node);
                let arg = self.operand(d, 1, node);
                Expr::FcnApply { fcn, arg }
            }
            "$FcnConstructor" | "$NonRecursiveFcnSpec" => {
                let bounds = self.bounds(d);
                let body = self.operand(d, 0, node);
                Expr::FcnCons {
                    bounds: bounds.into(),
                    body,
                }
            }
            "$RecursiveFcnSpec" => {
                let fname = d.unbounded.as_ref().and_then(|u| u.first()).copied();
                let bounds = self.bounds(d);
                let name = match fname {
                    Some(f) => self.bind_bound(f, BindKind::FcnName),
                    None => Slot(u32::MAX),
                };
                let body = self.operand(d, 0, node);
                Expr::RecFcn {
                    name,
                    bounds: bounds.into(),
                    body,
                }
            }
            "$IfThenElse" => {
                let cond = self.operand(d, 0, node);
                let then = self.operand(d, 1, node);
                let other = self.operand(d, 2, node);
                Expr::If { cond, then, other }
            }
            "$RcdConstructor" | "$SetOfRcds" => {
                let mut fields = Vec::new();
                for o in d.operands.clone().iter().flatten() {
                    let NodeKind::OpAppl(p) = &self.m.node(*o).kind else {
                        continue;
                    };
                    let fname = match p
                        .operands
                        .first()
                        .copied()
                        .flatten()
                        .map(|n| &self.m.node(n).kind)
                    {
                        Some(NodeKind::Str(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let value = self.opt_expr(p.operands.get(1), *o);
                    fields.push(Field {
                        name: fname.into(),
                        value,
                    });
                }
                if name == "$RcdConstructor" {
                    Expr::Record(fields.into())
                } else {
                    Expr::RecordSet(fields.into())
                }
            }
            "$RcdSelect" => {
                let record = self.operand(d, 0, node);
                if let Some(NodeKind::Str(s)) = d
                    .operands
                    .get(1)
                    .copied()
                    .flatten()
                    .map(|n| &self.m.node(n).kind)
                {
                    Expr::RecordSelect {
                        record,
                        field: s.as_str().into(),
                    }
                } else {
                    let arg = self.operand(d, 1, node);
                    Expr::FcnApply { fcn: record, arg }
                }
            }
            "$SetOfAll" => {
                let bounds = self.bounds(d);
                let expr = self.operand(d, 0, node);
                Expr::SetMap {
                    expr,
                    bounds: bounds.into(),
                }
            }
            "$SetOfFcns" => {
                let domain = self.operand(d, 0, node);
                let range = self.operand(d, 1, node);
                Expr::FcnSet { domain, range }
            }
            "$SubsetOf" => {
                let mut bounds = self.bounds(d);
                let pred = self.operand(d, 0, node);
                let bound = if bounds.is_empty() {
                    Bound {
                        vars: Box::new([]),
                        tuple: false,
                        domain: None,
                    }
                } else {
                    bounds.swap_remove(0)
                };
                Expr::SetFilter { bound, pred }
            }
            "$Nop" => Expr::Selected(self.operand(d, 0, node)),
            _ => {
                if let Some(op) = BuiltinOp::from_sany_name(name) {
                    let args = self.operands(d, node);
                    Expr::Builtin { op, args }
                } else {
                    // `$Pair`, `$Seq` and the proof pseudo-operators.
                    Expr::Unevaluable(Unevaluable::Proof)
                }
            }
        };
        self.bindings.truncate(mark);
        self.push(e, node)
    }

    // ---- Finishing: effects and hashes -------------------------------------

    fn finish(mut self) -> Result<Ir, LowerError> {
        let mut defs = Vec::with_capacity(self.defs.len());
        for (i, d) in std::mem::take(&mut self.defs).into_iter().enumerate() {
            match d {
                Some(d) => defs.push(d),
                None => {
                    return Err(LowerError {
                        message: format!(
                            "internal: definition {i} ({:?}) was allocated but never lowered",
                            self.def_nodes
                                .get(i)
                                .copied()
                                .flatten()
                                .and_then(|n| self.m.symbol_name(n))
                        ),
                    });
                }
            }
        }
        let mut ir = Ir {
            files: self.files,
            modules: self.modules,
            root: self.root,
            defs,
            exprs: self.exprs,
            decls: self.decls,
            vars: self.vars,
            consts: self.consts,
            refusals: self.refusals,
            sema_nodes: self.sema_nodes,
        };
        crate::analysis::compute_effects(&mut ir);
        crate::analysis::compute_hashes(&mut ir);
        Ok(ir)
    }
}
