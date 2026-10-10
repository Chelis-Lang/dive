//! Bounded lazy domains preserve ordinary enumeration; huge/symbolic operands
//! never reach the materializer merely because their final cardinality is small.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
mod harness;
use dive_eval::{ErrorClass, Evaluator, Options, St};
use dive_ir::Expr;
use dive_value::Value;
use std::sync::Arc;

fn evaluator(domain: &str, opts: Options) -> (Evaluator, dive_ir::DefId) {
    let source = format!(
        "---- MODULE Bounded ----\nEXTENDS Naturals, Sequences\nQ == \\A v \\in {domain} : v\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Bounded.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let def = ir.root_def("Q").expect("Q");
    let loaded = harness::Loaded {
        ir,
        root_text: source,
    };
    let model = harness::bind(&loaded, None, opts).expect("bind");
    (Evaluator::new(model, opts), def)
}

#[test]
fn finite_lazy_bindings_match_unbounded_enumeration_in_both_profiles() {
    for opts in [Options::tlc(), Options::dive()] {
        for domain in [
            "[{1,2} -> BOOLEAN]",
            r"{f \in [{1,2} -> BOOLEAN] : f[1] = FALSE}",
            r"{f[2] : f \in {g \in [{1,2} -> BOOLEAN] : g[1] = FALSE}}",
            r"{f \in {g \in [{1,2} -> BOOLEAN] : g[1] = FALSE} : f[2] = TRUE}",
            "[a : BOOLEAN, b : {\"z\",\"a\"}]",
            "BOOLEAN \\X {\"z\",\"a\"}",
            "SUBSET {\"z\",\"a\"}",
            "[{} -> Nat]",
            "[BOOLEAN -> {}]",
            "SUBSET SUBSET {1,2}",
            "[a : SUBSET {1,2}, b : BOOLEAN]",
            "{1,1,2}",
            "{}",
        ] {
            let (ev, def) = evaluator(domain, opts);
            let expr = ev.ir().def(def).body;
            let Expr::Quant { body, .. } = ev.ir().expr(expr) else {
                panic!("quantifier");
            };
            let scope = ev.scope(def);
            let st = St::constant();
            let collect = |scopes: Vec<dive_eval::Scope>| -> Vec<Value> {
                scopes
                    .into_iter()
                    .map(|s| ev.eval_scoped(&s, *body, &st).expect("body"))
                    .collect()
            };
            let bounded = collect(
                ev.scope_bindings_bounded(&scope, expr, &st, 4096)
                    .expect(domain),
            );
            let ordinary = collect(ev.scope_bindings(&scope, expr, &st).expect(domain));
            assert_eq!(bounded, ordinary, "{domain}/{opts:?}");
        }
    }
}

#[test]
fn finite_lazy_domains_bound_components_and_result_slots_before_allocating() {
    for domain in [
        "SUBSET (1..1000000000)",
        "[1..1000000000 -> {TRUE}]",
        "[BOOLEAN -> 1..1000000000]",
        "[1..32 -> BOOLEAN]",
        "BOOLEAN \\X (1..1000000000)",
        "[a : 1..1000000000]",
        "[{1} -> {1..1000000000}]",
        "{[1..32 -> BOOLEAN]}",
    ] {
        let (ev, def) = evaluator(domain, Options::tlc());
        let result =
            ev.scope_bindings_bounded(&ev.scope(def), ev.ir().def(def).body, &St::constant(), 4096);
        assert!(
            matches!(result, Err(ref e) if e.class() == ErrorClass::ResourceLimit),
            "{domain}: {result:?}"
        );
    }
    // Cardinality sixteen is small; sixteen 4-slot functions and their components are not.
    let (ev, def) = evaluator("[1..4 -> BOOLEAN]", Options::tlc());
    let result =
        ev.scope_bindings_bounded(&ev.scope(def), ev.ir().def(def).body, &St::constant(), 16);
    assert!(
        matches!(result, Err(ref e) if e.class() == ErrorClass::ResourceLimit),
        "{result:?}"
    );
}

#[test]
fn deferred_and_infinite_domains_are_explicit_refusals() {
    for domain in [
        "Nat",
        "Seq(BOOLEAN)",
        r"[{1} -> BOOLEAN] \cup [{2} -> BOOLEAN]",
        "SUBSET Nat",
        "[BOOLEAN -> Nat]",
        "{v \\in Nat : v < 2}",
    ] {
        let (ev, def) = evaluator(domain, Options::tlc());
        let result =
            ev.scope_bindings_bounded(&ev.scope(def), ev.ir().def(def).body, &St::constant(), 4096);
        assert!(
            matches!(result, Err(ref e) if e.class() == ErrorClass::Unsupported),
            "{domain}: {result:?}"
        );
    }
}

#[test]
fn eager_nonfinite_map_keeps_the_existing_tlc_error() {
    let (ev, def) = evaluator("{v : v \\in Nat}", Options::tlc());
    let result =
        ev.scope_bindings_bounded(&ev.scope(def), ev.ir().def(def).body, &St::constant(), 4096);
    assert!(
        matches!(result, Err(ref e) if e.class() == ErrorClass::Eval && e.message().contains("non-enumerable quantifier bound")),
        "{result:?}"
    );
}

#[test]
fn preflight_preserves_lazy_domain_comparison_errors() {
    for domain in [
        r#"{f \in [{1, "a"} -> {1, TRUE}] : TRUE}"#,
        r#"{f \in [a : {1,TRUE}, b : {1,"a"}] : TRUE}"#,
        r#"{f \in SUBSET {1,"a"} : TRUE}"#,
        "[{1, \"a\"} -> {1, TRUE}]",
        "[a : {1,TRUE}, b : {1,\"a\"}]",
        "SUBSET {1,\"a\"}",
    ] {
        let (ev, def) = evaluator(domain, Options::tlc());
        let expr = ev.ir().def(def).body;
        let ordinary = ev
            .scope_bindings(&ev.scope(def), expr, &St::constant())
            .expect_err("ordinary comparison error");
        let bounded = ev
            .scope_bindings_bounded(&ev.scope(def), expr, &St::constant(), 4096)
            .expect_err("bounded comparison error");
        assert_eq!(bounded.class(), ordinary.class(), "{domain}");
        assert_eq!(bounded.message(), ordinary.message(), "{domain}");
    }
}

#[test]
fn deferred_filter_preflight_bounds_input_before_predicate_and_cache() {
    for opts in [Options::tlc(), Options::dive()] {
        for domain in [
            r"{f \in [1..32 -> BOOLEAN] : FALSE}",
            r"{f \in [{1} -> BOOLEAN] : (1 \div 0) = 0}",
        ] {
            let (ev, def) = evaluator(domain, opts);
            let expr = ev.ir().def(def).body;
            let failure = ev
                .scope_bindings_bounded(&ev.scope(def), expr, &St::constant(), 1)
                .expect_err("input bounded before predicate");
            assert_eq!(failure.class(), ErrorClass::ResourceLimit, "{failure:?}");
        }
        let (ev, def) = evaluator(r"{f \in [{1} -> BOOLEAN] : (1 \div 0) = 0}", opts);
        let expr = ev.ir().def(def).body;
        let bounded = ev
            .scope_bindings_bounded(&ev.scope(def), expr, &St::constant(), 4096)
            .expect_err("predicate evaluation error");
        let ordinary = ev
            .scope_bindings(&ev.scope(def), expr, &St::constant())
            .expect_err("ordinary predicate error");
        assert_eq!(bounded.class(), ordinary.class());
        assert_eq!(bounded.message(), ordinary.message());
    }
}

#[test]
fn deferred_filter_rejects_effects_before_predicate_invocation() {
    let source = r"---- MODULE Bounded ----
EXTENDS Naturals, TLC
Q == \A v \in {f \in [{1} -> BOOLEAN] : PrintT(f)} : v
====
";
    for opts in [Options::tlc(), Options::dive()] {
        let ev = source_evaluator(source, opts);
        let def = ev.ir().root_def("Q").expect("Q");
        let failure = ev
            .scope_bindings_bounded(&ev.scope(def), ev.ir().def(def).body, &St::constant(), 4096)
            .expect_err("refused effectful predicate");
        assert_eq!(failure.class(), ErrorClass::Unsupported);
        assert!(
            ev.take_output().is_empty(),
            "PrintT must never run in preflight"
        );
    }
}

fn source_evaluator(source: &str, opts: Options) -> Evaluator {
    let a = dive_sema::analyze_source("Bounded.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    let loaded = harness::Loaded {
        ir: Arc::new(dive_ir::lower(&a).expect("IR")),
        root_text: source.to_owned(),
    };
    Evaluator::new(harness::bind(&loaded, None, opts).expect("bind"), opts)
}

#[test]
fn deferred_filter_retains_caller_state_and_primed_domains_rebind() {
    let source = r"---- MODULE Bounded ----
EXTENDS Naturals
VARIABLES x, d
Domain == {f \in [{1} -> BOOLEAN] : f[1] = x}
Q == \A f \in Domain : f[1]
Qc == \A f \in d : f[1]
Qp == (\A f \in Domain : f[1])'
====
";
    for opts in [Options::tlc(), Options::dive()] {
        let ev = source_evaluator(source, opts);
        let state = |x: bool, d: Value| {
            ev.ir()
                .vars()
                .iter()
                .map(|decl| {
                    Some(match ev.ir().decl(*decl).name.as_str() {
                        "x" => Value::from(x),
                        "d" => d.clone(),
                        other => panic!("{other}"),
                    })
                })
                .collect::<Arc<[Option<Value>]>>()
        };
        let first = state(false, Value::from(false));
        let domain = ev
            .eval_def(
                ev.ir().root_def("Domain").expect("Domain"),
                &St::state(first.clone()),
            )
            .expect("captured domain");
        let after = state(true, domain.clone());
        let before = state(false, domain);
        let q = ev.ir().root_def("Q").expect("Q");
        let qc = ev.ir().root_def("Qc").expect("Qc");
        let qp = ev.ir().root_def("Qp").expect("Qp");
        for (def, st, expected) in [
            (q, St::state(before.clone()), false),
            (q, St::state(after.clone()), true),
            // A previously captured value keeps its creation state, not the caller's new state.
            (qc, St::state(after.clone()), false),
            (qp, St::action(before.clone(), after.clone()), true),
            (qp, St::action(after, before.clone()), false),
        ] {
            ev.check_finite_bindings(&ev.scope(def), ev.ir().def(def).body, &st, 4096)
                .expect("guard");
            assert_eq!(ev.eval_def(def, &st).expect("truth"), Value::from(expected));
        }
        // Materialization of the captured domain does not let subsequent small budgets bypass its input bound.
        let expr = ev.ir().def(qc).body;
        let scope = ev.scope(qc);
        ev.scope_bindings_bounded(&scope, expr, &St::state(before.clone()), 4096)
            .expect("cache population");
        let failure = ev
            .scope_bindings_bounded(&scope, expr, &St::state(before), 1)
            .expect_err("cached input still bounded");
        assert_eq!(failure.class(), ErrorClass::ResourceLimit);
    }
}

#[test]
fn deferred_filter_rejects_unresolved_operator_predicates() {
    let source = r"---- MODULE Bounded ----
EXTENDS Naturals
CONSTANT P(_)
Q == \A v \in {f \in [{1} -> BOOLEAN] : P(f)} : v
====
";
    for opts in [Options::tlc(), Options::dive()] {
        let ev = source_evaluator(source, opts);
        let def = ev.ir().root_def("Q").expect("Q");
        let failure = ev
            .scope_bindings_bounded(&ev.scope(def), ev.ir().def(def).body, &St::constant(), 4096)
            .expect_err("refused unresolved effects");
        assert_eq!(failure.class(), ErrorClass::Unsupported);
        assert!(failure.message().contains("unresolved"), "{failure:?}");
    }
}

#[test]
fn deferred_filter_respects_bound_model_helper_redirect_effects() {
    let source = r"---- MODULE Bounded ----
EXTENDS Naturals, TLC
P(f) == TRUE
Pure(f) == f[1] = FALSE
Effect(f) == PrintT(f)
Q == \A v \in {f \in [{1} -> BOOLEAN] : P(f)} : v
====
";
    let a = dive_sema::analyze_source("Bounded.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    for opts in [Options::tlc(), Options::dive()] {
        for (target, permitted) in [("Effect", false), ("Pure", true)] {
            let mut model = dive_eval::Model::new(ir.clone());
            model.redirects.insert(
                ir.root_def("P").expect("P"),
                ir.root_def(target).expect("target"),
            );
            let ev = Evaluator::new(model, opts);
            let def = ir.root_def("Q").expect("Q");
            let result =
                ev.scope_bindings_bounded(&ev.scope(def), ir.def(def).body, &St::constant(), 4096);
            if permitted {
                assert_eq!(result.expect("pure override").len(), 1);
            } else {
                assert_eq!(
                    result.expect_err("refused bound effects").class(),
                    ErrorClass::Unsupported
                );
                assert!(ev.take_output().is_empty(), "bound PrintT must not run");
            }
        }
    }
}
