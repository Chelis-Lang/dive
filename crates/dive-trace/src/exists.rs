//! Independent bounded existential decisions using necessary finite covers.
//! No checker generator is called. Covers include every satisfying assignment;
//! the original predicate checks correlations. Missing covers cause refusal.

use crate::State;
use dive_eval::{Scope, St};
use dive_ir::{Arg, BuiltinOp, Effects, Expr, ExprId, JunctionKind, Level, OpRef, StdOp, VarId};
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
    fn scan(
        &mut self,
        expr: ExprId,
        prime: bool,
        needed: &mut BTreeSet<VarId>,
        seen: &mut BTreeSet<(ExprId, bool)>,
        depth: usize,
    ) -> Result<(), ExistsError> {
        self.step(depth)?;
        if !seen.insert((expr, prime)) {
            return Ok(());
        }
        let node = self.model.ir.node(expr);
        if node
            .effects
            .intersects(Effects::IMPURE | Effects::OPEN | Effects::TEMPORAL | Effects::USES_ENABLED)
        {
            return Err(ExistsError::Unsupported(
                "effects, temporal expressions or nested ENABLED".into(),
            ));
        }
        match &node.expr {
            Expr::Builtin {
                op: BuiltinOp::Prime | BuiltinOp::Unchanged,
                args,
            } => {
                for e in args {
                    self.scan(*e, true, needed, seen, depth + 1)?;
                }
            }
            Expr::Var { var, primed } => {
                if prime || *primed {
                    needed.insert(*var);
                }
            }
            Expr::Prime(e) | Expr::Unchanged(e) => self.scan(*e, true, needed, seen, depth + 1)?,
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                self.scan(*action, prime, needed, seen, depth + 1)?;
                self.scan(*sub, true, needed, seen, depth + 1)?;
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
                    if def.std == Some(StdOp::TlcTLCGet) {
                        return Err(ExistsError::Unsupported("checker introspection".into()));
                    }
                    if def.std.is_none() {
                        self.scan(def.body, prime, needed, seen, depth + 1)?;
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
                    OpRef::Std(StdOp::TlcTLCGet) => {
                        return Err(ExistsError::Unsupported("checker introspection".into()));
                    }
                    OpRef::Def(d) => {
                        let d = self.model.evaluator.redirected(*d);
                        let def = self.model.ir.def(d);
                        if !self.model.evaluator.is_value_override(d) && def.std.is_none() {
                            return Err(ExistsError::Unsupported(
                                "parameterized action application".into(),
                            ));
                        }
                        if def.std == Some(StdOp::TlcTLCGet) {
                            return Err(ExistsError::Unsupported("checker introspection".into()));
                        }
                    }
                    OpRef::Param(_) | OpRef::Const(_) | OpRef::Subst(_) => {
                        return Err(ExistsError::Unsupported("dynamic action operator".into()));
                    }
                    _ => {}
                }
                for e in node.expr.children() {
                    self.scan(e, prime, needed, seen, depth + 1)?;
                }
            }
            Expr::Local(_) | Expr::SubstRef { .. } | Expr::Subst { .. } | Expr::Let { .. } => {
                return Err(ExistsError::Unsupported(
                    "opaque local/substitution action context".into(),
                ));
            }
            _ => {
                for e in node.expr.children() {
                    self.scan(e, prime, needed, seen, depth + 1)?;
                }
            }
        }
        Ok(())
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
    fn junction(
        &mut self,
        kind: JunctionKind,
        items: &[ExprId],
        scope: &Scope,
        depth: usize,
    ) -> Result<Cover, ExistsError> {
        let mut combined: Option<Cover> = None;
        for e in items {
            let rhs = self.cover(*e, scope, depth + 1)?;
            let Some(mut lhs) = combined.take() else {
                combined = Some(rhs);
                continue;
            };
            if kind == JunctionKind::And {
                lhs.impossible |= rhs.impossible;
                // Either necessary cover suffices. Pick the first without
                // introducing comparison or intersection side effects.
                for (v, ds) in rhs.domains {
                    lhs.domains.entry(v).or_insert(ds);
                }
                combined = Some(lhs);
            } else if lhs.impossible {
                combined = Some(rhs);
            } else if rhs.impossible {
                combined = Some(lhs);
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
                combined = Some(lhs);
            }
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
            Expr::Label { body, .. } | Expr::Selected(body) => self.cover(*body, scope, depth + 1),
            Expr::Junction { kind, items, .. } => self.junction(*kind, items, scope, depth),
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
/// comparison errors, and the explicit work/entry limits.
pub fn exists_successor(
    model: &Model,
    action: &Pred,
    state: &State,
    nonstuttering: Option<&Pred>,
    limits: Limits,
) -> Result<Decision, ExistsError> {
    if state.len() != model.ir.vars().len() || state.iter().any(Option::is_none) {
        return Err(ExistsError::Invalid(
            "state does not assign every variable".into(),
        ));
    }
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
    let mut seen = BTreeSet::new();
    cx.scan(action.expr, false, &mut needed, &mut seen, 0)?;
    if let Some(sub) = nonstuttering {
        if model.ir.node(sub.expr).level > Level::State {
            return Err(ExistsError::Unsupported(
                "action-level change subscript".into(),
            ));
        }
        cx.scan(sub.expr, true, &mut needed, &mut seen, 0)?;
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
