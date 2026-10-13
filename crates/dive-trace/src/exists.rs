//! Independent bounded existential decisions using necessary finite covers.
//! No checker generator is called. Covers include every satisfying assignment;
//! the original predicate checks correlations. Missing covers cause refusal.

use crate::State;
use dive_eval::{ConstBinding, Scope, SlotView, St};
use dive_ir::{
    Arg, BuiltinOp, DefId, Effects, Expr, ExprId, JunctionKind, Level, OpRef, QuantKind, StdOp,
    VarId,
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
        /// Successor in IR variable order.
        witness: State,
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

type Domains = BTreeMap<VarId, Vec<Value>>;
#[derive(Default)]
struct Cover {
    impossible: bool,
    domains: Domains,
}
struct Search<'a> {
    model: &'a Model,
    state: &'a State,
    limits: Limits,
    steps: usize,
}
impl Search<'_> {
    fn step(&mut self, depth: usize) -> Result<(), ExistsError> {
        self.steps = self.steps.saturating_add(1);
        if depth > 256 || self.steps > self.limits.expressions {
            Err(ExistsError::Limit("expressions"))
        } else {
            Ok(())
        }
    }
    fn check_native(op: Option<StdOp>) -> Result<(), ExistsError> {
        if op == Some(StdOp::TlcTLCGet) {
            return Err(ExistsError::Unsupported("checker introspection".into()));
        }
        Ok(())
    }
    fn read(&self, expr: ExprId, scope: &Scope, st: &St) -> Result<Value, ExistsError> {
        self.model
            .evaluator
            .eval_scoped(scope, expr, st)
            .map_err(|e| {
                ExistsError::Invalid(format!("{}: {e}", self.model.evaluator.location(expr)))
            })
    }
    fn boolean(&self, expr: ExprId, scope: &Scope, st: &St) -> Result<bool, ExistsError> {
        match self.read(expr, scope, st)? {
            Value::Bool(b) => Ok(b),
            v => Err(ExistsError::Invalid(format!(
                "{} is not boolean",
                v.kind_name()
            ))),
        }
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
        needed: &mut BTreeSet<VarId>,
        depth: usize,
    ) -> Result<(), ExistsError> {
        self.step(depth)?;
        let node = self
            .model
            .ir
            .nodes()
            .get(expr.index())
            .ok_or_else(|| ExistsError::Unsupported("missing action expression".into()))?;
        if node
            .effects
            .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
            || self.model.evaluator.scope_shares_side_effects(scope)
        {
            return Err(ExistsError::Unsupported(
                "effects, temporal expressions or nested ENABLED".into(),
            ));
        }
        match &node.expr {
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
                    self.scan(*e, scope, true, needed, depth + 1)?;
                }
            }
            Expr::Var { var, primed } => {
                if prime || *primed {
                    needed.insert(*var);
                }
            }
            Expr::Prime(e) | Expr::Unchanged(e) => self.scan(*e, scope, true, needed, depth + 1)?,
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                self.scan(*action, scope, prime, needed, depth + 1)?;
                self.scan(*sub, scope, true, needed, depth + 1)?;
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
                            let sc = self.constant_call(scope, d, args, depth + 1)?;
                            return self.scan(def.body, &sc, prime, needed, depth + 1);
                        }
                        if !self.model.evaluator.is_value_override(d) {
                            Self::check_native(def.std)?;
                        }
                    }
                    OpRef::Param(_) | OpRef::Const(_) | OpRef::Subst(_) => {
                        return Err(ExistsError::Unsupported("dynamic action operator".into()));
                    }
                    _ => {}
                }
                for e in node.expr.children() {
                    self.scan(e, scope, prime, needed, depth + 1)?;
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
            Expr::Const(c) => match self.model.evaluator.constant_binding(*c) {
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
            },
            Expr::Let { body, .. } => {
                let sc = self
                    .model
                    .evaluator
                    .scope_let(scope, expr)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                self.scan(*body, &sc, prime, needed, depth + 1)?;
            }
            Expr::Local(r) => match self.model.evaluator.scope_slot(scope, *r) {
                SlotView::Value(_) => {}
                SlotView::Lazy(body, sc) => self.scan(body, &sc, prime, needed, depth + 1)?,
                _ => {
                    return Err(ExistsError::Unsupported(
                        "opaque local action context".into(),
                    ));
                }
            },
            Expr::SubstRef { .. } | Expr::Subst { .. } => {
                return Err(ExistsError::Unsupported(
                    "opaque substitution action context".into(),
                ));
            }
            _ => {
                for e in node.expr.children() {
                    self.scan(e, scope, prime, needed, depth + 1)?;
                }
            }
        }
        Ok(())
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
            values.push(self.read(*expr, scope, &St::constant())?);
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
            self.scan(dom, scope, prime, &mut reads, depth + 1)?;
            if !reads.is_empty() {
                return Err(ExistsError::Unsupported(
                    "primed action quantifier domain".into(),
                ));
            }
        }
        self.model
            .evaluator
            .scope_bindings_bounded(
                scope,
                expr,
                &St::state(self.state.clone()),
                self.limits.domain_values.min(self.limits.expressions),
            )
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

            Expr::Var { var, primed: false } => Ok(Cover {
                impossible: false,
                domains: Domains::from([(
                    *var,
                    vec![
                        self.state[var.index()]
                            .clone()
                            .ok_or_else(|| ExistsError::Invalid("unassigned variable".into()))?,
                    ],
                )]),
            }),
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
    fn binding(&self, op: BuiltinOp, args: &[ExprId], scope: &Scope) -> Result<Cover, ExistsError> {
        let mut a = args[0];
        let mut b = args[1];
        if op == BuiltinOp::Eq && matches!(self.model.ir.expr(b), Expr::Var { primed: true, .. }) {
            std::mem::swap(&mut a, &mut b);
        }
        let Expr::Var { var, primed: true } = self.model.ir.expr(a) else {
            return Ok(Cover::default());
        };
        if self.model.ir.node(b).level > Level::State {
            return Ok(Cover::default());
        }
        let value = self.read(b, scope, &St::state(self.state.clone()))?;
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
            domains: Domains::from([(*var, ds)]),
        })
    }
    fn cover(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> Result<Cover, ExistsError> {
        self.step(depth)?;
        let node = self.model.ir.node(expr);
        match &node.expr {
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
            _ => {}
        }
        if node.level <= Level::State {
            return Ok(Cover {
                impossible: !self.boolean(expr, scope, &St::state(self.state.clone()))?,
                domains: Domains::new(),
            });
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
                let sc = self.constant_call(scope, d, args, depth + 1)?;
                self.cover(self.model.ir.def(d).body, &sc, depth + 1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.cover(*body, scope, depth + 1),
            Expr::Junction { kind, items, .. } => self.junction(*kind, items, scope, depth),
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                self.quantified(expr, *kind, *body, scope, depth)
            }
            Expr::If { cond, then, other } if self.model.ir.node(*cond).level <= Level::State => {
                let b = self.boolean(*cond, scope, &St::state(self.state.clone()))?;
                self.cover(if b { *then } else { *other }, scope, depth + 1)
            }
            Expr::Unchanged(e) => self.unchanged(*e, scope, depth + 1),
            Expr::StepAngle { action, .. } => self.cover(*action, scope, depth + 1),
            Expr::Builtin { op, args }
                if matches!(op, BuiltinOp::Eq | BuiltinOp::In) && args.len() == 2 =>
            {
                self.binding(*op, args, scope)
            }
            _ => Ok(Cover::default()),
        }
    }
}

/// Decides existence from necessary finite covers, using TLA+ existential
/// semantics independently of the checker generator. Model constraints and
/// the reachable graph do not restrict `ENABLED`. A supplied subscript must
/// change (`<<A>>_v`). Every primed read must have a complete finite cover.
/// Variables never read primed may retain their current values.
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
    let decision = decide(model, action, state, nonstuttering, limits);
    if model.has_runtime_names() {
        return Err(ExistsError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
        ));
    }
    decision
}

fn decide(
    model: &Model,
    action: &Pred,
    state: &State,
    nonstuttering: Option<&Pred>,
    limits: Limits,
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
        state,
        limits,
        steps: 0,
    };
    let mut needed = BTreeSet::new();
    cx.scan(action.expr, &action.scope, false, &mut needed, 0)?;
    if let Some(sub) = nonstuttering {
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
        cx.scan(sub.expr, &sub.scope, true, &mut needed, 0)?;
    }
    let cover = cx.cover(action.expr, &action.scope, 0)?;
    if cover.impossible {
        return Ok(Decision::Disabled);
    }
    let domains: Vec<_> = needed
        .iter()
        .map(|v| {
            cover.domains.get(v).map(|d| (*v, d)).ok_or_else(|| {
                ExistsError::Unsupported(format!(
                    "no finite cover for primed variable {}",
                    v.index()
                ))
            })
        })
        .collect::<Result<_, _>>()?;
    let count = domains
        .iter()
        .try_fold(1_usize, |n, (_, d)| n.checked_mul(d.len()))
        .filter(|n| *n <= limits.assignments)
        .ok_or(ExistsError::Limit("assignments"))?;
    let before = nonstuttering
        .map(|p| cx.read(p.expr, &p.scope, &St::state(state.clone())))
        .transpose()?;
    for i in 0..count {
        let mut quotient = i;
        let mut values = state.to_vec();
        for (v, ds) in &domains {
            values[v.index()] = Some(ds[quotient % ds.len()].clone());
            quotient /= ds.len();
        }
        let witness: State = Arc::from(values);
        if !cx.boolean(
            action.expr,
            &action.scope,
            &St::action(state.clone(), witness.clone()),
        )? {
            continue;
        }
        if let (Some(sub), Some(before)) = (nonstuttering, &before) {
            let after = cx.read(sub.expr, &sub.scope, &St::state(witness.clone()))?;
            if model
                .evaluator
                .equal_values(before, &after)
                .map_err(|e| ExistsError::Invalid(e.to_string()))?
            {
                continue;
            }
        }
        return Ok(Decision::Enabled { witness });
    }
    Ok(Decision::Disabled)
}
