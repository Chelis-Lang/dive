//! Semantic graph generation: a port of SANY's `tla2sany/semantic/Generator.java`
//! and the symbol-table classes it uses (`SymbolTable`, `Context`), working
//! on `dive-syntax`'s concrete syntax tree.
//!
//! The port follows SANY's semantics check by check, so that a module is
//! rejected exactly when SANY rejects it. Method names in the doc comments
//! name the Java method each function matches. Where SANY throws an
//! `AbortException` (it stops analysing), the port returns [`Abort`].
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::single_match_else,
    clippy::comparison_chain,
    clippy::type_complexity,
    reason = "functions mirror SANY's Generator methods one to one, so they can be compared"
)]

mod cst;
mod expr;
mod inst;
mod proof;
mod select;

use crate::diagnostics::{Code, Diagnostic, FileId, Span};
use crate::model::{
    CtxId, DeclKind, DefKind, FormalParamData, Key, Kind, LabelsId, Model, ModuleData, Name,
    NodeId, NodeKind, OpApplData, OpDeclData, OpDefData, OpLevels, ThmDefData,
};
use cst::{nodes, toks};
use dive_syntax::ast::AstNode as _;
use dive_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken};
use std::collections::{BTreeMap, HashMap};

/// SANY threw an `AbortException`: analysis of the specification stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Abort;

pub(crate) type GResult<T> = Result<T, Abort>;

/// SANY's `MaxLetInLevel`.
const MAX_LET_IN_LEVEL: usize = 100;
/// SANY's `maxAPDepth`.
const MAX_AP_DEPTH: usize = 100;

/// The symbol table: a stack of contexts, top last (SANY's `SymbolTable`).
#[derive(Debug, Clone, Default)]
pub(crate) struct SymTab {
    stack: Vec<CtxId>,
}

/// State shared by the analysis of all modules of one specification.
pub(crate) struct Shared {
    /// SANY's `ExternalModuleTable`: module name to module node and context.
    pub(crate) ext: HashMap<Name, (NodeId, CtxId)>,
    /// The built-in operators' context (`Context.initialContext`).
    pub(crate) global: CtxId,
    /// SANY's `InAssumeDummyNode`.
    pub(crate) in_assume: NodeId,
    /// SANY's `nullOAN` and friends: one placeholder node.
    pub(crate) null: NodeId,
    /// Number of errors reported so far (SANY's `errors.getNumErrors()`).
    pub(crate) errors: usize,
}

/// The generator for one external module (SANY creates one per module).
pub(crate) struct Gen<'a> {
    pub(crate) m: &'a mut Model,
    pub(crate) diags: &'a mut Vec<Diagnostic>,
    pub(crate) sh: &'a mut Shared,
    file: FileId,
    st: SymTab,
    /// SANY's `context` field: the context new modules are given.
    context: CtxId,
    exc_stack: Vec<NodeId>,
    exc_spec_stack: Vec<NodeId>,
    functions: Vec<(Name, NodeId)>,
    ls_labels: Vec<Option<BTreeMap<Name, NodeId>>>,
    ls_params: Vec<Vec<Vec<NodeId>>>,
    ap_depth: usize,
    current_goal: Option<NodeId>,
    current_goal_clause: i32,
    in_scope_of_ap_decl: Vec<bool>,
    cur_level: usize,
    unresolved_cnt: Vec<i32>,
    unresolved_sum: i32,
    recursive_section_count: i32,
    nesting: i32,
    /// SANY's parser state for canonical step numbers (`proofLevelStack`).
    proof_levels: Vec<i32>,
    proof_depth: i32,
}

impl<'a> Gen<'a> {
    pub(crate) fn new(
        m: &'a mut Model,
        diags: &'a mut Vec<Diagnostic>,
        sh: &'a mut Shared,
        file: FileId,
    ) -> Self {
        // SymbolTable(mt, errs): a duplicate of the global context.
        let base = m.new_context();
        let global = sh.global;
        let entries: Vec<(Key, NodeId)> = m.context(global).entries().collect();
        if let Some(c) = m.ctx_mut(base) {
            for (k, v) in entries {
                c.put(k, v);
            }
        }
        Self {
            m,
            diags,
            sh,
            file,
            st: SymTab { stack: vec![base] },
            context: base,
            exc_stack: Vec::new(),
            exc_spec_stack: Vec::new(),
            functions: Vec::new(),
            ls_labels: Vec::new(),
            ls_params: Vec::new(),
            ap_depth: 0,
            current_goal: None,
            current_goal_clause: 0,
            in_scope_of_ap_decl: vec![false; MAX_AP_DEPTH + 1],
            cur_level: 0,
            unresolved_cnt: vec![0; MAX_LET_IN_LEVEL + 1],
            unresolved_sum: 0,
            recursive_section_count: 0,
            nesting: -1,
            proof_levels: vec![-1; 128],
            proof_depth: -1,
        }
    }

    /// The module's top-level context (`getSymbolTable().getExternalContext()`).
    pub(crate) fn external_context(&self) -> CtxId {
        self.st.stack.first().copied().unwrap_or(self.context)
    }

    /// `Generator.generate`.
    pub(crate) fn generate(&mut self, module: &SyntaxNode) -> GResult<NodeId> {
        self.context = self.top();
        self.generate_module(module, None)
    }

    // ---- Diagnostics --------------------------------------------------------

    #[allow(clippy::unnecessary_wraps, reason = "diagnostic spans are optional")]
    pub(crate) fn sp(&self, n: &SyntaxNode) -> Option<Span> {
        Some(Span {
            file: self.file,
            range: n.text_range(),
        })
    }

    #[allow(clippy::unnecessary_wraps, reason = "diagnostic spans are optional")]
    pub(crate) fn spt(&self, t: &SyntaxToken) -> Option<Span> {
        Some(Span {
            file: self.file,
            range: t.text_range(),
        })
    }

    pub(crate) fn report(&mut self, d: Diagnostic) {
        if d.is_error() {
            self.sh.errors += 1;
        }
        self.diags.push(d);
    }

    pub(crate) fn err(&mut self, code: Code, at: Option<Span>, msg: impl Into<String>) {
        self.report(Diagnostic::new(code, at, msg));
    }

    pub(crate) fn err_help(
        &mut self,
        code: Code,
        at: Option<Span>,
        msg: impl Into<String>,
        help: impl Into<String>,
    ) {
        self.report(Diagnostic::new(code, at, msg).with_help(help));
    }

    /// Reports an error and aborts, as SANY's `throw errors.addMessage(..)`.
    pub(crate) fn abort(&mut self, code: Code, at: Option<Span>, msg: impl Into<String>) -> Abort {
        self.err(code, at, msg);
        Abort
    }

    // ---- Names and node queries --------------------------------------------

    pub(crate) fn intern(&mut self, s: &str) -> Name {
        self.m.names.intern(s)
    }

    pub(crate) fn text(&self, n: Name) -> String {
        self.m.name(n).to_owned()
    }

    pub(crate) fn kind(&self, id: NodeId) -> Kind {
        self.m.kind(id)
    }

    pub(crate) fn null(&self) -> NodeId {
        self.sh.null
    }

    /// `SymbolNode.getArity` (and `OpArgNode.getArity`, `LabelNode.getArity`).
    pub(crate) fn arity(&self, id: NodeId) -> i32 {
        match &self.m.node(id).kind {
            NodeKind::OpDecl(d) => d.arity,
            NodeKind::FormalParam(d) => d.arity,
            NodeKind::OpDef(d) => d.arity,
            NodeKind::ThmOrAssumpDef(d) => i32::try_from(d.params.len()).unwrap_or(0),
            NodeKind::OpArg(d) => d.arity,
            NodeKind::Label(d) => i32::try_from(d.params.len()).unwrap_or(0),
            NodeKind::Module(_) => -2,
            _ => 0,
        }
    }

    pub(crate) fn sym_name(&self, id: NodeId) -> Option<Name> {
        match &self.m.node(id).kind {
            NodeKind::Module(d) => Some(d.name),
            NodeKind::OpDecl(d) => Some(d.name),
            NodeKind::FormalParam(d) => d.name,
            NodeKind::OpDef(d) => Some(d.name),
            NodeKind::ThmOrAssumpDef(d) => Some(d.name),
            NodeKind::Label(d) => Some(d.name),
            _ => None,
        }
    }

    pub(crate) fn sym_text(&self, id: NodeId) -> String {
        self.sym_name(id).map(|n| self.text(n)).unwrap_or_default()
    }

    /// `SymbolNode.isLocal`.
    fn is_local(&self, id: NodeId) -> bool {
        match &self.m.node(id).kind {
            NodeKind::FormalParam(_) => true,
            NodeKind::OpDef(d) => d.local,
            NodeKind::ThmOrAssumpDef(d) => d.local,
            _ => false,
        }
    }

    pub(crate) fn opdef(&self, id: NodeId) -> Option<&OpDefData> {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn opdef_mut(&mut self, id: NodeId) -> Option<&mut OpDefData> {
        match self.m.node_mut(id).map(|n| &mut n.kind) {
            Some(NodeKind::OpDef(d)) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn thmdef(&self, id: NodeId) -> Option<&ThmDefData> {
        match &self.m.node(id).kind {
            NodeKind::ThmOrAssumpDef(d) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn thmdef_mut(&mut self, id: NodeId) -> Option<&mut ThmDefData> {
        match self.m.node_mut(id).map(|n| &mut n.kind) {
            Some(NodeKind::ThmOrAssumpDef(d)) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn module_mut(&mut self, id: NodeId) -> Option<&mut ModuleData> {
        match self.m.node_mut(id).map(|n| &mut n.kind) {
            Some(NodeKind::Module(d)) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn module_data(&self, id: NodeId) -> Option<&ModuleData> {
        match &self.m.node(id).kind {
            NodeKind::Module(d) => Some(d),
            _ => None,
        }
    }

    /// `ModuleNode.getConstantDecls` / `getVariableDecls` (cached on first use,
    /// as SANY does).
    pub(crate) fn module_decls(&mut self, module: NodeId, variables: bool) -> Vec<NodeId> {
        if let Some(d) = self.module_data(module) {
            let cached = if variables {
                &d.variable_decls
            } else {
                &d.constant_decls
            };
            if let Some(v) = cached {
                return v.clone();
            }
        }
        let v = decls_in(self.m, module, variables);
        if let Some(d) = self.module_mut(module) {
            if variables {
                d.variable_decls = Some(v.clone());
            } else {
                d.constant_decls = Some(v.clone());
            }
        }
        v
    }

    /// `ModuleNode.isParameterFree`.
    pub(crate) fn is_parameter_free(&mut self, module: NodeId) -> bool {
        self.module_decls(module, false).is_empty() && self.module_decls(module, true).is_empty()
    }

    /// `OpDefNode.getSource` / `ThmOrAssumpDefNode.getSource`.
    fn source_of(&self, id: NodeId) -> NodeId {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => d.source.unwrap_or(id),
            NodeKind::ThmOrAssumpDef(d) => d.source.unwrap_or(id),
            _ => id,
        }
    }

    /// The "Java class" of a symbol, for SANY's class comparisons.
    fn class(&self, id: NodeId) -> u8 {
        match &self.m.node(id).kind {
            NodeKind::OpDef(_) => 1,
            NodeKind::ThmOrAssumpDef(_) => 2,
            NodeKind::OpDecl(_) => 3,
            NodeKind::FormalParam(_) => 4,
            NodeKind::Module(_) => 5,
            _ => 0,
        }
    }

    /// `SymbolNode.sameOriginallyDefinedInModule`.
    fn same_originally_defined(&mut self, a: NodeId, b: NodeId) -> bool {
        if self.class(a) != self.class(b) {
            return false;
        }
        let module = match &self.m.node(a).kind {
            NodeKind::OpDef(_) => {
                let src = self.source_of(a);
                if src != self.source_of(b) {
                    return false;
                }
                self.opdef(src).and_then(|d| d.module)
            }
            NodeKind::ThmOrAssumpDef(_) => {
                let src = self.source_of(a);
                if src != self.source_of(b) {
                    return false;
                }
                self.thmdef(src).and_then(|d| d.module)
            }
            _ => return false,
        };
        match module {
            None => true,
            Some(m) => self.is_parameter_free(m),
        }
    }

    // ---- Symbol table ----------------------------------------------------------

    pub(crate) fn top(&self) -> CtxId {
        self.st.stack.last().copied().unwrap_or(self.context)
    }

    pub(crate) fn push_context(&mut self, c: CtxId) {
        self.st.stack.push(c);
    }

    pub(crate) fn pop_context(&mut self) {
        self.st.stack.pop();
    }

    pub(crate) fn new_context(&mut self) -> CtxId {
        self.m.new_context()
    }

    /// `SymbolTable.resolveSymbol`.
    pub(crate) fn resolve_symbol(&self, name: Name) -> Option<NodeId> {
        self.st
            .stack
            .iter()
            .rev()
            .find_map(|&c| self.m.context(c).get(Key::Sym(name)))
    }

    pub(crate) fn resolve_symbol_str(&mut self, s: &str) -> Option<NodeId> {
        let n = self.m.names.get(s)?;
        self.resolve_symbol(n)
    }

    /// `SymbolTable.resolveModule`.
    pub(crate) fn resolve_module(&self, name: Name) -> Option<NodeId> {
        self.st
            .stack
            .iter()
            .rev()
            .find_map(|&c| self.m.context(c).get(Key::Module(name)))
            .or_else(|| self.sh.ext.get(&name).map(|e| e.0))
    }

    /// `Generator.getContext`.
    pub(crate) fn module_context(&self, name: Name) -> Option<CtxId> {
        match self.resolve_module(name) {
            Some(m) => self.module_data(m).map(|d| d.ctx),
            None => self.sh.ext.get(&name).map(|e| e.1),
        }
    }

    pub(crate) fn ctx_put(&mut self, c: CtxId, key: Key, node: NodeId) {
        if let Some(ctx) = self.m.ctx_mut(c) {
            ctx.put(key, node);
        }
    }

    /// `SymbolTable.addSymbol`.
    pub(crate) fn add_symbol(&mut self, name: Name, symbol: NodeId) -> bool {
        let Some(current) = self.resolve_symbol(name) else {
            let top = self.top();
            self.ctx_put(top, Key::Sym(name), symbol);
            return true;
        };
        if current == symbol {
            return true;
        }
        let here = self.m.node(symbol).span;
        let there = self.m.node(current).span;
        let text = self.text(name);
        if self.kind(current) == Kind::BuiltIn {
            self.err_help(
                Code::BuiltInRedefinition,
                here,
                format!("`{text}` is a built-in operator and cannot be redefined"),
                "choose another name",
            );
            return false;
        }
        let k = self.kind(symbol);
        if k == Kind::FormalParam
            || k == Kind::BoundSymbol
            || self.kind(current) != k
            || self.arity(current) != self.arity(symbol)
        {
            let mut d = Diagnostic::new(
                Code::Redefinition,
                here,
                format!("`{text}` is already defined or declared; TLA+ does not allow redefining or shadowing a name"),
            )
            .with_help("rename one of them");
            if let Some(s) = there {
                d = d.with_related(s, format!("`{text}` is defined or declared here"));
            }
            self.report(d);
            return false;
        }
        if self.same_originally_defined(symbol, current) {
            return true;
        }
        let mut d = Diagnostic::new(
            Code::MultipleDefinitions,
            here,
            format!("multiple declarations or definitions of `{text}`"),
        );
        if let Some(s) = there {
            d = d.with_related(s, "the other declaration or definition");
        }
        self.report(d);
        true
    }

    /// `SymbolTable.addModule`.
    pub(crate) fn add_module(&mut self, name: Name, module: NodeId) -> bool {
        match self.resolve_module(name) {
            Some(cur) if cur == module => true,
            None => {
                let top = self.top();
                self.ctx_put(top, Key::Module(name), module);
                true
            }
            Some(cur) => {
                let here = self.m.node(module).span;
                let there = self.m.node(cur).span;
                let text = self.text(name);
                let mut d = Diagnostic::new(
                    Code::DuplicateModule,
                    here,
                    format!("module `{text}` is defined twice"),
                );
                if let Some(s) = there {
                    d = d.with_related(s, "the other module");
                }
                self.report(d);
                false
            }
        }
    }

    /// `Context.mergeExtendContext`: imports the non-local symbols of an
    /// extended module's context into the current context.
    fn merge_extend_context(&mut self, target: CtxId, source: CtxId) {
        let entries: Vec<(Key, NodeId)> = self.m.context(source).entries().collect();
        for (key, sn) in entries {
            if self.is_local(sn) {
                continue;
            }
            match self.m.context(target).get(key) {
                None => self.ctx_put(target, key, sn),
                Some(symbol) if symbol == sn => {}
                Some(symbol) => {
                    let name = match key {
                        Key::Sym(n) | Key::Module(n) => self.text(n),
                    };
                    let here = self.m.node(sn).span;
                    let there = self.m.node(symbol).span;
                    if self.class(symbol) == self.class(sn) {
                        if !self.same_originally_defined(symbol, sn) {
                            let mut d = Diagnostic::new(
                                Code::ExtendsAmbiguity,
                                here,
                                format!("`{name}` is defined differently by two extended modules"),
                            );
                            if let Some(s) = there {
                                d = d.with_related(s, "the other definition");
                            }
                            self.report(d);
                        }
                    } else {
                        let mut d = Diagnostic::new(
                            Code::ExtendsConflict,
                            here,
                            format!(
                                "`{name}` is a {} in one extended module and a {} in another",
                                self.kind_word(sn),
                                self.kind_word(symbol)
                            ),
                        );
                        if let Some(s) = there {
                            d = d.with_related(s, "the other meaning");
                        }
                        self.report(d);
                    }
                }
            }
        }
    }

    /// `Context.kindOfNode`.
    fn kind_word(&self, id: NodeId) -> &'static str {
        match &self.m.node(id).kind {
            NodeKind::OpDef(_) | NodeKind::FormalParam(_) => "definition",
            _ => "declaration",
        }
    }

    // ---- Node construction ------------------------------------------------------

    /// `new OpDeclNode(..)`, adding it to the symbol table when `declare`.
    pub(crate) fn new_opdecl(
        &mut self,
        name: Name,
        decl: DeclKind,
        level: u8,
        arity: i32,
        module: Option<NodeId>,
        declare: bool,
        span: Option<Span>,
    ) -> NodeId {
        let id = self.m.add(
            span,
            NodeKind::OpDecl(OpDeclData {
                name,
                decl,
                arity,
                module,
            }),
        );
        if let Some(n) = self.m.node_mut(id) {
            n.level.level = level;
            n.level.checked = 1;
            if decl == DeclKind::Constant {
                n.level.level_params.insert(id);
                n.level.all_params.insert(id);
            }
        }
        if declare {
            self.add_symbol(name, id);
        }
        id
    }

    /// `new FormalParamNode(..)`.
    pub(crate) fn new_param(
        &mut self,
        name: Option<Name>,
        arity: i32,
        span: Option<Span>,
        declare: bool,
        module: Option<NodeId>,
    ) -> NodeId {
        let id = self.m.add(
            span,
            NodeKind::FormalParam(FormalParamData {
                name,
                arity,
                module,
            }),
        );
        if declare && let Some(n) = name {
            self.add_symbol(n, id);
        }
        id
    }

    /// The `OpDefNode` constructor for user definitions.
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors SANY's OpDefNode constructor"
    )]
    pub(crate) fn new_opdef(
        &mut self,
        name: Name,
        kind: DefKind,
        params: Vec<NodeId>,
        local: bool,
        body: Option<NodeId>,
        module: Option<NodeId>,
        declare: bool,
        span: Option<Span>,
        defined: bool,
        source: Option<NodeId>,
    ) -> NodeId {
        let n = params.len();
        let arity = i32::try_from(n).unwrap_or(0);
        let id = self.m.add(
            span,
            NodeKind::OpDef(Box::new(OpDefData {
                name,
                kind,
                params,
                arity,
                local,
                body,
                module,
                source,
                defined,
                step: None,
                labels: None,
                let_in_level: -1,
                in_recursive: false,
                in_recursive_section: false,
                recursive_section: -1,
                lv: OpLevels {
                    max_levels: vec![3; n],
                    weights: vec![0; n],
                    min_max_level: None,
                    op_level_cond: None,
                    is_leibniz_arg: Some(vec![true; n]),
                    is_leibniz: true,
                },
            })),
        );
        if declare {
            self.add_symbol(name, id);
        }
        id
    }

    /// The `OpDefNode` constructor for a module instance name (`I` of
    /// `I == INSTANCE M`).
    pub(crate) fn new_instance_def(
        &mut self,
        name: Name,
        params: Vec<NodeId>,
        local: bool,
        module: Option<NodeId>,
        declare: bool,
        span: Option<Span>,
        source: Option<NodeId>,
    ) -> NodeId {
        let arity = i32::try_from(params.len()).unwrap_or(0);
        let id = self.m.add(
            span,
            NodeKind::OpDef(Box::new(OpDefData {
                name,
                kind: DefKind::ModuleInstance,
                params,
                arity,
                local,
                body: None,
                module,
                source,
                defined: false,
                step: None,
                labels: None,
                let_in_level: -1,
                in_recursive: false,
                in_recursive_section: false,
                recursive_section: -1,
                lv: OpLevels::default(),
            })),
        );
        if declare {
            self.add_symbol(name, id);
        }
        id
    }

    /// The `OpDefNode` constructor for a numbered proof step.
    pub(crate) fn new_step_def(
        &mut self,
        name: Name,
        step: NodeId,
        module: Option<NodeId>,
        span: Option<Span>,
    ) -> NodeId {
        let id = self.m.add(
            span,
            NodeKind::OpDef(Box::new(OpDefData {
                name,
                kind: DefKind::NumberedProofStep,
                params: Vec::new(),
                arity: 0,
                local: false,
                body: None,
                module,
                source: None,
                defined: false,
                step: Some(step),
                labels: None,
                let_in_level: -1,
                in_recursive: false,
                in_recursive_section: false,
                recursive_section: -1,
                lv: OpLevels::default(),
            })),
        );
        self.add_symbol(name, id);
        id
    }

    /// `new ThmOrAssumpDefNode(name, stn)`: a named theorem or step, filled
    /// in later by [`Gen::construct_thm`].
    pub(crate) fn new_thm_shell(&mut self, name: Name, span: Option<Span>) -> NodeId {
        self.m.add(
            span,
            NodeKind::ThmOrAssumpDef(Box::new(ThmDefData {
                name,
                theorem: true,
                body: None,
                module: None,
                params: Vec::new(),
                instantiated_from: None,
                source: None,
                local: false,
                suffices: false,
                labels: None,
                lv: OpLevels::default(),
            })),
        )
    }

    /// `ThmOrAssumpDefNode.construct`.
    pub(crate) fn construct_thm(
        &mut self,
        id: NodeId,
        theorem: bool,
        body: Option<NodeId>,
        module: Option<NodeId>,
    ) {
        let name = if let Some(d) = self.thmdef_mut(id) {
            d.theorem = theorem;
            d.body = body;
            d.module = module;
            Some(d.name)
        } else {
            None
        };
        if let Some(n) = name {
            self.add_symbol(n, id);
        }
    }

    /// The full `ThmOrAssumpDefNode` constructor.
    #[allow(clippy::too_many_arguments, reason = "mirrors SANY's constructor")]
    pub(crate) fn new_thmdef(
        &mut self,
        name: Name,
        theorem: bool,
        body: Option<NodeId>,
        module: Option<NodeId>,
        declare: bool,
        span: Option<Span>,
        params: Vec<NodeId>,
        instantiated_from: Option<NodeId>,
        source: Option<NodeId>,
    ) -> NodeId {
        let id = self.m.add(
            span,
            NodeKind::ThmOrAssumpDef(Box::new(ThmDefData {
                name,
                theorem,
                body,
                module,
                params,
                instantiated_from,
                source,
                local: false,
                suffices: false,
                labels: None,
                lv: OpLevels::default(),
            })),
        );
        if declare {
            self.add_symbol(name, id);
        }
        id
    }

    pub(crate) fn set_labels(&mut self, id: NodeId, labels: Option<BTreeMap<Name, NodeId>>) {
        let lid = labels.map(|t| self.m.new_labels(t));
        self.set_labels_id(id, lid);
    }

    pub(crate) fn set_labels_id(&mut self, id: NodeId, lid: Option<LabelsId>) {
        match self.m.node_mut(id).map(|n| &mut n.kind) {
            Some(NodeKind::OpDef(d)) => d.labels = lid,
            Some(NodeKind::ThmOrAssumpDef(d)) => d.labels = lid,
            Some(NodeKind::Label(d)) => d.labels = lid,
            _ => {}
        }
    }

    pub(crate) fn labels_id(&self, id: NodeId) -> Option<LabelsId> {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => d.labels,
            NodeKind::ThmOrAssumpDef(d) => d.labels,
            NodeKind::Label(d) => d.labels,
            _ => None,
        }
    }

    /// The built-in operator called `name`.
    pub(crate) fn builtin(&mut self, name: &str) -> NodeId {
        let n = self.intern(name);
        self.m
            .context(self.sh.global)
            .get(Key::Sym(n))
            .unwrap_or(self.sh.null)
    }

    fn appl_node(&mut self, operator: NodeId, data: OpApplData, span: Option<Span>) -> NodeId {
        let _ = operator;
        self.m.add(span, NodeKind::OpAppl(Box::new(data)))
    }

    /// `new OpApplNode(op, operands, stn, mn, errors)`: checks the operands
    /// with the operator's `match`.
    pub(crate) fn appl_match(
        &mut self,
        op: NodeId,
        operands: Vec<Option<NodeId>>,
        span: Option<Span>,
    ) -> GResult<NodeId> {
        let id = self.appl_node(
            op,
            OpApplData {
                operator: op,
                operands,
                unbounded: None,
                bounded: None,
                tuples: None,
                ranges: Vec::new(),
                subexpression_of: None,
            },
            span,
        );
        self.match_op(op, id)?;
        Ok(id)
    }

    /// `new OpApplNode(builtinName, operands, stn, mn)`.
    pub(crate) fn appl_builtin(
        &mut self,
        name: &str,
        operands: Vec<Option<NodeId>>,
        span: Option<Span>,
    ) -> NodeId {
        let op = self.builtin(name);
        self.appl_node(
            op,
            OpApplData {
                operator: op,
                operands,
                unbounded: None,
                bounded: None,
                tuples: None,
                ranges: Vec::new(),
                subexpression_of: None,
            },
            span,
        )
    }

    /// `new OpApplNode(builtinName, operands, unboundedSymbols, stn, mn)`.
    pub(crate) fn appl_unbounded(
        &mut self,
        name: &str,
        operands: Vec<Option<NodeId>>,
        symbols: Vec<NodeId>,
        span: Option<Span>,
    ) -> NodeId {
        let op = self.builtin(name);
        self.appl_node(
            op,
            OpApplData {
                operator: op,
                operands,
                unbounded: Some(symbols),
                bounded: None,
                tuples: None,
                ranges: Vec::new(),
                subexpression_of: None,
            },
            span,
        )
    }

    /// `new OpApplNode(builtinName, fcnName, operands, params, tuples, ranges, ..)`.
    pub(crate) fn appl_bounded(
        &mut self,
        name: &str,
        fcn: Option<Vec<NodeId>>,
        operands: Vec<Option<NodeId>>,
        bounded: Vec<Vec<NodeId>>,
        tuples: Vec<bool>,
        ranges: Vec<NodeId>,
        span: Option<Span>,
    ) -> NodeId {
        let op = self.builtin(name);
        self.appl_node(
            op,
            OpApplData {
                operator: op,
                operands,
                unbounded: fcn,
                bounded: Some(bounded),
                tuples: Some(tuples),
                ranges,
                subexpression_of: None,
            },
            span,
        )
    }

    pub(crate) fn appl_data(&self, id: NodeId) -> Option<&OpApplData> {
        match &self.m.node(id).kind {
            NodeKind::OpAppl(d) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn appl_data_mut(&mut self, id: NodeId) -> Option<&mut OpApplData> {
        match self.m.node_mut(id).map(|n| &mut n.kind) {
            Some(NodeKind::OpAppl(d)) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn appl_operator_name(&self, id: NodeId) -> Option<String> {
        self.appl_data(id).map(|d| self.sym_text(d.operator))
    }

    /// The operator's `match` method (`OpDefNode`, `OpDeclNode`,
    /// `FormalParamNode`, `ThmOrAssumpDefNode`, `ModuleNode`).
    fn match_op(&mut self, op: NodeId, appl: NodeId) -> GResult<()> {
        let args: Vec<Option<NodeId>> = self
            .appl_data(appl)
            .map(|d| d.operands.clone())
            .unwrap_or_default();
        let at = self.m.node(appl).span;
        let opname = self.sym_text(op);
        match &self.m.node(op).kind {
            NodeKind::OpDecl(d) => {
                if usize::try_from(d.arity).ok() != Some(args.len()) {
                    self.err(
                        Code::WrongArgumentCount,
                        at,
                        format!("`{opname}` is used with the wrong number of arguments"),
                    );
                }
            }
            NodeKind::OpDef(d) => {
                let kind = d.kind;
                let arity = d.arity;
                let params = d.params.clone();
                if kind == DefKind::ModuleInstance {
                    self.err(
                        Code::IncompleteOperatorName,
                        at,
                        format!("module instance `{opname}` used where an operator is required"),
                    );
                } else if arity == -1 {
                    for (i, a) in args.iter().enumerate() {
                        if a.is_some_and(|a| self.kind(a) == Kind::OpArg) {
                            self.err(
                                Code::InvalidArgument,
                                at,
                                format!("argument {} of `{opname}` must be an expression", i + 1),
                            );
                        }
                    }
                } else if params.len() != args.len() {
                    self.err(
                        Code::WrongArgumentCount,
                        at,
                        format!(
                            "`{opname}` takes {} arguments but is given {}",
                            params.len(),
                            args.len()
                        ),
                    );
                } else if kind == DefKind::BuiltIn {
                    for (i, a) in args.iter().enumerate() {
                        if a.is_some_and(|a| self.kind(a) == Kind::OpArg) {
                            self.err(
                                Code::InvalidArgument,
                                at,
                                format!(
                                    "argument {} of the built-in operator `{opname}` must be an expression",
                                    i + 1
                                ),
                            );
                        }
                    }
                } else if kind == DefKind::UserDefined {
                    for (i, (p, a)) in params.iter().zip(args.iter()).enumerate() {
                        let pa = self.arity(*p);
                        if pa == 0 {
                            if a.is_some_and(|a| self.kind(a) == Kind::OpArg) {
                                self.err(
                                    Code::InvalidArgument,
                                    at,
                                    format!(
                                        "argument {} of `{opname}` must be an expression, not an operator",
                                        i + 1
                                    ),
                                );
                            }
                        } else if pa > 0 {
                            if !self.matching_op_arg_operand(*a, pa) {
                                self.err(
                                    Code::OperatorArgumentArity,
                                    at,
                                    format!(
                                        "argument {} of `{opname}` must be an operator with {pa} parameters",
                                        i + 1
                                    ),
                                );
                            }
                        } else {
                            self.err(
                                Code::SanyInternal,
                                at,
                                format!("operator `{opname}` has a parameter of negative arity"),
                            );
                        }
                    }
                } else {
                    return Err(self.abort(
                        Code::SanyInternal,
                        at,
                        format!("`{opname}` is neither a built-in nor a user-defined operator"),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `OpDefNode.matchingOpArgOperand`.
    fn matching_op_arg_operand(&self, arg: Option<NodeId>, arity: i32) -> bool {
        let Some(arg) = arg else { return false };
        let NodeKind::OpArg(d) = &self.m.node(arg).kind else {
            return false;
        };
        if d.arity != arity {
            return false;
        }
        if let NodeKind::OpDef(od) = &self.m.node(d.op).kind {
            for p in &od.params {
                if self.arity(*p) > 0 {
                    return false;
                }
            }
        }
        true
    }

    // ---- Modules ------------------------------------------------------------------

    /// `Generator.generateModule`.
    fn generate_module(&mut self, node: &SyntaxNode, parent: Option<NodeId>) -> GResult<NodeId> {
        self.nesting += 1;
        let name_text: String = dive_syntax::ast::Module::cast(node.clone())
            .and_then(|m| m.name())
            .unwrap_or_default();
        let name = self.intern(&name_text);
        let header_span = node
            .children()
            .find(|c| c.kind() == K::MODULE_HEADER)
            .map_or_else(|| self.sp(node), |h| self.sp(&h));
        let ctx = self.context;
        let module = self.m.add(
            header_span,
            NodeKind::Module(Box::new(ModuleData {
                name,
                ctx,
                extendees: Vec::new(),
                definitions: Vec::new(),
                instances: Vec::new(),
                assumptions: Vec::new(),
                theorems: Vec::new(),
                top_level: Vec::new(),
                nesting: u32::try_from(self.nesting).unwrap_or(0),
                is_standard: false,
                is_instantiated: false,
                recursive_decls: Vec::new(),
                op_defs_in_recursive_section: Vec::new(),
                constant_decls: None,
                variable_decls: None,
            })),
        );
        if let Some(p) = parent
            && let Some(d) = self.module_mut(p)
        {
            d.definitions.push(module);
        }
        let extends: Vec<SyntaxToken> = node
            .children()
            .find(|c| c.kind() == K::EXTENDS)
            .map(|e| {
                toks(&e)
                    .into_iter()
                    .filter(|t| t.kind() == K::IDENT)
                    .collect()
            })
            .unwrap_or_default();
        self.process_extends_list(&extends, module)?;
        for unit in nodes(node) {
            let s = self.sp(&unit);
            match unit.kind() {
                K::VARIABLE_DECL => {
                    self.check_if_in_recursive_section(s, "a VARIABLE declaration");
                    self.process_variables(&unit, module);
                }
                K::CONSTANT_DECL => {
                    self.check_if_in_recursive_section(s, "a CONSTANT declaration");
                    self.process_parameters(&unit, module);
                }
                K::OPERATOR_DEF => {
                    let d = self.process_operator(&unit, None, module)?;
                    if let Some(md) = self.module_mut(module) {
                        md.definitions.push(d);
                    }
                }
                K::FUNCTION_DEF => self.process_function(&unit, None, module)?,
                K::MODULE_DEF => {
                    self.process_module_definition(&unit, None, None, module)?;
                }
                K::MODULE => {
                    self.check_if_in_recursive_section(s, "a module");
                    let old = std::mem::take(&mut self.st);
                    self.st = SymTab {
                        stack: old.stack.clone(),
                    };
                    let ctx = self.new_context();
                    self.context = ctx;
                    self.push_context(ctx);
                    let r = self.generate_module(&unit, Some(module));
                    self.pop_context();
                    self.st = old;
                    let mn = r?;
                    let mname = self.module_data(mn).map(|d| d.name);
                    if let Some(n) = mname {
                        self.add_module(n, mn);
                    }
                    let inner_rec = self
                        .module_data(mn)
                        .map(|d| d.op_defs_in_recursive_section.clone())
                        .unwrap_or_default();
                    if let Some(md) = self.module_mut(module) {
                        md.op_defs_in_recursive_section.extend(inner_rec);
                    }
                }
                K::INSTANCE => {
                    self.generate_instance(&unit, module, true)?;
                }
                K::THEOREM => {
                    self.check_if_in_recursive_section(s, "a THEOREM");
                    self.process_theorem(&unit, module)?;
                }
                K::ASSUMPTION => {
                    self.check_if_in_recursive_section(s, "an ASSUME");
                    self.process_assumption(&unit, module)?;
                }
                K::USE_OR_HIDE => {
                    self.check_if_in_recursive_section(s, "a USE or HIDE");
                    let u = self.generate_use_or_hide(&unit, module)?;
                    self.fact_check(u);
                    if self.use_is_empty(u) {
                        self.err(Code::EmptyUse, s, "empty USE or HIDE");
                    }
                    if let Some(md) = self.module_mut(module) {
                        md.top_level.push(u);
                    }
                }
                K::RECURSIVE_DECL => self.process_recursive(&unit, module),
                K::MODULE_HEADER | K::EXTENDS => {}
                _ => {
                    return Err(self.abort(
                        Code::SanyInternal,
                        s,
                        format!("unexpected syntax node {} in a module", unit.kind().name()),
                    ));
                }
            }
        }
        self.check_for_undefined_recursive_ops(module);
        self.nesting -= 1;
        Ok(module)
    }

    /// `Generator.processExtendsList`.
    fn process_extends_list(&mut self, names: &[SyntaxToken], module: NodeId) -> GResult<()> {
        let mut extendees = Vec::new();
        for t in names {
            let name = self.intern(t.text());
            let Some(extendee) = self.resolve_module(name) else {
                return Err(self.abort(
                    Code::ModuleUnavailable,
                    self.spt(t),
                    format!("could not find module `{}`", t.text()),
                ));
            };
            extendees.push(extendee);
            if let Some(ctx) = self.module_context(name) {
                let top = self.top();
                self.merge_extend_context(top, ctx);
            } else {
                self.err(
                    Code::SanyInternal,
                    self.spt(t),
                    format!("could not find the context of module `{}`", t.text()),
                );
            }
            // copyAssumes, copyTheorems, copyTopLevel
            let (a, th, tl) = self
                .module_data(extendee)
                .map(|d| {
                    (
                        d.assumptions.clone(),
                        d.theorems.clone(),
                        d.top_level.clone(),
                    )
                })
                .unwrap_or_default();
            if let Some(md) = self.module_mut(module) {
                md.assumptions.extend(a);
                md.theorems.extend(th);
                md.top_level.extend(tl);
            }
        }
        if let Some(md) = self.module_mut(module) {
            md.extendees = extendees;
        }
        Ok(())
    }

    /// `Generator.processVariables`.
    fn process_variables(&mut self, node: &SyntaxNode, module: NodeId) {
        for t in toks(node).into_iter().filter(|t| t.kind() == K::IDENT) {
            if t.text() == "@" {
                self.err(
                    Code::AtOutsideExcept,
                    self.spt(&t),
                    "`@` cannot be declared as a variable",
                );
            }
            let name = self.intern(t.text());
            self.new_opdecl(
                name,
                DeclKind::Variable,
                1,
                0,
                Some(module),
                true,
                self.spt(&t),
            );
        }
    }

    /// `Generator.buildParameter`: the name and arity of a declaration item
    /// (`IDENT_DECL`, `PREFIX_DECL`, `INFIX_DECL`, `POSTFIX_DECL`).
    pub(crate) fn decl_name_arity(&mut self, item: &SyntaxNode) -> Option<(Name, i32)> {
        let ts = toks(item);
        let (text, arity) = match item.kind() {
            K::IDENT_DECL => {
                let n = ts.iter().find(|t| t.kind() == K::IDENT)?;
                let a = ts.iter().filter(|t| t.kind() == K::UNDERSCORE).count();
                (n.text().to_owned(), i32::try_from(a).unwrap_or(0))
            }
            K::PREFIX_DECL => (ts.first()?.text().to_owned(), 1),
            K::INFIX_DECL => (ts.get(1)?.text().to_owned(), 2),
            K::POSTFIX_DECL => (ts.get(1)?.text().to_owned(), 1),
            _ => return None,
        };
        let resolved = crate::builtins::resolve_synonym(&text).to_owned();
        Some((self.intern(&resolved), arity))
    }

    /// `Generator.buildParameter`.
    pub(crate) fn build_parameter(
        &mut self,
        item: &SyntaxNode,
        decl: DeclKind,
        level: u8,
        module: NodeId,
        declare: bool,
    ) -> NodeId {
        let Some((name, arity)) = self.decl_name_arity(item) else {
            self.err(
                Code::SanyInternal,
                self.sp(item),
                "unknown parameter declaration",
            );
            return self.null();
        };
        self.new_opdecl(
            name,
            decl,
            level,
            arity,
            Some(module),
            declare,
            self.sp(item),
        )
    }

    /// `Generator.processParameters`.
    fn process_parameters(&mut self, node: &SyntaxNode, module: NodeId) {
        for item in nodes(node) {
            self.build_parameter(&item, DeclKind::Constant, 0, module, true);
        }
    }

    /// The parameters of an `IDENT_LHS` / prefix / infix / postfix left-hand
    /// side, declared in the current context. Returns `(name, params)`.
    fn process_lhs(&mut self, lhs: &SyntaxNode, module: NodeId) -> (Option<Name>, Vec<NodeId>) {
        let ts = toks(lhs);
        let mut params = Vec::new();
        match lhs.kind() {
            K::IDENT_LHS => {
                for p in nodes(lhs) {
                    let pt = toks(&p);
                    let (name, arity) = match p.kind() {
                        K::IDENT_DECL => {
                            let n = pt
                                .iter()
                                .find(|t| t.kind() == K::IDENT)
                                .map(|t| t.text().to_owned());
                            let a = pt.iter().filter(|t| t.kind() == K::UNDERSCORE).count();
                            (n, i32::try_from(a).unwrap_or(0))
                        }
                        K::INFIX_DECL => (
                            pt.get(1)
                                .map(|t| crate::builtins::resolve_synonym(t.text()).to_owned()),
                            2,
                        ),
                        K::PREFIX_DECL => (pt.first().map(|t| t.text().to_owned()), 1),
                        _ => (pt.get(1).map(|t| t.text().to_owned()), 1),
                    };
                    let name = name.map(|n| self.intern(&n));
                    let id = self.new_param(name, arity, self.sp(&p), true, Some(module));
                    params.push(id);
                }
                let name = ts
                    .iter()
                    .find(|t| t.kind() == K::IDENT)
                    .map(|t| t.text().to_owned());
                (name.map(|n| self.intern(&n)), params)
            }
            K::PREFIX_LHS => {
                if let Some(p) = ts.get(1) {
                    let pn = self.intern(p.text());
                    params.push(self.new_param(Some(pn), 0, self.spt(p), true, Some(module)));
                }
                let name = ts
                    .first()
                    .map(|t| crate::builtins::resolve_synonym(t.text()).to_owned());
                (name.map(|n| self.intern(&n)), params)
            }
            K::INFIX_LHS => {
                for i in [0, 2] {
                    if let Some(p) = ts.get(i) {
                        let pn = self.intern(p.text());
                        params.push(self.new_param(Some(pn), 0, self.spt(p), true, Some(module)));
                    }
                }
                let name = ts
                    .get(1)
                    .map(|t| crate::builtins::resolve_synonym(t.text()).to_owned());
                (name.map(|n| self.intern(&n)), params)
            }
            _ => {
                if let Some(p) = ts.first() {
                    let pn = self.intern(p.text());
                    params.push(self.new_param(Some(pn), 0, self.spt(p), true, Some(module)));
                }
                let name = ts
                    .get(1)
                    .map(|t| crate::builtins::resolve_synonym(t.text()).to_owned());
                (name.map(|n| self.intern(&n)), params)
            }
        }
    }

    /// `Generator.processOperator`: an `OPERATOR_DEF`. Appends the new
    /// definition to `defs` (for a `LET`) and returns it.
    pub(crate) fn process_operator(
        &mut self,
        node: &SyntaxNode,
        defs: Option<&mut Vec<NodeId>>,
        module: NodeId,
    ) -> GResult<NodeId> {
        let local = toks(node).iter().any(|t| t.kind() == K::LOCAL_KW);
        let lhs = nodes(node).into_iter().find(|n| {
            matches!(
                n.kind(),
                K::IDENT_LHS | K::PREFIX_LHS | K::INFIX_LHS | K::POSTFIX_LHS
            )
        });
        let body_node = cst::last_expr(node);
        let ctxt = self.new_context();
        self.push_context(ctxt);
        let (name, params) = match &lhs {
            Some(l) => self.process_lhs(l, module),
            None => {
                self.err(Code::SanyInternal, self.sp(node), "unknown definition form");
                (None, Vec::new())
            }
        };
        let name = name.unwrap_or_else(|| self.intern(""));
        let span = self.sp(node);
        let mut odn: Option<NodeId> = None;
        let mut is_recursive = false;
        if let Some(sym) = self.resolve_symbol(name) {
            if self.opdef(sym).is_some() {
                odn = Some(sym);
            }
            let recursive_decl = odn
                .and_then(|o| self.opdef(o))
                .is_some_and(|d| d.in_recursive && !d.defined);
            if let (true, Some(o)) = (recursive_decl, odn) {
                let lvl = self.opdef(o).map_or(-1, |d| d.let_in_level);
                if usize::try_from(lvl).ok() == Some(self.cur_level) {
                    is_recursive = true;
                    let decl_params = self.opdef(o).map_or(0, |d| d.params.len());
                    let mut matches = decl_params == params.len();
                    if matches {
                        for p in &params {
                            matches = self.arity(*p) == 0;
                        }
                    }
                    if !matches {
                        self.err(
                            Code::RecursiveArityMismatch,
                            span,
                            format!(
                                "the definition of `{}` has a different arity than its RECURSIVE declaration",
                                self.text(name)
                            ),
                        );
                    }
                    if let Some(d) = self.opdef_mut(o) {
                        d.params.clone_from(&params);
                    }
                } else {
                    self.err(
                        Code::RecursiveWrongLevel,
                        span,
                        format!(
                            "recursive operator `{}` is defined at a different LET level than its RECURSIVE declaration",
                            self.text(name)
                        ),
                    );
                    odn = None;
                }
            } else {
                let there = self.m.node(sym).span;
                let mut d = Diagnostic::new(
                    Code::Redefinition,
                    span,
                    format!(
                        "operator `{}` is already defined or declared",
                        self.text(name)
                    ),
                )
                .with_help(
                    "TLA+ does not allow redefining or shadowing a name; rename one of them",
                );
                if let Some(s) = there {
                    d = d.with_related(s, "the earlier definition or declaration");
                }
                self.report(d);
            }
        }
        self.push_ls();
        let exp = match &body_node {
            Some(b) => self.generate_expression(b, module),
            None => Ok(self.null()),
        };
        self.pop_context();
        let exp = exp?;
        let result = if is_recursive {
            let o = odn.unwrap_or_else(|| self.null());
            self.end_op_def_node(o, exp, span);
            o
        } else {
            let o = self.new_opdef(
                name,
                DefKind::UserDefined,
                params,
                local,
                Some(exp),
                Some(module),
                true,
                span,
                true,
                None,
            );
            self.set_op_def_node_recursion_fields(o, module);
            odn = Some(o);
            o
        };
        let ht = self.pop_label_node_set();
        if let Some(o) = odn {
            self.set_labels(o, ht);
        }
        if let Some(d) = defs {
            d.push(result);
        }
        Ok(result)
    }

    /// `Generator.processQuantBoundArgs`: the bounds' sets first (in the
    /// outer scope), then the bound names (declared in the current context).
    pub(crate) fn process_quant_bounds(
        &mut self,
        bounds: &[SyntaxNode],
        module: NodeId,
    ) -> GResult<(Vec<Vec<NodeId>>, Vec<bool>, Vec<NodeId>)> {
        let mut ranges = Vec::new();
        for b in bounds {
            let set = match cst::last_expr(b) {
                Some(e) => self.generate_expression(&e, module)?,
                None => self.null(),
            };
            ranges.push(set);
        }
        let mut params = Vec::new();
        let mut tuples = Vec::new();
        for b in bounds {
            let mut group = Vec::new();
            if let Some(t) = b.children().find(|c| c.kind() == K::IDENT_TUPLE) {
                tuples.push(true);
                for id in toks(&t).into_iter().filter(|t| t.kind() == K::IDENT) {
                    let n = self.intern(id.text());
                    group.push(self.new_param(Some(n), 0, self.spt(&id), true, Some(module)));
                }
            } else {
                tuples.push(false);
                for id in toks(b).into_iter().filter(|t| t.kind() == K::IDENT) {
                    let n = self.intern(id.text());
                    group.push(self.new_param(Some(n), 0, self.spt(&id), true, Some(module)));
                }
            }
            params.push(group);
        }
        Ok((params, tuples, ranges))
    }

    /// `Generator.processFunction`: `f[x \in S] == e`.
    pub(crate) fn process_function(
        &mut self,
        node: &SyntaxNode,
        defs: Option<&mut Vec<NodeId>>,
        module: NodeId,
    ) -> GResult<()> {
        let local = toks(node).iter().any(|t| t.kind() == K::LOCAL_KW);
        let span = self.sp(node);
        let name_tok = toks(node).into_iter().find(|t| t.kind() == K::IDENT);
        let name = self.intern(name_tok.as_ref().map_or("", |t| t.text()));
        let bounds: Vec<SyntaxNode> = nodes(node)
            .into_iter()
            .filter(|n| n.kind() == K::QUANT_BOUND)
            .collect();
        let body_node = cst::last_expr(node);
        let new_ctx = self.new_context();
        self.push_context(new_ctx);
        let r = self.process_quant_bounds(&bounds, module);
        let (quants, tuples, domains) = match r {
            Ok(v) => v,
            Err(e) => {
                self.pop_context();
                return Err(e);
            }
        };
        let symbol = self.resolve_symbol(name);
        let fcn_decl = self.new_param(Some(name), 0, span, symbol.is_none(), Some(module));
        self.pop_context();
        let oan = self.appl_bounded(
            "$NonRecursiveFcnSpec",
            Some(vec![fcn_decl]),
            Vec::new(),
            quants.clone(),
            tuples,
            domains,
            span,
        );
        let mut odn = None;
        match symbol {
            None => {
                let o = self.new_opdef(
                    name,
                    DefKind::UserDefined,
                    Vec::new(),
                    local,
                    Some(oan),
                    Some(module),
                    true,
                    span,
                    true,
                    None,
                );
                self.set_op_def_node_recursion_fields(o, module);
                odn = Some(o);
            }
            Some(sym) => {
                let rec = self
                    .opdef(sym)
                    .filter(|d| d.in_recursive && !d.defined)
                    .map(|d| (d.let_in_level, d.arity));
                match rec {
                    Some((lvl, arity)) => {
                        if usize::try_from(lvl).ok() == Some(self.cur_level) {
                            odn = Some(sym);
                            if arity == 0 {
                                self.end_op_def_node(sym, oan, span);
                            } else {
                                self.err(
                                    Code::RecursiveArityMismatch,
                                    span,
                                    format!(
                                        "function `{}` has operator arguments in its RECURSIVE declaration",
                                        self.text(name)
                                    ),
                                );
                            }
                        } else {
                            self.err(
                                Code::RecursiveWrongLevel,
                                span,
                                format!(
                                    "recursive function `{}` is defined at a different LET level than its RECURSIVE declaration",
                                    self.text(name)
                                ),
                            );
                        }
                    }
                    None => {
                        let there = self.m.node(sym).span;
                        let mut d = Diagnostic::new(
                            Code::Redefinition,
                            span,
                            format!(
                                "function name `{}` is already defined or declared",
                                self.text(name)
                            ),
                        );
                        if let Some(s) = there {
                            d = d.with_related(s, "the earlier definition or declaration");
                        }
                        self.report(d);
                    }
                }
            }
        }
        if let Some(o) = odn {
            if let Some(md) = self.module_mut(module) {
                md.definitions.push(o);
            }
            if let Some(d) = defs {
                d.push(o);
            }
            self.push_context(new_ctx);
        }
        self.functions.push((name, oan));
        self.push_ls();
        self.push_formal_params(quants.into_iter().flatten().collect());
        let body = match &body_node {
            Some(b) => self.generate_expression(b, module),
            None => Ok(self.null()),
        };
        self.pop_formal_params();
        let ht = self.pop_label_node_set();
        if let Some(o) = odn {
            self.set_labels(o, ht);
        }
        self.functions.pop();
        let body = body?;
        if let Some(d) = self.appl_data_mut(oan) {
            d.operands = vec![Some(body)];
        }
        if odn.is_some() {
            self.pop_context();
        }
        if self.appl_operator_name(oan).as_deref() == Some("$NonRecursiveFcnSpec")
            && let Some(d) = self.appl_data_mut(oan)
        {
            d.unbounded = None;
        }
        Ok(())
    }

    /// `Generator.processLetIn`.
    pub(crate) fn process_let_in(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let let_ctx = self.new_context();
        self.push_context(let_ctx);
        if self.cur_level < MAX_LET_IN_LEVEL - 1 {
            self.cur_level += 1;
        } else {
            return Err(self.abort(
                Code::LetTooDeep,
                self.sp(node),
                format!("LET nested more than {MAX_LET_IN_LEVEL} deep"),
            ));
        }
        self.unresolved_cnt[self.cur_level] = 0;
        let mut defs = Vec::new();
        let mut insts = Vec::new();
        let def_nodes = node
            .children()
            .find(|c| c.kind() == K::LET_DEFS)
            .map(|d| nodes(&d))
            .unwrap_or_default();
        for d in def_nodes {
            match d.kind() {
                K::OPERATOR_DEF => {
                    let o = self.process_operator(&d, Some(&mut defs), module)?;
                    if let Some(md) = self.module_mut(module) {
                        md.definitions.push(o);
                    }
                }
                K::FUNCTION_DEF => self.process_function(&d, Some(&mut defs), module)?,
                K::MODULE_DEF => {
                    self.process_module_definition(&d, Some(&mut defs), Some(&mut insts), module)?;
                }
                K::RECURSIVE_DECL => self.process_recursive(&d, module),
                _ => {
                    return Err(self.abort(
                        Code::SanyInternal,
                        self.sp(&d),
                        "unexpected syntax in LET",
                    ));
                }
            }
        }
        self.check_for_undefined_recursive_ops(module);
        self.cur_level = self.cur_level.saturating_sub(1);
        let body = match cst::last_expr(node) {
            Some(b) => self.generate_expression(&b, module)?,
            None => self.null(),
        };
        let id = self.m.add(
            self.sp(node),
            NodeKind::LetIn(Box::new(crate::model::LetInData {
                defs,
                insts,
                body: Some(body),
                ctx: let_ctx,
            })),
        );
        self.pop_context();
        Ok(id)
    }

    /// `Generator.processRecursive`.
    pub(crate) fn process_recursive(&mut self, node: &SyntaxNode, module: NodeId) {
        let items = nodes(node);
        if self.unresolved_sum == 0 {
            self.recursive_section_count += 1;
        }
        let n = i32::try_from(items.len()).unwrap_or(0);
        self.unresolved_cnt[self.cur_level] += n;
        self.unresolved_sum += n;
        for item in items {
            let Some((name, arity)) = self.decl_name_arity(&item) else {
                continue;
            };
            let params: Vec<NodeId> = (0..arity)
                .map(|_| self.new_param(None, 0, None, false, Some(module)))
                .collect();
            let odn = self.start_op_def_node(name, self.sp(&item), params, module);
            if let Some(md) = self.module_mut(module) {
                md.recursive_decls.push(odn);
            }
        }
    }

    // ---- Recursion bookkeeping -------------------------------------------------

    /// `Generator.startOpDefNode`.
    fn start_op_def_node(
        &mut self,
        name: Name,
        span: Option<Span>,
        params: Vec<NodeId>,
        module: NodeId,
    ) -> NodeId {
        let odn = self.new_opdef(
            name,
            DefKind::UserDefined,
            params,
            false,
            None,
            Some(module),
            true,
            span,
            false,
            None,
        );
        let lvl = i32::try_from(self.cur_level).unwrap_or(0);
        let sect = self.recursive_section_count;
        if let Some(d) = self.opdef_mut(odn) {
            d.let_in_level = lvl;
            d.in_recursive = true;
            d.in_recursive_section = true;
            d.recursive_section = sect;
        }
        if let Some(md) = self.module_mut(module) {
            md.op_defs_in_recursive_section.push(odn);
        }
        odn
    }

    /// `Generator.endOpDefNode`.
    fn end_op_def_node(&mut self, odn: NodeId, body: NodeId, span: Option<Span>) {
        let in_rec = if let Some(d) = self.opdef_mut(odn) {
            d.defined = true;
            d.body = Some(body);
            d.in_recursive
        } else {
            false
        };
        if let Some(n) = self.m.node_mut(odn) {
            n.span = span;
        }
        if in_rec {
            self.unresolved_cnt[self.cur_level] -= 1;
            self.unresolved_sum -= 1;
            if self.unresolved_sum < 0 {
                self.err(
                    Code::RecursiveBookkeeping,
                    span,
                    "more recursive operators are defined than were declared in RECURSIVE statements",
                );
            }
        }
    }

    /// `Generator.setOpDefNodeRecursionFields`.
    pub(crate) fn set_op_def_node_recursion_fields(&mut self, odn: NodeId, module: NodeId) {
        let lvl = i32::try_from(self.cur_level).unwrap_or(0);
        let in_section = self.unresolved_sum > 0;
        let sect = self.recursive_section_count;
        let cnt = self.unresolved_cnt[self.cur_level];
        if let Some(d) = self.opdef_mut(odn) {
            d.let_in_level = lvl;
            if in_section {
                d.recursive_section = sect;
                d.in_recursive_section = cnt > 0;
            }
        }
        if in_section && let Some(md) = self.module_mut(module) {
            md.op_defs_in_recursive_section.push(odn);
        }
    }

    /// `Generator.checkIfInRecursiveSection`.
    pub(crate) fn check_if_in_recursive_section(&mut self, at: Option<Span>, what: &str) {
        if self.unresolved_sum > 0 {
            self.err_help(
                Code::RecursiveSectionIllegal,
                at,
                format!("{what} may not appear between a RECURSIVE declaration and its definition"),
                "move it before the RECURSIVE declaration or after the definition",
            );
        }
    }

    /// `Generator.checkForUndefinedRecursiveOps`.
    fn check_for_undefined_recursive_ops(&mut self, module: NodeId) {
        if self.unresolved_cnt[self.cur_level] > 0 {
            let decls = self
                .module_data(module)
                .map(|d| d.recursive_decls.clone())
                .unwrap_or_default();
            for o in decls {
                let undefined = self.opdef(o).is_some_and(|d| {
                    usize::try_from(d.let_in_level).ok() == Some(self.cur_level)
                        && d.in_recursive
                        && !d.defined
                });
                if undefined {
                    let at = self.m.node(o).span;
                    let n = self.sym_text(o);
                    self.err(
                        Code::RecursiveNotDefined,
                        at,
                        format!("`{n}` is declared RECURSIVE but never defined"),
                    );
                }
            }
            self.unresolved_sum -= self.unresolved_cnt[self.cur_level];
        }
    }

    // ---- Labels (SANY's LS stack) ----------------------------------------------------

    pub(crate) fn push_ls(&mut self) {
        self.ls_labels.push(None);
        self.ls_params.push(Vec::new());
    }

    pub(crate) fn pop_label_node_set(&mut self) -> Option<BTreeMap<Name, NodeId>> {
        self.ls_params.pop();
        self.ls_labels.pop().flatten()
    }

    pub(crate) fn add_label_node_to_set(&mut self, label: NodeId, name: Name) -> bool {
        let Some(top) = self.ls_labels.last_mut() else {
            return false;
        };
        let t = top.get_or_insert_with(BTreeMap::new);
        if t.contains_key(&name) {
            return false;
        }
        t.insert(name, label);
        true
    }

    pub(crate) fn push_formal_params(&mut self, params: Vec<NodeId>) {
        if let Some(top) = self.ls_params.last_mut() {
            top.push(params);
        }
    }

    pub(crate) fn pop_formal_params(&mut self) {
        if let Some(top) = self.ls_params.last_mut() {
            top.pop();
        }
    }

    pub(crate) fn in_op_def_node(&self) -> bool {
        !self.ls_labels.is_empty()
    }
}

/// The `CONSTANT` (or `VARIABLE`) declarations in a module's context, in
/// SANY's order (`ModuleNode.getConstantDecls`, `getVariableDecls`): the
/// reverse of the context `Hashtable`'s enumeration order
/// ([`crate::hashtable`]). The entries are fed in insertion order, except that
/// SANY inserted the copied built-ins newest first, which cannot change the
/// declarations' order (see the module's documentation).
pub(crate) fn decls_in(m: &Model, module: NodeId, variables: bool) -> Vec<NodeId> {
    let NodeKind::Module(d) = &m.node(module).kind else {
        return Vec::new();
    };
    let ctx = m.context(d.ctx);
    let keys: Vec<(Key, NodeId)> = ctx.entries().collect();
    let hashes: Vec<i32> = keys
        .iter()
        .map(|(k, _)| match k {
            Key::Sym(n) | Key::Module(n) => crate::hashtable::java_string_hash(m.name(*n)),
        })
        .collect();
    let want = if variables {
        DeclKind::Variable
    } else {
        DeclKind::Constant
    };
    let mut out: Vec<NodeId> = crate::hashtable::enumeration_order(&hashes)
        .into_iter()
        .filter_map(|i| keys.get(i).map(|e| e.1))
        .filter(|v| matches!(&m.node(*v).kind, NodeKind::OpDecl(od) if od.decl == want))
        .collect();
    out.reverse();
    out
}
