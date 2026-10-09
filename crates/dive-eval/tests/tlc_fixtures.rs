//! Differential tests against transcripts of the pinned TLC
//! (`tlc-2026.10.06.014338`).
//!
//! Each spec under `tests/fixtures/specs/` evaluates constant expressions in
//! `ASSUME`s, printing values with `Print`/`PrintT` or ending in one error.
//! Its TLC transcript, captured with `dive-conformance dev capture`, is under
//! `tests/fixtures/tlc/`. The test runs every spec through dive under
//! `--compat=tlc` and requires the printed output and the assumption result
//! (TRUE, `TLC_ASSUMPTION_FALSE` or `TLC_ASSUMPTION_EVALUATION_ERROR` with
//! TLC's nested error class and message) to match TLC's byte for byte. A spec
//! dive refuses (`unsupported`) must be listed in `REFUSED` with the reason.
//! No Java is needed.
//!
//! Set `DIVE_EVAL_FIXTURES=<dir>` (containing `specs/` and `tlc/<oracle>/`)
//! to run another fixture set, and `DIVE_EVAL_REPORT=1` to print every
//! disagreement.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

mod harness;

use std::fs;
use std::path::{Path, PathBuf};

use dive_eval::{AssumptionOutcome, ErrorClass, Options};

const ORACLE: &str = "tlc-2026.10.06.014338";

/// Specs dive refuses, with the feature named in the refusal.
const REFUSED: &[(&str, &str)] = &[("PrintFp", "TLCExt!TLCFP")];

/// Known disagreements, each with its cause. They are reported, never counted
/// as agreeing.
///
/// There are none. The three there were (`ErGeqNaturals`,
/// `ErPlusNonIntNaturals`, `RvNatDiv`) came from the old, perturbed capture
/// setup: `dev capture` passed `-dumpTrace json`, which makes TLC load
/// `_JsonTrace` and with it `Integers`, whose Java overrides then replaced
/// `Naturals`' in specifications that extend only `Naturals` (issue #35).
/// Recaptured without it, the first two agree with dive as they were, and
/// `RvNatDiv` agrees once `\div` follows `Naturals.Divide`
/// (`Int::div_naturals`), which the old transcript had hidden.
const KNOWN: &[(&str, &str)] = &[];

fn fixtures() -> PathBuf {
    std::env::var_os("DIVE_EVAL_FIXTURES").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        PathBuf::from,
    )
}

#[test]
fn assumptions_match_tlc() {
    run_set(&fixtures(), |_| true);
}

/// `dive-value`'s TLC fixtures (value printing, comparison and arithmetic
/// errors), evaluated end to end through the front end and the evaluator.
/// Specs with state variables are not constant-only and are skipped.
#[test]
fn value_fixtures_match_tlc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-value/tests/fixtures");
    run_set(&root, |text| !text.contains("VARIABLE"));
}

/// End-to-end finite facts without enumerating an infinite function operand.
#[test]
fn symbolic_function_emptiness_matches_clean_tlc_assumptions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-value/tests/fixtures/printing");
    run_set(&root, |text| !text.contains("MODULE NonenumerableFunction"));
}

fn run_set(root: &Path, keep: impl Fn(&str) -> bool) {
    let specs = root.join("specs");
    let mut names: Vec<String> = fs::read_dir(&specs)
        .expect("fixtures/specs")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            fs::read_to_string(specs.join(n).join(format!("{n}.tla"))).is_ok_and(|t| keep(&t))
        })
        .collect();
    names.sort();
    let report = std::env::var_os("DIVE_EVAL_REPORT").is_some();
    let (mut agree, mut refused, mut known, mut failures) = (0usize, 0usize, 0usize, Vec::new());
    for name in &names {
        let stdout = fs::read_to_string(root.join(format!("tlc/{ORACLE}/{name}.stdout")))
            .unwrap_or_else(|e| panic!("{name}.stdout: {e}"));
        let expected = harness::tlc_assumption_part(&stdout);
        let (_, printed, outcome) = match harness::run_spec(&specs, name, Options::tlc()) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{name}: harness: {e}"));
                continue;
            }
        };
        if let AssumptionOutcome::Error { error, .. } = &outcome
            && error.class() == ErrorClass::ResourceLimit
            && stdout.contains("Java StackOverflowError")
        {
            // TLC overflowed its stack: a resource limit on both sides, never an
            // oracle fact (ADR 0003).
            refused += 1;
            continue;
        }
        if let AssumptionOutcome::Error { error, .. } = &outcome
            && error.class() != ErrorClass::Eval
        {
            match REFUSED.iter().find(|(n, _)| n == name) {
                Some((_, why)) if error.message().contains(why) => refused += 1,
                _ => failures.push(format!("{name}: {} ({})", error, error.class() as u8)),
            }
            continue;
        }
        let got = harness::dive_assumption_part(&printed, &outcome);
        if got == expected {
            assert!(
                !KNOWN.iter().any(|(n, _)| n == name),
                "{name} agrees now: remove it from KNOWN"
            );
            agree += 1;
        } else if KNOWN.iter().any(|(n, _)| n == name) {
            known += 1;
        } else {
            failures.push(format!("{name}:\n--- TLC\n{expected}\n--- dive\n{got}\n"));
        }
    }
    eprintln!(
        "TLC fixtures: {} specs, {agree} agree, {refused} refused, {known} known disagreements, {} disagree",
        names.len(),
        failures.len()
    );
    if report || !failures.is_empty() {
        for f in &failures {
            eprintln!("{f}");
        }
    }
    assert!(
        failures.is_empty(),
        "{} fixtures disagree with TLC",
        failures.len()
    );
}
