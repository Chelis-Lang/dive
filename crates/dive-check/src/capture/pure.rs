//! Conservative context-aware purity proof for the concrete pool capture backend.
//!
//! IR effects alone cannot describe a public caller's captured operator slots.
//! Follow lazy/context bindings without evaluating them; opaque captures refuse.

use dive_eval::{ConstBinding, Scope, SlotView};
use dive_ir::{Arg, Effects, Expr, ExprId, OpRef};
use dive_model::{Model, Pred};
use dive_value::{Interner, Value, ValueOrder};

fn predicate(model: &Model, pred: &Pred) -> bool {
    let mut budget = 50_000usize;
    expression(model, pred.expr, &pred.scope, 0, &mut budget)
}

fn slot(model: &Model, slot: SlotView, depth: u32, budget: &mut usize) -> bool {
    match slot {
        SlotView::AnalysisBound => true,
        SlotView::Value(value) => material(
            &model.names,
            model.evaluator.options().ctx.order,
            &value,
            depth + 1,
            budget,
        ),
        SlotView::Lazy(expr, scope) => expression(model, expr, &scope, depth + 1, budget),
        // Operator arguments and opaque lazy functions carry hidden environments.
        SlotView::Unset | SlotView::Other => false,
    }
}

fn material(
    names: &Interner,
    order: ValueOrder,
    value: &Value,
    depth: u32,
    budget: &mut usize,
) -> bool {
    // Admission has already established model/scope ownership. Only stored
    // representations are inspected here: no normalization, enumeration,
    // equality, hashing or deferred provider methods are purity authority.
    // Stored sets/records/general functions must share the run order: same
    // names alone cannot make their cached sequence equivalent. Tuple domains
    // are implicit ascending integers and intentionally store Canonical order.
    if depth > 128 || *budget == 0 {
        return false;
    }
    *budget -= 1;
    match value {
        Value::Bool(_) | Value::Int(_) | Value::BigInt(_) => true,
        Value::Str(atom) | Value::Model(atom) => names
            .resolve(atom.tok())
            .is_some_and(|owned| owned.as_str() == atom.as_str()),
        Value::Fcn(function) => {
            (function.kind() == dive_value::FcnKind::Tuple || function.order() == order)
                && function.iter().all(|(domain, range)| {
                    material(names, order, &domain, depth + 1, budget)
                        && material(names, order, range, depth + 1, budget)
                })
        }
        // Cached deferred elements remain deferred. stored_elements exposes
        // only explicit Enum/Raw storage, and never calls a provider.
        Value::Set(set) => {
            set.order() == order
                && set.stored_elements().is_some_and(|elements| {
                    elements
                        .iter()
                        .all(|element| material(names, order, element, depth + 1, budget))
                })
        }
    }
}

fn constant(
    model: &Model,
    constant: dive_ir::ConstId,
    scope: &Scope,
    args: &[Arg],
    depth: u32,
    budget: &mut usize,
) -> bool {
    match model.evaluator.constant_binding(constant) {
        Some(ConstBinding::Value(value)) => material(
            &model.names,
            model.evaluator.options().ctx.order,
            &value,
            depth + 1,
            budget,
        ),
        Some(ConstBinding::Def(def)) => definition(model, def, scope, args, depth + 1, budget),
        None => false,
    }
}

fn definition(
    model: &Model,
    def: dive_ir::DefId,
    scope: &Scope,
    args: &[Arg],
    depth: u32,
    budget: &mut usize,
) -> bool {
    if depth > 128 || *budget == 0 {
        return false;
    }
    let def = model.evaluator.redirected(def);
    // Value overrides are opaque to this context proof.
    if model.evaluator.is_value_override(def) {
        return false;
    }
    let Some(node) = model.ir.defs().get(def.index()) else {
        return false;
    };
    if let Some(op) = node.std {
        return !op.impure()
            && !op.creates_runtime_names()
            && !matches!(
                op.support(),
                dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
            )
            && args
                .iter()
                .all(|arg| argument(model, arg, scope, depth + 1, budget));
    }
    let called = if args.is_empty() {
        model.evaluator.scope_enter(scope, def)
    } else {
        model.evaluator.scope_apply(scope, def, args)
    };
    called.is_ok_and(|called| expression(model, node.body, &called, depth + 1, budget))
}

fn argument(model: &Model, arg: &Arg, scope: &Scope, depth: u32, budget: &mut usize) -> bool {
    match arg {
        Arg::Expr { expr, .. } => expression(model, *expr, scope, depth + 1, budget),
        Arg::Op { op, .. } => operator(model, op, scope, depth + 1, budget),
    }
}

fn operator(model: &Model, op: &OpRef, scope: &Scope, depth: u32, budget: &mut usize) -> bool {
    depth <= 128
        && *budget > 0
        && model
            .evaluator
            .scope_operator_analysis(scope, op)
            .is_ok_and(|known| {
                known.is_some_and(|(def, captured)| {
                    model
                        .evaluator
                        .scope_operator_captures(&captured, budget)
                        .is_ok_and(|captures| {
                            captures.is_some_and(|captures| {
                                captures.into_iter().all(|(body, scope)| {
                                    expression(model, body, &scope, depth + 1, budget)
                                })
                            })
                        })
                        && expression(model, model.ir.def(def).body, &captured, depth + 1, budget)
                })
            })
}

#[allow(
    clippy::too_many_lines,
    reason = "conservative lexical scope proof has explicit refusal branches"
)]
fn expression(model: &Model, expr: ExprId, scope: &Scope, depth: u32, budget: &mut usize) -> bool {
    if depth > 128 || *budget == 0 {
        return false;
    }
    *budget -= 1;
    // OPEN includes every configured constant, even a stored scalar. Resolve
    // those through the scoped walk below; unresolved/opaque slot and operator
    // branches still refuse. An annotation alone never admits a capture.
    let forbidden = Effects::IMPURE.union(Effects::LIVE_TLCGET);
    let Some(node) = model.ir.nodes().get(expr.index()) else {
        return false;
    };
    if node.effects.intersects(forbidden) {
        return false;
    }
    match model.ir.expr(expr) {
        Expr::Unbound(_) | Expr::Unevaluable(_) => false,
        Expr::Let { defs, body } => {
            defs.iter()
                .all(|d| model.ir.defs().get(d.index()).is_some())
                && model
                    .evaluator
                    .scope_let(scope, expr)
                    .is_ok_and(|sc| expression(model, *body, &sc, depth + 1, budget))
        }
        Expr::Subst { body, .. } => model
            .evaluator
            .scope_subst(scope, expr)
            .is_ok_and(|sc| expression(model, *body, &sc, depth + 1, budget)),
        Expr::Quant { bounds, body, .. }
        | Expr::FcnCons { bounds, body, .. }
        | Expr::SetMap { bounds, expr: body } => {
            bindings(model, expr, bounds, *body, scope, depth, budget)
        }
        Expr::SetFilter { bound, pred } => bindings(
            model,
            expr,
            std::slice::from_ref(bound),
            *pred,
            scope,
            depth,
            budget,
        ),
        Expr::Local(local) => slot(
            model,
            model.evaluator.scope_slot(scope, *local),
            depth,
            budget,
        ),
        Expr::SubstRef { decl, .. } => {
            let bound = model.evaluator.scope_subst_slot(scope, *decl);
            if matches!(bound, SlotView::Unset) {
                model.ir.decls().get(decl.index()).is_some_and(|d| {
                    d.constant
                        .is_none_or(|c| constant(model, c, scope, &[], depth, budget))
                })
            } else {
                slot(model, bound, depth, budget)
            }
        }
        Expr::Const(c) => constant(model, *c, scope, &[], depth, budget),
        Expr::Def(def) => definition(model, *def, scope, &[], depth, budget),
        Expr::Apply { callee, args } => match callee {
            OpRef::Def(def) => definition(model, *def, scope, args, depth, budget),
            OpRef::Const(c) => constant(model, *c, scope, args, depth, budget),
            OpRef::Param(local) => {
                operator(model, callee, scope, depth + 1, budget)
                    && args.iter().all(|arg| {
                        matches!(arg, Arg::Expr { .. })
                            && argument(model, arg, scope, depth + 1, budget)
                    })
                    && model
                        .evaluator
                        .scope_apply_param(scope, *local, args)
                        .is_some_and(|called| {
                            called.is_ok_and(|(def, called)| {
                                expression(
                                    model,
                                    model.ir.def(def).body,
                                    &called,
                                    depth + 1,
                                    budget,
                                )
                            })
                        })
            }
            OpRef::Subst(_) | OpRef::Unbound(_) => false,
            OpRef::Std(op) => {
                !op.impure()
                    && !op.creates_runtime_names()
                    && !matches!(
                        op.support(),
                        dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                    )
                    && args
                        .iter()
                        .all(|a| argument(model, a, scope, depth, budget))
            }
            OpRef::Builtin(_) => args
                .iter()
                .all(|a| argument(model, a, scope, depth, budget)),
        },
        _ => model
            .ir
            .expr(expr)
            .children()
            .iter()
            .all(|child| expression(model, *child, scope, depth + 1, budget)),
    }
}

fn bindings(
    model: &Model,
    expr: ExprId,
    bounds: &[dive_ir::Bound],
    body: ExprId,
    scope: &Scope,
    depth: u32,
    budget: &mut usize,
) -> bool {
    bounds.iter().all(|b| {
        b.domain
            .is_some_and(|dom| expression(model, dom, scope, depth + 1, budget))
    }) && model
        .evaluator
        .scope_binder_analysis(scope, expr)
        .is_ok_and(|sc| expression(model, body, &sc, depth + 1, budget))
}

pub(super) fn model(model: &Model) -> bool {
    let predicates = model
        .init
        .iter()
        .map(|i| &i.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|n| &n.pred))
        .chain(model.implied_inits.iter().map(|n| &n.pred))
        .chain(model.implied_actions.iter().map(|n| &n.pred))
        .chain(model.constraints.iter().map(|n| &n.pred))
        .chain(model.action_constraints.iter().map(|n| &n.pred));
    predicates.into_iter().all(|p| predicate(model, p))
        && model.assumptions.iter().all(|d| {
            model.ir.defs().get(d.index()).is_some_and(|def| {
                model
                    .evaluator
                    .scope(*d)
                    .is_ok_and(|scope| predicate(model, &Pred::new(def.body, scope)))
            })
        })
        && model
            .init_actions
            .iter()
            .chain(&model.actions)
            .all(|a| predicate(model, &Pred::new(a.expr(), a.scope())))
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "bounded admission assertions")]
mod tests {
    use super::{expression, material};
    use dive_value::{Ctx, DeferredKind, DeferredSet, Interner, Value};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Debug)]
    struct Opaque(Arc<AtomicUsize>);
    impl DeferredSet for Opaque {
        fn kind(&self) -> DeferredKind {
            self.0.fetch_add(1, Ordering::Relaxed);
            DeferredKind::Filter
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(vec![Value::Int(1)])
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Ok(true))
        }
        fn key(&self) -> Vec<Value> {
            self.0.fetch_add(1, Ordering::Relaxed);
            vec![Value::Int(1)]
        }
    }

    #[test]
    fn stored_values_names_and_provider_caches_are_not_callback_authority() {
        for ctx in [Ctx::tlc(), Ctx::dive()] {
            let names = Interner::new();
            let string = Value::Str(names.intern("string"));
            let model = Value::Model(names.intern("m"));
            let explicit = Value::set(vec![Value::Int(2), Value::Int(1)], ctx).expect("set");
            let raw = Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx);
            let record = Value::record(vec![(names.intern("field"), model)], ctx).expect("record");
            let function = Value::fcn(vec![(string, explicit)], ctx).expect("function");
            let general = Value::fcn(
                vec![
                    (Value::Int(1), Value::Bool(true)),
                    (Value::Int(3), Value::Bool(false)),
                ],
                ctx,
            )
            .expect("sparse function");
            assert!(matches!(&general, Value::Fcn(f) if f.kind()==dive_value::FcnKind::General));
            let value = Value::tuple(vec![record, function, general, raw]);
            assert!(material(&names, ctx.order, &value, 0, &mut 50_000));
            let calls = Arc::new(AtomicUsize::new(0));
            let provider = Value::deferred(Arc::new(Opaque(calls.clone())), ctx);
            // A composite constructor can normalize/hash the nested provider.
            // Retain that baseline, then require admission adds no callbacks.
            let direct_baseline = calls.load(Ordering::Relaxed);
            assert!(!material(&names, ctx.order, &provider, 0, &mut 50_000));
            assert_eq!(calls.load(Ordering::Relaxed), direct_baseline);
            let nested = Value::tuple(vec![provider.clone()]);
            let baseline = calls.load(Ordering::Relaxed);
            assert!(!material(&names, ctx.order, &provider, 0, &mut 50_000));
            assert!(!material(&names, ctx.order, &nested, 0, &mut 50_000));
            assert_eq!(calls.load(Ordering::Relaxed), baseline);
            // The constructor has cached enumeration, but representation remains
            // Deferred: no cached-provider promotion into a pure stored set.
            assert!(baseline > 0);
            assert!(matches!(provider, Value::Set(set) if set.stored_elements().is_none()));
            assert_eq!(calls.load(Ordering::Relaxed), baseline);
        }
    }

    #[test]
    fn material_work_depth_and_namespace_bounds_are_recursive() {
        let names = Interner::new();
        names.intern("owned");
        let foreign = Interner::new();
        let collision = Value::Str(foreign.intern("foreign"));
        assert!(!material(
            &names,
            Ctx::tlc().order,
            &collision,
            0,
            &mut 50_000
        ));
        let record = Value::record(vec![(foreign.intern("key"), Value::Int(0))], Ctx::tlc())
            .expect("record");
        assert!(!material(&names, Ctx::tlc().order, &record, 0, &mut 50_000));
        let tuple = Value::tuple(vec![Value::Int(0); 24_999]);
        let mut exact = 49_999;
        assert!(material(&names, Ctx::tlc().order, &tuple, 0, &mut exact));
        assert_eq!(exact, 0);
        let mut short = 49_998;
        assert!(!material(&names, Ctx::tlc().order, &tuple, 0, &mut short));
        assert_eq!(short, 0);
        assert!(!material(
            &names,
            Ctx::tlc().order,
            &Value::Bool(true),
            0,
            &mut 0
        ));
        let mut deep = Value::Bool(true);
        for _ in 0..128 {
            deep = Value::tuple(vec![deep]);
        }
        assert!(material(&names, Ctx::tlc().order, &deep, 0, &mut 50_000));
        deep = Value::tuple(vec![deep]);
        assert!(!material(&names, Ctx::tlc().order, &deep, 0, &mut 50_000));
    }

    #[test]
    fn unknown_and_unevaluable_open_leaves_never_pass_vacuously() {
        let analysis = dive_sema::analyze_source(
            "M.tla",
            "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n",
            &dive_sema::Options::default(),
        );
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("cfg");
        let mut model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind");
        let pred = model.next.clone().expect("Next");
        let leaves = [
            dive_ir::Expr::Unbound("hidden".into()),
            dive_ir::Expr::Unevaluable(dive_ir::Unevaluable::Proof),
            dive_ir::Expr::Apply {
                callee: dive_ir::OpRef::Param(dive_ir::LocalRef {
                    up: 0,
                    slot: dive_ir::Slot(u32::MAX),
                }),
                args: Box::new([]),
            },
            dive_ir::Expr::Apply {
                callee: dive_ir::OpRef::Subst(dive_ir::DeclId(u32::MAX)),
                args: Box::new([]),
            },
            dive_ir::Expr::Apply {
                callee: dive_ir::OpRef::Unbound("missing".into()),
                args: Box::new([]),
            },
        ];
        for leaf in leaves {
            let mut edited = serde_json::to_value(model.ir.as_ref()).expect("arena JSON");
            edited["exprs"][pred.expr.index()]["expr"] = serde_json::to_value(leaf).expect("leaf");
            edited["exprs"][pred.expr.index()]["effects"] =
                serde_json::to_value(dive_ir::Effects::OPEN).expect("effects");
            model.ir = Arc::new(serde_json::from_value(edited).expect("edited arena"));
            assert!(!expression(&model, pred.expr, &pred.scope, 0, &mut 50_000));
        }
        assert!(!expression(
            &model,
            dive_ir::ExprId(u32::MAX),
            &pred.scope,
            0,
            &mut 50_000
        ));
        assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
    }
}
