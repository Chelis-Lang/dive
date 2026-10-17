//! Source-ordered independent query execution over partial named assignments.
//!
//! Finite covers remain an independent admission/completeness preflight. They
//! never create runtime scopes or choose the order of source assignments. The
//! pending predicates retain their actual lexical frames across the choices
//! inside them; a LET reached after a choice creates a new frame for that choice.

use super::{Decision, ExistsError, Search, Variable};
use dive_eval::{NamedState, Scope, SlotView};
use dive_ir::{BuiltinOp, Expr, ExprId, JunctionKind, OpRef, QuantKind};
use dive_model::Pred;
use dive_value::Value;
use std::{collections::BTreeMap, sync::Arc};

type Bindings = BTreeMap<Variable, Value>;
type Tasks = Option<Arc<Task>>;

#[derive(Clone, Copy)]
enum Mode {
    Predicate,
    Unchanged,
    Changed,
}

struct Task {
    pred: Pred,
    mode: Mode,
    next: Tasks,
}

impl Drop for Task {
    fn drop(&mut self) {
        // Wide conjunctions retain a pending list, not a recursively owned
        // Rust stack. Releasing a bounded failed search must stay iterative.
        let mut next = self.next.take();
        while let Some(link) = next {
            let Ok(mut task) = Arc::try_unwrap(link) else {
                break;
            };
            next = task.next.take();
        }
    }
}

impl Search<'_> {
    fn task(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        mode: Mode,
        next: Tasks,
        depth: usize,
    ) -> Result<Tasks, ExistsError> {
        self.step(depth)?;
        Ok(Some(Arc::new(Task {
            pred: Pred::new(expr, scope.clone()),
            mode,
            next,
        })))
    }

    fn assignment(&mut self, bindings: &Bindings) -> Result<Arc<NamedState>, ExistsError> {
        // Charge the copy before constructing the evaluator-owned state. The
        // original declarations supply checked names, including hidden names.
        for _ in bindings {
            self.step(0)?;
        }
        self.model
            .evaluator
            .named_state(
                bindings
                    .iter()
                    .map(|(v, value)| (v.declaration, value.clone())),
            )
            .map_err(|e| ExistsError::Unsupported(e.to_string()))
    }

    pub(super) fn runtime_action(
        &mut self,
        action: &Pred,
        nonstuttering: Option<&Pred>,
        before: Option<&Value>,
        depth: usize,
    ) -> Result<Decision, ExistsError> {
        let tasks = self.task(action.expr, &action.scope, Mode::Predicate, None, depth)?;
        let assignment =
            self.runtime_tasks(tasks, &Bindings::new(), nonstuttering.zip(before), depth)?;
        let Some(assignment) = assignment else {
            return Ok(Decision::Disabled);
        };
        let mut projection = self.state.to_vec();
        for (index, decl) in self.model.ir.vars().iter().enumerate() {
            if let Some(value) = assignment.get(&self.model.ir.decl(*decl).name) {
                projection[index] = Some(value.clone());
            }
        }
        Ok(Decision::Enabled {
            witness: projection.into(),
            assignment,
        })
    }

    fn failed_runtime_branch(&mut self) -> Result<Option<Arc<NamedState>>, ExistsError> {
        if self.assignments >= self.limits.assignments {
            return Err(ExistsError::Limit("assignments"));
        }
        self.assignments += 1;
        Ok(None)
    }

    fn runtime_bind(
        &mut self,
        tasks: Tasks,
        bindings: &Bindings,
        binding: (Variable, Value),
        changed: Option<(&Pred, &Value)>,
        depth: usize,
    ) -> Result<Option<Arc<NamedState>>, ExistsError> {
        self.step(depth)?;
        let (variable, value) = binding;
        if let Some(old) = bindings.get(&variable) {
            if !self
                .model
                .evaluator
                .equal_values(old, &value)
                .map_err(|e| ExistsError::Invalid(e.to_string()))?
            {
                return self.failed_runtime_branch();
            }
            return self.runtime_tasks(tasks, bindings, changed, depth + 1);
        }
        for _ in bindings {
            self.step(depth)?;
        }
        let mut extended = bindings.clone();
        extended.insert(variable, value);
        self.runtime_tasks(tasks, &extended, changed, depth + 1)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one source-ordered transaction keeps partial state and pending lexical contexts together"
    )]
    fn runtime_tasks(
        &mut self,
        mut tasks: Tasks,
        bindings: &Bindings,
        changed: Option<(&Pred, &Value)>,
        depth: usize,
    ) -> Result<Option<Arc<NamedState>>, ExistsError> {
        let model = self.model;
        let ev = &model.evaluator;
        while let Some(task) = tasks {
            self.step(depth)?;
            let expr = task.pred.expr;
            let scope = &task.pred.scope;
            let rest = task.next.clone();
            let state = self
                .current_state()
                .enabled_candidate(self.assignment(bindings)?);
            if matches!(task.mode, Mode::Changed) {
                let before = self.read(expr, scope, &self.current_state())?;
                let after = self.read(expr, scope, &state.primed_successor())?;
                if ev
                    .equal_values(&before, &after)
                    .map_err(|e| ExistsError::Invalid(e.to_string()))?
                {
                    return self.failed_runtime_branch();
                }
                tasks = rest;
                continue;
            }
            if matches!(task.mode, Mode::Unchanged) {
                if let Some((variable, _)) = self.target(expr, scope, true, depth + 1)? {
                    let value = self.read(expr, scope, &self.current_state())?;
                    return self.runtime_bind(
                        rest,
                        bindings,
                        (variable, value),
                        changed,
                        depth + 1,
                    );
                }
                if let Expr::Tuple(items) = model.ir.expr(expr) {
                    tasks = rest;
                    for item in items.iter().rev() {
                        tasks = self.task(*item, scope, Mode::Unchanged, tasks, depth + 1)?;
                    }
                    continue;
                }
            }
            // These wrappers create real runtime bindings only when reached.
            // Their body remains ahead of the caller's pending continuation.
            let expanded = match model.ir.expr(expr) {
                Expr::Def(def)
                    if !ev.is_value_override(ev.redirected(*def))
                        && model.ir.def(ev.redirected(*def)).std.is_none() =>
                {
                    Some(self.enter(scope, *def)?)
                }
                Expr::Apply {
                    callee: OpRef::Def(def),
                    args,
                } if !ev.is_value_override(ev.redirected(*def))
                    && model.ir.def(ev.redirected(*def)).std.is_none() =>
                {
                    let def = ev.redirected(*def);
                    Some((
                        model.ir.def(def).body,
                        ev.scope_apply(scope, def, args)
                            .map_err(|e| ExistsError::Unsupported(e.to_string()))?,
                    ))
                }
                Expr::Apply {
                    callee: OpRef::Const(constant),
                    args,
                } => {
                    let (def, called) = self.configured_call(scope, *constant, args, depth + 1)?;
                    Some((model.ir.def(def).body, called))
                }
                Expr::Let { body, .. } => Some((
                    *body,
                    ev.scope_eval_let(scope, expr)
                        .map_err(|e| ExistsError::Unsupported(e.to_string()))?,
                )),
                Expr::Subst { body, .. } => Some((
                    *body,
                    ev.scope_subst(scope, expr)
                        .map_err(|e| ExistsError::Unsupported(e.to_string()))?,
                )),
                Expr::Label { body, .. } | Expr::Selected(body) => Some((*body, scope.clone())),
                Expr::Local(local) => match ev.scope_slot(scope, *local) {
                    SlotView::Lazy(body, captured) => Some((body, captured)),
                    _ => None,
                },
                Expr::SubstRef {
                    decl,
                    primed: false,
                } => match ev
                    .scope_substitution(scope, *decl, &state, false)
                    .map_err(|e| ExistsError::Unsupported(e.to_string()))?
                {
                    Some(SlotView::Lazy(body, captured)) => Some((body, captured)),
                    _ => None,
                },
                _ => None,
            };
            if let Some((body, called)) = expanded {
                tasks = self.task(body, &called, task.mode, rest, depth + 1)?;
                continue;
            }
            if matches!(task.mode, Mode::Unchanged) {
                let before = self.read(expr, scope, &self.current_state())?;
                let after = self.read(expr, scope, &state.primed_successor())?;
                if !ev
                    .equal_values(&before, &after)
                    .map_err(|e| ExistsError::Invalid(e.to_string()))?
                {
                    return self.failed_runtime_branch();
                }
                tasks = rest;
                continue;
            }
            match model.ir.expr(expr) {
                Expr::Junction {
                    kind: JunctionKind::And,
                    items,
                    ..
                } => {
                    tasks = rest;
                    for item in items.iter().rev() {
                        tasks = self.task(*item, scope, Mode::Predicate, tasks, depth + 1)?;
                    }
                    continue;
                }
                Expr::Junction {
                    kind: JunctionKind::Or,
                    items,
                    ..
                } => {
                    for item in items {
                        let branch =
                            self.task(*item, scope, Mode::Predicate, rest.clone(), depth + 1)?;
                        if let Some(witness) =
                            self.runtime_tasks(branch, bindings, changed, depth + 1)?
                        {
                            return Ok(Some(witness));
                        }
                    }
                    return Ok(None);
                }
                Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                    let scopes = self.finite_scopes(expr, scope, false, depth + 1)?;
                    if *kind == QuantKind::Forall {
                        tasks = rest;
                        // Pinned forall reaches the first context and then
                        // the remaining pending contexts in reverse order.
                        for called in scopes.iter().skip(1) {
                            tasks = self.task(*body, called, Mode::Predicate, tasks, depth + 1)?;
                        }
                        if let Some(first) = scopes.first() {
                            tasks = self.task(*body, first, Mode::Predicate, tasks, depth + 1)?;
                        }
                        continue;
                    }
                    for called in scopes {
                        let branch =
                            self.task(*body, &called, Mode::Predicate, rest.clone(), depth + 1)?;
                        if let Some(witness) =
                            self.runtime_tasks(branch, bindings, changed, depth + 1)?
                        {
                            return Ok(Some(witness));
                        }
                    }
                    return Ok(None);
                }
                Expr::Case { arms, other } => {
                    let mut selected = *other;
                    for arm in arms {
                        if self.boolean_at(arm.guard, scope, &state, depth + 1)? {
                            selected = Some(arm.value);
                            break;
                        }
                    }
                    let body = selected.ok_or_else(|| {
                        ExistsError::Invalid("CASE has no true condition or OTHER".into())
                    })?;
                    tasks = self.task(body, scope, Mode::Predicate, rest, depth + 1)?;
                    continue;
                }
                Expr::StepAngle { action, sub } => {
                    let tail = self.task(*sub, scope, Mode::Changed, rest, depth + 1)?;
                    tasks = self.task(*action, scope, Mode::Predicate, tail, depth + 1)?;
                    continue;
                }
                Expr::StepBox { action, sub } => {
                    let branch =
                        self.task(*action, scope, Mode::Predicate, rest.clone(), depth + 1)?;
                    if let Some(witness) =
                        self.runtime_tasks(branch, bindings, changed, depth + 1)?
                    {
                        return Ok(Some(witness));
                    }
                    tasks = self.task(*sub, scope, Mode::Unchanged, rest, depth + 1)?;
                    continue;
                }
                Expr::If { cond, then, other } => {
                    let body = if self.boolean_at(*cond, scope, &state, depth + 1)? {
                        *then
                    } else {
                        *other
                    };
                    tasks = self.task(body, scope, Mode::Predicate, rest, depth + 1)?;
                    continue;
                }
                Expr::Implies(cond, body) => {
                    tasks = if self.boolean_at(*cond, scope, &state, depth + 1)? {
                        self.task(*body, scope, Mode::Predicate, rest, depth + 1)?
                    } else {
                        rest
                    };
                    continue;
                }
                Expr::Unchanged(body) => {
                    tasks = self.task(*body, scope, Mode::Unchanged, rest, depth + 1)?;
                    continue;
                }
                Expr::Builtin {
                    op: BuiltinOp::Unchanged,
                    args,
                } if args.len() == 1 => {
                    tasks = self.task(args[0], scope, Mode::Unchanged, rest, depth + 1)?;
                    continue;
                }
                Expr::Builtin { op, args }
                    if matches!(op, BuiltinOp::Eq | BuiltinOp::In) && args.len() == 2 =>
                {
                    if let Some((variable, true)) = self.target(args[0], scope, false, depth + 1)? {
                        if *op == BuiltinOp::Eq {
                            let value = self.read(args[1], scope, &state)?;
                            return self.runtime_bind(
                                rest,
                                bindings,
                                (variable, value),
                                changed,
                                depth + 1,
                            );
                        }
                        if !bindings.contains_key(&variable) {
                            let value = self.read(args[1], scope, &state)?;
                            let Value::Set(set) = value else {
                                return Err(ExistsError::Invalid(
                                    "membership domain is not a set".into(),
                                ));
                            };
                            let ctx = ev.options().ctx;
                            let size = set
                                .size(ctx)
                                .map_err(|e| ExistsError::Unsupported(e.to_string()))?;
                            if size > u64::try_from(self.limits.domain_values).unwrap_or(u64::MAX) {
                                return Err(ExistsError::Limit("domain values"));
                            }
                            let elements = set
                                .elements(ctx)
                                .map_err(|e| ExistsError::Invalid(e.to_string()))?;
                            for value in elements.iter() {
                                if let Some(witness) = self.runtime_bind(
                                    rest.clone(),
                                    bindings,
                                    (variable.clone(), value.clone()),
                                    changed,
                                    depth + 1,
                                )? {
                                    return Ok(Some(witness));
                                }
                            }
                            return Ok(None);
                        }
                    }
                }
                _ => {}
            }
            if !self.boolean_at(expr, scope, &state, depth + 1)? {
                return self.failed_runtime_branch();
            }
            tasks = rest;
        }
        if self.assignments >= self.limits.assignments {
            return Err(ExistsError::Limit("assignments"));
        }
        self.assignments += 1;
        let assignment = self.assignment(bindings)?;
        if let Some((sub, before)) = changed {
            let state = self.current_state().enabled_candidate(assignment.clone());
            let after = self.read(sub.expr, &sub.scope, &state.primed_successor())?;
            if ev
                .equal_values(before, &after)
                .map_err(|e| ExistsError::Invalid(e.to_string()))?
            {
                return Ok(None);
            }
        }
        Ok(Some(assignment))
    }
}
