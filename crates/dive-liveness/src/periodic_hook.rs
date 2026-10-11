//! Bounded pure supplied-expression composition at native dispatch pauses.
//!
//! This is not binding of the configuration's `_PERIODIC` snapshot. Ordinary
//! binding and ordinary temporal scheduling remain explicitly unsupported.

use dive_eval::{ConstBinding, EvalError, Scope, SlotView};
use dive_ir::{Arg, BuiltinOp, Expr, ExprId, OpRef, StdOp};
use dive_model::{Model, Pred};
use dive_value::{FcnKind, Value};

/// Original evaluation or FALSE-equality result from a real native hook call.
/// An evaluation is diagnostic evidence, never a model-checking verdict.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub(crate) source: u64,
    pub(crate) expression: ExprId,
    pub(crate) result: Result<bool, EvalError>,
}
impl Evaluation {
    /// Actual committed source identity whose dispatch pause was held.
    #[must_use]
    pub fn source(&self) -> u64 {
        self.source
    }
    /// Supplied expression identity, not a configuration role-name assertion.
    #[must_use]
    pub fn expression(&self) -> ExprId {
        self.expression
    }
    /// TLC's `FALSE.equals(value)` result, or the original typed error.
    #[must_use]
    pub fn result(&self) -> &Result<bool, EvalError> {
        &self.result
    }
}

/// Compose a pure zero-state expression with the producer-owned lifecycle.
///
/// The expression is supplied explicitly: it is **not** the configuration's
/// `_PERIODIC` role. Its callback-free admission precedes clocks, workers and
/// evaluation. Stored literals/scalars, finite explicit tuples/records/sets,
/// boolean combinations and a small TLC integer arithmetic whitelist are supported.
/// State/action/temporal references, quantifiers, deferred values, recursion,
/// configured operator replacements, applied operator captures, host access and
/// runtime-name creation refuse. Already-proved pure native slots may remain
/// unused. The existing 50,000-unit/128-depth capture inspection bounds apply,
/// including inspected text bytes. Each evaluation reserves a checked cubic
/// policy weight against the shared graph-work quota. This conservative charge
/// is not a proved evaluator-complexity, instruction, byte or heap bound.
/// Nested construction, sorting and equality can cost more than linear visits.
///
/// Startup effects may already have occurred in the supplied bound model.
/// This API neither reverses them nor admits a fresh configured-hook binding.
/// No run certifies an ordinary schedule, Holds, or a temporal violation.
///
/// # Errors
/// Foreign/malformed model or predicate, unsupported authority or bounded
/// callback-free admission failure, before native execution starts.
pub fn execute(
    model: &Model,
    predicate: &Pred,
    options: &dive_check::CheckOptions,
    limits: crate::phase_graph::Limits,
    policy: crate::periodic_lifecycle::Options,
) -> Result<crate::periodic_lifecycle::Run, EvalError> {
    let work = admit(model, predicate)?;
    Ok(crate::phase_execution::execute_periodic_lifecycle_hook(
        model,
        options,
        limits,
        policy,
        Some((predicate.clone(), work)),
    ))
}

pub(crate) fn execute_requests(
    model: &Model,
    predicate: &Pred,
    options: &dive_check::CheckOptions,
    limits: crate::phase_graph::Limits,
    policy: crate::periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
) -> Result<crate::periodic_lifecycle::Run, EvalError> {
    let work = admit(model, predicate)?;
    Ok(crate::phase_execution::execute_periodic_lifecycle_requests(
        model,
        options,
        limits,
        policy,
        Some((predicate.clone(), work)),
        Some(requests),
    ))
}

pub(crate) fn admit(model: &Model, pred: &Pred) -> Result<usize, EvalError> {
    model.validate_context()?;
    model.evaluator.validate_predicate(&pred.scope, pred.expr)?;
    if model.ir.nodes().get(pred.location.index()).is_none() {
        return Err(EvalError::unsupported(
            "invalid supplied PERIODIC location (DIVE-EVAL-C001)",
        ));
    }
    let mut work = 50_000;
    let captures = model
        .evaluator
        .scope_operator_captures(&pred.scope, &mut work)?;
    let Some(captures) = captures else {
        return Err(refusal());
    };
    if model.evaluator.scope_shares_side_effects(&pred.scope)
        || !captures
            .iter()
            .all(|(expr, scope)| expression(model, *expr, scope, 0, &mut work))
        || !expression(model, pred.expr, &pred.scope, 0, &mut work)
    {
        return Err(refusal());
    }
    Ok(50_000 - work)
}
fn refusal() -> EvalError {
    EvalError::unsupported("bounded pure zero-state supplied PERIODIC expression (DIVE-L0057)")
}
fn charge(depth: u32, work: &mut usize) -> bool {
    if depth > 128 || *work == 0 {
        return false;
    }
    *work -= 1;
    true
}
pub(crate) fn evaluation_charge(weight: usize) -> Option<usize> {
    weight
        .checked_mul(weight)?
        .checked_mul(weight)?
        .checked_add(1)
}
fn text_charge(bytes: usize, work: &mut usize) -> bool {
    let Some(remaining) = work.checked_sub(bytes) else {
        return false;
    };
    *work = remaining;
    true
}
fn material(model: &Model, value: &Value, depth: u32, work: &mut usize) -> bool {
    if !charge(depth, work) {
        return false;
    }
    match value {
        Value::Bool(_) | Value::Int(_) => true,
        Value::Str(a) | Value::Model(a) => {
            text_charge(a.as_str().len(), work)
                && model
                    .names
                    .resolve(a.tok())
                    .is_some_and(|n| n.as_str() == a.as_str())
        }
        Value::Fcn(f) => {
            (f.kind() == FcnKind::Tuple || f.order() == model.evaluator.options().ctx.order)
                && f.iter().all(|(k, v)| {
                    material(model, &k, depth + 1, work) && material(model, v, depth + 1, work)
                })
        }
        Value::Set(s) => {
            s.order() == model.evaluator.options().ctx.order
                && s.stored_elements()
                    .is_some_and(|v| v.iter().all(|x| material(model, x, depth + 1, work)))
        }
        Value::BigInt(_) => false,
    }
}
fn slot(model: &Model, value: SlotView, depth: u32, work: &mut usize) -> bool {
    match value {
        SlotView::Value(value) => material(model, &value, depth + 1, work),
        SlotView::Lazy(expr, scope) => expression(model, expr, &scope, depth + 1, work),
        _ => false,
    }
}
fn constant(model: &Model, c: dive_ir::ConstId, depth: u32, work: &mut usize) -> bool {
    match model.evaluator.constant_binding(c) {
        Some(ConstBinding::Value(value)) => material(model, &value, depth + 1, work),
        // Configured operator replacement authority is a separate obligation.
        _ => false,
    }
}
pub(crate) fn native(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::NaturalsPlus
            | StdOp::NaturalsMinus
            | StdOp::NaturalsTimes
            | StdOp::NaturalsLt
            | StdOp::NaturalsLeq
            | StdOp::NaturalsGt
            | StdOp::NaturalsGeq
            | StdOp::NaturalsDiv
            | StdOp::NaturalsMod
            | StdOp::IntegersPlus
            | StdOp::IntegersMinus
            | StdOp::IntegersTimes
            | StdOp::IntegersLt
            | StdOp::IntegersLeq
            | StdOp::IntegersGt
            | StdOp::IntegersGeq
            | StdOp::IntegersDiv
            | StdOp::IntegersMod
            | StdOp::IntegersNeg
    )
}
pub(crate) fn builtin(op: BuiltinOp) -> bool {
    matches!(
        op,
        BuiltinOp::Eq
            | BuiltinOp::Neq
            | BuiltinOp::Not
            | BuiltinOp::And
            | BuiltinOp::Or
            | BuiltinOp::Implies
            | BuiltinOp::Equiv
    )
}
fn definition(
    model: &Model,
    def: dive_ir::DefId,
    args: &[Arg],
    scope: &Scope,
    depth: u32,
    work: &mut usize,
) -> bool {
    if !charge(depth, work)
        || model.evaluator.redirected(def) != def
        || model.evaluator.is_value_override(def)
    {
        return false;
    }
    let Some(d) = model.ir.defs().get(def.index()) else {
        return false;
    };
    if d.recursive {
        return false;
    }
    if let Some(op) = d.std {
        return native(op)
            && i32::try_from(args.len()).ok() == Some(op.java_arity())
            && arguments(model, args, scope, depth, work);
    }
    if args.len() != d.params.len() || !arguments(model, args, scope, depth, work) {
        return false;
    }
    let called = if args.is_empty() {
        model.evaluator.scope_enter(scope, def)
    } else {
        model.evaluator.scope_apply(scope, def, args)
    };
    called.is_ok_and(|sc| expression(model, d.body, &sc, depth + 1, work))
}
fn arguments(model: &Model, args: &[Arg], scope: &Scope, depth: u32, work: &mut usize) -> bool {
    args.iter().all(|a| match a {
        Arg::Expr { expr, .. } => expression(model, *expr, scope, depth + 1, work),
        Arg::Op { .. } => false,
    })
}
#[allow(
    clippy::too_many_lines,
    reason = "explicit bounded zero-state grammar refuses unknown expressions instead of trusting levels"
)]
fn expression(model: &Model, expr: ExprId, scope: &Scope, depth: u32, work: &mut usize) -> bool {
    if !charge(depth, work) {
        return false;
    }
    let Some(node) = model.ir.nodes().get(expr.index()) else {
        return false;
    };
    if node.effects.intersects(
        dive_ir::Effects::IMPURE
            .union(dive_ir::Effects::LIVE_TLCGET)
            .union(dive_ir::Effects::TEMPORAL),
    ) {
        return false;
    }
    match &node.expr {
        Expr::Bool(_) => true,
        Expr::Num(text) | Expr::Str(text) => text_charge(text.len(), work),
        Expr::Const(c) => constant(model, *c, depth, work),
        Expr::Local(l) => slot(model, model.evaluator.scope_slot(scope, *l), depth, work),
        Expr::Def(d) => definition(model, *d, &[], scope, depth, work),
        Expr::Apply { callee, args } => match callee {
            OpRef::Def(d) => definition(model, *d, args, scope, depth, work),
            OpRef::Std(op) => {
                native(*op)
                    && i32::try_from(args.len()).ok() == Some(op.java_arity())
                    && arguments(model, args, scope, depth, work)
            }
            OpRef::Builtin(op) => builtin(*op) && arguments(model, args, scope, depth, work),
            _ => false,
        },
        Expr::Builtin { op, args } => {
            builtin(*op)
                && args
                    .iter()
                    .all(|e| expression(model, *e, scope, depth + 1, work))
        }
        Expr::Let { defs, body } => {
            defs.iter()
                .all(|d| model.ir.defs().get(d.index()).is_some_and(|d| !d.recursive))
                && model
                    .evaluator
                    .scope_let(scope, expr)
                    .is_ok_and(|sc| expression(model, *body, &sc, depth + 1, work))
        }
        Expr::Record(fields) => fields.iter().all(|field| {
            text_charge(field.name.len(), work)
                && expression(model, field.value, scope, depth + 1, work)
        }),
        Expr::Junction { .. }
        | Expr::Not(_)
        | Expr::Implies(..)
        | Expr::Equiv(..)
        | Expr::If { .. }
        | Expr::Case { .. }
        | Expr::SetEnum(_)
        | Expr::Tuple(_)
        | Expr::Selected(_)
        | Expr::Label { .. } => node
            .expr
            .children()
            .iter()
            .all(|e| expression(model, *e, scope, depth + 1, work)),
        // No default children().all: unknown leaves cannot be vacuously pure.
        _ => false,
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "bounded callback-free material admission controls"
)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct Opaque(Arc<AtomicUsize>);
    impl dive_value::DeferredSet for Opaque {
        fn kind(&self) -> dive_value::DeferredKind {
            self.0.fetch_add(1, Ordering::Relaxed);
            dive_value::DeferredKind::Filter
        }
        fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(vec![Value::Int(1)])
        }
        fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Ok(true))
        }
        fn key(&self) -> Vec<Value> {
            self.0.fetch_add(1, Ordering::Relaxed);
            vec![Value::Int(1)]
        }
    }
    #[test]
    fn reservation_overflow_and_text_quota_refuse_without_scans() {
        assert_eq!(evaluation_charge(2), Some(9));
        assert_eq!(evaluation_charge(usize::MAX), None);
        let mut work = 2;
        assert!(!text_charge(3, &mut work));
        assert_eq!(work, 2);
        assert!(text_charge(2, &mut work));
        assert_eq!(work, 0);
    }
    #[test]
    fn repeated_calls_accumulate_the_same_graph_quota_before_evaluation() {
        let source = "---- MODULE QuotaHook ----\nVARIABLE x\nInit == x=0\nNext == x'=1\nLive == []<>(x=0)\nHook == <<1,2,3>> = <<1,2,3>>\n====\n";
        let a = dive_sema::analyze_source("QuotaHook.tla", source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&a).expect("ir"));
        let def = dive_ir::DefId(
            u32::try_from(
                ir.defs()
                    .iter()
                    .position(|d| d.name == "Hook")
                    .expect("Hook"),
            )
            .expect("id"),
        );
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
            .into_config()
            .expect("cfg");
        let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind");
        let pred = Pred::new(
            model.ir.def(def).body,
            model.evaluator.scope(def).expect("scope"),
        );
        let reservation =
            evaluation_charge(admit(&model, &pred).expect("admission")).expect("weight");
        let policy = crate::periodic_lifecycle::Options {
            progress_interval: 0,
            ratio_limit: 0.0,
            ..crate::periodic_lifecycle::Options::default()
        };
        for (limit, evaluations) in [(reservation - 1, 0), (2 * reservation - 1, 1)] {
            let mut limits = crate::phase_graph::Limits::default();
            limits.graph.steps = limit;
            let run = execute(
                &model,
                &pred,
                &dive_check::CheckOptions::tlc(0, 0),
                limits,
                policy,
            )
            .expect("shape");
            assert_eq!(run.hook_evaluations().len(), evaluations);
            assert!(matches!(
                run.backend().stop().expect("quota stop").error,
                crate::phase_graph::Error::Limit(_)
            ));
            assert!(run.backend().graph_inspections() <= limit);
            assert!(run.safety_eof().is_none());
            assert!(!run.model_proof());
        }
    }
    #[test]
    fn retained_material_ownership_order_depth_and_quota_never_force_providers() {
        let src = "---- MODULE MaterialHook ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=0)\n====\n";
        let a = dive_sema::analyze_source("MaterialHook.tla", src, &dive_sema::Options::default());
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
            .into_config()
            .expect("cfg");
        let m = dive_model::bind(
            Arc::new(dive_ir::lower(&a).expect("ir")),
            &cfg,
            &dive_model::BindOptions::tlc(),
        )
        .expect("model");
        let ctx = m.evaluator.options().ctx;
        let stored = Value::set_raw(vec![Value::Int(1)], ctx);
        assert!(material(&m, &stored, 0, &mut 50_000));
        assert!(!material(&m, &stored, 0, &mut 1));
        assert!(!material(&m, &Value::Bool(true), 129, &mut 50_000));
        let foreign = dive_value::Interner::new();
        let string = Value::Str(foreign.intern("foreign"));
        assert!(!material(&m, &string, 0, &mut 50_000));
        let opposite = Value::set_raw(vec![Value::Int(1)], dive_value::Ctx::dive());
        assert!(!material(&m, &opposite, 0, &mut 50_000));
        // A tuple's implicit integer domain intentionally stores Canonical.
        assert!(material(
            &m,
            &Value::tuple(vec![Value::Bool(true)]),
            0,
            &mut 50_000
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Value::deferred(Arc::new(Opaque(calls.clone())), ctx);
        let baseline = calls.load(Ordering::Relaxed);
        assert!(!material(&m, &provider, 0, &mut 50_000));
        assert_eq!(calls.load(Ordering::Relaxed), baseline);
        let nested = Value::tuple(vec![provider.clone()]);
        let baseline = calls.load(Ordering::Relaxed);
        assert!(!material(&m, &nested, 0, &mut 50_000));
        assert!(!material(&m, &provider, 0, &mut 50_000));
        assert_eq!(calls.load(Ordering::Relaxed), baseline);
        assert!(matches!(provider,Value::Set(s) if s.stored_elements().is_none()));
    }
}
