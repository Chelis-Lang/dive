//! Admission of public evaluator inputs, without executing expressions or callbacks.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use dive_ir::{Arg, DefId, DefScope, Expr, ExprId, OpRef};
use dive_value::Interner;

use crate::{ConstBinding, EvalError, Evaluator, Model, Scope};

/// A malformed public evaluator input. Numeric IDs are relative to the supplied IR;
/// equal indices alone cannot establish which IR a caller obtained them from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextError {
    kind: ContextErrorKind,
    detail: String,
}

/// Stable classifications for context admission failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContextErrorKind {
    /// An arena ID does not exist in the supplied IR.
    UnknownId,
    /// A definition, constant or operator argument has inconsistent arity.
    Arity,
    /// An override chain contains a nontrivial cycle.
    RedirectCycle,
    /// A captured scope or deferred value belongs to another IR or name table.
    ForeignContext,
    /// The IR or its binding table has an inconsistent shape.
    Structure,
}

impl ContextError {
    pub(crate) fn new(kind: ContextErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
    /// The typed failure category.
    #[must_use]
    pub const fn kind(&self) -> ContextErrorKind {
        self.kind
    }
    /// A stable diagnostic code independent of the message text.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self.kind {
            ContextErrorKind::UnknownId => "DIVE-EVAL-C001",
            ContextErrorKind::Arity => "DIVE-EVAL-C002",
            ContextErrorKind::RedirectCycle => "DIVE-EVAL-C003",
            ContextErrorKind::ForeignContext => "DIVE-EVAL-C004",
            ContextErrorKind::Structure => "DIVE-EVAL-C005",
        }
    }
    /// What failed admission, without formatting or evaluating user values.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}
impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.detail)
    }
}
impl std::error::Error for ContextError {}
impl From<ContextError> for EvalError {
    fn from(error: ContextError) -> Self {
        Self::unsupported(format!("invalid evaluator context ({error})"))
    }
}

type CResult<T> = Result<T, ContextError>;

/// Admits a public IR arena without creating an evaluator, interning literals,
/// evaluating expressions or invoking caller code. Uses the same structural
/// authority as evaluator construction with empty constant/override bindings.
///
/// # Errors
/// A typed malformed arena or bounded structural admission refusal. Numeric IDs
/// remain relative to this arena; this does not certify their foreign provenance.
///
/// ```
/// use std::sync::Arc;
/// let analysis = dive_sema::analyze_source("M.tla",
///     "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\n====\n", &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
/// dive_eval::validate_ir(&ir).expect("well-formed arena");
/// ```
pub fn validate_ir(ir: &Arc<dive_ir::Ir>) -> CResult<()> {
    validate(&mut Model::new(ir.clone()))
}

fn unknown(what: impl Into<String>) -> ContextError {
    ContextError::new(ContextErrorKind::UnknownId, what)
}

#[allow(
    clippy::too_many_lines,
    reason = "the ordered admission checks are reviewed together before any interning or callback"
)]
pub(crate) fn validate(model: &mut Model) -> CResult<()> {
    let ir = &model.ir;
    if ir.modules().get(ir.root().index()).is_none() {
        return Err(unknown("root module"));
    }
    for module in ir.modules() {
        if module
            .parent
            .is_some_and(|p| p.index() >= ir.modules().len())
            || module
                .extends
                .iter()
                .any(|p| p.index() >= ir.modules().len())
            || module
                .assumptions
                .iter()
                .chain(&module.theorems)
                .any(|p| p.index() >= ir.defs().len())
            || module
                .tlc_constants
                .iter()
                .any(|p| p.index() >= ir.decls().len())
            || module.context.iter().any(|(_, s)| match s {
                dive_ir::Symbol::Def(d) => d.index() >= ir.defs().len(),
                dive_ir::Symbol::Decl(d) => d.index() >= ir.decls().len(),
                dive_ir::Symbol::Instance => false,
            })
        {
            return Err(unknown("module references"));
        }
    }
    for decl in ir.decls() {
        if decl.module.index() >= ir.modules().len()
            || decl.var.is_some_and(|v| v.index() >= ir.vars().len())
            || decl
                .constant
                .is_some_and(|c| c.index() >= ir.consts().len())
        {
            return Err(unknown("declaration references"));
        }
    }
    if ir.tlc_var_order().len() != ir.vars().len()
        || ir
            .tlc_var_order()
            .iter()
            .map(|v| v.index())
            .collect::<HashSet<_>>()
            .len()
            != ir.vars().len()
        || ir
            .tlc_var_order()
            .iter()
            .any(|v| v.index() >= ir.vars().len())
    {
        return Err(ContextError::new(
            ContextErrorKind::Structure,
            "TLC variable order",
        ));
    }
    for def in ir.defs() {
        if def.params.len() > def.frame.len() {
            return Err(ContextError::new(
                ContextErrorKind::Arity,
                "parameters exceed frame slots",
            ));
        }
        let parent = match def.scope {
            DefScope::TopLevel => None,
            DefScope::Local { parent }
            | DefScope::Inline { parent }
            | DefScope::LetValue { parent, .. } => Some(parent),
        };
        if parent.is_some_and(|d| d.index() >= ir.defs().len())
            || def.source.is_some_and(|d| d.index() >= ir.defs().len())
            || def.frame.iter().any(|slot| matches!(slot.kind, dive_ir::SlotKind::LetValue(d) if d.index() >= ir.defs().len()))
        { return Err(unknown("definition frame references")); }
        if def.recursive && matches!(def.scope, DefScope::Inline { .. }) {
            return Err(ContextError::new(
                ContextErrorKind::Structure,
                "recursive inline frame",
            ));
        }
    }
    // Reject malformed trees before the existing frame-aware structural walk.
    for (i, node) in ir.nodes().iter().enumerate() {
        if let Expr::Apply { callee, args } = &node.expr {
            let expected = match callee {
                OpRef::Def(d) => Some(
                    ir.defs()
                        .get(d.index())
                        .ok_or_else(|| unknown("application definition"))?
                        .params
                        .len(),
                ),
                OpRef::Const(c) => Some(
                    ir.consts()
                        .get(c.index())
                        .and_then(|d| ir.decls().get(d.index()))
                        .ok_or_else(|| unknown("application constant"))?
                        .arity as usize,
                ),
                OpRef::Subst(d) => Some(
                    ir.decls()
                        .get(d.index())
                        .ok_or_else(|| unknown("application substitution"))?
                        .arity as usize,
                ),
                _ => None,
            };
            if expected.is_some_and(|n| n != args.len()) {
                return Err(ContextError::new(
                    ContextErrorKind::Arity,
                    "IR application argument count",
                ));
            }
        }
        if node
            .expr
            .try_for_each_child(|e| if e.index() >= i { Err(()) } else { Ok(()) })
            .is_err()
        {
            return Err(ContextError::new(
                ContextErrorKind::Structure,
                "expression child is not older than its parent",
            ));
        }
    }
    // Duplicate child references can turn a finite arena into an exponential
    // walk. Admit tree ownership before walking any lexical frame expansion.
    let mut parents = vec![false; ir.nodes().len()];
    let mut claim = |expr: ExprId| -> CResult<()> {
        let parent = parents
            .get_mut(expr.index())
            .ok_or_else(|| unknown("expression body or child"))?;
        if std::mem::replace(parent, true) {
            return Err(ContextError::new(
                ContextErrorKind::Structure,
                "expression has multiple parents",
            ));
        }
        Ok(())
    };
    for def in ir.defs() {
        claim(def.body)?;
    }
    for node in ir.nodes() {
        node.expr.try_for_each_child(&mut claim)?;
    }
    frame_expansion(ir)?;
    let (owners, _) = frame_metadata(ir);
    for (i, node) in ir.nodes().iter().enumerate() {
        let frame = ir
            .defs()
            .get(owners[i].index())
            .ok_or_else(|| unknown("expression frame"))?;
        let bounds: &[dive_ir::Bound] = match &node.expr {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. }
            | Expr::RecFcn { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => &[],
        };
        if bounds
            .iter()
            .flat_map(|b| b.vars.iter())
            .any(|s| s.index() >= frame.frame.len())
            || matches!(&node.expr, Expr::TemporalQuant { vars, .. } if vars.iter().any(|s| s.index() >= frame.frame.len()))
            || matches!(&node.expr, Expr::RecFcn { name, .. } if name.index() >= frame.frame.len())
        {
            return Err(unknown("binder frame slot"));
        }
    }
    let violations = dive_ir::validate::check(ir);
    if let Some(first) = violations.first() {
        return Err(ContextError::new(ContextErrorKind::Structure, first));
    }
    if model.constants.len() != ir.consts().len() {
        return Err(ContextError::new(
            ContextErrorKind::Structure,
            "constant binding table length",
        ));
    }
    for (&source, &target) in &model.redirects {
        let a = ir
            .defs()
            .get(source.index())
            .ok_or_else(|| unknown("redirect source"))?;
        let b = ir
            .defs()
            .get(target.index())
            .ok_or_else(|| unknown("redirect target"))?;
        if a.params
            .iter()
            .map(|p| p.arity)
            .ne(b.params.iter().map(|p| p.arity))
        {
            return Err(ContextError::new(
                ContextErrorKind::Arity,
                "redirect signature",
            ));
        }
    }
    let mut normalized = std::collections::HashMap::new();
    for &source in model.redirects.keys() {
        let mut seen = HashSet::new();
        let mut target = source;
        while let Some(&next) = model.redirects.get(&target) {
            if next == target {
                break;
            }
            if !seen.insert(target) {
                return Err(ContextError::new(
                    ContextErrorKind::RedirectCycle,
                    "definition redirect cycle",
                ));
            }
            target = next;
        }
        normalized.insert(source, target);
    }
    for (i, binding) in model.constants.iter().enumerate() {
        match binding {
            Some(ConstBinding::Def(d)) => {
                let def = ir
                    .defs()
                    .get(d.index())
                    .ok_or_else(|| unknown("constant definition binding"))?;
                if def.params.len() != ir.decl(ir.consts()[i]).arity as usize {
                    return Err(ContextError::new(
                        ContextErrorKind::Arity,
                        "constant definition arity",
                    ));
                }
            }
            Some(ConstBinding::Value(_)) if ir.decl(ir.consts()[i]).arity != 0 => {
                return Err(ContextError::new(
                    ContextErrorKind::Arity,
                    "parameterized constant bound to a value",
                ));
            }
            Some(ConstBinding::Value(_)) | None => {}
        }
    }
    for &d in model.def_values.keys() {
        let def = ir
            .defs()
            .get(d.index())
            .ok_or_else(|| unknown("definition value binding"))?;
        if !def.params.is_empty() {
            return Err(ContextError::new(
                ContextErrorKind::Arity,
                "parameterized definition bound to a value",
            ));
        }
    }
    model.redirects = normalized;
    Ok(())
}

pub(crate) fn install_configured_booleans(
    model: &mut Model,
    configured: std::collections::HashMap<DefId, bool>,
) -> CResult<()> {
    for (id, value) in configured {
        let def = model
            .ir
            .defs()
            .get(id.index())
            .ok_or_else(|| unknown("configured Boolean operator binding"))?;
        if def.params.is_empty() || def.std.is_some() || def.scope != DefScope::TopLevel {
            return Err(ContextError::new(
                ContextErrorKind::Arity,
                "configured Boolean must replace a parameterized user definition",
            ));
        }
        model.def_values.insert(id, dive_value::Value::Bool(value));
    }
    Ok(())
}

impl Evaluator {
    pub(crate) fn admit_def(&self, d: DefId) -> crate::Result<&dive_ir::Def> {
        self.core
            .ir
            .defs()
            .get(d.index())
            .ok_or_else(|| unknown("definition ID").into())
    }
    pub(crate) fn admit_expr(&self, e: ExprId) -> crate::Result<()> {
        if self.core.ir.nodes().get(e.index()).is_none() {
            return Err(unknown("expression ID").into());
        }
        Ok(())
    }
    /// Verifies that a scope was created by this evaluator.
    ///
    /// # Errors
    /// A scope created by another evaluator, even one over an equal IR.
    pub fn validate_scope(&self, scope: &Scope) -> crate::Result<()> {
        if !Arc::ptr_eq(&self.core.ir, &scope.owner)
            || !Arc::ptr_eq(&self.core.names, &scope.names)
            || !std::sync::Weak::ptr_eq(&scope.context, &Arc::downgrade(&self.core))
        {
            return Err(ContextError::new(
                ContextErrorKind::ForeignContext,
                "scope belongs to another evaluator",
            )
            .into());
        }
        Ok(())
    }
    pub(crate) fn admit_parent(
        &self,
        scope: &Scope,
        def: DefId,
    ) -> crate::Result<Option<Arc<crate::env::Frame>>> {
        let d = self.admit_def(def)?;
        let (parent, depth) = match d.scope {
            DefScope::TopLevel => return Ok(None),
            DefScope::Local { parent } | DefScope::Inline { parent } => {
                (parent, d.depth.saturating_sub(1))
            }
            DefScope::LetValue { parent, .. } => (parent, d.depth),
        };
        let frame = scope
            .env
            .frame_at_depth(depth)
            .filter(|f| f.def == parent)
            .ok_or_else(|| {
                ContextError::new(
                    ContextErrorKind::Structure,
                    "missing or incompatible enclosing frame",
                )
            })?;
        Ok(Some(frame.clone()))
    }
    pub(crate) fn admit_args(&self, scope: &Scope, def: DefId, args: &[Arg]) -> crate::Result<()> {
        self.validate_scope(scope)?;
        let d = self.admit_def(def)?;
        if args.len() != d.params.len() {
            return Err(
                ContextError::new(ContextErrorKind::Arity, "operator argument count").into(),
            );
        }
        for (arg, param) in args.iter().zip(&d.params) {
            let arity = match arg {
                Arg::Expr { expr, .. } => {
                    self.admit_scoped_expr(scope, *expr)?;
                    0
                }
                Arg::Op { op, .. } => match op {
                    OpRef::Def(d) => {
                        self.admit_parent(scope, *d)?;
                        self.admit_def(*d)?.params.len()
                    }
                    OpRef::Const(c) => {
                        self.core
                            .ir
                            .consts()
                            .get(c.index())
                            .and_then(|d| self.core.ir.decls().get(d.index()))
                            .ok_or_else(|| unknown("operator constant"))?
                            .arity as usize
                    }
                    OpRef::Subst(d) => {
                        self.core
                            .ir
                            .decls()
                            .get(d.index())
                            .ok_or_else(|| unknown("substituted operator"))?
                            .arity as usize
                    }
                    OpRef::Param(r) => {
                        let frame = scope
                            .env
                            .frame_up(r.up)
                            .ok_or_else(|| unknown("operator parameter frame"))?;
                        let owner = self.admit_def(frame.def)?;
                        if !matches!(frame.slots.get(r.slot.index()), Some(crate::env::Sv::Op(_))) {
                            return Err(ContextError::new(
                                ContextErrorKind::Structure,
                                "operator parameter is not bound to an operator",
                            )
                            .into());
                        }
                        owner
                            .params
                            .get(r.slot.index())
                            .ok_or_else(|| unknown("operator parameter slot"))?
                            .arity as usize
                    }
                    OpRef::Std(op) => usize::try_from(op.java_arity()).map_err(|_| {
                        ContextError::new(
                            ContextErrorKind::Arity,
                            "variadic standard operator argument",
                        )
                    })?,
                    OpRef::Builtin(op) => {
                        use dive_ir::BuiltinOp::{
                            Always, BigUnion, Boolean, Domain, Enabled, Eventually, Not, PowerSet,
                            Prime, String, Unchanged,
                        };
                        match op {
                            Boolean | String => 0,
                            PowerSet | BigUnion | Domain | Not | Prime | Enabled | Unchanged
                            | Always | Eventually => 1,
                            _ => 2,
                        }
                    }
                    OpRef::Unbound(_) => {
                        return Err(ContextError::new(
                            ContextErrorKind::UnknownId,
                            "unbound operator argument",
                        )
                        .into());
                    }
                },
            };
            if arity != param.arity as usize {
                return Err(ContextError::new(
                    ContextErrorKind::Arity,
                    "operator argument signature",
                )
                .into());
            }
        }
        Ok(())
    }
}

fn frame_expansion(ir: &dive_ir::Ir) -> CResult<()> {
    let mut incoming = vec![0_usize; ir.defs().len()];
    let mut edges = vec![Vec::new(); ir.defs().len()];
    for (i, def) in ir.defs().iter().enumerate() {
        let mut work = vec![def.body];
        while let Some(e) = work.pop() {
            let node = ir
                .nodes()
                .get(e.index())
                .ok_or_else(|| unknown("definition body"))?;
            let mut add = |d: DefId| -> CResult<()> {
                let count = incoming
                    .get_mut(d.index())
                    .ok_or_else(|| unknown("frame expansion definition"))?;
                *count += 1;
                edges[i].push(d.index());
                Ok(())
            };
            if let Expr::Let { defs, .. } = &node.expr {
                for &d in defs {
                    add(d)?;
                }
            }
            let inline = |d: DefId| {
                ir.defs()
                    .get(d.index())
                    .is_some_and(|d| matches!(d.scope, DefScope::Inline { .. }))
            };
            if let Expr::Def(d) = &node.expr
                && inline(*d)
            {
                add(*d)?;
            }
            for op in node.expr.op_refs() {
                if let OpRef::Def(d) = op
                    && inline(*d)
                {
                    add(*d)?;
                }
            }
            node.expr.for_each_child(|child| work.push(child));
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut count = 0;
    while let Some(i) = ready.pop() {
        count += 1;
        for &to in &edges[i] {
            incoming[to] -= 1;
            if incoming[to] == 0 {
                ready.push(to);
            }
        }
    }
    if count != incoming.len() {
        return Err(ContextError::new(
            ContextErrorKind::Structure,
            "cyclic lexical frame expansion",
        ));
    }
    Ok(())
}

pub(crate) fn frame_metadata(ir: &dive_ir::Ir) -> (Vec<DefId>, Vec<bool>) {
    let mut owners = vec![DefId(0); ir.nodes().len()];
    for (i, def) in ir.defs().iter().enumerate() {
        let mut owner = DefId(u32::try_from(i).unwrap_or(u32::MAX));
        for _ in 0..ir.defs().len() {
            match ir.defs().get(owner.index()).map(|d| d.scope) {
                Some(DefScope::LetValue { parent, .. }) if parent != owner => owner = parent,
                _ => break,
            }
        }
        let mut work = vec![def.body];
        while let Some(e) = work.pop() {
            if let Some(o) = owners.get_mut(e.index()) {
                *o = owner;
            }
            if let Some(node) = ir.nodes().get(e.index()) {
                node.expr.for_each_child(|child| work.push(child));
            }
        }
    }
    let mut required = vec![false; ir.nodes().len()];
    for (i, node) in ir.nodes().iter().enumerate() {
        let local_def = |d: DefId| {
            ir.defs()
                .get(d.index())
                .is_some_and(|d| d.scope != DefScope::TopLevel)
        };
        let local = matches!(
            node.expr,
            Expr::Local(_)
                | Expr::Let { .. }
                | Expr::Quant { .. }
                | Expr::SetFilter { .. }
                | Expr::SetMap { .. }
                | Expr::FcnCons { .. }
                | Expr::RecFcn { .. }
                | Expr::TemporalQuant { .. }
        ) || matches!(node.expr, Expr::Def(d) if local_def(d))
            || node.expr.op_refs().iter().any(|op| {
                matches!(op, OpRef::Param(_)) || matches!(op, OpRef::Def(d) if local_def(*d))
            });
        required[i] = local
            || node
                .expr
                .try_for_each_child(|e| {
                    if required.get(e.index()).copied().unwrap_or(true) {
                        Err(())
                    } else {
                        Ok(())
                    }
                })
                .is_err();
    }
    (owners, required)
}

impl Evaluator {
    /// Whether `expr` may be evaluated in `scope`: the scope was created by
    /// this evaluator and, when the expression needs a lexical frame, the
    /// scope is that frame's.
    ///
    /// ```no_run
    /// # fn check(ev: &dive_eval::Evaluator, scope: &dive_eval::Scope, e: dive_ir::ExprId) {
    /// if !ev.admits_scoped_expr(scope, e) {
    ///     // a foreign or mismatched scope
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn admits_scoped_expr(&self, scope: &Scope, expr: ExprId) -> bool {
        self.admit_scoped_expr(scope, expr).is_ok()
    }

    pub(crate) fn admit_scoped_expr(&self, scope: &Scope, expr: ExprId) -> crate::Result<()> {
        self.validate_scope(scope)?;
        self.admit_expr(expr)?;
        if self.core.frame_required[expr.index()]
            && scope.def() != self.core.frame_owners[expr.index()]
        {
            return Err(ContextError::new(
                ContextErrorKind::Structure,
                "expression requires another lexical frame",
            )
            .into());
        }
        Ok(())
    }
}

impl Evaluator {
    /// Checks that public model metadata still describes this evaluator's IR
    /// and name table. Cloning the same Arcs is valid; substituting another arena
    /// is not a certified binding, even when its numeric IDs happen to match.
    ///
    /// # Errors
    /// A foreign IR or name table.
    pub fn validate_model_context(
        &self,
        ir: &Arc<dive_ir::Ir>,
        names: &Arc<Interner>,
    ) -> CResult<()> {
        if !Arc::ptr_eq(ir, &self.core.ir) || !Arc::ptr_eq(names, &self.core.names) {
            return Err(ContextError::new(
                ContextErrorKind::ForeignContext,
                "model metadata belongs to another evaluator context",
            ));
        }
        Ok(())
    }
    /// Admits a predicate's ID and lexical frame before interpreting it.
    /// Closed expressions can be reused in any scope from the same IR.
    ///
    /// # Errors
    /// A foreign scope, missing expression or incompatible lexical frame.
    pub fn validate_predicate(&self, scope: &Scope, expr: ExprId) -> crate::Result<()> {
        self.admit_scoped_expr(scope, expr)
    }
}
