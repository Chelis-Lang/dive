//! The Core enumerates `{E \in SUBSET T : \A e \in E : Q}` as
//! `SUBSET {e \in T : Q}` only when the compiler proves `Q` pure and
//! independent of `E` (#853), with the agreed results (`support/agreed.rs`).
#![allow(clippy::expect_used, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_subset_filter.json");

use std::path::PathBuf;

use dive_api::{CheckRequest, check_file};
use serde_json::Value;

fn run(name: &str) -> Value {
    run_config(name, name)
}

fn run_config(name: &str, config: &str) -> Value {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-subset-filter")
        .join(format!("{name}.tla"));
    check_file(&CheckRequest {
        config: Some(file.with_file_name(format!("{config}.cfg"))),
        file,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    })
    .expect("check runs")
    .to_json()
}

fn outcome(v: &Value) -> (Value, Value, Value, Value, Value) {
    (
        v["verdict"].clone(),
        v["statistics"]["generated"].clone(),
        v["statistics"]["distinct"].clone(),
        v["statistics"]["depth"].clone(),
        v["message"].clone(),
    )
}

/// `SUBSET (SUBSET (1..5))` has 2^32 elements, past what the Core
/// enumerates; the rewrite enumerates the 2^10 sets of pairs. Membership in
/// the same filter (the invariant) keeps the filter's own evaluation.
#[test]
fn pure_forall_filter_over_a_powerset_enumerates_the_subsets_of_its_filter() {
    let core = run("Pairs5");
    assert_eq!(core["verdict"], "success", "{core:#}");
    assert_eq!(core["statistics"]["distinct"], 1024, "{core:#}");
    assert_eq!(core["statistics"]["generated"], 2048, "{core:#}");
}

/// A body that reads the filtered subset itself is not rewritten, so the
/// 2^32 enumeration still refuses by name.
#[test]
fn a_body_reading_the_subset_keeps_the_named_refusal() {
    let core = run("ReadsSet5");
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
    assert!(
        core["message"]
            .as_str()
            .is_some_and(|m| m.contains("SUBSET of a set of more than 24 elements")),
        "{core:#}"
    );
}

/// Rewritten filters (a constant bound, a state variable in an action)
/// next to ones the side conditions exclude (a primed read in `Init`, a
/// read of the subset, a quantifier over another set): the reference
/// engine's counts.
#[test]
fn rewritten_and_excluded_filters_agree_with_the_reference_engine() {
    let core = run("Small");
    assert_eq!(core["verdict"], "success", "{core:#}");
    agreed::check_eq(
        AGREED,
        "rewritten_and_excluded_filters_agree_with_the_reference_engine/0",
        &(outcome(&core)),
        &(outcome(&run("Small"))),
    );
}

/// A body that fails on some element falls back to the filter itself, so
/// the error, its site and the counts at the stop are the reference
/// engine's.
#[test]
fn an_evaluation_error_in_the_rewritten_body_is_raised_as_by_the_filter() {
    let core = run("DivZero");
    assert_eq!(core["verdict"], "evaluation-error", "{core:#}");
    let reference = run("DivZero");
    agreed::check_eq(
        AGREED,
        "an_evaluation_error_in_the_rewritten_body_is_raised_as_by_the_filter/0",
        &(outcome(&core)),
        &(outcome(&reference)),
    );
    agreed::check_eq(
        AGREED,
        "an_evaluation_error_in_the_rewritten_body_is_raised_as_by_the_filter/1",
        &(core["tlc"]),
        &(reference["tlc"]),
    );
}

/// Two of forty messages are eligible. Each of the four admissible subsets
/// is emitted by two branches of the original action.
#[test]
fn an_action_keeps_each_witness_over_a_wide_guarded_powerset() {
    let core = run("GuardedAction");
    assert_eq!(core["verdict"], "success", "{core:#}");
    assert_eq!(core["statistics"]["distinct"], 5);
    assert_eq!(core["statistics"]["generated"], 9);
    assert_eq!(core["statistics"]["depth"], 2);
}

/// Predicates that use the chosen set itself cannot filter individual elements.
#[test]
fn an_action_reading_the_selected_set_preserves_the_wide_domain_refusal() {
    let core = run_config("GuardedAction", "ReadsSelected");
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
    assert!(
        core["message"]
            .as_str()
            .is_some_and(|m| m.contains("SUBSET of a set of more than 24 elements")),
        "{core:#}"
    );
}

/// The original search may visit the empty set before any field selection.
/// Empty membership must still yield its counterexample on a partially defined guard.
#[test]
fn a_partial_guard_preserves_the_empty_subset_counterexample() {
    let core = run_config("GuardedAction", "PartialGuard");
    assert_eq!(core["verdict"], "invariant-violation", "{core:#}");
    assert_eq!(core["statistics"]["depth"], 2);
}
