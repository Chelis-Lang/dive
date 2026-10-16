//! Independent bounded existential decisions using necessary finite covers.
//! No checker generator is called. Covers include every satisfying assignment;
//! the original predicate checks correlations. Missing covers cause refusal.

use crate::State;
use dive_eval::{ConstBinding, Scope, SlotView, St};
use dive_ir::{
    Arg, BuiltinOp, ConstId, DeclId, DefId, DefScope, Effects, Expr, ExprId, JunctionKind, Level,
    OpRef, QuantKind, StdOp,
};
use dive_model::{Model, Pred};
use dive_value::{SetKind, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Work and retained-entry limits, not a total byte-memory guarantee.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Values in each variable cover.
    pub domain_values: usize,
    /// Cartesian assignments tested, including duplicates.
    pub assignments: usize,
    /// Expression visits in scanning and cover derivation.
    pub expressions: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            domain_values: 65_536,
            assignments: 1_000_000,
            expressions: 100_000,
        }
    }
}
/// A certified result. Refusal can never be interpreted as disabledness.
#[derive(Clone, Debug)]
pub enum Decision {
    /// A concrete assignment satisfies the requested relation.
    Enabled {
        /// Root projection in IR variable order. Hidden query assignments are
        /// retained separately; this projection need not satisfy ordinary Next.
        witness: State,
        /// Actual partial existential assignment, keyed by canonical variable
        /// name. Evaluate the original relation with ENABLED control and its
        /// branched lexical scope, rather than as an ordinary graph edge.
        assignment: Arc<dive_eval::NamedState>,
    },
    /// Every assignment in a proved finite cover fails the relation.
    Disabled,
}
/// A named inability to decide independently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExistsError {
    /// Unsupported finite-cover rule or evaluation context.
    Unsupported(String),
    /// Malformed input or an evaluation/comparison error.
    Invalid(String),
    /// A named work/entry bound.
    Limit(&'static str),
}
impl ExistsError {
    /// Stable independent-existence diagnostic.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "DIVE-T0020",
            Self::Invalid(_) => "DIVE-T0021",
            Self::Limit(_) => "DIVE-T0022",
        }
    }
}
impl std::fmt::Display for ExistsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: independent existence: ", self.code())?;
        match self {
            Self::Unsupported(s) => write!(f, "unsupported {s}"),
            Self::Invalid(s) => write!(f, "invalid {s}"),
            Self::Limit(s) => write!(f, "resource limit {s}"),
        }
    }
}
impl std::error::Error for ExistsError {}

// TLC's existential state compares variable names even when distinct module
// declarations supplied them. Retain a real declaration for checked construction,
// but use only the canonical name as the independent cover/search key.
#[derive(Clone, Debug)]
struct Variable {
    declaration: DeclId,
    name: Arc<str>,
}
impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Variable {}
impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Variable {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}
type Needed = BTreeSet<Variable>;
type Domains = BTreeMap<Variable, Vec<Value>>;
#[derive(Default)]
struct Cover {
    impossible: bool,
    domains: Domains,
}
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent lexical traversal modes are saved and restored, never admission certificates"
)]
struct Search<'a> {
    model: &'a Model,
    state: State,
    current: St,
    limits: Limits,
    steps: usize,
    invocation: Option<St>,
    state_source: bool,
    inspect_only: bool,
    configured_cover: bool,

    recursive_analysis_depth: usize,
    source_only: bool,
    query_allowed: bool,
    query_cover: bool,
    query_depth: usize,
    saw_query: bool,
    binding_limit: Option<usize>,
    assignments: usize,
}
impl Search<'_> {
    fn variable(&self, declaration: DeclId) -> Result<Variable, ExistsError> {
        let decl = self
            .model
            .ir
            .decls()
            .get(declaration.index())
            .ok_or_else(|| {
                ExistsError::Invalid("unknown existential variable declaration".into())
            })?;
        if !decl.variable {
            return Err(ExistsError::Invalid(
                "nonvariable existential target".into(),
            ));
        }
        Ok(Variable {
            declaration,
            name: Arc::from(decl.name.as_str()),
        })
    }

    fn root_variable(&self, var: dive_ir::VarId) -> Result<Variable, ExistsError> {
        let decl = self
            .model
            .ir
            .vars()
            .get(var.index())
            .ok_or_else(|| ExistsError::Invalid("unknown existential root variable".into()))?;
        self.variable(*decl)
    }

    // Independent structural lvalue resolution. Only canonical variable names
    // come from the evaluator; no checker target/cover/search decision is used.
    fn target(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        prime: bool,
        depth: usize,
    ) -> Result<Option<(Variable, bool)>, ExistsError> {
        let ev = &self.model.evaluator;
        ev.validate_predicate(scope, expr)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        // Preserve the original direct-variable cover's work charge. Additional
        // lexical wrappers each consume a visit before their projection.
        if let Expr::Var { var, primed } = self.model.ir.expr(expr) {
            return self
                .root_variable(*var)
                .map(|var| Some((var, prime || *primed)));
        }
        if !matches!(
            self.model.ir.expr(expr),
            Expr::Prime(_)
                | Expr::Builtin {
                    op: BuiltinOp::Prime,
                    ..
                }
                | Expr::SubstRef { .. }
                | Expr::Subst { .. }
                | Expr::Local(_)
                | Expr::Let { .. }
                | Expr::Label { .. }
                | Expr::Selected(_)
                | Expr::Def(_)
                | Expr::Apply {
                    callee: OpRef::Def(_),
                    ..
                }
        ) {
            return Ok(None);
        }
        self.step(depth)?;
        match self.model.ir.expr(expr) {
            Expr::Prime(body) => self.target(*body, scope, true, depth + 1),
            Expr::Builtin {
                op: BuiltinOp::Prime,
                args,
            } if args.len() == 1 => self.target(args[0], scope, true, depth + 1),
            Expr::SubstRef { decl, primed } => {
                let prime = prime || *primed;
                match ev
                    .scope_substitution(scope, *decl, &self.current_state(), prime)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    Some(SlotView::Lazy(body, captured)) => {
                        self.target(body, &captured, prime, depth + 1)
                    }
                    Some(SlotView::Unset) => Err(ExistsError::Unsupported(
                        "unbound substitution target".into(),
                    )),
                    None if self.model.ir.decl(*decl).variable => {
                        self.variable(*decl).map(|v| Some((v, prime)))
                    }
                    _ => Ok(None),
                }
            }
            Expr::Subst { body, .. } => {
                let entered = ev
                    .scope_subst(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.target(*body, &entered, prime, depth + 1)
            }
            Expr::Local(local) => match ev.scope_slot(scope, *local) {
                SlotView::Lazy(body, captured) => self.target(body, &captured, prime, depth + 1),
                _ => Ok(None),
            },
            Expr::Let { body, .. } => {
                let entered = ev
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.target(*body, &entered, prime, depth + 1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.target(*body, scope, prime, depth + 1)
            }
            Expr::Def(def) => {
                let def = ev.redirected(*def);
                let definition = self.model.ir.def(def);
                if definition.recursive || definition.std.is_some() || ev.is_value_override(def) {
                    return Ok(None);
                }
                let (body, entered) = self.enter(scope, def)?;
                self.target(body, &entered, prime, depth + 1)
            }
            Expr::Apply {
                callee: OpRef::Def(def),
                args,
            } => {
                let def = ev.redirected(*def);
                let definition = self.model.ir.def(def);
                if definition.recursive || definition.std.is_some() || ev.is_value_override(def) {
                    return Ok(None);
                }
                let entered = ev
                    .scope_apply(scope, def, args)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.target(definition.body, &entered, prime, depth + 1)
            }
            _ => Ok(None),
        }
    }

    fn step(&mut self, depth: usize) -> Result<(), ExistsError> {
        self.steps = self.steps.saturating_add(1);
        if depth > 256 || self.steps > self.limits.expressions {
            Err(ExistsError::Limit("expressions"))
        } else {
            Ok(())
        }
    }
    // Direct configured declarations retain their origin. Captured/formal
    // operators keep their existing refusal. These are actual lazy contexts,
    // never a finite-cover or successor-existence certificate.
    fn configured_call(
        &mut self,
        scope: &Scope,
        constant: ConstId,
        args: &[Arg],
        depth: usize,
    ) -> Result<(DefId, Scope), ExistsError> {
        self.step(depth)?;
        let ev = &self.model.evaluator;
        if self.invocation.is_none() {
            return Err(ExistsError::Unsupported(
                "configured operator outside pure existence".into(),
            ));
        }
        let Some(ConstBinding::Def(def)) = ev.constant_binding(constant) else {
            return Err(ExistsError::Unsupported(
                "unbound/value-bound configured action operator".into(),
            ));
        };
        let def = ev.redirected(def);
        let target = self.model.ir.defs().get(def.index()).ok_or_else(|| {
            ExistsError::Unsupported("missing configured action definition".into())
        })?;
        if self
            .model
            .ir
            .node(target.body)
            .effects
            .contains(Effects::USES_ENABLED)
        {
            return Err(ExistsError::Unsupported("configured ENABLED target".into()));
        }
        if target.scope != DefScope::TopLevel
            || target.recursive
            || target.std.is_some()
            || ev.is_value_override(def)
            || target.level > Level::Action
            || target.params.iter().any(|p| p.arity != 0)
        {
            return Err(ExistsError::Unsupported(
                "configured action operator target".into(),
            ));
        }
        if args.len() > self.limits.expressions {
            return Err(ExistsError::Limit("configured argument bindings"));
        }
        let called = ev
            .scope_apply(scope, def, args)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        let saved = (self.inspect_only, self.query_allowed);
        self.inspect_only = true;
        self.query_allowed = false;
        let result = (|| {
            for arg in args {
                let Arg::Expr { expr, .. } = arg else {
                    return Err(ExistsError::Unsupported(
                        "higher-order configured action argument".into(),
                    ));
                };
                self.scan(*expr, scope, false, &mut BTreeSet::new(), depth + 1)?;
            }
            // Retain configured origin through helpers, lazy slots and empty binders.
            // Inspect the target before any cover/domain evaluation can occur.
            self.scan(target.body, &called, false, &mut BTreeSet::new(), depth + 1)?;
            Ok((def, called))
        })();
        (self.inspect_only, self.query_allowed) = saved;
        result
    }

    fn check_native(op: Option<StdOp>) -> Result<(), ExistsError> {
        if op == Some(StdOp::TlcTLCGet) {
            return Err(ExistsError::Unsupported("checker introspection".into()));
        }
        Ok(())
    }
    fn read(&self, expr: ExprId, scope: &Scope, st: &St) -> Result<Value, ExistsError> {
        // Every core read is data-only, including queries hidden by actual
        // lazy/formal bindings. Query decisions belong to boolean_at instead.
        let limit = self
            .binding_limit
            .unwrap_or(self.limits.expressions)
            .min(self.limits.expressions);
        self.model
            .evaluator
            .check_finite_data_bindings(scope, expr, st, limit)
            .map_err(|e| match e.class() {
                dive_eval::ErrorClass::Unsupported => ExistsError::Unsupported(e.to_string()),
                dive_eval::ErrorClass::ResourceLimit => ExistsError::Limit("pure predicate work"),
                _ => ExistsError::Invalid(format!("{}: {e}", self.model.evaluator.location(expr))),
            })?;
        let result = if self.invocation.is_some() {
            self.model
                .evaluator
                .eval_scoped_pure(scope, expr, st, self.limits.expressions)
        } else {
            self.model.evaluator.eval_scoped(scope, expr, st)
        };
        result.map_err(|e| {
            if self.invocation.is_some() {
                match e.class() {
                    dive_eval::ErrorClass::Unsupported => {
                        return ExistsError::Unsupported(e.to_string());
                    }
                    dive_eval::ErrorClass::ResourceLimit => {
                        return ExistsError::Limit("pure predicate work");
                    }
                    _ => {}
                }
            }
            ExistsError::Invalid(format!("{}: {e}", self.model.evaluator.location(expr)))
        })
    }
    fn constant_state(&self) -> St {
        self.invocation
            .as_ref()
            .map_or_else(St::constant, St::at_constant)
    }
    fn current_state(&self) -> St {
        self.current.clone()
    }
    fn boolean(&mut self, expr: ExprId, scope: &Scope, st: &St) -> Result<bool, ExistsError> {
        if self
            .model
            .ir
            .node(expr)
            .effects
            .contains(Effects::USES_ENABLED)
            || matches!(
                self.model.ir.expr(expr),
                Expr::Def(_)
                    | Expr::Apply { .. }
                    | Expr::Local(_)
                    | Expr::Let { .. }
                    | Expr::Subst { .. }
                    | Expr::SubstRef { .. }
                    | Expr::Label { .. }
                    | Expr::Selected(_)
                    | Expr::Junction { .. }
                    | Expr::Not(_)
                    | Expr::Quant { .. }
            )
        {
            return self.boolean_at(expr, scope, st, 0);
        }
        match self.read(expr, scope, st)? {
            Value::Bool(b) => Ok(b),
            v => Err(ExistsError::Invalid(format!(
                "{} is not boolean",
                v.kind_name()
            ))),
        }
    }

    fn query_at(
        &mut self,
        action: ExprId,
        scope: &Scope,
        st: &St,
        depth: usize,
    ) -> Result<bool, ExistsError> {
        if !matches!(
            &st.s0,
            dive_eval::StateRef::Vals(_) | dive_eval::StateRef::Named(_)
        ) {
            return Err(ExistsError::Unsupported(
                "ENABLED needs a current state".into(),
            ));
        }
        let saved_current = std::mem::replace(&mut self.current, st.enabled_current());
        let saved = self.state.clone();
        if let dive_eval::StateRef::Vals(state) = &st.s0 {
            self.state = state.clone();
        }
        self.query_depth += 1;
        let result = self.decide_action(&Pred::new(action, scope.clone()), None, depth + 1);
        self.query_depth -= 1;
        self.state = saved;
        self.current = saved_current;
        result.map(|d| matches!(d, Decision::Enabled { .. }))
    }

    // Only Boolean spines are interpreted here. Data/native leaves remain in
    // the core evaluator; ENABLED is never delegated to its optional enumerator.
    fn boolean_at(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        st: &St,
        depth: usize,
    ) -> Result<bool, ExistsError> {
        self.step(depth)?;
        let ev = &self.model.evaluator;
        match self.model.ir.expr(expr) {
            Expr::Enabled(action) => return self.query_at(*action, scope, st, depth),
            Expr::Not(body) => return self.boolean_at(*body, scope, st, depth + 1).map(|b| !b),
            Expr::Junction { kind, items, .. } => {
                let conjunction = *kind == JunctionKind::And;
                for item in items {
                    if self.boolean_at(*item, scope, st, depth + 1)? != conjunction {
                        return Ok(!conjunction);
                    }
                }
                return Ok(conjunction);
            }
            Expr::Def(d)
                if !ev.is_value_override(ev.redirected(*d))
                    && self.model.ir.def(ev.redirected(*d)).std.is_none() =>
            {
                let (body, called) = self.enter(scope, *d)?;
                return self.boolean_at(body, &called, st, depth + 1);
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } if !ev.is_value_override(ev.redirected(*d))
                && self.model.ir.def(ev.redirected(*d)).std.is_none() =>
            {
                let called = ev
                    .scope_apply(scope, ev.redirected(*d), args)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.boolean_at(
                    self.model.ir.def(ev.redirected(*d)).body,
                    &called,
                    st,
                    depth + 1,
                );
            }
            Expr::Let { body, .. } => {
                let called = ev
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.boolean_at(*body, &called, st, depth + 1);
            }
            Expr::Subst { body, .. } => {
                let called = ev
                    .scope_subst(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.boolean_at(*body, &called, st, depth + 1);
            }
            Expr::SubstRef { decl, primed } => {
                if let Some(SlotView::Lazy(body, captured)) = ev
                    .scope_substitution(scope, *decl, st, *primed)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    let context = if *primed {
                        st.primed_successor()
                    } else {
                        st.clone()
                    };
                    return self.boolean_at(body, &captured, &context, depth + 1);
                }
            }
            Expr::Local(local) => {
                if let SlotView::Lazy(body, captured) = ev.scope_slot(scope, *local) {
                    return self.boolean_at(body, &captured, st, depth + 1);
                }
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                return self.boolean_at(*body, scope, st, depth + 1);
            }
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                let scopes = self.finite_scopes(expr, scope, false, depth + 1)?;
                let conjunction = *kind == QuantKind::Forall;
                for called in scopes {
                    if self.boolean_at(*body, &called, st, depth + 1)? != conjunction {
                        return Ok(!conjunction);
                    }
                }
                return Ok(conjunction);
            }
            _ => {}
        }
        if self
            .model
            .ir
            .node(expr)
            .effects
            .contains(Effects::USES_ENABLED)
        {
            return Err(ExistsError::Unsupported(
                "ENABLED outside the Boolean spine".into(),
            ));
        }
        match self.read(expr, scope, st)? {
            Value::Bool(b) => Ok(b),
            v => Err(ExistsError::Invalid(format!(
                "{} is not boolean",
                v.kind_name()
            ))),
        }
    }

    fn scan_data(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        prime: bool,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        let saved = self.query_allowed;
        self.query_allowed = false;
        let result = self.scan(expr, scope, prime, needed, depth);
        self.query_allowed = saved;
        result
    }
    // Visit all primed reads, including expressions without a cover rule.
    #[allow(
        clippy::too_many_lines,
        reason = "explicit priming, constant-binding and opaque-context preflight rules"
    )]
    fn scan(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        prime: bool,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        self.step(depth)?;
        let node = self
            .model
            .ir
            .nodes()
            .get(expr.index())
            .ok_or_else(|| ExistsError::Unsupported("missing action expression".into()))?;
        if node.effects.intersects(Effects::IMPURE | Effects::TEMPORAL)
            || self.model.evaluator.scope_shares_side_effects(scope)
        {
            return Err(ExistsError::Unsupported(
                "effects, temporal expressions or nested ENABLED".into(),
            ));
        }
        if node.effects.contains(Effects::USES_ENABLED)
            && (!self.query_allowed
                || !matches!(
                    node.expr,
                    Expr::Enabled(_)
                        | Expr::Not(_)
                        | Expr::Junction { .. }
                        | Expr::Def(_)
                        | Expr::Apply {
                            callee: OpRef::Def(_),
                            ..
                        }
                        | Expr::Let { .. }
                        | Expr::Local(_)
                        | Expr::Subst { .. }
                        | Expr::SubstRef { .. }
                        | Expr::Quant {
                            kind: QuantKind::Forall | QuantKind::Exists,
                            ..
                        }
                        | Expr::Label { .. }
                        | Expr::Selected(_)
                ))
        {
            return Err(ExistsError::Unsupported(
                "ENABLED outside the Boolean spine or under priming".into(),
            ));
        }
        if self.state_source
            && (node.level > Level::State
                || matches!(
                    node.expr,
                    Expr::Prime(_)
                        | Expr::Unchanged(_)
                        | Expr::Var { primed: true, .. }
                        | Expr::StepBox { .. }
                        | Expr::StepAngle { .. }
                        | Expr::Cdot(_, _)
                        | Expr::Builtin {
                            op: BuiltinOp::Prime | BuiltinOp::Unchanged | BuiltinOp::Cdot,
                            ..
                        }
                        | Expr::Apply {
                            callee: OpRef::Builtin(
                                BuiltinOp::Prime | BuiltinOp::Unchanged | BuiltinOp::Cdot
                            ),
                            ..
                        }
                ))
        {
            return Err(ExistsError::Unsupported(
                "action/primed state helper source".into(),
            ));
        }
        if self.inspect_only {
            let body = match &node.expr {
                Expr::Quant { body, .. }
                | Expr::FcnCons { body, .. }
                | Expr::SetMap { expr: body, .. }
                | Expr::SetFilter { pred: body, .. } => Some(*body),
                _ => None,
            };
            if let Some(body) = body {
                let sc = self
                    .model
                    .evaluator
                    .scope_binder_analysis(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                for child in node.expr.children() {
                    self.scan(
                        child,
                        if child == body { &sc } else { scope },
                        prime,
                        needed,
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
        }
        match &node.expr {
            Expr::Enabled(action) => {
                if !self.query_allowed || prime || self.inspect_only {
                    return Err(ExistsError::Unsupported(
                        "primed/configured/captured ENABLED".into(),
                    ));
                }
                self.saw_query = true;
                // Inner primes belong only to the inner existential namespace.
                // Source inspection never evaluates its domains or runs a query.
                let saved = (self.source_only, self.state_source);
                self.source_only = true;
                self.state_source = false;
                self.query_depth += 1;
                let entered = self
                    .model
                    .evaluator
                    .scope_enabled(scope)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                let old_current = self.current.clone();
                self.current = self.current.enabled_current();
                let result = self.scan(*action, &entered, false, &mut BTreeSet::new(), depth + 1);
                self.current = old_current;
                self.query_depth -= 1;
                (self.source_only, self.state_source) = saved;
                result?;
            }
            Expr::Not(body) => self.scan(*body, scope, prime, needed, depth + 1)?,
            Expr::Junction { items, .. } => {
                for item in items {
                    self.scan(*item, scope, prime, needed, depth + 1)?;
                }
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.scan(*body, scope, prime, needed, depth + 1)?;
            }
            Expr::Cdot(..) => {
                return Err(ExistsError::Unsupported(
                    crate::COMPOSITION_UNSUPPORTED.to_owned(),
                ));
            }
            Expr::Builtin {
                op: BuiltinOp::Prime | BuiltinOp::Unchanged,
                args,
            } => {
                for e in args {
                    self.scan_data(*e, scope, true, needed, depth + 1)?;
                }
            }
            Expr::Var { var, primed } => {
                if prime || *primed {
                    needed.insert(self.root_variable(*var)?);
                }
            }
            Expr::Prime(e) | Expr::Unchanged(e) => {
                self.scan_data(*e, scope, true, needed, depth + 1)?;
            }
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                self.scan(*action, scope, prime, needed, depth + 1)?;
                self.scan_data(*sub, scope, true, needed, depth + 1)?;
            }
            Expr::Def(d) => {
                let d = self.model.evaluator.redirected(*d);
                if !self.model.evaluator.is_value_override(d) {
                    let def = self.model.ir.def(d);
                    if def.recursive || !def.params.is_empty() {
                        return Err(ExistsError::Unsupported(
                            "recursive/parameterized action definition".into(),
                        ));
                    }
                    Self::check_native(def.std)?;
                    if def.std.is_none() {
                        let sc = self
                            .model
                            .evaluator
                            .scope_enter(scope, d)
                            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                        self.scan(def.body, &sc, prime, needed, depth + 1)?;
                    }
                }
            }
            Expr::Apply { callee, args } => {
                if args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                    if (self.invocation.is_some() || self.source_only) && !self.inspect_only {
                        let mut remaining = self.limits.expressions.saturating_sub(self.steps);
                        let projection = self
                            .model
                            .evaluator
                            .scope_selectseq_call(scope, callee, args, &mut remaining)
                            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                        self.steps = self.limits.expressions.saturating_sub(remaining);
                        if let Some((sequence, bodies)) = projection {
                            self.scan(sequence, scope, prime, needed, depth + 1)?;
                            for (body, captured) in bodies {
                                self.scan_state_source(body, &captured, prime, needed, depth + 1)?;
                            }
                            return Ok(());
                        }
                    }
                    return Err(ExistsError::Unsupported(
                        "higher-order action argument".into(),
                    ));
                }
                match callee {
                    OpRef::Builtin(BuiltinOp::Prime | BuiltinOp::Unchanged) => {
                        return Err(ExistsError::Unsupported(
                            "indirect priming action operator".into(),
                        ));
                    }
                    OpRef::Std(op) => Self::check_native(Some(*op))?,
                    OpRef::Def(d) => {
                        let d = self.model.evaluator.redirected(*d);
                        let def = self.model.ir.def(d);
                        if !self.model.evaluator.is_value_override(d) && def.std.is_none() {
                            let sc = if self.inspect_only {
                                if def.recursive || def.params.iter().any(|p| p.arity != 0) {
                                    return Err(ExistsError::Unsupported(
                                        "recursive/higher-order configured argument".into(),
                                    ));
                                }
                                let sc = self
                                    .model
                                    .evaluator
                                    .scope_apply(scope, d, args)
                                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                                for arg in args {
                                    let Arg::Expr { expr, .. } = arg else {
                                        return Err(ExistsError::Unsupported(
                                            "higher-order configured argument".into(),
                                        ));
                                    };
                                    self.scan(*expr, scope, prime, needed, depth + 1)?;
                                }
                                sc
                            } else {
                                self.scoped_call(scope, d, args, prime, needed, depth + 1)?
                            };
                            return self.scan(def.body, &sc, prime, needed, depth + 1);
                        }
                        if !self.model.evaluator.is_value_override(d) {
                            Self::check_native(def.std)?;
                        }
                    }
                    OpRef::Const(constant) => {
                        let (def, called) =
                            self.configured_call(scope, *constant, args, depth + 1)?;
                        return self.scan_data(
                            self.model.ir.def(def).body,
                            &called,
                            prime,
                            needed,
                            depth + 1,
                        );
                    }
                    OpRef::Param(_) | OpRef::Subst(_) => {
                        return Err(ExistsError::Unsupported("dynamic action operator".into()));
                    }
                    _ => {}
                }
                for e in node.expr.children() {
                    self.scan_data(e, scope, prime, needed, depth + 1)?;
                }
            }
            Expr::RecFcn { bounds, body, .. } => {
                if self.inspect_only {
                    return Err(ExistsError::Unsupported(
                        "recursive function in configured action context".into(),
                    ));
                }
                if prime {
                    return Err(ExistsError::Unsupported(
                        "primed recursive function domain/capture".into(),
                    ));
                }
                for bound in bounds {
                    let domain = bound.domain.ok_or_else(|| {
                        ExistsError::Unsupported("unbounded recursive domain".into())
                    })?;
                    self.scan_state_data(domain, scope, false, needed, depth + 1)?;
                }
                let analysis = self
                    .model
                    .evaluator
                    .scope_binder_analysis(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.scan_recursive_source(*body, &analysis, needed, depth + 1)?;
                if self.recursive_analysis_depth == 0 {
                    let state = self.current_state();
                    let function = self
                        .model
                        .evaluator
                        .scope_recursive_function_bounded(
                            scope,
                            expr,
                            &state,
                            self.limits.domain_values.min(self.limits.expressions),
                        )
                        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                    self.scan_recursive_function(scope, &function, needed, depth + 1)?;
                }
            }
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                let scopes = self.finite_scopes(expr, scope, prime, depth + 1)?;
                for sc in scopes {
                    self.scan(*body, &sc, prime, needed, depth + 1)?;
                }
            }
            Expr::SetMap { expr: body, .. }
            | Expr::SetFilter { pred: body, .. }
            | Expr::FcnCons { body, .. } => {
                let scopes = self.finite_scopes(expr, scope, prime, depth + 1)?;
                for sc in scopes {
                    self.scan(*body, &sc, prime, needed, depth + 1)?;
                }
            }
            Expr::Const(c) => self.scan_constant(*c, scope, prime, needed, depth)?,
            Expr::Let { body, .. } => {
                let sc = self
                    .model
                    .evaluator
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.scan(*body, &sc, prime, needed, depth + 1)?;
            }
            Expr::Local(r) => match self.model.evaluator.scope_slot(scope, *r) {
                SlotView::Function(_) if self.inspect_only => {
                    return Err(ExistsError::Unsupported(
                        "recursive function in configured action context".into(),
                    ));
                }
                SlotView::Value(_) => {}
                SlotView::Lazy(body, _)
                    if self.query_depth != 0 && self.model.ir.node(body).level > Level::State =>
                {
                    return Err(ExistsError::Unsupported(
                        "captured primed ENABLED source".into(),
                    ));
                }
                SlotView::Lazy(body, sc) => self.scan(body, &sc, prime, needed, depth + 1)?,
                SlotView::AnalysisBound
                    if self.state_source || self.inspect_only || self.source_only => {}
                SlotView::Function(function)
                    if !prime && self.recursive_analysis_depth == 0 && !self.inspect_only =>
                {
                    self.scan_recursive_function_source(scope, &function, needed, depth + 1)?;
                }
                _ => {
                    return Err(ExistsError::Unsupported(
                        "opaque local action context".into(),
                    ));
                }
            },
            Expr::Subst { body, .. } => {
                let entered = self
                    .model
                    .evaluator
                    .scope_subst(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.scan(*body, &entered, prime, needed, depth + 1)?;
            }
            Expr::SubstRef { decl, primed } => {
                let prime = prime || *primed;
                match self
                    .model
                    .evaluator
                    .scope_substitution(scope, *decl, &self.current_state(), prime)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    Some(SlotView::Value(_)) => {}
                    Some(SlotView::Lazy(body, captured)) => {
                        self.scan(body, &captured, prime, needed, depth + 1)?;
                    }
                    Some(SlotView::AnalysisBound)
                        if self.state_source || self.inspect_only || self.source_only => {}
                    None if self.model.ir.decl(*decl).variable => {
                        if prime {
                            needed.insert(self.variable(*decl)?);
                        }
                    }
                    None => {
                        let Some(constant) = self.model.ir.decl(*decl).constant else {
                            return Err(ExistsError::Unsupported(
                                "unbound substituted declaration".into(),
                            ));
                        };
                        self.scan_constant(constant, scope, prime, needed, depth)?;
                    }
                    _ => {
                        return Err(ExistsError::Unsupported(
                            "opaque substituted action binding".into(),
                        ));
                    }
                }
            }
            _ => {
                for e in node.expr.children() {
                    self.scan_data(e, scope, prime, needed, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    fn scan_constant(
        &mut self,
        constant: ConstId,
        scope: &Scope,
        prime: bool,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        match self.model.evaluator.constant_binding(constant) {
            Some(ConstBinding::Value(_)) => {}
            Some(ConstBinding::Def(d)) => {
                let d = self.model.evaluator.redirected(d);
                let def = self.model.ir.def(d);
                if def.level != Level::Constant || def.recursive || !def.params.is_empty() {
                    return Err(ExistsError::Unsupported(
                        "nonconstant/recursive/parameterized constant override".into(),
                    ));
                }
                if !self.model.evaluator.is_value_override(d) {
                    Self::check_native(def.std)?;
                }
                let sc = self
                    .model
                    .evaluator
                    .scope_enter(scope, d)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.scan(def.body, &sc, prime, needed, depth + 1)?;
            }
            None => return Err(ExistsError::Unsupported("unbound action constant".into())),
        }
        Ok(())
    }
    fn scan_recursive_source(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        let saved = (self.recursive_analysis_depth, self.query_allowed);
        self.recursive_analysis_depth += 1;
        self.query_allowed = false;
        let result = self.scan_state_source(expr, scope, false, needed, depth);
        (self.recursive_analysis_depth, self.query_allowed) = saved;
        result
    }

    fn recursive_contexts(
        &self,
        scope: &Scope,
        function: &Arc<dive_eval::LazyFcn>,
    ) -> Result<dive_eval::RecursiveScopes, ExistsError> {
        self.model
            .evaluator
            .recursive_function_body_scopes_bounded(
                scope,
                function,
                &self.current_state(),
                self.limits.domain_values.min(self.limits.expressions),
            )
            .map_err(|e| match e.class() {
                dive_eval::ErrorClass::ResourceLimit => ExistsError::Limit("recursive bindings"),
                _ => ExistsError::Unsupported(e.to_string()),
            })
    }

    fn scan_recursive_function_source(
        &mut self,
        scope: &Scope,
        function: &Arc<dive_eval::LazyFcn>,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        let captured = self.recursive_contexts(scope, function)?;
        let analysis = self
            .model
            .evaluator
            .scope_binder_analysis(&captured.source, captured.constructor)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        self.scan_recursive_source(captured.body, &analysis, needed, depth + 1)
    }

    fn scan_recursive_function(
        &mut self,
        scope: &Scope,
        function: &Arc<dive_eval::LazyFcn>,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        let captured = self.recursive_contexts(scope, function)?;
        for binding in captured.bindings {
            self.scan(captured.body, &binding, false, needed, depth + 1)?;
        }
        Ok(())
    }

    fn scan_state_data(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        prime: bool,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        let saved = self.query_allowed;
        self.query_allowed = false;
        let result = self.scan_state_source(expr, scope, prime, needed, depth);
        self.query_allowed = saved;
        result
    }

    fn scan_state_source(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        prime: bool,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<(), ExistsError> {
        let saved = self.state_source;
        self.state_source = true;
        let result = self.scan(expr, scope, prime, needed, depth);
        self.state_source = saved;
        result
    }

    fn scoped_call(
        &mut self,
        scope: &Scope,
        def: DefId,
        args: &[Arg],
        prime: bool,
        needed: &mut Needed,
        depth: usize,
    ) -> Result<Scope, ExistsError> {
        let ev = &self.model.evaluator;
        let d = self.model.ir.def(def);
        if self.source_only {
            if d.recursive
                || d.std.is_some()
                || ev.is_value_override(def)
                || d.params.iter().any(|p| p.arity != 0)
                || args.iter().any(|a| matches!(a, Arg::Op { .. }))
            {
                return Err(ExistsError::Unsupported(
                    "opaque nested ENABLED helper".into(),
                ));
            }
            for arg in args {
                if let Arg::Expr { expr, .. } = arg {
                    self.scan_state_data(*expr, scope, prime, needed, depth + 1)?;
                }
            }
            return ev
                .scope_apply(scope, def, args)
                .map_err(|e| ExistsError::Unsupported(e.to_string()));
        }
        if self.invocation.is_none() || d.params.len() != 1 || d.level > Level::State {
            return self.constant_call(scope, def, args, depth);
        }
        self.step(depth)?;
        if d.recursive
            || d.std.is_some()
            || ev.is_value_override(def)
            || d.params.iter().any(|p| p.arity != 0)
            || args.len() != 1
        {
            return Err(ExistsError::Unsupported(
                "recursive/overridden/higher-order state helper".into(),
            ));
        }
        let Arg::Expr { expr: actual, .. } = args[0] else {
            return Err(ExistsError::Unsupported(
                "operator state helper argument".into(),
            ));
        };
        self.scan_state_data(actual, scope, prime, needed, depth + 1)?;
        // This retains source lexical thunks, never current-state actual values.
        let called = ev
            .scope_apply(scope, def, args)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        let mut remaining = self.limits.expressions.saturating_sub(self.steps);
        let captures = ev
            .scope_operator_captures(&called, &mut remaining)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?
            .ok_or_else(|| ExistsError::Unsupported("opaque state helper captures".into()))?;
        self.steps = self.limits.expressions.saturating_sub(remaining);
        for (body, captured) in captures {
            self.scan_state_source(body, &captured, prime, needed, depth + 1)?;
        }
        self.scan_state_source(d.body, &called, prime, needed, depth + 1)?;
        Ok(called)
    }

    // Necessary covers may inspect a parameter only after proving its actual
    // argument is pure constant data. No state/action thunk is evaluated early.
    fn constant_call(
        &mut self,
        scope: &Scope,
        def: DefId,
        args: &[Arg],
        depth: usize,
    ) -> Result<Scope, ExistsError> {
        self.step(depth)?;
        let ev = &self.model.evaluator;
        let d = self.model.ir.def(def);
        if args.len() > self.limits.expressions {
            return Err(ExistsError::Limit("constant argument bindings"));
        }
        if d.recursive
            || d.std.is_some()
            || ev.is_value_override(def)
            || d.params.iter().any(|p| p.arity != 0)
        {
            return Err(ExistsError::Unsupported(
                "recursive/overridden/higher-order action call".into(),
            ));
        }
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            let Arg::Expr { expr, .. } = arg else {
                return Err(ExistsError::Unsupported("operator action argument".into()));
            };
            if self.model.ir.node(*expr).level != Level::Constant {
                return Err(ExistsError::Unsupported(
                    "nonconstant action argument".into(),
                ));
            }
            // A LET/local reference can have constant formal metadata while
            // its captured expression reads a state variable. Prove data is
            // state-independent before evaluating it under the empty state.
            let mut variables = BTreeSet::new();
            self.scan(*expr, scope, true, &mut variables, depth + 1)?;
            if !variables.is_empty() {
                return Err(ExistsError::Unsupported(
                    "state-dependent action argument".into(),
                ));
            }
            values.push(self.read(*expr, scope, &self.constant_state())?);
        }
        ev.scope_call_values(scope, def, values)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))
    }

    // Finite binder values are concrete data in the current state. A prime
    // applies to a constructor's domain as well as its body; never speculate
    // about successor-dependent or unbounded domains.
    fn finite_scopes(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        prime: bool,
        depth: usize,
    ) -> Result<Vec<Scope>, ExistsError> {
        self.step(depth)?;
        let bounds: &[dive_ir::Bound] = match self.model.ir.expr(expr) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => return Err(ExistsError::Invalid("expected finite binder".into())),
        };
        for bound in bounds {
            let dom = bound
                .domain
                .ok_or_else(|| ExistsError::Unsupported("unbounded action quantifier".into()))?;
            if self.model.ir.node(dom).level > Level::State {
                return Err(ExistsError::Unsupported(
                    "primed action quantifier domain".into(),
                ));
            }
            let mut reads = BTreeSet::new();
            // A prime outside the quantifier also primes its domain. Binding
            // from the current state would hide body reads when that domain
            // is empty, and could falsely certify Disabled. Such domains need
            // a successor-dependent proof; they must not be guessed here.
            self.scan_data(dom, scope, prime, &mut reads, depth + 1)?;
            if !reads.is_empty() {
                return Err(ExistsError::Unsupported(
                    "primed action quantifier domain".into(),
                ));
            }
        }
        if self.recursive_analysis_depth != 0 || self.source_only {
            return self
                .model
                .evaluator
                .scope_binder_analysis(scope, expr)
                .map(|s| vec![s])
                .map_err(|e| ExistsError::Unsupported(e.to_string()));
        }
        let mut limit = self.limits.domain_values.min(self.limits.expressions);
        if self.query_depth != 0
            || self
                .model
                .ir
                .node(expr)
                .effects
                .contains(Effects::USES_ENABLED)
        {
            limit = limit.min(self.binding_limit.unwrap_or(limit));
        }
        self.model
            .evaluator
            .scope_bindings_bounded(scope, expr, &self.current_state(), limit)
            .map_err(|e| {
                if e.class() == dive_eval::ErrorClass::ResourceLimit {
                    ExistsError::Limit("quantifier bindings")
                } else {
                    ExistsError::Unsupported(format!("action quantifier bindings: {e}"))
                }
            })
    }

    fn enter(&self, scope: &Scope, d: dive_ir::DefId) -> Result<(ExprId, Scope), ExistsError> {
        let d = self.model.evaluator.redirected(d);
        let s = self
            .model
            .evaluator
            .scope_enter(scope, d)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        Ok((self.model.ir.def(d).body, s))
    }
    fn unchanged(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Cover, ExistsError> {
        self.step(depth)?;
        if let Some((variable, _)) = self.target(expr, scope, true, depth)? {
            let value = self.read(expr, scope, &self.current_state())?;
            return Ok(Cover {
                impossible: false,
                domains: Domains::from([(variable, vec![value])]),
            });
        }
        match self.model.ir.expr(expr) {
            Expr::Let { body, .. } => {
                let sc = self
                    .model
                    .evaluator
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.unchanged(*body, &sc, depth + 1)
            }
            Expr::Local(r) => match self.model.evaluator.scope_slot(scope, *r) {
                SlotView::Lazy(body, sc) => self.unchanged(body, &sc, depth + 1),
                _ => Ok(Cover::default()),
            },

            Expr::Subst { body, .. } => {
                let entered = self
                    .model
                    .evaluator
                    .scope_subst(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.unchanged(*body, &entered, depth + 1)
            }
            Expr::SubstRef { decl, .. } => {
                match self
                    .model
                    .evaluator
                    .scope_substitution(scope, *decl, &self.current_state(), true)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    Some(SlotView::Lazy(body, captured)) => {
                        self.unchanged(body, &captured, depth + 1)
                    }
                    _ => Ok(Cover::default()),
                }
            }
            Expr::Tuple(es) => {
                let mut c = Cover::default();
                for e in es {
                    c.domains
                        .extend(self.unchanged(*e, scope, depth + 1)?.domains);
                }
                Ok(c)
            }
            Expr::Def(d)
                if !self
                    .model
                    .evaluator
                    .is_value_override(self.model.evaluator.redirected(*d)) =>
            {
                let (body, s) = self.enter(scope, *d)?;
                self.unchanged(body, &s, depth + 1)
            }
            _ => Ok(Cover::default()),
        }
    }
    fn combine(
        &self,
        kind: JunctionKind,
        mut lhs: Cover,
        rhs: Cover,
    ) -> Result<Cover, ExistsError> {
        if kind == JunctionKind::And {
            lhs.impossible |= rhs.impossible;
            // Either necessary cover suffices. Preserve the first cover,
            // avoiding value-comparison side effects and retaining correlations
            // for the original relation to check.
            for (v, ds) in rhs.domains {
                lhs.domains.entry(v).or_insert(ds);
            }
            Ok(lhs)
        } else if lhs.impossible {
            Ok(rhs)
        } else if rhs.impossible {
            Ok(lhs)
        } else {
            lhs.domains.retain(|v, ds| {
                if let Some(other) = rhs.domains.get(v) {
                    ds.extend(other.iter().cloned());
                    true
                } else {
                    false
                }
            });
            if lhs
                .domains
                .values()
                .any(|d| d.len() > self.limits.domain_values)
            {
                return Err(ExistsError::Limit("domain values"));
            }
            Ok(lhs)
        }
    }
    fn junction(
        &mut self,
        kind: JunctionKind,
        items: &[ExprId],
        scope: &Scope,
        depth: usize,
    ) -> Result<Cover, ExistsError> {
        let mut combined = None;
        for e in items {
            let rhs = self.cover(*e, scope, depth + 1)?;
            combined = Some(match combined {
                Some(lhs) => self.combine(kind, lhs, rhs)?,
                None => rhs,
            });
        }
        Ok(combined.unwrap_or(Cover {
            impossible: kind == JunctionKind::Or,
            domains: Domains::new(),
        }))
    }
    fn quantified(
        &mut self,
        expr: ExprId,
        kind: QuantKind,
        body: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Cover, ExistsError> {
        let kind = if kind == QuantKind::Exists {
            JunctionKind::Or
        } else {
            JunctionKind::And
        };
        let scopes = self.finite_scopes(expr, scope, false, depth + 1)?;
        let mut combined = None;
        for sc in scopes {
            let rhs = self.cover(body, &sc, depth + 1)?;
            combined = Some(match combined {
                Some(lhs) => self.combine(kind, lhs, rhs)?,
                None => rhs,
            });
        }
        Ok(combined.unwrap_or(Cover {
            impossible: kind == JunctionKind::Or,
            domains: Domains::new(),
        }))
    }
    // Resolve only original lexical lazy expressions. A bound variable is
    // recognized from the resulting IR Var node, never from a numeric slot ID.
    fn operand(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<(ExprId, Scope), ExistsError> {
        self.step(depth)?;
        self.model
            .evaluator
            .validate_predicate(scope, expr)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        match self.model.ir.expr(expr) {
            Expr::Local(local) => match self.model.evaluator.scope_slot(scope, *local) {
                SlotView::Lazy(body, captured) => self.operand(body, &captured, depth + 1),
                _ => Ok((expr, scope.clone())),
            },
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.operand(*body, scope, depth + 1)
            }
            Expr::Let { body, .. } => {
                let entered = self
                    .model
                    .evaluator
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.operand(*body, &entered, depth + 1)
            }
            _ => Ok((expr, scope.clone())),
        }
    }

    fn binding(
        &mut self,
        op: BuiltinOp,
        args: &[ExprId],
        scope: &Scope,
        depth: usize,
    ) -> Result<Cover, ExistsError> {
        // Lazy operand resolution is needed only inside configured calls.
        // Ordinary covers retain their original work charges and callback order.
        let (mut a, mut b) = if self.configured_cover {
            (
                self.operand(args[0], scope, depth + 1)?,
                self.operand(args[1], scope, depth + 1)?,
            )
        } else {
            ((args[0], scope.clone()), (args[1], scope.clone()))
        };
        if op == BuiltinOp::Eq
            && self
                .target(b.0, &b.1, false, depth + 1)?
                .is_some_and(|(_, primed)| primed)
        {
            std::mem::swap(&mut a, &mut b);
        }
        let (a, target_scope) = a;
        let (b, scope) = b;
        let Some((var, true)) = self.target(a, &target_scope, false, depth + 1)? else {
            return Ok(Cover::default());
        };
        if self.model.ir.node(b).level > Level::State {
            return Ok(Cover::default());
        }
        if self.configured_cover
            || scope.has_substitutions()
            || matches!(
                self.model.ir.expr(b),
                Expr::Local(_)
                    | Expr::Def(_)
                    | Expr::Apply { .. }
                    | Expr::Subst { .. }
                    | Expr::SubstRef { .. }
                    | Expr::Let { .. }
            )
        {
            // A formal's constant metadata is insufficient for a RHS read.
            // Refuse a successor-dependent source rather than evaluate it as
            // current-state data or invent a cover from one guessed successor.
            let mut primed = BTreeSet::new();
            let saved = (self.inspect_only, self.source_only);
            if self.configured_cover {
                self.inspect_only = true;
            } else {
                self.source_only = true;
            }
            let result = self.scan_data(b, &scope, false, &mut primed, depth + 1);
            (self.inspect_only, self.source_only) = saved;
            result?;
            if !primed.is_empty() {
                return Ok(Cover::default());
            }
        }
        let value = self.read(b, &scope, &self.current_state())?;
        let ds = if op == BuiltinOp::Eq {
            vec![value]
        } else {
            let Value::Set(set) = value else {
                return Err(ExistsError::Invalid("membership cover is not a set".into()));
            };
            if !matches!(set.kind(), SetKind::Enum | SetKind::Raw | SetKind::Interval) {
                return Err(ExistsError::Unsupported(
                    "cover requires an explicit set or interval".into(),
                ));
            }
            let ctx = self.model.evaluator.options().ctx;
            let size = set
                .size(ctx)
                .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
            if size > u64::try_from(self.limits.domain_values).unwrap_or(u64::MAX) {
                return Err(ExistsError::Limit("domain values"));
            }
            set.elements(ctx)
                .map_err(|e| ExistsError::Invalid(e.to_string()))?
                .to_vec()
        };
        if ds.len() > self.limits.domain_values {
            return Err(ExistsError::Limit("domain values"));
        }
        Ok(Cover {
            impossible: ds.is_empty(),
            domains: Domains::from([(var, ds)]),
        })
    }
    #[allow(
        clippy::too_many_lines,
        reason = "original cover rules plus structural query-neutral guards; no query is evaluated for a cover"
    )]
    fn cover(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> Result<Cover, ExistsError> {
        self.step(depth)?;
        let node = self.model.ir.node(expr);
        match &node.expr {
            Expr::Subst { body, .. } => {
                let entered = self
                    .model
                    .evaluator
                    .scope_subst(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.cover(*body, &entered, depth + 1);
            }
            Expr::SubstRef { decl, primed } => {
                if *primed {
                    // The original relation still checks this primed value.
                    // Its captured body cannot be evaluated as a current-state
                    // guard or supply an inverse cover for a state expression.
                    return Ok(Cover::default());
                }
                if let Some(SlotView::Lazy(body, captured)) = self
                    .model
                    .evaluator
                    .scope_substitution(scope, *decl, &self.current_state(), *primed)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    return self.cover(body, &captured, depth + 1);
                }
            }
            Expr::Def(def)
                if !self
                    .model
                    .evaluator
                    .is_value_override(self.model.evaluator.redirected(*def))
                    && self
                        .model
                        .ir
                        .def(self.model.evaluator.redirected(*def))
                        .std
                        .is_none() =>
            {
                let (body, entered) = self.enter(scope, *def)?;
                return self.cover(body, &entered, depth + 1);
            }
            Expr::Apply {
                callee: OpRef::Def(def),
                args,
            } if !self
                .model
                .evaluator
                .is_value_override(self.model.evaluator.redirected(*def))
                && self
                    .model
                    .ir
                    .def(self.model.evaluator.redirected(*def))
                    .std
                    .is_none() =>
            {
                let def = self.model.evaluator.redirected(*def);
                let entered =
                    self.scoped_call(scope, def, args, false, &mut Needed::new(), depth + 1)?;
                return self.cover(self.model.ir.def(def).body, &entered, depth + 1);
            }
            Expr::Let { body, .. } => {
                let sc = self
                    .model
                    .evaluator
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.cover(*body, &sc, depth + 1);
            }
            Expr::Local(r) => {
                if let SlotView::Lazy(body, sc) = self.model.evaluator.scope_slot(scope, *r) {
                    return self.cover(body, &sc, depth + 1);
                }
            }
            Expr::Apply {
                callee: OpRef::Const(constant),
                args,
            } => {
                let (def, called) = self.configured_call(scope, *constant, args, depth + 1)?;
                let saved = self.configured_cover;
                self.configured_cover = true;
                let result = self.cover(self.model.ir.def(def).body, &called, depth + 1);
                self.configured_cover = saved;
                return result;
            }
            _ => {}
        }
        if node.effects.contains(Effects::USES_ENABLED) {
            match &node.expr {
                Expr::Def(d) => {
                    let (body, called) = self.enter(scope, *d)?;
                    return self.cover(body, &called, depth + 1);
                }
                Expr::Apply {
                    callee: OpRef::Def(d),
                    args,
                } => {
                    let d = self.model.evaluator.redirected(*d);
                    let called = self
                        .model
                        .evaluator
                        .scope_apply(scope, d, args)
                        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                    return self.cover(self.model.ir.def(d).body, &called, depth + 1);
                }
                Expr::Junction { kind, items, .. } => {
                    return self.junction(*kind, items, scope, depth + 1);
                }
                Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                    return self.quantified(expr, *kind, *body, scope, depth + 1);
                }
                Expr::Label { body, .. } | Expr::Selected(body) => {
                    return self.cover(*body, scope, depth + 1);
                }
                _ => return Ok(Cover::default()),
            }
        }
        if self.query_cover && node.level <= Level::State {
            return Ok(Cover::default());
        }
        if node.level <= Level::State {
            let mut primed = BTreeSet::new();
            if self.configured_cover {
                // Only lazy configured formals need this extra level check.
                // Preserve the ordinary cover path's work and callback order.
                let saved = self.inspect_only;
                self.inspect_only = true;
                let result = self.scan(expr, scope, false, &mut primed, depth + 1);
                self.inspect_only = saved;
                result?;
            }
            if primed.is_empty() {
                return Ok(Cover {
                    impossible: !self.boolean(expr, scope, &self.current_state())?,
                    domains: Domains::new(),
                });
            }
        }
        match &node.expr {
            Expr::Def(d)
                if !self
                    .model
                    .evaluator
                    .is_value_override(self.model.evaluator.redirected(*d)) =>
            {
                let (body, s) = self.enter(scope, *d)?;
                self.cover(body, &s, depth + 1)
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } if self
                .model
                .ir
                .def(self.model.evaluator.redirected(*d))
                .std
                .is_none()
                && !self
                    .model
                    .evaluator
                    .is_value_override(self.model.evaluator.redirected(*d)) =>
            {
                let d = self.model.evaluator.redirected(*d);
                let sc =
                    self.scoped_call(scope, d, args, false, &mut BTreeSet::new(), depth + 1)?;
                self.cover(self.model.ir.def(d).body, &sc, depth + 1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.cover(*body, scope, depth + 1),
            Expr::Junction { kind, items, .. } => self.junction(*kind, items, scope, depth),
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                self.quantified(expr, *kind, *body, scope, depth)
            }
            Expr::If { cond, then, other } if self.model.ir.node(*cond).level <= Level::State => {
                let b = self.boolean(*cond, scope, &self.current_state())?;
                self.cover(if b { *then } else { *other }, scope, depth + 1)
            }
            Expr::Unchanged(e) => self.unchanged(*e, scope, depth + 1),
            Expr::StepAngle { action, .. } => self.cover(*action, scope, depth + 1),
            Expr::Builtin { op, args }
                if matches!(op, BuiltinOp::Eq | BuiltinOp::In) && args.len() == 2 =>
            {
                self.binding(*op, args, scope, depth + 1)
            }
            _ => Ok(Cover::default()),
        }
    }
}

/// Decides existence from necessary finite covers, using TLA+ existential
/// semantics independently of the checker generator. Model constraints and
/// the reachable graph do not restrict `ENABLED`. A supplied subscript must
/// change (`<<A>>_v`). Every primed read must have a complete finite cover.
/// The root witness projection retains current values for unassigned roots.
/// The actual named assignment contains only the existential candidate's keys.
///
/// # Errors
/// Unsupported contexts/unbounded reads, malformed states, evaluation or
/// comparison errors, and the explicit work/entry limits. A string or model
/// value created after binding under encounter order refuses
/// ([`crate::RUNTIME_NAMES_UNSUPPORTED`]), whatever the decision.
pub fn exists_successor(
    model: &Model,
    action: &Pred,
    state: &State,
    nonstuttering: Option<&Pred>,
    limits: Limits,
) -> Result<Decision, ExistsError> {
    let decision = decide(model, action, state, nonstuttering, limits, None);
    if model.has_runtime_names() {
        return Err(ExistsError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
        ));
    }
    decision
}

/// Decides the original finite-cover membership relation with one fresh pure
/// predicate resource budget shared across cover reads and original candidate
/// evaluations. No checker context or admission decision is accepted.
///
/// # Errors
/// As [`exists_successor`], including bounded callback/context refusals.
pub fn exists_successor_pure(
    model: &Model,
    action: &Pred,
    state: &State,
    nonstuttering: Option<&Pred>,
    limits: Limits,
    binding_limit: usize,
) -> Result<Decision, ExistsError> {
    let decision = decide(
        model,
        action,
        state,
        nonstuttering,
        limits,
        Some(binding_limit),
    );
    if model.has_runtime_names() {
        return Err(ExistsError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
        ));
    }
    decision
}

#[allow(
    clippy::too_many_lines,
    reason = "keep original validation, cover and first-witness order explicit with resource initialization"
)]
fn decide(
    model: &Model,
    action: &Pred,
    state: &State,
    nonstuttering: Option<&Pred>,
    limits: Limits,
    binding_limit: Option<usize>,
) -> Result<Decision, ExistsError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    model
        .evaluator
        .validate_predicate(&action.scope, action.expr)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    if let Some(sub) = nonstuttering {
        model
            .evaluator
            .validate_predicate(&sub.scope, sub.expr)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    }
    if state.len() != model.ir.vars().len() || state.iter().any(Option::is_none) {
        return Err(ExistsError::Invalid(
            "state does not assign every variable".into(),
        ));
    }
    crate::trace::validate_state_names(model, state).map_err(ExistsError::Unsupported)?;
    if model.evaluator.scope_shares_side_effects(&action.scope)
        || nonstuttering.is_some_and(|s| model.evaluator.scope_shares_side_effects(&s.scope))
    {
        return Err(ExistsError::Unsupported("dynamic predicate scope".into()));
    }
    let mut cx = Search {
        model,
        state: state.clone(),
        current: St::state(state.clone()),
        limits,
        steps: 0,
        assignments: 0,
        source_only: false,
        query_allowed: true,
        query_cover: false,
        query_depth: 0,
        saw_query: false,
        recursive_analysis_depth: 0,
        binding_limit,
        invocation: binding_limit
            .map(|limit| {
                model.evaluator.begin_pure_predicate(
                    &action.scope,
                    &St::state(state.clone()),
                    limit,
                )
            })
            .transpose()
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?,
        state_source: false,
        inspect_only: false,
        configured_cover: false,
    };
    if let Some(invocation) = &cx.invocation {
        cx.current = invocation.clone();
    }
    cx.decide_action(action, nonstuttering, 0)
}

impl Search<'_> {
    // Only a query-free, state-only source prefix may decide false before
    // constructing covers. Later state guards remain neutral until candidates
    // are checked, so a later FALSE cannot erase an earlier query error.
    fn leading_guard(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Option<bool>, ExistsError> {
        self.step(depth)?;
        let ev = &self.model.evaluator;
        let node = self.model.ir.node(expr);
        match &node.expr {
            Expr::Enabled(_) | Expr::Quant { .. } => return Ok(None),
            Expr::Def(def)
                if !ev.is_value_override(ev.redirected(*def))
                    && self.model.ir.def(ev.redirected(*def)).std.is_none() =>
            {
                let (body, entered) = self.enter(scope, *def)?;
                return self.leading_guard(body, &entered, depth + 1);
            }
            Expr::Apply {
                callee: OpRef::Def(def),
                args,
            } if !ev.is_value_override(ev.redirected(*def))
                && self.model.ir.def(ev.redirected(*def)).std.is_none() =>
            {
                let def = ev.redirected(*def);
                let entered = ev
                    .scope_apply(scope, def, args)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.leading_guard(self.model.ir.def(def).body, &entered, depth + 1);
            }
            Expr::Let { body, .. } => {
                let entered = ev
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.leading_guard(*body, &entered, depth + 1);
            }
            Expr::Subst { body, .. } => {
                let entered = ev
                    .scope_subst(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                return self.leading_guard(*body, &entered, depth + 1);
            }
            Expr::SubstRef { decl, primed } => {
                if *primed {
                    return Ok(None);
                }
                if let Some(SlotView::Lazy(body, captured)) = ev
                    .scope_substitution(scope, *decl, &self.current_state(), false)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    return self.leading_guard(body, &captured, depth + 1);
                }
            }
            Expr::Local(local) => {
                if let SlotView::Lazy(body, captured) = ev.scope_slot(scope, *local) {
                    return self.leading_guard(body, &captured, depth + 1);
                }
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                return self.leading_guard(*body, scope, depth + 1);
            }
            Expr::Not(body) => {
                return self
                    .leading_guard(*body, scope, depth + 1)
                    .map(|b| b.map(|b| !b));
            }
            Expr::Junction { kind, items, .. } => {
                let conjunction = *kind == JunctionKind::And;
                for item in items {
                    match self.leading_guard(*item, scope, depth + 1)? {
                        Some(value) if value == conjunction => {}
                        other => return Ok(other),
                    }
                }
                return Ok(Some(conjunction));
            }
            _ => {}
        }
        if node.level <= Level::State && !node.effects.contains(Effects::USES_ENABLED) {
            self.boolean(expr, scope, &self.current_state()).map(Some)
        } else {
            Ok(None)
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "shared original scan, complete cover and first-witness order form one bounded transaction"
    )]
    fn decide_action(
        &mut self,
        action: &Pred,
        nonstuttering: Option<&Pred>,
        depth: usize,
    ) -> Result<Decision, ExistsError> {
        let child = if depth == 0 {
            0
        } else {
            self.step(depth)?;
            depth + 1
        };
        let model = self.model;
        let state = self.state.clone();
        let limits = self.limits;
        let mut action = action.clone();
        action.scope = model
            .evaluator
            .scope_enabled(&action.scope)
            .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
        let nonstuttering = nonstuttering
            .map(|sub| {
                let mut sub = sub.clone();
                sub.scope = model
                    .evaluator
                    .scope_enabled(&sub.scope)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                Ok::<_, ExistsError>(sub)
            })
            .transpose()?;
        self.current = self.current.enabled_current();
        let mut needed = BTreeSet::new();
        let saved_query = std::mem::replace(&mut self.saw_query, false);
        let scan: Result<(), ExistsError> = (|| {
            self.scan(action.expr, &action.scope, false, &mut needed, child)?;
            if let Some(sub) = &nonstuttering {
                if model
                    .ir
                    .nodes()
                    .get(sub.expr.index())
                    .ok_or_else(|| ExistsError::Unsupported("missing change subscript".into()))?
                    .level
                    > Level::State
                {
                    return Err(ExistsError::Unsupported(
                        "action-level change subscript".into(),
                    ));
                }
                self.scan(sub.expr, &sub.scope, true, &mut needed, child)?;
            }
            Ok(())
        })();
        let saw_query = self.saw_query;
        self.saw_query |= saved_query;
        scan?;
        let nested = self.query_depth != 0 || saw_query;
        if nested && self.leading_guard(action.expr, &action.scope, depth + 1)? == Some(false) {
            return Ok(Decision::Disabled);
        }
        let saved = self.query_cover;
        self.query_cover = nested;
        let cover = self.cover(action.expr, &action.scope, child);
        self.query_cover = saved;
        let cover = cover?;
        if nested && cover.impossible {
            return Err(ExistsError::Unsupported(
                "empty nested query cover needs source-ordered candidate evidence".into(),
            ));
        }
        if cover.impossible {
            return Ok(Decision::Disabled);
        }
        let mut domains: Vec<_> = needed
            .iter()
            .map(|v| {
                cover.domains.get(v).map(|d| (v, d)).ok_or_else(|| {
                    ExistsError::Unsupported(format!(
                        "no finite cover for primed variable {}",
                        v.name
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        // Retain the existing first-witness order for ordinary roots. Hidden
        // names come after them, while aliases of a root share its position.
        let root_order: BTreeMap<_, _> = model
            .ir
            .vars()
            .iter()
            .enumerate()
            .map(|(index, decl)| (model.ir.decl(*decl).name.as_str(), index))
            .collect();
        domains.sort_by(|(a, _), (b, _)| {
            let a_root = root_order.get(a.name.as_ref());
            let b_root = root_order.get(b.name.as_ref());
            match (a_root, b_root) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        });
        let count = domains
            .iter()
            .try_fold(1_usize, |n, (_, d)| n.checked_mul(d.len()))
            .filter(|n| *n <= limits.assignments.saturating_sub(self.assignments))
            .ok_or(ExistsError::Limit("assignments"))?;
        let before = nonstuttering
            .as_ref()
            .map(|p| self.read(p.expr, &p.scope, &self.current_state()))
            .transpose()?;
        for i in 0..count {
            if self.assignments >= limits.assignments {
                return Err(ExistsError::Limit("assignments"));
            }
            self.assignments += 1;
            let mut quotient = i;
            let mut values = Vec::with_capacity(domains.len());
            for (v, ds) in &domains {
                values.push((v.declaration, ds[quotient % ds.len()].clone()));
                quotient /= ds.len();
            }
            let assignment = model
                .evaluator
                .named_state(values)
                .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
            let action_state = self.current_state().enabled_candidate(assignment.clone());
            let holds = if nested {
                self.boolean_at(action.expr, &action.scope, &action_state, depth + 1)?
            } else {
                self.boolean(action.expr, &action.scope, &action_state)?
            };
            if !holds {
                continue;
            }
            if let (Some(sub), Some(before)) = (&nonstuttering, &before) {
                let after = self.read(sub.expr, &sub.scope, &action_state.primed_successor())?;
                if model
                    .evaluator
                    .equal_values(before, &after)
                    .map_err(|e| ExistsError::Invalid(e.to_string()))?
                {
                    continue;
                }
            }
            let mut projection = state.to_vec();
            for (index, declaration) in model.ir.vars().iter().enumerate() {
                if let Some(value) = assignment.get(&model.ir.decl(*declaration).name) {
                    projection[index] = Some(value.clone());
                }
            }
            return Ok(Decision::Enabled {
                witness: Arc::from(projection),
                assignment,
            });
        }
        Ok(Decision::Disabled)
    }
}

/// Independent Boolean predicate evaluation used only by periodic validation.
/// Nested queries share the enclosing source/candidate counters and invocation.
fn pure_search<'a>(
    model: &'a Model,
    pred: &Pred,
    st: &St,
    limits: Limits,
    binding_limit: usize,
) -> Result<(Search<'a>, St), ExistsError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    model
        .evaluator
        .validate_predicate(&pred.scope, pred.expr)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    let dive_eval::StateRef::Vals(state) = &st.s0 else {
        return Err(ExistsError::Unsupported(
            "nested predicate needs a current state".into(),
        ));
    };
    if state.len() != model.ir.vars().len() || state.iter().any(Option::is_none) {
        return Err(ExistsError::Invalid(
            "incomplete nested predicate state".into(),
        ));
    }
    crate::trace::validate_state_names(model, state).map_err(ExistsError::Unsupported)?;
    let invocation = model
        .evaluator
        .begin_pure_predicate(&pred.scope, st, binding_limit)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    let cx = Search {
        model,
        state: state.clone(),
        current: invocation.clone(),
        limits,
        steps: 0,
        assignments: 0,
        invocation: Some(invocation.clone()),
        state_source: false,
        inspect_only: false,
        configured_cover: false,
        source_only: true,
        query_allowed: true,
        query_cover: false,
        query_depth: 0,
        saw_query: false,
        recursive_analysis_depth: 0,
        binding_limit: Some(binding_limit),
    };
    Ok((cx, invocation))
}

// Runtime entry from an ordinary interpreter occurrence. The source scope is
// unbranched; decide_action owns the one query branch. Counters are carried
// across sibling occurrences, and nested occurrences use this same Search.
pub(crate) fn reached_query(
    model: &Model,
    action: &Pred,
    st: &St,
    limits: Limits,
    expressions: &mut usize,
    assignments: &mut usize,
) -> Result<bool, ExistsError> {
    let (mut cx, _) = pure_search(model, action, st, limits, 4096)?;
    cx.invocation = Some(st.clone());
    cx.current = st.clone();
    cx.steps = *expressions;
    cx.assignments = *assignments;
    cx.source_only = false;
    cx.query_depth = 1;
    let result = cx.decide_action(action, None, 0);
    *expressions = cx.steps;
    *assignments = cx.assignments;
    if model.has_runtime_names() {
        return Err(ExistsError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.into(),
        ));
    }
    result.map(|decision| matches!(decision, Decision::Enabled { .. }))
}

pub(crate) fn boolean_pure(
    model: &Model,
    pred: &Pred,
    st: &St,
    limits: Limits,
    binding_limit: usize,
) -> Result<bool, ExistsError> {
    let (mut cx, invocation) = pure_search(model, pred, st, limits, binding_limit)?;
    cx.scan(pred.expr, &pred.scope, false, &mut BTreeSet::new(), 0)?;
    cx.source_only = false;
    let result = cx.boolean_at(pred.expr, &pred.scope, &invocation, 0);
    if model.has_runtime_names() {
        return Err(ExistsError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.into(),
        ));
    }
    result
}

// The coordinator owns one invocation for subscript and body. In particular an
// action's nested queries cannot replenish work already consumed by its subscript.
pub(crate) fn action_pure(
    model: &Model,
    action: &Pred,
    sub: &Pred,
    st: &St,
    boxed: bool,
) -> Result<bool, ExistsError> {
    let (mut cx, invocation) = pure_search(model, action, st, Limits::default(), 4096)?;
    model
        .evaluator
        .validate_predicate(&sub.scope, sub.expr)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    cx.scan(action.expr, &action.scope, false, &mut BTreeSet::new(), 0)?;
    cx.scan_data(sub.expr, &sub.scope, true, &mut BTreeSet::new(), 0)?;
    let dive_eval::StateRef::Vals(after) = &st.s1 else {
        return Err(ExistsError::Unsupported(
            "temporal action needs an edge".into(),
        ));
    };
    if after.len() != model.ir.vars().len() || after.iter().any(Option::is_none) {
        return Err(ExistsError::Invalid("incomplete successor state".into()));
    }
    crate::trace::validate_state_names(model, after).map_err(ExistsError::Unsupported)?;
    let before = cx.current_state();
    let after = invocation.at_state(after.clone());
    model
        .evaluator
        .check_finite_data_bindings(&sub.scope, sub.expr, &before, 4096)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    model
        .evaluator
        .check_finite_data_bindings(&sub.scope, sub.expr, &after, 4096)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    let a = cx.read(sub.expr, &sub.scope, &before)?;
    let b = cx.read(sub.expr, &sub.scope, &after)?;
    let same = model
        .evaluator
        .equal_values(&a, &b)
        .map_err(|e| ExistsError::Invalid(e.to_string()))?;
    cx.source_only = false;
    let result = if same {
        Ok(boxed)
    } else {
        cx.boolean_at(action.expr, &action.scope, &invocation, 0)
    };
    if model.has_runtime_names() {
        return Err(ExistsError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.into(),
        ));
    }
    result
}

pub(crate) fn check_boolean_context(model: &Model, pred: &Pred) -> Result<(), ExistsError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    model
        .evaluator
        .validate_predicate(&pred.scope, pred.expr)
        .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
    let mut cx = Search {
        model,
        state: Arc::from([]),
        current: St::constant(),
        limits: Limits::default(),
        steps: 0,
        assignments: 0,
        invocation: None,
        state_source: false,
        inspect_only: false,
        configured_cover: false,
        source_only: true,
        query_allowed: true,
        query_cover: false,
        query_depth: 0,
        saw_query: false,
        recursive_analysis_depth: 0,
        binding_limit: None,
    };
    cx.scan(pred.expr, &pred.scope, false, &mut BTreeSet::new(), 0)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "owned recursive source-mode control"
)]
mod nested_source_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct Hooks(AtomicUsize);
    impl dive_eval::Host for Hooks {
        fn check_native_access(&self, _: StdOp) -> Result<(), dive_eval::HostError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "keep owned source/context and restoration assertions together"
    )]
    fn nested_recursive_source_refuses_actual_lazy_queries_and_restores_modes() {
        for options in [dive_eval::Options::tlc(), dive_eval::Options::dive()] {
            let source = "---- MODULE RecursiveSource ----\nEXTENDS Naturals\nVARIABLE x\nInit==x=0\nNext==x'=0\nMaker(p)==LET f[t \\in {0}]==IF t=0 THEN p ELSE f[t-1] IN f\nQ==ENABLED TRUE\nStart==Maker(Q)\n====\n";
            let a = dive_sema::analyze_source(
                "RecursiveSource.tla",
                source,
                &dive_sema::Options::default(),
            );
            assert!(a.accepted(), "{:?}", a.diagnostics());
            let ir = Arc::new(dive_ir::lower(&a).expect("owned IR"));
            assert_eq!(dive_ir::validate::check(&ir), [] as [String; 0]);
            let model = dive_model::bind(
                ir,
                &dive_config::parse("INIT Init\nNEXT Next\n")
                    .into_config()
                    .expect("config"),
                &dive_model::BindOptions {
                    eval: options,
                    ..dive_model::BindOptions::tlc()
                },
            )
            .expect("model");
            let hooks = Arc::new(Hooks::default());
            model.evaluator.set_host(hooks.clone());
            let start = model.ir.root_def("Start").expect("Start");
            let maker = model.ir.root_def("Maker").expect("Maker");
            let Expr::Apply { args, .. } = model.ir.expr(model.ir.def(start).body) else {
                panic!("actual call")
            };
            let scope = model
                .evaluator
                .scope_apply(&model.evaluator.scope(start).expect("scope"), maker, args)
                .expect("actual lazy argument");
            let outer = model.ir.def(maker).body;
            let Expr::Let { defs, .. } = model.ir.expr(outer) else {
                panic!("LET")
            };
            let rec = model.ir.def(defs[0]).body;
            let Expr::RecFcn { body, .. } = model.ir.expr(rec) else {
                panic!("recursive constructor")
            };
            assert!(!model.ir.node(*body).effects.contains(Effects::USES_ENABLED));
            let scope = model
                .evaluator
                .scope_let(&scope, outer)
                .expect("lexical scope");
            // Public construction rejects before creating a Function slot. The
            // private source-mode control below exercises its owned body directly.
            assert!(
                model
                    .evaluator
                    .scope_recursive_function_bounded(&scope, rec, &St::constant(), 4096)
                    .is_err()
            );
            let analysis = model
                .evaluator
                .scope_binder_analysis(&scope, rec)
                .expect("owned binder context");
            let next = model.next.as_ref().expect("Next");
            let (mut search, _) = pure_search(
                &model,
                next,
                &St::state(Arc::from([Some(0.into())])),
                Limits::default(),
                4096,
            )
            .expect("source search");
            let mut needed = BTreeSet::new();
            assert!(matches!(
                search.scan_recursive_source(*body, &analysis, &mut needed, 0),
                Err(ExistsError::Unsupported(_))
            ));
            assert_eq!(search.recursive_analysis_depth, 0);
            assert!(search.query_allowed);
            assert!(!search.state_source);
            let q = model.ir.root_def("Q").expect("Q");
            search
                .scan(
                    model.ir.def(q).body,
                    &model.evaluator.scope(q).expect("query scope"),
                    false,
                    &mut needed,
                    0,
                )
                .expect("ordinary source query permission restored");
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        }
    }
}
