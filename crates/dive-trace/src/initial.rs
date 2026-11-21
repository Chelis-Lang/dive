//! Independent Init membership and single-candidate Next construction.

use std::sync::Arc;

use dive_eval::next::Flow;
use dive_eval::{ErrorClass, EvalError, HostControl, Scope, SlotView, St};
use dive_ir::{BuiltinOp, Effects, Expr, ExprId, JunctionKind, OpRef, VarId};
use dive_model::Model;
use dive_value::Value;

use crate::State;

pub(crate) const UNSUPPORTED: &str = "private register Init: the independent single-candidate operation cannot determine every assignment from empty slots";

/// Structural decline is separate from an error raised by source evaluation.
enum ConstructionError {
    Declined,
    Evaluation(EvalError),
}

impl From<EvalError> for ConstructionError {
    fn from(error: EvalError) -> Self {
        Self::Evaluation(error)
    }
}

impl ConstructionError {
    fn into_evaluation(self) -> EvalError {
        match self {
            Self::Declined => EvalError::unsupported(UNSUPPORTED),
            Self::Evaluation(error) => error,
        }
    }
}

/// One complete single-variable equality, with no candidate-dependent filters.
/// `None` delegates to the deterministic operation; it grants no random access.
pub(crate) fn random_check(model: &Model, witness: &State) -> Result<Option<bool>, EvalError> {
    if model.ir.vars().len() != 1 || model.init.len() != 1 {
        return Ok(None);
    }
    if witness
        .iter()
        .flatten()
        .any(|value| matches!(value, Value::Set(_) | Value::Fcn(_)))
    {
        return Ok(None);
    }
    let operation = Initial {
        model,
        slots: vec![None],
        source: None,
        target: None,
        choices: None,
    };
    let init = &model.init[0].pred;
    operation.random_predicate(&init.scope, init.expr, witness, 0)
}

/// `None` grants no membership proof. Pure callers may use a fresh predicate
/// runtime after a structural decline or a witness-comparison evaluation error.
/// Errors raised while actually evaluating source are retained separately.
///
/// With `choices` (a trace whose source draws), `v \in S` binds `v` to each
/// element of the evaluated `S` in turn ([`Initial::member`]).
pub(crate) fn check(
    model: &Model,
    witness: &State,
    control: Option<&dyn HostControl>,
    choices: Option<&crate::choices::Search>,
) -> Result<Option<bool>, EvalError> {
    let mut operation = Initial {
        model,
        slots: vec![None; model.ir.vars().len()],
        source: None,
        target: Some(witness),
        choices,
    };
    for init in &model.init {
        match operation.predicate(&init.pred.scope, init.pred.expr, 0) {
            Ok(true) => {}
            Ok(false) => return Ok(Some(false)),
            Err(ConstructionError::Declined) => return ordered_check(model, witness, control),
            Err(ConstructionError::Evaluation(error)) => return Err(error),
        }
    }
    if operation.slots.iter().any(Option::is_none) {
        return Ok(None);
    }
    match operation.matches(witness) {
        Ok(member) => Ok(Some(member)),
        // A supplied value of the wrong kind must keep the pure predicate
        // reader's Invalid classification. This is a comparison with witness
        // data, not an exception raised by the source construction above.
        Err(error) if error.class() == ErrorClass::Eval => Ok(None),
        Err(error) => Err(error),
    }
}

/// Pure relational Init membership uses actual source emissions, including empty
/// branches. A source error or partial emission is never proof of nonmembership.
/// Declining effectful/open contexts retains the existing private replay paths.
fn ordered_check(
    model: &Model,
    witness: &State,
    control: Option<&dyn HostControl>,
) -> Result<Option<bool>, EvalError> {
    let forbidden = Effects::IMPURE | Effects::LIVE_TLCGET | Effects::OPEN | Effects::TEMPORAL;
    if model.init.iter().any(|init| {
        model.ir.node(init.pred.expr).effects.intersects(forbidden)
            || model.evaluator.scope_has_effects(&init.pred.scope)
    }) || model.ir.defs().iter().enumerate().any(|(index, _)| {
        u32::try_from(index).map_or(true, |index| {
            let id = dive_ir::DefId(index);
            model.evaluator.redirected(id) != id
        })
    }) {
        return Ok(None);
    }
    // Finite private binding deliberately does not expand Next actions. Build
    // only these source-bound Init predicates, without evaluating them yet.
    let actions = model
        .init
        .iter()
        .enumerate()
        .map(|(index, init)| {
            model.evaluator.predicate_in(
                &init.pred.scope,
                init.pred.expr,
                dive_ir::actions::ActionName::Unnamed,
                index,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let poll = || control.map_or(Ok(()), |c| c.check().map_err(EvalError::interrupted));
    poll()?;
    let mut matched = false;
    let mut failure = None;
    let outcome = model.evaluator.init_states(&actions, &mut |candidate| {
        let compare = || {
            poll()?;
            if !candidate.complete() {
                return Err(EvalError::unsupported(
                    "independent Init membership encountered a partial initial state",
                ));
            }
            for (actual, expected) in candidate.state.iter().zip(witness.iter()) {
                let (Some(actual), Some(expected)) = (actual, expected) else {
                    return Ok(false);
                };
                if !model.evaluator.equal_values(actual, expected)? {
                    return Ok(false);
                }
            }
            Ok(true)
        };
        match compare() {
            Ok(false) => Flow::Continue,
            Ok(true) => {
                matched = true;
                Flow::Stop
            }
            Err(error) => {
                failure = Some(error);
                Flow::Stop
            }
        }
    });
    poll()?;
    if let Some(error) = failure {
        return Err(error);
    }
    outcome.map_err(|error| error.error)?;
    Ok(Some(matched))
}

/// `None` is a structural decline before any source evaluation. Its caller may
/// use ordinary predicate validation only with reached random calls refused.
pub(crate) fn next(
    model: &Model,
    source: &State,
    witness: &State,
    choices: &crate::choices::Search,
) -> Result<Option<bool>, EvalError> {
    // Binding can freeze constant action arguments or retain uncached LET
    // scopes. Reconstruct those source-bound actions, never their producer
    // states or generator output. The unsplit raw predicate loses that context.
    if model
        .actions
        .iter()
        .any(|action| !action_shape(model, action.expr(), 0))
    {
        return Ok(None);
    }
    if model.actions.is_empty() {
        return Ok(Some(false));
    }
    let choice = if model.actions.len() == 1 {
        0
    } else {
        choices
            .branch(model.actions.len())
            .map_err(|error| EvalError::unsupported(error.message))?
    };
    let action = model
        .actions
        .get(choice)
        .ok_or_else(|| EvalError::unsupported("independent random Next action choice"))?;
    let mut operation = Initial {
        model,
        slots: vec![None; model.ir.vars().len()],
        source: Some(source),
        target: Some(witness),
        choices: Some(choices),
    };
    if !operation
        .predicate(&action.scope(), action.expr(), 0)
        .map_err(ConstructionError::into_evaluation)?
    {
        return Ok(Some(false));
    }
    operation.matches(witness).map(Some)
}

fn action_shape(model: &Model, expr: ExprId, depth: u32) -> bool {
    if depth > 128 {
        return false;
    }
    match model.ir.expr(expr) {
        Expr::Def(def)
        | Expr::Apply {
            callee: OpRef::Def(def),
            ..
        } => {
            let def = model.evaluator.redirected(*def);
            model.evaluator.is_value_override(def)
                || model.evaluator.precomputed(def).is_some()
                || model.ir.def(def).std.is_some()
                || action_shape(model, model.ir.def(def).body, depth + 1)
        }
        Expr::Junction { items, .. } => items
            .iter()
            .all(|&expr| action_shape(model, expr, depth + 1)),
        Expr::Subst { body, .. }
        | Expr::Selected(body)
        | Expr::Label { body, .. }
        | Expr::Let { body, .. } => action_shape(model, *body, depth + 1),
        Expr::If { then, other, .. } => {
            action_shape(model, *then, depth + 1) && action_shape(model, *other, depth + 1)
        }
        Expr::Quant { .. }
        | Expr::Case { .. }
        | Expr::StepBox { .. }
        | Expr::StepAngle { .. }
        | Expr::Apply {
            callee: OpRef::Param(_),
            ..
        }
        | Expr::Local(_) => false,
        // `x' \in S` binds `x'` by choice ([`Initial::member`]).
        Expr::Builtin {
            op: BuiltinOp::In,
            args,
        } => args.len() == 2 && primed_variable(model, args[0]),
        _ => true,
    }
}

/// `x'` or `(x)'` for a state variable `x`, syntactically.
fn primed_variable(model: &Model, expr: ExprId) -> bool {
    match model.ir.expr(expr) {
        Expr::Var { primed, .. } => *primed,
        Expr::Prime(body) => matches!(model.ir.expr(*body), Expr::Var { primed: false, .. }),
        _ => false,
    }
}

/// The most elements `x \in S` offers as choices.
const MAX_MEMBERS: usize = 65_536;

struct Initial<'a> {
    model: &'a Model,
    slots: Vec<Option<Value>>,
    source: Option<&'a State>,
    /// The state the trace claims: only orders the alternatives tried.
    target: Option<&'a State>,
    choices: Option<&'a crate::choices::Search>,
}

impl Initial<'_> {
    fn random_predicate(
        &self,
        scope: &Scope,
        expr: ExprId,
        witness: &State,
        depth: u32,
    ) -> Result<Option<bool>, EvalError> {
        if depth > 128 {
            return Ok(None);
        }
        let ev = &self.model.evaluator;
        match self.model.ir.expr(expr) {
            Expr::Apply {
                callee: OpRef::Def(_),
                args,
            } if !args.is_empty() => Ok(None),
            Expr::Def(_)
            | Expr::Apply {
                callee: OpRef::Def(_),
                ..
            } => {
                let (inner, def) = ev.scope_call(scope, expr)?;
                if ev.is_value_override(def)
                    || ev.precomputed(def).is_some()
                    || self.model.ir.def(def).std.is_some()
                {
                    Ok(None)
                } else {
                    self.random_predicate(&inner, self.model.ir.def(def).body, witness, depth + 1)
                }
            }
            Expr::Selected(body) | Expr::Label { body, .. } => {
                self.random_predicate(scope, *body, witness, depth + 1)
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => {
                let var = match self.variable(scope, args[0], 0) {
                    Ok(Some(var)) => var,
                    Ok(None) | Err(ConstructionError::Declined) => return Ok(None),
                    Err(ConstructionError::Evaluation(error)) => return Err(error),
                };
                let Some(Some(candidate)) = witness.get(var.index()) else {
                    return Ok(None);
                };
                let st = St::initial(Arc::from(self.slots.clone()));
                ev.random_element_member(scope, args[1], &st, candidate, 65_536)
            }
            _ => Ok(None),
        }
    }

    fn st(&self) -> St {
        let slots: State = Arc::from(self.slots.clone());
        if let Some(source) = self.source {
            St::action(source.clone(), slots)
        } else {
            St::initial(slots)
        }
    }

    fn matches(&self, witness: &State) -> Result<bool, EvalError> {
        for (actual, expected) in self.slots.iter().zip(witness.iter()) {
            let (Some(actual), Some(expected)) = (actual, expected) else {
                return Err(EvalError::unsupported(UNSUPPORTED));
            };
            if !self.model.evaluator.equal_values(actual, expected)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn variable(
        &self,
        scope: &Scope,
        expr: ExprId,
        depth: u32,
    ) -> Result<Option<VarId>, ConstructionError> {
        self.variable_side(scope, expr, self.source.is_some(), depth)
    }

    fn variable_side(
        &self,
        scope: &Scope,
        expr: ExprId,
        primed: bool,
        depth: u32,
    ) -> Result<Option<VarId>, ConstructionError> {
        if depth > 128 {
            return Err(ConstructionError::Declined);
        }
        Ok(match self.model.ir.expr(expr) {
            Expr::Var {
                var,
                primed: actual,
            } if *actual == primed => Some(*var),
            Expr::Prime(body) if primed => {
                return self.variable_side(scope, *body, false, depth + 1);
            }
            Expr::Local(local) => match self.model.evaluator.scope_slot(scope, *local) {
                SlotView::Lazy(expr, scope) => {
                    return self.variable_side(&scope, expr, primed, depth + 1);
                }
                _ => None,
            },
            Expr::SubstRef {
                decl,
                primed: actual,
            } if *actual == primed => match self.model.evaluator.scope_subst_slot(scope, *decl) {
                SlotView::Unset => self.model.ir.decl(*decl).var,
                SlotView::Lazy(expr, scope) => {
                    return self.variable_side(&scope, expr, primed, depth + 1);
                }
                _ => None,
            },
            _ => None,
        })
    }

    fn unchanged(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        depth: u32,
    ) -> Result<bool, ConstructionError> {
        if depth > 128 {
            return Err(ConstructionError::Declined);
        }
        let ev = &self.model.evaluator;
        if let Some(var) = self.variable_side(scope, expr, false, depth + 1)? {
            let value = self
                .source
                .and_then(|source| source.get(var.index()))
                .and_then(Option::as_ref)
                .ok_or(ConstructionError::Declined)?
                .clone();
            if let Some(previous) = &self.slots[var.index()] {
                return ev.equal_values(previous, &value).map_err(Into::into);
            }
            self.slots[var.index()] = Some(value);
            return Ok(true);
        }
        match self.model.ir.expr(expr) {
            Expr::Tuple(items) => {
                for &item in items {
                    if !self.unchanged(scope, item, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Expr::Def(_)
            | Expr::Apply {
                callee: OpRef::Def(_),
                ..
            } => {
                let (inner, def) = ev.scope_call(scope, expr)?;
                self.unchanged(&inner, self.model.ir.def(def).body, depth + 1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.unchanged(scope, *body, depth + 1)
            }
            _ => Err(ConstructionError::Declined),
        }
    }

    fn predicate(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        depth: u32,
    ) -> Result<bool, ConstructionError> {
        if depth > 128 {
            return Err(ConstructionError::Declined);
        }
        let ev = &self.model.evaluator;
        match self.model.ir.expr(expr) {
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &item in items {
                    if !self.predicate(scope, item, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Expr::Def(_)
            | Expr::Apply {
                callee: OpRef::Def(_),
                ..
            } => {
                let (inner, def) = ev.scope_call(scope, expr)?;
                if ev.is_value_override(def)
                    || ev.precomputed(def).is_some()
                    || self.model.ir.def(def).std.is_some()
                {
                    self.filter(scope, expr)
                } else {
                    self.predicate(&inner, self.model.ir.def(def).body, depth + 1)
                }
            }
            Expr::Subst { body, .. } => {
                let inner = ev.scope_subst(scope, expr)?;
                self.predicate(&inner, *body, depth + 1)
            }
            Expr::Apply {
                callee: OpRef::Subst(decl),
                args,
            } => self.substituted_predicate(scope, expr, *decl, args, depth),
            Expr::Selected(body) | Expr::Label { body, .. } => {
                self.predicate(scope, *body, depth + 1)
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => self.equality(scope, expr, args[0], args[1]),
            Expr::Builtin {
                op: BuiltinOp::In,
                args,
            } if args.len() == 2 && self.choices.is_some() => {
                self.member(scope, expr, args[0], args[1])
            }
            Expr::Let { body, .. } => {
                let inner = ev.scope_let(scope, expr)?;
                self.predicate(&inner, *body, depth + 1)
            }
            Expr::Junction {
                kind: JunctionKind::Or,
                items,
                ..
            } if self.source.is_some() || self.choices.is_some() => {
                if items.is_empty() {
                    return Ok(false);
                }
                let choice = self
                    .choices
                    .ok_or(ConstructionError::Declined)?
                    .branch(items.len())
                    .map_err(|error| EvalError::unsupported(error.message))?;
                let item = *items.get(choice).ok_or(ConstructionError::Declined)?;
                self.predicate(scope, item, depth + 1)
            }
            Expr::If { cond, then, other } if self.source.is_some() || self.choices.is_some() => {
                let condition = ev.eval_scoped(scope, *cond, &self.st())?;
                let Some(condition) = condition.as_bool() else {
                    return Err(EvalError::unsupported(
                        "independent random Next condition is not Boolean",
                    )
                    .into());
                };
                self.predicate(scope, if condition { *then } else { *other }, depth + 1)
            }
            Expr::Unchanged(body) if self.source.is_some() => {
                self.unchanged(scope, *body, depth + 1)
            }
            // These relation forms can visit other candidates or bind slots.
            // Do not evaluate one selected by the supplied trace.
            Expr::Junction {
                kind: JunctionKind::Or,
                ..
            }
            | Expr::Quant { .. }
            | Expr::If { .. }
            | Expr::Case { .. }
            | Expr::Builtin {
                op: BuiltinOp::In, ..
            } => Err(ConstructionError::Declined),
            _ => self.filter(scope, expr),
        }
    }

    fn substituted_predicate(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        decl: dive_ir::DeclId,
        args: &[dive_ir::Arg],
        depth: u32,
    ) -> Result<bool, ConstructionError> {
        let ev = &self.model.evaluator;
        let def = ev
            .scope_subst_operator(scope, decl)?
            .ok_or(ConstructionError::Declined)?;
        if ev.is_value_override(def) || ev.precomputed(def).is_some() {
            return self.filter(scope, expr);
        }
        let inner = ev.scope_apply(scope, def, args)?;
        self.predicate(&inner, self.model.ir.def(def).body, depth + 1)
    }

    fn equality(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        left: ExprId,
        right: ExprId,
    ) -> Result<bool, ConstructionError> {
        let Some(var) = self.variable(scope, left, 0)? else {
            return self.filter(scope, expr);
        };
        if let Some(decided) = self.drawn_relation(scope, var, right, false)? {
            return Ok(decided);
        }
        let ev = &self.model.evaluator;
        let value = ev.eval_scoped(scope, right, &self.st())?;
        if let Some(previous) = &self.slots[var.index()] {
            ev.equal_values(previous, &value).map_err(Into::into)
        } else {
            self.slots[var.index()] = Some(value);
            Ok(true)
        }
    }

    /// `v \in S` for an unbound variable `v`: `S` is evaluated, then `v` is
    /// bound. With the state the trace claims, only its value of `v` can
    /// end in a match, so `v` is bound to that value when it is a member of
    /// `S` and the conjunct is false otherwise; without one, `v` is bound
    /// to each element in turn, as a choice point of the search. Anything
    /// else is a filter.
    fn member(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        left: ExprId,
        right: ExprId,
    ) -> Result<bool, ConstructionError> {
        let (Some(var), Some(choices)) = (self.variable(scope, left, 0)?, self.choices) else {
            return self.filter(scope, expr);
        };
        if self.slots[var.index()].is_some() {
            return self.filter(scope, expr);
        }
        if let Some(decided) = self.drawn_relation(scope, var, right, true)? {
            return Ok(decided);
        }
        let ev = &self.model.evaluator;
        let domain = ev.eval_scoped(scope, right, &self.st())?;
        if let Some(wanted) = self
            .target
            .and_then(|state| state.get(var.index()))
            .and_then(Option::as_ref)
        {
            if !dive_value::tlc::member(wanted, &domain, ev.options().ctx)
                .map_err(EvalError::from)?
            {
                return Ok(false);
            }
            self.slots[var.index()] = Some(wanted.clone());
            return Ok(true);
        }
        let Value::Set(set) = &domain else {
            return Err(EvalError::unsupported(
                "independent `\\in` choice over a value that is not a set",
            )
            .into());
        };
        let size = set.size(ev.options().ctx).map_err(EvalError::from)?;
        if size > MAX_MEMBERS as u64 {
            return Err(EvalError::unsupported(format!(
                "independent `\\in` choice over more than {MAX_MEMBERS} elements"
            ))
            .into());
        }
        let elements = set.elements(ev.options().ctx).map_err(EvalError::from)?;
        if elements.is_empty() {
            return Ok(false);
        }
        let index = choices
            .element(elements.len(), 0)
            .map_err(|error| EvalError::unsupported(error.message))?;
        let value = elements
            .get(index)
            .cloned()
            .ok_or(ConstructionError::Declined)?;
        self.slots[var.index()] = Some(value);
        Ok(true)
    }

    /// `v = Op(..)` (or `v \in Op(..)` with `member`) for a direct
    /// `RandomSubset` or `RandomSetOfSubsets` application `Op(..)`, in a
    /// search that replays draws, and an unbound `v` the trace's state
    /// gives: the arguments are evaluated and the relation decided from them
    /// by [`crate::draws`], with no draw. That draw is used nowhere else, so
    /// whether some draw satisfies the conjunct is the whole question. `None`
    /// leaves the conjunct to ordinary evaluation (not such an application,
    /// no target value, or arguments the operator rejects).
    fn drawn_relation(
        &mut self,
        scope: &Scope,
        var: VarId,
        right: ExprId,
        member: bool,
    ) -> Result<Option<bool>, ConstructionError> {
        let ev = &self.model.evaluator;
        // Only a run whose draws the search replays grants this authority;
        // `RandomElement` keeps its existing relation and enumeration.
        if self.choices.is_none() {
            return Ok(None);
        }
        let Some((op, args)) = ev
            .direct_draw(right)
            .filter(|(op, _)| *op != dive_ir::StdOp::TlcRandomElement)
        else {
            return Ok(None);
        };
        let Some(wanted) = self
            .target
            .and_then(|state| state.get(var.index()))
            .and_then(Option::as_ref)
        else {
            return Ok(None);
        };
        if self.slots[var.index()].is_some() {
            return Ok(None);
        }
        let st = self.st();
        let values = args
            .iter()
            .map(|&arg| ev.eval_scoped(scope, arg, &st))
            .collect::<Result<Vec<_>, _>>()?;
        let ctx = ev.options().ctx;
        let decided = if member {
            crate::draws::decide_member(op, &values, wanted, ctx)
        } else {
            crate::draws::decide(op, &values, wanted, ctx)
        };
        if decided == Some(true) {
            self.slots[var.index()] = Some(wanted.clone());
        }
        Ok(decided)
    }

    fn filter(&self, scope: &Scope, expr: ExprId) -> Result<bool, ConstructionError> {
        let st = self.st();
        match self.model.evaluator.eval_scoped(scope, expr, &st) {
            Ok(Value::Bool(value)) => Ok(value),
            Ok(value) => Err(EvalError::unsupported(format!(
                "{UNSUPPORTED}: a filter returned {}",
                value.kind_name()
            ))
            .into()),
            Err(error) => Err(error.into()),
        }
    }
}
