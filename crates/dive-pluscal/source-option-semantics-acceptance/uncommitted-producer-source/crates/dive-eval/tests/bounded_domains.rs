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
