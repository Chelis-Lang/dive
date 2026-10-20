//! Deferred-filter metadata inspection never evaluates a redirect or its actuals.
#![allow(clippy::expect_used, reason = "bounded source regression assertions")]
use dive_eval::{ConstBinding, ErrorClass, Evaluator, Model, Options, St};
use dive_ir::{Arg, Expr, OpRef};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = r"---- MODULE Redirects ----
EXTENDS Naturals, Sequences, TLC
CONSTANT Max
Unused(s) == s
Pure(s) == Max
Callee(s) == s
Through(s) == Callee(s) + Max
Zero == 0
Bad == PrintT(0)
Effect(s) == PrintT(s)
RECURSIVE Rec(_)
Rec(s) == IF Max = 0 THEN 0 ELSE Rec(s)
Captured(s) == LET K == Max IN K
LazyActual == Unused(1 \div 0)
Domain == {f \in [{1} -> BOOLEAN] : f[1] = FALSE}
ReadDomain == Domain
Q == \A v \in Domain : v
ErrorQ == \A v \in {f \in [{1} -> BOOLEAN] : (1 \div 0) = 0} : v
Injected(P(_)) == \A v \in {f \in [{1} -> BOOLEAN] : P(f)} : v
====";
fn model(source: &str) -> Model {
    let a = dive_sema::analyze_source("Redirects.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    Model::new(Arc::new(dive_ir::lower(&a).expect("IR")))
}
fn configure(model: &mut Model, target: &str) {
    model.redirects.insert(
        model.ir.root_def("Unused").expect("Unused"),
        model.ir.root_def(target).expect("target"),
    );
}
fn bindings(
    ev: &Evaluator,
    name: &str,
    limit: usize,
    bounded: bool,
) -> dive_eval::Result<Vec<Value>> {
    let d = ev.ir().root_def(name).expect("quantifier");
    let expr = ev.ir().def(d).body;
    let Expr::Quant { body, .. } = ev.ir().expr(expr) else {
        unreachable_quantifier()
    };
    let scope = ev.scope(d).expect("scope");
    let scopes = if bounded {
        ev.scope_bindings_bounded(&scope, expr, &St::constant(), limit)?
    } else {
        ev.scope_bindings(&scope, expr, &St::constant())?
    };
    scopes
        .into_iter()
        .map(|scope| ev.eval_scoped(&scope, *body, &St::constant()))
        .collect()
}
#[allow(clippy::panic, reason = "test fixture invariant")]
fn unreachable_quantifier() -> ! {
    panic!("quantifier fixture")
}
#[derive(Debug, Default)]
struct Dormant(AtomicUsize);
impl dive_value::DeferredSet for Dormant {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(false))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}
#[derive(Debug, Default)]
struct Hooks(AtomicUsize);
impl dive_eval::Host for Hooks {
    fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn evaluator(model: Model, opts: Options) -> (Evaluator, Arc<Hooks>) {
    let ev = Evaluator::new(model, opts).expect("valid evaluator");
    let hooks = Arc::new(Hooks::default());
    ev.set_host(hooks.clone());
    (ev, hooks)
}
#[test]
fn scalar_bound_redirects_preserve_bindings_cold_and_warm_without_forcing_unused_actuals() {
    for opts in [Options::tlc(), Options::dive()] {
        for scalar in 0..4 {
            for warm in [false, true] {
                let mut m = model(SOURCE);
                configure(&mut m, "Pure");
                let value = match scalar {
                    0 => Value::Int(1),
                    1 => Value::TRUE,
                    2 => Value::string(&m.names, "one"),
                    _ => Value::model(&m.names, "one"),
                };
                m.constants[0] = Some(ConstBinding::Value(value.clone()));
                let (ev, hooks) = evaluator(m, opts);
                let domain_id = ev
                    .ir()
                    .root_def("ReadDomain")
                    .expect("cached definition use");
                let domain = ev
                    .eval_def(domain_id, &St::constant())
                    .expect("domain constructor");
                let set = domain.as_set().expect("deferred domain");
                assert!(set.as_deferred().is_some());
                let cache = warm.then(|| set.elements(opts.ctx).expect("warm exact retained set"));
                let reread = ev
                    .eval_def(domain_id, &St::constant())
                    .expect("same domain");
                assert!(set.ptr_eq(reread.as_set().expect("same set")));
                let actual = bindings(&ev, "Q", 4096, true).expect("scalar metadata");
                assert_eq!(actual.len(), 1);
                assert_eq!(
                    actual,
                    bindings(&ev, "Q", 4096, false).expect("ordinary bindings")
                );
                assert_eq!(
                    ev.eval_def(
                        ev.ir().root_def("LazyActual").expect("lazy call"),
                        &St::constant()
                    )
                    .expect("unused division remains lazy"),
                    value
                );
                for limit in [0, 1] {
                    assert_eq!(
                        bindings(&ev, "Q", limit, true)
                            .expect_err("same small bound, also warm")
                            .class(),
                        ErrorClass::ResourceLimit
                    );
                }
                if let Some(cache) = cache {
                    let reread = set.elements(opts.ctx).expect("same warm elements");
                    assert!(Arc::ptr_eq(&cache, &reread));
                }
                assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
                assert_eq!(ev.take_output(), Vec::<String>::new());
            }
        }
    }
}
#[test]
fn unresolved_nonscalar_definition_and_effectful_redirects_remain_callback_free_refusals() {
    for opts in [Options::tlc(), Options::dive()] {
        for case in 0..9 {
            let mut m = model(SOURCE);
            configure(&mut m, "Pure");
            let dormant = Arc::new(Dormant::default());
            m.constants[0] = match case {
                0 => None,
                1 => Some(ConstBinding::Def(m.ir.root_def("Zero").expect("Zero"))),
                2 => Some(ConstBinding::Def(m.ir.root_def("Bad").expect("Bad"))),
                3 => Some(ConstBinding::Value(Value::set_raw(
                    vec![1.into()],
                    opts.ctx,
                ))),
                4 => Some(ConstBinding::Value(Value::tuple(vec![1.into()]))),
                8 => Some(ConstBinding::Value(Value::deferred(
                    dormant.clone(),
                    opts.ctx,
                ))),
                _ => Some(ConstBinding::Value(1.into())),
            };
            if (5..8).contains(&case) {
                configure(&mut m, ["Effect", "Rec", "Captured"][case - 5]);
            }
            let (ev, hooks) = evaluator(m, opts);
            assert_eq!(dormant.0.load(Ordering::SeqCst), 0);
            for warm in [false, true] {
                if warm {
                    bindings(&ev, "Q", 4096, false).expect("pure original filter warmup");
                }
                let e = bindings(&ev, "Q", 4096, true).expect_err("unsupported metadata");
                assert_eq!(e.class(), ErrorClass::Unsupported, "case{case}: {e}");
            }
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
            assert_eq!(dormant.0.load(Ordering::SeqCst), 0);
            assert_eq!(ev.take_output(), Vec::<String>::new());
        }
    }
}
#[test]
fn actual_redirected_callees_and_original_predicate_error_order_are_retained() {
    for opts in [Options::tlc(), Options::dive()] {
        let mut m = model(SOURCE);
        configure(&mut m, "Through");
        m.constants[0] = Some(ConstBinding::Value(1.into()));
        m.redirects.insert(
            m.ir.root_def("Callee").expect("Callee"),
            m.ir.root_def("Pure").expect("Pure"),
        );
        let (ev, hooks) = evaluator(m, opts);
        assert_eq!(
            bindings(&ev, "Q", 4096, true)
                .expect("redirected pure callee")
                .len(),
            1
        );
        assert_eq!(
            hooks.0.load(Ordering::SeqCst),
            0,
            "metadata and pure Q call no native hook"
        );
        let actual = bindings(&ev, "ErrorQ", 4096, true).expect_err("actual predicate error");
        let ordinary = bindings(&ev, "ErrorQ", 4096, false).expect_err("ordinary error");
        assert_eq!(
            (actual.class(), actual.ec(), actual.message()),
            (ordinary.class(), ordinary.ec(), ordinary.message())
        );
        assert_eq!(
            bindings(&ev, "ErrorQ", 1, true)
                .expect_err("bound before predicate")
                .class(),
            ErrorClass::ResourceLimit
        );
        // Each actually evaluated division invokes its original native hook.
        // The small-quota refusal adds no predicate/hook execution.
        assert_eq!(hooks.0.load(Ordering::SeqCst), 2);
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}
#[test]
fn open_global_redirect_does_not_admit_rust_injected_operators() {
    for opts in [Options::tlc(), Options::dive()] {
        let mut m = model(SOURCE);
        configure(&mut m, "Pure");
        m.constants[0] = Some(ConstBinding::Value(1.into()));
        let (ev, hooks) = evaluator(m, opts);
        let d = ev.ir().root_def("Injected").expect("Injected");
        let scope = ev
            .scope_apply(
                &ev.scope(d).expect("scope"),
                d,
                &[Arg::Op {
                    op: OpRef::Std(dive_ir::StdOp::TlcPrintT),
                    level: dive_ir::Level::Constant,
                }],
            )
            .expect("injected original context");
        let error = ev
            .scope_bindings_bounded(&scope, ev.ir().def(d).body, &St::constant(), 4096)
            .expect_err("captured operator remains opaque");
        assert_eq!(error.class(), ErrorClass::Unsupported);
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}

#[test]
fn operator_bound_constants_and_higher_order_redirect_targets_stay_opaque() {
    for opts in [Options::tlc(), Options::dive()] {
        for operator in [false, true] {
            let (constant, formal, body) = if operator {
                ("Max(_)", "s", "Max(s)")
            } else {
                ("Max", "P(_)", "Max")
            };
            let source = format!(
                "---- MODULE Redirects ----\nEXTENDS Naturals\nCONSTANT {constant}\nUnused({formal}) == TRUE\nPure({formal}) == {body}\nCallee(s) == s\nQ == \\A v \\in {{f \\in [{{1}} -> BOOLEAN] : f[1] = FALSE}} : v\n====\n"
            );
            let mut m = model(&source);
            configure(&mut m, "Pure");
            m.constants[0] = Some(if operator {
                ConstBinding::Def(m.ir.root_def("Callee").expect("Callee"))
            } else {
                ConstBinding::Value(1.into())
            });
            let (ev, hooks) = evaluator(m, opts);
            assert_eq!(
                bindings(&ev, "Q", 4096, true)
                    .expect_err("operator metadata remains unsupported")
                    .class(),
                ErrorClass::Unsupported
            );
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
            assert_eq!(ev.take_output(), Vec::<String>::new());
        }
    }
}

#[test]
fn mixed_definitely_effectful_redirects_refuse_before_open_metadata_even_without_work() {
    const MIXED: &str = r"---- MODULE Redirects ----
EXTENDS Naturals, TLC
CONSTANT Max
A(s) == s
B(s) == s
Pure(s) == Max
Bad(s) == PrintT(s)
Domain == {f \in [{1} -> BOOLEAN] : f[1] = FALSE}
ReadDomain == Domain
Q == \A v \in Domain : v
====";
    for opts in [Options::tlc(), Options::dive()] {
        // Fresh maps exercise arbitrary iteration orders; no particular order
        // or distribution is required for any assertion.
        for _ in 0..32 {
            for warm in [false, true] {
                let mut m = model(MIXED);
                m.constants[0] = Some(ConstBinding::Value(1.into()));
                for (from, to) in [("A", "Pure"), ("B", "Bad")] {
                    m.redirects.insert(
                        m.ir.root_def(from).expect("from"),
                        m.ir.root_def(to).expect("target"),
                    );
                }
                assert!(
                    m.ir.def(m.ir.root_def("Pure").expect("Pure"))
                        .effects
                        .intersects(dive_ir::Effects::OPEN)
                );
                assert!(
                    m.ir.def(m.ir.root_def("Bad").expect("Bad"))
                        .effects
                        .intersects(dive_ir::Effects::IMPURE)
                );
                let (ev, hooks) = evaluator(m, opts);
                let read = ev.ir().root_def("ReadDomain").expect("cached domain use");
                let domain = ev
                    .eval_def(read, &St::constant())
                    .expect("domain constructor");
                let set = domain.as_set().expect("deferred domain");
                assert!(set.as_deferred().is_some());
                let cache = warm.then(|| set.elements(opts.ctx).expect("warm actual domain"));
                let same = ev
                    .eval_def(read, &St::constant())
                    .expect("same cached domain");
                assert!(set.ptr_eq(same.as_set().expect("set")));
                for limit in [0, 1, 3, 4096] {
                    let error = bindings(&ev, "Q", limit, true)
                        .expect_err("original definite effect refusal");
                    assert_eq!(
                        error.class(),
                        ErrorClass::Unsupported,
                        "limit{limit}/warm{warm}: {error}"
                    );
                    assert_eq!(
                        error.message(),
                        "effectful or unresolved deferred binder predicate"
                    );
                }
                if let Some(cache) = cache {
                    assert!(Arc::ptr_eq(
                        &cache,
                        &set.elements(opts.ctx).expect("same warm cache")
                    ));
                }
                assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
                assert_eq!(ev.take_output(), Vec::<String>::new());
            }
        }
    }
}
