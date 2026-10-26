//! Budget-backed canonical inspection. Tasks own only metadata/storage owners;
//! executing a task never evaluates an expression or consults a provider.

use std::sync::Arc;

use dive_ir::{
    Arg, BuiltinOp, Effects, Expr, ExprId, OpRef, QuantKind, StdKind, StdOp, StdSupport,
};
use dive_value::{SetKind, Value};

use super::{Fact, Mode, ProducerStability};
use crate::env::{Env, Ev, Frame, OpVal, StateRef, Sv};
use crate::{ConstBinding, Evaluator, Scope, SlotView, St};

#[derive(Clone, Copy)]
struct Info {
    fact: Fact,
    non_string: bool,
}

impl Info {
    fn canonical(non_string: bool) -> Self {
        Self {
            fact: Fact::Canonical,
            non_string,
        }
    }
}

enum Task {
    Expression(Scope, ExprId, Mode, bool),
    ExpressionEnd(usize, bool, Mode),
    Inner(Scope, ExprId, Mode),
    Result(Info),
    Join(usize),
    UnitEnd(usize),
    Environment(Env),
    Frame(Arc<Frame>),
    Slot(Sv),
    Thunk(Arc<crate::env::Thunk>),
    Value(Value),
    State(StateRef),
    Operator(OpVal),
    OperatorBody(OpVal),
    Call(Scope, OpRef, Vec<Arg>, Mode),
    CallReady(Scope, OpRef, Vec<Arg>, Mode, usize),
    CallValue(ResolvedCall),
    Callback(Scope, OpRef),
    CallbackBody(Scope, OpRef),
    Quantified(Scope, ExprId, Mode),
    Nested(Scope, ExprId, ExprId, Mode),
    CachedBody(Scope, ExprId, Mode, bool),
}

struct Walk<'a> {
    evaluator: &'a Evaluator,
    control: Option<&'a dyn crate::HostControl>,
    tasks: Vec<Task>,
    results: Vec<Info>,
    frames: Vec<Arc<Frame>>,
    abstracts: Vec<Arc<Frame>>,
    active: Vec<(ExprId, Scope, Mode)>,
    // In the subset proof, reject reads of the actual outer binder through any
    // resolved frame/capture. This is not a public analysis-scope attestation.
    forbidden: Option<(dive_ir::DefId, usize)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallKey {
    Definition(dive_ir::DefId),
    Native(StdOp),
}

struct ResolvedCall {
    scope: Scope,
    original: OpRef,
    value: OpVal,
    args: Vec<Arg>,
    mode: Mode,
    non_string: bool,
    seen: Vec<CallKey>,
}

pub(super) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> crate::Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| crate::EvalError::resource_limit("canonical proof allocation"))
}

pub(super) fn prove(
    evaluator: &Evaluator,
    producers: &[(ExprId, Scope)],
    predicates: &[(ExprId, Scope)],
    control: Option<&dyn crate::HostControl>,
) -> crate::Result<Option<ProducerStability>> {
    let mut walk = Walk {
        evaluator,
        control,
        tasks: Vec::new(),
        results: Vec::new(),
        frames: Vec::new(),
        abstracts: Vec::new(),
        active: Vec::new(),
        forbidden: None,
    };
    walk.poll()?;
    for (roots, mode) in [(producers, Mode::Producer), (predicates, Mode::Reordered)] {
        for (expr, scope) in roots {
            walk.poll()?;
            if evaluator.admit_scoped_expr(scope, *expr).is_err() {
                return Ok(None);
            }
            walk.schedule([
                Task::Environment(scope.env.clone()),
                Task::Expression(scope.clone(), *expr, mode, false),
            ])?;
            if !walk.run()? {
                return Ok(None);
            }
            walk.results.clear();
        }
    }
    walk.poll()?;
    Ok(Some(ProducerStability {
        owner: Arc::downgrade(&evaluator.core),
    }))
}

pub(super) fn closed_forall(
    evaluator: &Evaluator,
    original: Scope,
    quantifier: ExprId,
    body: ExprId,
    outer_slot: usize,
    state: &St,
    base: &[Value],
) -> crate::Result<bool> {
    let Ok(bound) = evaluator.scope_binder_analysis(&original, quantifier) else {
        return Ok(false);
    };
    let mut walk = Walk {
        evaluator,
        control: None,
        tasks: Vec::new(),
        results: Vec::new(),
        frames: Vec::new(),
        abstracts: Vec::new(),
        active: Vec::new(),
        forbidden: Some((original.env.frame.def, outer_slot)),
    };
    walk.poll()?;
    reserve(&mut walk.abstracts, 1)?;
    walk.abstracts.push(bound.env.frame.clone());
    walk.schedule([
        Task::Environment(original.env),
        Task::State(state.s0.clone()),
        Task::State(state.s1.clone()),
        Task::Environment(bound.env.clone()),
        Task::Expression(bound, body, Mode::Reordered, true),
    ])?;
    if !walk.run()? {
        return Ok(false);
    }
    for value in base {
        walk.poll()?;
        walk.push(Task::Value(value.clone()))?;
        if !walk.run()? {
            return Ok(false);
        }
        walk.results.clear();
    }
    Ok(true)
}

impl Walk<'_> {
    fn poll(&self) -> crate::Result<()> {
        if let Some(control) = self.control {
            control.check().map_err(crate::EvalError::interrupted)
        } else {
            self.evaluator.core.host.check_control()
        }
    }

    fn push(&mut self, task: Task) -> crate::Result<()> {
        reserve(&mut self.tasks, 1)?;
        self.tasks.push(task);
        Ok(())
    }

    fn schedule<const N: usize>(&mut self, tasks: [Task; N]) -> crate::Result<()> {
        reserve(&mut self.tasks, N)?;
        self.tasks.extend(tasks.into_iter().rev());
        Ok(())
    }

    fn sequence(&mut self, tasks: Vec<Task>) -> crate::Result<()> {
        reserve(&mut self.tasks, tasks.len())?;
        self.tasks.extend(tasks.into_iter().rev());
        Ok(())
    }

    fn result(&mut self, info: Info) -> crate::Result<()> {
        reserve(&mut self.results, 1)?;
        self.results.push(info);
        Ok(())
    }

    fn scope(&self, env: Env) -> Scope {
        Scope {
            owner: self.evaluator.core.ir.clone(),
            names: self.evaluator.core.names.clone(),
            context: Arc::downgrade(&self.evaluator.core),
            env,
            wrapper: None,
        }
    }

    fn children(
        &mut self,
        scope: &Scope,
        expressions: impl IntoIterator<Item = ExprId>,
        mode: Mode,
        canonical: bool,
        result: Info,
    ) -> crate::Result<()> {
        let mut tasks = Vec::new();
        for expr in expressions {
            self.poll()?;
            reserve(&mut tasks, 1)?;
            tasks.push(Task::Expression(scope.clone(), expr, mode, canonical));
        }
        reserve(&mut tasks, 1)?;
        tasks.push(Task::Result(result));
        self.sequence(tasks)
    }

    // Each semantic inspection boundary is a task here. A task
    // either schedules its prerequisites/continuation or completes one check.
    #[allow(
        clippy::too_many_lines,
        reason = "one explicit continuation per canonical proof boundary"
    )]
    fn run(&mut self) -> crate::Result<bool> {
        while let Some(task) = self.tasks.pop() {
            self.poll()?;
            let ev = self.evaluator;
            match task {
                Task::Result(info) => self.result(info)?,
                Task::Expression(scope, expr, mode, canonical) => {
                    if self.forbidden.is_some()
                        && (matches!(
                            ev.ir().expr(expr),
                            Expr::Var { .. }
                                | Expr::Prime(_)
                                | Expr::Unchanged(_)
                                | Expr::SubstRef { primed: true, .. }
                                | Expr::StepBox { .. }
                                | Expr::StepAngle { .. }
                        ) || matches!(
                            ev.ir().expr(expr),
                            Expr::Builtin {
                                op: BuiltinOp::Prime | BuiltinOp::Unchanged | BuiltinOp::Enabled,
                                ..
                            }
                        ) || self.active.iter().any(|(active, _, _)| *active == expr))
                    {
                        return Ok(false);
                    }
                    if ev.admit_scoped_expr(&scope, expr).is_err()
                        || ev.ir().node(expr).effects.intersects(
                            Effects::IMPURE
                                | Effects::LIVE_TLCGET
                                | Effects::USES_ENABLED
                                | Effects::TEMPORAL,
                        )
                        || self.active.iter().any(|(prior, captured, prior_mode)| {
                            *prior == expr
                                && *prior_mode == mode
                                && Arc::ptr_eq(&captured.env.frame, &scope.env.frame)
                                && match (&captured.env.dynamic.subst, &scope.env.dynamic.subst) {
                                    (None, None) => true,
                                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                                    _ => false,
                                }
                        })
                    {
                        return Ok(false);
                    }
                    reserve(&mut self.active, 1)?;
                    self.active.push((expr, scope.clone(), mode));
                    self.schedule([
                        Task::Inner(scope, expr, mode),
                        Task::ExpressionEnd(self.results.len(), canonical, mode),
                    ])?;
                }
                Task::ExpressionEnd(base, canonical, mode) => {
                    let Some(info) = self.results.last().copied() else {
                        return Ok(false);
                    };
                    if (canonical || mode == Mode::Reordered) && info.fact != Fact::Canonical {
                        return Ok(false);
                    }
                    self.results.truncate(base);
                    self.result(info)?;
                    self.active.pop();
                }
                Task::Inner(scope, expr, mode) => {
                    if !self.inner(&scope, expr, mode)? {
                        return Ok(false);
                    }
                }
                Task::Join(base) => {
                    let values = &self.results[base..];
                    let fact = if values.iter().all(|v| v.fact == Fact::Canonical) {
                        Fact::Canonical
                    } else {
                        Fact::YieldCanonical
                    };
                    let non_string = !values.is_empty() && values.iter().all(|v| v.non_string);
                    self.result(Info { fact, non_string })?;
                }
                Task::UnitEnd(base) => self.results.truncate(base),
                Task::Environment(env) => {
                    if env.dynamic.at.is_some() || env.dynamic.cut.is_some() {
                        return Ok(false);
                    }
                    let mut tasks = Vec::new();
                    let mut link = env.dynamic.subst.as_deref();
                    while let Some(binding) = link {
                        self.poll()?;
                        reserve(&mut tasks, 1)?;
                        tasks.push(Task::Slot(binding.val.clone()));
                        link = binding.next.as_deref();
                    }
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Frame(env.frame));
                    self.sequence(tasks)?;
                }
                Task::Frame(frame) => {
                    let mut tasks = Vec::new();
                    if !self.frames.iter().any(|prior| Arc::ptr_eq(prior, &frame)) {
                        reserve(&mut self.frames, 1)?;
                        self.frames.push(frame.clone());
                        let abstract_frame = self.abstracts.iter().any(|f| Arc::ptr_eq(f, &frame));
                        for slot in &frame.slots {
                            self.poll()?;
                            if abstract_frame
                                && matches!(slot, Sv::AnalysisBound | Sv::AnalysisConstant)
                            {
                                continue;
                            }
                            reserve(&mut tasks, 1)?;
                            tasks.push(Task::Slot(slot.clone()));
                        }
                    }
                    if let Some(parent) = &frame.parent {
                        reserve(&mut tasks, 1)?;
                        tasks.push(Task::Frame(parent.clone()));
                    }
                    self.sequence(tasks)?;
                }
                Task::Slot(slot) => {
                    self.push(Task::UnitEnd(self.results.len()))?;
                    match slot {
                        Sv::Unset => {}
                        Sv::Val(value) => self.push(Task::Value(value))?,
                        Sv::AnalysisBound | Sv::AnalysisConstant | Sv::Fcn(_) => return Ok(false),
                        Sv::Thunk(thunk) => {
                            // Shared mutable cache contents cannot be inspected
                            // as a closed immutable context on a public Core.
                            if self.forbidden.is_some() {
                                return Ok(false);
                            }
                            self.schedule([
                                Task::Environment(thunk.env.clone()),
                                Task::Thunk(thunk),
                            ])?;
                        }
                        Sv::Op(operator) => {
                            if let OpVal::Def {
                                parent: Some(parent),
                                dynamic,
                                ..
                            } = &*operator
                            {
                                self.push(Task::Operator(*operator.clone()))?;
                                self.push(Task::Environment(Env {
                                    frame: parent.clone(),
                                    dynamic: dynamic.clone().unwrap_or_default(),
                                }))?;
                            } else {
                                self.push(Task::Operator(*operator))?;
                            }
                        }
                    }
                }
                Task::Thunk(thunk) => {
                    let mut tasks = Vec::new();
                    for (value, current, successor) in thunk.stored_caches().into_iter().flatten() {
                        let Ev::V(value) = value else {
                            return Ok(false);
                        };
                        reserve(&mut tasks, 3)?;
                        tasks.extend([
                            Task::Value(value),
                            Task::State(current),
                            Task::State(successor),
                        ]);
                    }
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Expression(
                        self.scope(thunk.env.clone()),
                        thunk.expr,
                        Mode::Reordered,
                        true,
                    ));
                    self.sequence(tasks)?;
                }
                Task::State(state) => match state {
                    StateRef::Null | StateRef::Empty => {}
                    StateRef::Named(_) => return Ok(false),
                    StateRef::Vals(values) => {
                        let mut tasks = Vec::new();
                        for value in values.iter().flatten() {
                            self.poll()?;
                            reserve(&mut tasks, 1)?;
                            tasks.push(Task::Value(value.clone()));
                        }
                        self.sequence(tasks)?;
                    }
                },
                Task::Value(value) => {
                    if !self.value(&value)? {
                        return Ok(false);
                    }
                }
                Task::Operator(operator) => {
                    self.push(Task::UnitEnd(self.results.len()))?;
                    if !self.operator(operator)? {
                        return Ok(false);
                    }
                }
                Task::OperatorBody(operator) => {
                    let OpVal::Def {
                        def,
                        parent,
                        dynamic,
                    } = &operator
                    else {
                        return Ok(false);
                    };
                    let Some(definition) = ev.ir().defs().get(def.index()) else {
                        return Ok(false);
                    };
                    if definition.std.is_some() || definition.recursive {
                        return Ok(false);
                    }
                    let base = if let Some(parent) = parent {
                        self.scope(Env {
                            frame: parent.clone(),
                            dynamic: dynamic.clone().unwrap_or_default(),
                        })
                    } else {
                        let Ok(mut base) = ev.scope(*def) else {
                            return Ok(false);
                        };
                        if let Some(dynamic) = dynamic {
                            base.env.dynamic = dynamic.clone();
                        }
                        base
                    };
                    let Ok(Some((actual, called))) = ev.operator_value_analysis(&base, operator)
                    else {
                        return Ok(false);
                    };
                    reserve(&mut self.abstracts, 1)?;
                    self.abstracts.push(called.env.frame.clone());
                    self.push(Task::Expression(
                        called,
                        ev.ir().def(actual).body,
                        Mode::Reordered,
                        true,
                    ))?;
                }
                Task::Call(scope, operator, args, mode) => {
                    if matches!(operator, OpRef::Subst(_)) {
                        return Ok(false);
                    }
                    let mut tasks = Vec::new();
                    reserve(&mut tasks, args.len().saturating_add(1))?;
                    for arg in &args {
                        self.poll()?;
                        tasks.push(match arg {
                            Arg::Expr { expr, .. } => {
                                Task::Expression(scope.clone(), *expr, mode, true)
                            }
                            Arg::Op { op, .. } => Task::Callback(scope.clone(), op.clone()),
                        });
                    }
                    tasks.push(Task::CallReady(
                        scope,
                        operator,
                        args,
                        mode,
                        self.results.len(),
                    ));
                    self.sequence(tasks)?;
                }
                Task::CallReady(scope, operator, args, mode, base) => {
                    let non_string = args.len() == 2
                        && args.iter().all(|a| matches!(a, Arg::Expr { .. }))
                        && self.results[base..].iter().any(|v| v.non_string);
                    let Ok(value) = ev.core.op_value(&scope.env, &operator) else {
                        return Ok(false);
                    };
                    self.push(Task::CallValue(ResolvedCall {
                        scope,
                        original: operator,
                        value,
                        args,
                        mode,
                        non_string,
                        seen: Vec::new(),
                    }))?;
                }
                Task::CallValue(call) => {
                    if !self.call_value(call)? {
                        return Ok(false);
                    }
                }
                Task::Callback(scope, operator) => {
                    self.push(Task::UnitEnd(self.results.len()))?;
                    let Ok(value) = ev.core.op_value(&scope.env, &operator) else {
                        return Ok(false);
                    };
                    self.schedule([Task::Operator(value), Task::CallbackBody(scope, operator)])?;
                }
                Task::CallbackBody(scope, operator) => {
                    let Ok(Some((def, called))) = ev.scope_operator_analysis(&scope, &operator)
                    else {
                        return Ok(false);
                    };
                    reserve(&mut self.abstracts, 1)?;
                    self.abstracts.push(called.env.frame.clone());
                    // Inspect every original/unused captured slot and body;
                    // no bounded operator_captures helper is called here.
                    self.schedule([
                        Task::Environment(called.env.clone()),
                        Task::Expression(called, ev.ir().def(def).body, Mode::Reordered, true),
                    ])?;
                }
                Task::Quantified(scope, expr, mode) => {
                    let Expr::Quant { body, .. } = ev.ir().expr(expr) else {
                        return Ok(false);
                    };
                    let Ok(called) = ev.scope_binder_analysis(&scope, expr) else {
                        return Ok(false);
                    };
                    reserve(&mut self.abstracts, 1)?;
                    self.abstracts.push(called.env.frame.clone());
                    self.schedule([
                        Task::Expression(called, *body, mode, true),
                        Task::Result(Info::canonical(true)),
                    ])?;
                }
                Task::Nested(scope, expr, body, mode) => {
                    let called = match ev.ir().expr(expr) {
                        Expr::Let { .. } => ev.scope_eval_let(&scope, expr),
                        Expr::Subst { bindings, .. } => {
                            if bindings.iter().any(|b| {
                                matches!(
                                    b.value,
                                    Arg::Expr {
                                        pass: dive_ir::Pass::Strict,
                                        ..
                                    }
                                )
                            }) {
                                return Ok(false);
                            }
                            ev.scope_subst(&scope, expr)
                        }
                        _ => return Ok(false),
                    };
                    let Ok(called) = called else {
                        return Ok(false);
                    };
                    self.schedule([
                        Task::Environment(called.env.clone()),
                        Task::Expression(called, body, mode, false),
                    ])?;
                }
                Task::CachedBody(scope, body, mode, cacheable) => {
                    self.push(Task::Expression(scope, body, mode, cacheable))?;
                }
            }
        }
        Ok(true)
    }

    fn value(&mut self, value: &Value) -> crate::Result<bool> {
        let ev = self.evaluator;
        let mut tasks = Vec::new();
        match value {
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) => {}
            Value::Str(atom) | Value::Model(atom) => {
                if ev
                    .core
                    .names
                    .resolve(atom.tok())
                    .is_none_or(|n| n.as_str() != atom.as_str())
                {
                    return Ok(false);
                }
            }
            Value::Fcn(function) => {
                // The row proof retains its literal-only canonical contract.
                // Closed-singleton inspection additionally admits immutable
                // stored FcnRcdValues, auditing every key/value below. Their
                // deterministic representation metadata is not a mutable
                // evaluator cache; actual per-element success is learned only
                // by the original complete singleton predicate evaluations.
                if (self.forbidden.is_none()
                    && !function.is_tuple_literal()
                    && !function.is_record_literal())
                    || (function.as_tuple().is_none() && function.order() != ev.core.ctx.order)
                {
                    return Ok(false);
                }
                for (key, value) in function.iter() {
                    self.poll()?;
                    reserve(&mut tasks, 2)?;
                    tasks.extend([Task::Value(key), Task::Value(value.clone())]);
                }
            }
            Value::Set(set) => {
                if set.kind() != SetKind::Enum || set.order() != ev.core.ctx.order {
                    return Ok(false);
                }
                let Some(elements) = set.stored_elements() else {
                    return Ok(false);
                };
                for value in elements {
                    self.poll()?;
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Value(value.clone()));
                }
            }
        }
        self.sequence(tasks)?;
        Ok(true)
    }

    fn operator(&mut self, operator: OpVal) -> crate::Result<bool> {
        match &operator {
            OpVal::Def {
                parent, dynamic, ..
            } => {
                let mut tasks = Vec::new();
                if let Some(dynamic) = dynamic {
                    if dynamic.at.is_some() || dynamic.cut.is_some() {
                        return Ok(false);
                    }
                    let mut link = dynamic.subst.as_deref();
                    while let Some(binding) = link {
                        self.poll()?;
                        reserve(&mut tasks, 1)?;
                        tasks.push(Task::Slot(binding.val.clone()));
                        link = binding.next.as_deref();
                    }
                }
                if let Some(parent) = parent {
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Environment(Env {
                        frame: parent.clone(),
                        dynamic: dynamic.clone().unwrap_or_default(),
                    }));
                }
                reserve(&mut tasks, 1)?;
                tasks.push(Task::OperatorBody(operator));
                self.sequence(tasks)?;
            }
            OpVal::Std(op) => {
                if let Some(&target) = self.evaluator.core.std_redirects.get(op) {
                    self.push(Task::Operator(OpVal::Def {
                        def: target,
                        parent: None,
                        dynamic: None,
                    }))?;
                } else if native_info(*op, false).is_none() {
                    return Ok(false);
                }
            }
            OpVal::Const(constant) => match self.evaluator.constant_binding(*constant) {
                Some(ConstBinding::Def(def)) => self.push(Task::Operator(OpVal::Def {
                    def,
                    parent: None,
                    dynamic: None,
                }))?,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn slot_view(&mut self, value: SlotView, mode: Mode) -> crate::Result<bool> {
        match value {
            SlotView::Value(value) => self.schedule([
                Task::Value(value.clone()),
                Task::Result(Info::canonical(!matches!(value, Value::Str(_)))),
            ])?,
            SlotView::Lazy(body, captured) => {
                self.push(Task::Expression(captured, body, mode, false))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn constant(
        &mut self,
        scope: &Scope,
        constant: dive_ir::ConstId,
        mode: Mode,
    ) -> crate::Result<bool> {
        match self.evaluator.constant_binding(constant) {
            Some(ConstBinding::Value(value)) => self.slot_view(SlotView::Value(value), mode),
            Some(ConstBinding::Def(def)) => {
                self.push(Task::Call(scope.clone(), OpRef::Def(def), Vec::new(), mode))?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit IR transfer cases preserve the canonical representation capability"
    )]
    fn inner(&mut self, scope: &Scope, expr: ExprId, mode: Mode) -> crate::Result<bool> {
        use Expr as E;
        let ev = self.evaluator;
        match ev.ir().expr(expr) {
            E::Bool(_) | E::Num(_) => self.result(Info::canonical(true))?,
            E::Str(_) | E::Var { .. } => self.result(Info::canonical(false))?,
            E::Local(local) => {
                if self.forbidden.is_some_and(|(definition, slot)| {
                    local.slot.index() == slot
                        && scope
                            .env
                            .frame_up(local.up)
                            .is_some_and(|frame| frame.def == definition)
                }) {
                    return Ok(false);
                }
                let value = ev.scope_slot(scope, *local);
                if matches!(value, SlotView::AnalysisBound | SlotView::AnalysisConstant) {
                    let Some(frame) = scope.env.frame_up(local.up) else {
                        return Ok(false);
                    };
                    if !self.abstracts.iter().any(|f| Arc::ptr_eq(f, frame)) {
                        return Ok(false);
                    }
                    self.result(Info::canonical(false))?;
                } else if !self.slot_view(value, mode)? {
                    return Ok(false);
                }
            }
            E::Const(constant) => return self.constant(scope, *constant, mode),
            E::Def(def) => self.push(Task::Call(
                scope.clone(),
                OpRef::Def(*def),
                Vec::new(),
                mode,
            ))?,
            E::Apply { callee, args } => {
                let mut actuals = Vec::new();
                reserve(&mut actuals, args.len())?;
                actuals.extend(args.iter().cloned());
                self.push(Task::Call(scope.clone(), callee.clone(), actuals, mode))?;
            }
            E::SubstRef { decl, primed } => {
                let Ok(value) = ev.scope_substitution(scope, *decl, &St::constant(), *primed)
                else {
                    return Ok(false);
                };
                if let Some(value) = value {
                    return self.slot_view(value, mode);
                }
                let Some(declaration) = ev.ir().decls().get(decl.index()) else {
                    return Ok(false);
                };
                if declaration.variable {
                    if self.forbidden.is_some() {
                        return Ok(false);
                    }
                    self.result(Info::canonical(false))?;
                } else if let Some(constant) = declaration.constant {
                    return self.constant(scope, constant, mode);
                } else {
                    return Ok(false);
                }
            }
            E::Builtin { op, args } => {
                use BuiltinOp as B;
                if mode == Mode::Producer && *op == B::Subseteq {
                    return Ok(false);
                }
                let fact = match op {
                    B::Eq
                    | B::Neq
                    | B::In
                    | B::NotIn
                    | B::Subseteq
                    | B::Not
                    | B::And
                    | B::Or
                    | B::Implies
                    | B::Equiv
                    | B::Prime
                    | B::Unchanged
                    | B::Boolean => Fact::Canonical,
                    B::Times | B::Domain | B::String => Fact::YieldCanonical,
                    _ => return Ok(false),
                };
                if mode == Mode::Producer && *op == B::In {
                    let [left, right] = args.as_ref() else {
                        return Ok(false);
                    };
                    self.schedule([
                        Task::Expression(scope.clone(), *left, mode, true),
                        Task::Expression(scope.clone(), *right, mode, false),
                        Task::Result(Info::canonical(true)),
                    ])?;
                } else {
                    self.children(
                        scope,
                        args.iter().copied(),
                        mode,
                        true,
                        Info {
                            fact,
                            non_string: *op != B::Prime,
                        },
                    )?;
                }
            }
            E::Tuple(items) | E::Junction { items, .. } => self.children(
                scope,
                items.iter().copied(),
                mode,
                true,
                Info::canonical(true),
            )?,
            E::Record(fields) => self.children(
                scope,
                fields.iter().map(|f| f.value),
                mode,
                true,
                Info::canonical(true),
            )?,
            E::SetEnum(items) | E::Cartesian(items) => {
                self.children(
                    scope,
                    items.iter().copied(),
                    mode,
                    matches!(ev.ir().expr(expr), E::SetEnum(_)),
                    Info {
                        fact: Fact::YieldCanonical,
                        non_string: true,
                    },
                )?;
            }
            E::Not(child) | E::Unchanged(child) => {
                self.children(scope, [*child], mode, true, Info::canonical(true))?;
            }
            E::Prime(child) | E::Selected(child) | E::Label { body: child, .. } => {
                self.push(Task::Expression(scope.clone(), *child, mode, true))?;
            }
            E::Implies(a, b)
            | E::Equiv(a, b)
            | E::StepBox { action: a, sub: b }
            | E::StepAngle { action: a, sub: b } => {
                self.children(scope, [*a, *b], mode, true, Info::canonical(true))?;
            }
            E::If { cond, then, other } => {
                let base = self.results.len();
                self.schedule([
                    Task::Expression(scope.clone(), *cond, mode, true),
                    Task::Expression(scope.clone(), *then, mode, false),
                    Task::Expression(scope.clone(), *other, mode, false),
                    Task::Join(base),
                ])?;
            }
            E::Case { arms, other } => {
                let base = self.results.len();
                let mut tasks = Vec::new();
                for arm in arms {
                    self.poll()?;
                    reserve(&mut tasks, 2)?;
                    tasks.extend([
                        Task::Expression(scope.clone(), arm.guard, mode, true),
                        Task::Expression(scope.clone(), arm.value, mode, false),
                    ]);
                }
                if let Some(other) = other {
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Expression(scope.clone(), *other, mode, false));
                }
                reserve(&mut tasks, 1)?;
                tasks.push(Task::Join(base));
                self.sequence(tasks)?;
            }
            E::Let { body, .. } | E::Subst { body, .. } => {
                self.push(Task::Nested(scope.clone(), expr, *body, mode))?;
            }
            E::Quant {
                kind: QuantKind::Forall | QuantKind::Exists,
                bounds,
                ..
            } => {
                let mut tasks = Vec::new();
                for bound in bounds {
                    self.poll()?;
                    let Some(domain) = bound.domain else {
                        return Ok(false);
                    };
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Expression(scope.clone(), domain, mode, false));
                }
                reserve(&mut tasks, 1)?;
                tasks.push(Task::Quantified(scope.clone(), expr, mode));
                self.sequence(tasks)?;
            }
            E::FcnApply { fcn, arg } => {
                self.children(scope, [*fcn, *arg], mode, true, Info::canonical(false))?;
            }
            E::RecordSelect { record, .. } => {
                self.children(scope, [*record], mode, true, Info::canonical(false))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn called_body(
        &self,
        scope: &Scope,
        original: &OpRef,
        def: dive_ir::DefId,
        args: &[Arg],
        mode: Mode,
    ) -> Option<Task> {
        let called = match original {
            OpRef::Param(local) => match self.evaluator.scope_apply_param(scope, *local, args) {
                Some(Ok((_, called))) => Some(called),
                _ => None,
            },
            _ if args.is_empty() => self.evaluator.scope_enter(scope, def).ok(),
            _ => self.evaluator.scope_apply(scope, def, args).ok(),
        }?;
        Some(Task::CachedBody(
            called,
            self.evaluator.ir().def(def).body,
            mode,
            self.evaluator.core.cacheable(def),
        ))
    }

    fn call_value(&mut self, call: ResolvedCall) -> crate::Result<bool> {
        let ResolvedCall {
            scope,
            original,
            value,
            args,
            mode,
            non_string,
            mut seen,
        } = call;
        let ev = self.evaluator;
        let key = match &value {
            OpVal::Std(op) => Some(CallKey::Native(*op)),
            OpVal::Def { def, .. } => Some(CallKey::Definition(*def)),
            _ => None,
        };
        if let Some(key) = key {
            if seen.contains(&key) {
                return Ok(false);
            }
            reserve(&mut seen, 1)?;
            seen.push(key);
        }
        match value {
            OpVal::Std(op) => {
                if let Some(&target) = ev.core.std_redirects.get(&op) {
                    self.push(Task::CallValue(ResolvedCall {
                        scope,
                        original: OpRef::Def(target),
                        value: OpVal::Def {
                            def: target,
                            parent: None,
                            dynamic: None,
                        },
                        args,
                        mode,
                        non_string,
                        seen,
                    }))?;
                } else if let Some(info) = native_info(op, non_string) {
                    self.result(info)?;
                } else {
                    return Ok(false);
                }
            }
            OpVal::Def { def, .. } => {
                if ev.core.def_values.contains_key(&def) {
                    return Ok(false);
                }
                let Some(definition) = ev.ir().defs().get(def.index()) else {
                    return Ok(false);
                };
                if definition.recursive {
                    return Ok(false);
                }
                let mut tasks = Vec::new();
                for value in [
                    ev.core.startup.get(def.index()),
                    ev.core.def_cache.get(def.index()),
                ]
                .into_iter()
                .flatten()
                .filter_map(std::sync::OnceLock::get)
                {
                    self.poll()?;
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::Value(value.clone()));
                }
                if let Some(op) = definition.std {
                    reserve(&mut tasks, 1)?;
                    tasks.push(Task::CallValue(ResolvedCall {
                        scope,
                        original,
                        value: OpVal::Std(op),
                        args,
                        mode,
                        non_string,
                        seen,
                    }));
                } else {
                    let Some(body) = self.called_body(&scope, &original, def, &args, mode) else {
                        return Ok(false);
                    };
                    reserve(&mut tasks, 1)?;
                    tasks.push(body);
                }
                self.sequence(tasks)?;
            }
            OpVal::Const(constant) => match ev.constant_binding(constant) {
                Some(ConstBinding::Value(value)) if args.is_empty() => {
                    return self.slot_view(SlotView::Value(value), mode);
                }
                _ => return Ok(false),
            },
            _ => return Ok(false),
        }
        Ok(true)
    }
}

fn native_info(op: StdOp, non_string: bool) -> Option<Info> {
    use StdOp as S;
    if op.impure()
        || op.support() != StdSupport::Core
        || op.kind() != StdKind::Value
        || (op.creates_runtime_names() && !(op == S::SequencesConcat && non_string))
    {
        return None;
    }
    let fact = match op {
        S::NaturalsNat
        | S::IntegersNat
        | S::IntegersInt
        | S::StringsSTRING
        | S::NaturalsDotDot
        | S::IntegersDotDot => Fact::YieldCanonical,
        S::NaturalsPlus
        | S::IntegersPlus
        | S::NaturalsMinus
        | S::IntegersMinus
        | S::NaturalsTimes
        | S::IntegersTimes
        | S::NaturalsDiv
        | S::IntegersDiv
        | S::NaturalsMod
        | S::IntegersMod
        | S::NaturalsExpt
        | S::IntegersExpt
        | S::IntegersNeg
        | S::NaturalsLt
        | S::NaturalsLeq
        | S::NaturalsGt
        | S::NaturalsGeq
        | S::IntegersLt
        | S::IntegersLeq
        | S::IntegersGt
        | S::IntegersGeq
        | S::FiniteSetsCardinality
        | S::FiniteSetsIsFiniteSet
        | S::SequencesLen
        | S::SequencesHead
        | S::SequencesTail
        | S::SequencesCons
        | S::SequencesAppend
        | S::SequencesConcat
        | S::SequencesSubSeq
        | S::SequencesSelectSeq
        | S::TlcAssert
        | S::TlcSortSeq => Fact::Canonical,
        _ => return None,
    };
    // Only audited scalar families imply non-string from their kernel result.
    // Selection and sequence operations conservatively keep an unknown kind.
    let non_string = matches!(
        op,
        S::NaturalsPlus
            | S::IntegersPlus
            | S::NaturalsMinus
            | S::IntegersMinus
            | S::NaturalsTimes
            | S::IntegersTimes
            | S::NaturalsDiv
            | S::IntegersDiv
            | S::NaturalsMod
            | S::IntegersMod
            | S::NaturalsExpt
            | S::IntegersExpt
            | S::IntegersNeg
            | S::NaturalsLt
            | S::NaturalsLeq
            | S::NaturalsGt
            | S::NaturalsGeq
            | S::IntegersLt
            | S::IntegersLeq
            | S::IntegersGt
            | S::IntegersGeq
            | S::FiniteSetsCardinality
            | S::FiniteSetsIsFiniteSet
            | S::SequencesLen
    ) || op == S::SequencesConcat && non_string;
    Some(Info { fact, non_string })
}
