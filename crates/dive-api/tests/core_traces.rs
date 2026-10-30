//! Counterexamples of the new core (`--engine=core`): rebuilt from the
//! predecessor log by replay (`docs/developer/core/07-engine.md` §4), they
//! equal the reference engine's at one worker (both are minimal-depth BFS
//! traces over the same successor order). `dive-trace`, the independent
//! validator, checks their words using the reference evaluator
//! (`08-correctness.md` §7); known unconfirmed endings are counted separately.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::trace::{Ending, Failure, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.tla` with a same-named `.cfg` under `dir`, recursively.
fn specs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("entry").path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            specs(&p, out);
        } else if p.extension().is_some_and(|x| x == "tla") && p.with_extension("cfg").exists() {
            out.push(p);
        }
    }
}

fn run(spec: &Path, engine: Engine) -> Value {
    check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
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

/// The validator's known refutations of correct error endings (the
/// reference engine's counterexamples are refuted the same way; see
/// `counterexample_validation.rs`): #490, an evaluation error re-raised by a
/// check of the last state, and #499, a last state outside `CONSTRAINT`.
fn known(ending: Option<&Ending>, f: &Failure, last: usize) -> bool {
    let check = f.check.name();
    let raises = matches!(ending, Some(Ending::Error { .. } | Ending::Assertion));
    f.state == last
        && ((raises
            && [
                "invariant",
                "constraint",
                "action-constraint",
                "implied-action",
            ]
            .contains(&check)
            && !f.detail.contains("may lie outside the model"))
            || (raises
                && matches!(check, "constraint" | "action-constraint")
                && f.detail.contains(
                    "only a checked terminal invariant or property may lie outside the model",
                )))
}

#[derive(Debug, PartialEq, Eq)]
enum Checked {
    Valid,
    Unconfirmed,
}

fn validate(spec: &Path, trace: &Value) -> Result<Checked, String> {
    let doc = TraceDoc::from_json(trace).map_err(|e| format!("unreadable trace: {e}"))?;
    let options = TraceOptions {
        run: Some((0, 0)),
        ..TraceOptions::default()
    };
    let model = SpecModel::load(spec, &spec.with_extension("cfg"), &options)
        .map_err(|e| format!("validator cannot bind: {e}"))?;
    let report = model
        .try_validate(&doc)
        .map_err(|e| format!("validator did not complete: {e}"))?;
    match report.validation {
        Validation::Valid => Ok(Checked::Valid),
        // The validator cannot confirm a deadlock on its own; the states and
        // steps it checks before saying so are still checked.
        Validation::Unsupported(why) if why.contains("deadlock") => Ok(Checked::Unconfirmed),
        // Nor re-raise an error while expanding the last state, which needs
        // the successor generator; it checks the steps before it says so.
        Validation::Unsupported(why) if why.contains("the steps were checked") => {
            Ok(Checked::Unconfirmed)
        }
        Validation::Unsupported(why)
            if matches!(doc.ending, Some(Ending::NextStateError { .. }))
                && [
                    "Next error replay emitted a candidate before the claimed error",
                    "Next error replay needs successor-side ordering before its error",
                ]
                .contains(&why.as_str()) =>
        {
            // This bounded endpoint cannot confirm errors after any candidate.
            // Check the finite prefix independently, and retain the unknown
            // ending rather than counting it as an independently valid word.
            let prefix = model
                .try_validate(&doc.with_ending(Ending::Behavior))
                .map_err(|e| format!("prefix validation did not complete: {e}"))?;
            if prefix.validation != Validation::Valid {
                return Err(format!("unconfirmed Next prefix: {:?}", prefix.validation));
            }
            Ok(Checked::Unconfirmed)
        }
        Validation::Unsupported(why) => Err(format!("unsupported: {why}")),
        Validation::Invalid(f)
            if known(doc.ending.as_ref(), &f, doc.states.len().saturating_sub(1)) =>
        {
            Ok(Checked::Unconfirmed)
        }
        Validation::Invalid(f) => Err(format!("invalid at state {}: {}", f.state + 1, f.detail)),
    }
}

#[test]
fn core_counterexamples_equal_reference_with_explicit_unconfirmed_endings() {
    let mut all = Vec::new();
    for dir in [
        "crates/dive-eval/tests/fixtures/next/specs",
        "crates/dive-check/tests/fixtures/tlc-probes/specs",
        "crates/dive-check/tests/fixtures/errors/specs",
        "crates/dive-api/tests/fixtures/core-m0",
        "crates/dive-api/tests/fixtures/core-m1",
    ] {
        specs(&repo().join(dir), &mut all);
    }
    let mut traced = 0;
    let mut validated = 0;
    let mut ordered_next = 0;
    for spec in &all {
        let name = spec.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
        let core = run(spec, Engine::Core);
        if core["verdict"] == "unsupported" {
            continue;
        }
        let reference = run(spec, Engine::Reference);
        assert_eq!(
            core["counterexample"]["trace"], reference["counterexample"]["trace"],
            "{name}: core and reference traces differ"
        );
        assert_eq!(
            core["counterexample"]["length"], reference["counterexample"]["length"],
            "{name}"
        );
        let trace = &core["counterexample"]["trace"];
        if trace.is_null() {
            // No trace document: no counterexample, or one with a state
            // only TLC's text describes (an incomplete state), exactly as
            // the reference engine reports it (compared above).
            continue;
        }
        let checked = validate(spec, trace).unwrap_or_else(|e| panic!("{name}: {e}"));
        if ["EvalErrorMidLevel", "ErNextChoose", "ErNextTypeErr"].contains(&name) {
            // Actual ordered candidate and diagnostic replay now supplies the
            // proof these formerly unconfirmed generator endings lacked.
            assert_eq!(checked, Checked::Valid, "{name}");
            ordered_next += 1;
        }
        validated += usize::from(checked == Checked::Valid);
        traced += 1;
    }
    assert!(
        traced >= 60,
        "only {traced} core counterexamples were checked"
    );
    assert!(
        // Measured ordered/partial inventory: 110 valid of 153 documents.
        // Unknown endings remain outside this independently validated floor.
        validated >= 110,
        "only {validated} core counterexamples independently validated ({traced} checked)"
    );
    assert_eq!(ordered_next, 3);
    eprintln!(
        "core counterexamples: {validated} valid, {} unconfirmed, {traced} checked",
        traced - validated
    );
}
