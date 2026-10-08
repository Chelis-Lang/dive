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
fn default_profile_chooses_canonically_but_explores_in_tlc_order() {
    // Open question 9 is pending: exploration keeps TLC's order by default,
    // while CHOOSE follows the owner-approved canonical order.
    let (out, _) = run(&module(ORDER), Options::dive());
    assert_eq!(out, ["\"b\"", "\"a\"", "\"a\""]);
}

#[test]
fn exploration_can_follow_the_value_order() {
    // Option (a) of open question 9.
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
