//! Non-evaluating scoped level resolution for the independent endpoint compiler.
//! Actual lazy scopes are retained; static formal levels never replace actuals.

use super::{FormulaError, push, refused};
use dive_eval::{ConstBinding, HostControl, Scope, SlotView, St};
use dive_ir::{Arg, BuiltinOp, DefId, Expr, ExprId, Level, OpRef};
use dive_model::{Model, Pred};

enum Task {
    Enter(Pred),
    Leave,
    Maximum {
        base: Level,
        count: usize,
    },
    Prime,
    Binder {
        pred: Pred,
        body: ExprId,
        count: usize,
        recursive: bool,
    },
}

#[allow(
    clippy::too_many_lines,
    reason = "one non-evaluating scoped traversal preserves actual lazy/binder contexts"
)]
pub(super) fn level(
    model: &Model,
    pred: &Pred,
    control: Option<&dyn HostControl>,
) -> Result<Level, FormulaError> {
    let mut tasks = Vec::new();
    let mut values = Vec::new();
    let mut active: Vec<Pred> = Vec::new();
    push(&mut tasks, Task::Enter(pred.clone()))?;
    while let Some(task) = tasks.pop() {
        if let Some(control) = control {
            control.check().map_err(dive_eval::EvalError::interrupted)?;
        }
        match task {
            Task::Leave => {
                active.pop();
            }
            Task::Prime => {
                let child = take(&mut values)?;
                push(
                    &mut values,
                    if child == Level::Constant {
                        child
                    } else {
                        Level::Action
                    },
                )?;
            }
            Task::Maximum { mut base, count } => {
                for _ in 0..count {
                    base = base.max(take(&mut values)?);
                }
                push(&mut values, base)?;
            }
            Task::Binder {
                pred,
                body,
                count,
                recursive,
            } => {
                let mut domains = Level::Constant;
                for _ in 0..count {
                    domains = domains.max(take(&mut values)?);
                }
                if domains > Level::State {
                    return Err(refused("action/temporal endpoint binder domain"));
                }
                let scope = if domains == Level::Constant && !recursive {
                    model
                        .evaluator
                        .scope_constant_binder_analysis(&pred.scope, pred.expr)?
                } else {
                    model
                        .evaluator
                        .scope_binder_analysis(&pred.scope, pred.expr)?
                };
                let base = model.ir.node(pred.expr).level.max(domains);
                push(&mut tasks, Task::Maximum { base, count: 1 })?;
                push(&mut tasks, Task::Enter(Pred::new(body, scope)))?;
            }
            Task::Enter(pred) => {
                super::enter_active(&mut active, &pred, control)?;
                push(&mut tasks, Task::Leave)?;
                let ev = &model.evaluator;
                let ir = &model.ir;
                let scope = &pred.scope;
                ev.validate_predicate(scope, pred.expr)?;
                let node = ir.node(pred.expr);
                match &node.expr {
                    Expr::Prime(body) => {
                        push(&mut tasks, Task::Prime)?;
                        push(&mut tasks, Task::Enter(Pred::new(*body, scope.clone())))?;
                    }
                    Expr::Builtin {
                        op: BuiltinOp::Always | BuiltinOp::Eventually | BuiltinOp::LeadsTo,
                        ..
                    }
                    | Expr::Always(_)
                    | Expr::Eventually(_)
                    | Expr::LeadsTo(..)
                    | Expr::WhilePlus(..)
                    | Expr::Wf { .. }
                    | Expr::Sf { .. }
                    | Expr::TemporalQuant { .. } => push(&mut values, Level::Temporal)?,
                    Expr::Builtin {
                        op: BuiltinOp::Prime | BuiltinOp::Unchanged,
                        ..
                    }
                    | Expr::Var { primed: true, .. }
                    | Expr::Unchanged(_)
                    | Expr::StepAngle { .. }
                    | Expr::StepBox { .. } => push(&mut values, Level::Action)?,
                    Expr::Subst { body, .. } => push(
                        &mut tasks,
                        Task::Enter(Pred::new(*body, substitution(model, &pred)?)),
                    )?,
                    Expr::Let { body, .. } => push(
                        &mut tasks,
                        Task::Enter(Pred::new(*body, ev.scope_let(scope, pred.expr)?)),
                    )?,
                    Expr::Local(local) => {
                        slot(&mut tasks, &mut values, ev.scope_slot(scope, *local))?
                    }
                    Expr::SubstRef { decl, primed } => {
                        if *primed {
                            push(&mut tasks, Task::Prime)?;
                        }
                        match ev.scope_substitution(scope, *decl, &St::constant(), *primed)? {
                            Some(value) => slot(&mut tasks, &mut values, value)?,
                            None => {
                                let declaration = ir.decl(*decl);
                                if declaration.variable {
                                    push(&mut values, Level::State)?;
                                } else if let Some(constant) = declaration.constant {
                                    match ev.constant_binding(constant) {
                                        Some(ConstBinding::Value(_)) => {
                                            push(&mut values, Level::Constant)?
                                        }
                                        Some(ConstBinding::Def(def)) => {
                                            enter(model, &mut tasks, scope, def, &[])?
                                        }
                                        None => {
                                            return Err(refused(
                                                "unbound endpoint substitution constant",
                                            ));
                                        }
                                    }
                                } else {
                                    return Err(refused(
                                        "missing endpoint substitution declaration",
                                    ));
                                }
                            }
                        }
                    }
                    Expr::Def(def) => {
                        let target = ev.redirected(*def);
                        let definition = ir.def(target);
                        if definition.recursive {
                            return Err(refused("recursive endpoint source metadata"));
                        }
                        if definition.std.is_none() && !ev.is_value_override(target) {
                            enter(model, &mut tasks, scope, target, &[])?;
                        } else {
                            push(&mut values, node.level)?;
                        }
                    }
                    Expr::Apply { callee, args } => {
                        let mut work = usize::MAX;
                        let called = match callee {
                            OpRef::Def(def) => Some(ev.redirected(*def)),
                            OpRef::Const(constant) => match ev.constant_binding(*constant) {
                                Some(ConstBinding::Def(def)) => Some(ev.redirected(def)),
                                _ => return Err(refused("opaque endpoint configured call")),
                            },
                            OpRef::Param(local) => {
                                if let Some((def, scope)) =
                                    ev.scope_apply_param(scope, *local, args).transpose()?
                                {
                                    let target = ir.def(def);
                                    if target.recursive {
                                        return Err(refused("recursive endpoint operator actual"));
                                    }
                                    push(&mut tasks, Task::Enter(Pred::new(target.body, scope)))?;
                                    continue;
                                }
                                if ev
                                    .scope_native_operator_analysis(scope, callee, &mut work)?
                                    .is_none()
                                {
                                    return Err(refused("opaque endpoint operator actual"));
                                }
                                None
                            }
                            OpRef::Std(_) | OpRef::Builtin(_) => None,
                            _ => return Err(refused("opaque endpoint callee")),
                        };
                        if let Some(def) = called {
                            let target = ir.def(def);
                            if target.recursive {
                                return Err(refused("recursive endpoint source metadata"));
                            }
                            if target.std.is_none() && !ev.is_value_override(def) {
                                enter(model, &mut tasks, scope, def, args)?;
                                continue;
                            }
                        }
                        let mut children = Vec::new();
                        for arg in args {
                            match arg {
                                Arg::Expr { expr, .. } => {
                                    push(&mut children, Pred::new(*expr, scope.clone()))?
                                }
                                Arg::Op { op, .. } => {
                                    // Level analysis only, not callback/capture eligibility.
                                    // The independent budgeted closure proves all retained captures.
                                    let Some((def, captured)) =
                                        ev.scope_operator_analysis(scope, op)?
                                    else {
                                        return Err(refused("opaque endpoint native callback"));
                                    };
                                    push(&mut children, Pred::new(ir.def(def).body, captured))?;
                                }
                            }
                        }
                        schedule(&mut tasks, node.level, children)?;
                    }
                    Expr::Quant { bounds, body, .. }
                    | Expr::FcnCons { bounds, body, .. }
                    | Expr::RecFcn { bounds, body, .. } => {
                        binder(
                            &mut tasks,
                            &pred,
                            *body,
                            bounds,
                            matches!(node.expr, Expr::RecFcn { .. }),
                        )?;
                    }
                    Expr::SetMap { bounds, expr, .. } => {
                        binder(&mut tasks, &pred, *expr, bounds, false)?
                    }
                    Expr::SetFilter { bound, pred: body } => {
                        binder(&mut tasks, &pred, *body, std::slice::from_ref(bound), false)?
                    }
                    _ => {
                        if node.level == Level::Temporal {
                            push(&mut values, Level::Temporal)?;
                            continue;
                        }
                        let mut children = Vec::new();
                        for child in node.expr.children() {
                            push(&mut children, Pred::new(child, scope.clone()))?;
                        }
                        schedule(&mut tasks, node.level, children)?;
                    }
                }
            }
        }
    }
    take(&mut values)
}
fn take(values: &mut Vec<Level>) -> Result<Level, FormulaError> {
    values
        .pop()
        .ok_or_else(|| refused("missing endpoint scoped level"))
}
fn slot(
    tasks: &mut Vec<Task>,
    values: &mut Vec<Level>,
    slot: SlotView,
) -> Result<(), FormulaError> {
    match slot {
        SlotView::Value(_) | SlotView::AnalysisConstant => push(values, Level::Constant),
        SlotView::AnalysisBound => push(values, Level::State),
        SlotView::Lazy(expr, scope) => push(tasks, Task::Enter(Pred::new(expr, scope))),
        _ => Err(refused("opaque endpoint scoped value")),
    }
}
fn enter(
    model: &Model,
    tasks: &mut Vec<Task>,
    scope: &Scope,
    def: DefId,
    args: &[Arg],
) -> Result<(), FormulaError> {
    let target = model.evaluator.redirected(def);
    let definition = model.ir.def(target);
    if definition.recursive || definition.std.is_some() || model.evaluator.is_value_override(target)
    {
        return Err(refused("opaque endpoint definition metadata"));
    }
    let called = if args.is_empty() {
        model.evaluator.scope_enter(scope, target)?
    } else {
        model.evaluator.scope_apply(scope, target, args)?
    };
    push(tasks, Task::Enter(Pred::new(definition.body, called)))
}
fn schedule(tasks: &mut Vec<Task>, base: Level, children: Vec<Pred>) -> Result<(), FormulaError> {
    push(
        tasks,
        Task::Maximum {
            base,
            count: children.len(),
        },
    )?;
    for child in children.into_iter().rev() {
        push(tasks, Task::Enter(child))?;
    }
    Ok(())
}
fn binder(
    tasks: &mut Vec<Task>,
    pred: &Pred,
    body: ExprId,
    bounds: &[dive_ir::Bound],
    recursive: bool,
) -> Result<(), FormulaError> {
    push(
        tasks,
        Task::Binder {
            pred: pred.clone(),
            body,
            count: bounds.len(),
            recursive,
        },
    )?;
    for bound in bounds.iter().rev() {
        let domain = bound
            .domain
            .ok_or_else(|| refused("unbounded endpoint predicate domain"))?;
        push(tasks, Task::Enter(Pred::new(domain, pred.scope.clone())))?;
    }
    Ok(())
}

// scope_subst retains eager semantics for explicitly strict substitutions.
// Do not execute one while deciding whether endpoint replay is admissible.
pub(super) fn substitution(model: &Model, pred: &Pred) -> Result<Scope, FormulaError> {
    let Expr::Subst { bindings, .. } = model.ir.expr(pred.expr) else {
        return Err(refused("missing endpoint substitution"));
    };
    if bindings.iter().any(|binding| {
        matches!(
            binding.value,
            Arg::Expr {
                pass: dive_ir::Pass::Strict,
                ..
            }
        )
    }) {
        return Err(refused(
            "strict substitution requires an independent no-evaluation binding proof",
        ));
    }
    Ok(model.evaluator.scope_subst(&pred.scope, pred.expr)?)
}
