//! The profile-dependent options: exploration order (open question 9) and
//! comparison semantics (open question 12, D-009), in both settings.

#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

mod harness;

use std::sync::Arc;

use dive_eval::{AssumptionOutcome, Comparisons, ErrorClass, Evaluator, ExplorationOrder, Options};
use dive_value::{Ctx, IntMode, ValueOrder};

/// Evaluates a module's assumptions under `opts`; returns the printed
/// output and the outcome.
fn run(src: &str, opts: Options) -> (Vec<String>, AssumptionOutcome) {
    let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "front end rejects:\n{src}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let loaded = harness::Loaded {
        ir,
        root_text: src.to_owned(),
    };
    let model = harness::bind(&loaded, None, opts).expect("binds");
    let ev = Evaluator::new(model, opts);
    let outcome = ev.check_assumptions(|_| false);
    (ev.take_output(), outcome)
}

fn module(body: &str) -> String {
    format!("---- MODULE M ----\nEXTENDS Integers, FiniteSets, Sequences, TLC\n{body}\n====\n")
}

/// `"b"` is interned before `"a"`, so TLC's order puts it first.
const ORDER: &str = r#"Names == {"b", "a"}
ASSUME \A x \in Names : PrintT(x)
ASSUME PrintT(CHOOSE x \in Names : TRUE)"#;

#[test]
fn tlc_profile_explores_and_chooses_in_tlc_order() {
    let (out, outcome) = run(&module(ORDER), Options::tlc());
    assert!(matches!(outcome, AssumptionOutcome::AllTrue));
    assert_eq!(out, ["\"b\"", "\"a\"", "\"b\""]);
}

#[test]
fn default_profile_chooses_and_explores_canonically() {
    // Open question 9, option (a) (owner decision 2026-10-08): the default
    // profile explores in the canonical order as well as choosing in it.
    let (out, _) = run(&module(ORDER), Options::dive());
    assert_eq!(out, ["\"a\"", "\"b\"", "\"a\""]);
}

#[test]
fn exploration_can_keep_tlc_order() {
    // `--explore-order=tlc`, part of `--compat=tlc`.
    let opts = Options {
        exploration: ExplorationOrder::Tlc,
        ..Options::dive()
    };
    let (out, _) = run(&module(ORDER), opts);
    assert_eq!(out, ["\"b\"", "\"a\"", "\"a\""]);
}

#[test]
fn exploration_follows_the_value_order() {
    // The default profile's setting, stated explicitly.
    let opts = Options {
        exploration: ExplorationOrder::Value,
        ..Options::dive()
    };
    let (out, _) = run(&module(ORDER), opts);
    assert_eq!(out, ["\"a\"", "\"b\"", "\"a\""]);
}

fn tla_comparisons() -> Options {
    Options {
        comparisons: Comparisons::Tla,
        ..Options::tlc()
    }
}

#[test]
fn tlc_comparisons_refuse_what_tla_answers() {
    for expr in [
        "[a |-> 1, b |-> 1] = <<1>>",
        "<<1>> \\in [a : {1}]",
        "[a |-> 1] \\in {1} \\X {2}",
        "\"a\" \\in {1, 2}",
    ] {
        let src = module(&format!("ASSUME PrintT({expr})"));
        let (_, outcome) = run(&src, Options::tlc());
        assert!(
            matches!(&outcome, AssumptionOutcome::Error { error, .. } if error.class() == ErrorClass::Eval),
            "{expr}: {outcome:?}"
        );
    }
}

#[test]
fn tla_comparisons_answer_functions_and_sets() {
    for (expr, want) in [
        ("[a |-> 1, b |-> 1] = <<1>>", "FALSE"),
        ("<<1>> = [a |-> 1, b |-> 1]", "FALSE"),
        ("<<1>> \\in [a : {1}]", "FALSE"),
        ("[a |-> 1] \\in {1} \\X {2}", "FALSE"),
        ("<<1>> = <<1>>", "TRUE"),
        ("1 \\in {1, \"a\"}", "TRUE"),
    ] {
        let src = module(&format!("ASSUME PrintT({expr})"));
        let (out, outcome) = run(&src, tla_comparisons());
        assert!(
            matches!(outcome, AssumptionOutcome::AllTrue),
            "{expr}: {outcome:?}"
        );
        assert_eq!(out, [want], "{expr}");
    }
}

#[test]
fn tla_comparisons_keep_the_error_where_tla_is_silent() {
    for expr in ["1 = \"a\"", "{1} = 1", "\"a\" \\in {1, 2}"] {
        let src = module(&format!("ASSUME PrintT({expr})"));
        let (_, outcome) = run(&src, tla_comparisons());
        assert!(
            matches!(&outcome, AssumptionOutcome::Error { error, .. } if error.class() == ErrorClass::Eval),
            "{expr}: {outcome:?}"
        );
    }
}

#[test]
fn integer_modes() {
    let src = module("ASSUME PrintT(2147483647 + 1)");
    let (_, tlc) = run(&src, Options::tlc());
    match tlc {
        AssumptionOutcome::Error { error, .. } => {
            assert_eq!(error.message(), "Overflow when computing 2147483647+1");
        }
        other => panic!("{other:?}"),
    }
    let big = Options {
        ctx: Ctx {
            int_mode: IntMode::Big,
            order: ValueOrder::Tlc,
        },
        ..Options::tlc()
    };
    let (out, outcome) = run(&src, big);
    assert!(matches!(outcome, AssumptionOutcome::AllTrue));
    assert_eq!(out, ["2147483648"]);
}

#[test]
fn equality_api_obeys_the_evaluator_comparison_profile() {
    let source = module("A == <<1>>\nB == [a |-> 1, b |-> 1]");
    let analysis = dive_sema::analyze_source("M.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    for opts in [Options::tlc(), Options::dive()] {
        let evaluator = Evaluator::new(dive_eval::Model::new(ir.clone()), opts);
        let a = evaluator
            .eval_def(ir.root_def("A").expect("A"), &dive_eval::St::constant())
            .expect("tuple");
        let b = evaluator
            .eval_def(ir.root_def("B").expect("B"), &dive_eval::St::constant())
            .expect("record");
        assert!(!evaluator.equal_values(&a, &b).expect("tuple/record"));
        let reverse = evaluator.equal_values(&b, &a);
        if opts.comparisons == Comparisons::Tlc {
            assert!(reverse.is_err(), "TLC record/tuple equality must refuse");
        } else {
            assert!(!reverse.expect("TLA equality is symmetric"));
        }
    }
}
