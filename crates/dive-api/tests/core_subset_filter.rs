//! The Core enumerates `{E \in SUBSET T : \A e \in E : Q}` as
//! `SUBSET {e \in T : Q}` only when the compiler proves `Q` pure and
//! independent of `E` (#853), with the reference engine's results.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

fn run(name: &str, engine: Engine) -> Value {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-subset-filter")
        .join(format!("{name}.tla"));
    check_file_engine(
        &CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        engine,
    )
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
    let core = run("Pairs5", Engine::Core);
    assert_eq!(core["verdict"], "success", "{core:#}");
    assert_eq!(core["statistics"]["distinct"], 1024, "{core:#}");
    assert_eq!(core["statistics"]["generated"], 2048, "{core:#}");
}

/// A body that reads the filtered subset itself is not rewritten, so the
/// 2^32 enumeration still refuses by name.
#[test]
fn a_body_reading_the_subset_keeps_the_named_refusal() {
    let core = run("ReadsSet5", Engine::Core);
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
    let core = run("Small", Engine::Core);
    assert_eq!(core["verdict"], "success", "{core:#}");
    assert_eq!(outcome(&core), outcome(&run("Small", Engine::Reference)));
}

/// A body that fails on some element falls back to the filter itself, so
/// the error, its site and the counts at the stop are the reference
/// engine's.
#[test]
fn an_evaluation_error_in_the_rewritten_body_is_raised_as_by_the_filter() {
    let core = run("DivZero", Engine::Core);
    assert_eq!(core["verdict"], "evaluation-error", "{core:#}");
    let reference = run("DivZero", Engine::Reference);
    assert_eq!(outcome(&core), outcome(&reference));
    assert_eq!(core["tlc"], reference["tlc"]);
}
