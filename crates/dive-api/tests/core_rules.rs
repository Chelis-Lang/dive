//! The new core's successor generator against the rule fixtures of
//! `docs/developer/core/05-successor-generation.md` §7 (the `Nx*` specs of
//! `crates/dive-eval/tests/fixtures/next/`) and the TLC probes of
//! `crates/dive-check/tests/fixtures/tlc-probes/`.
//!
//! Each fixture runs under `--engine=core` and the reference engine in the
//! default profile at one worker. Where the core runs it, its verdict,
//! violated property, generated and distinct counts, depth, queue and exit
//! must equal the reference engine's, and, for a run TLC completes, the
//! counts must equal TLC's transcript. Counts at a stop depend on the
//! value order (D-010), so for those the reference engine is the oracle;
//! the TLC transcript is checked where the two profiles agree.
//!
//! Fixtures the core does not run yet must be refused, naming the
//! construct, never answered wrongly. `NxInLazyCup` must stay refused in
//! the default profile (§6).
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

const ORACLE: &str = "tlc-2026.10.06.014338";

/// Fixtures the core refuses, with a word its refusal must contain.
const REFUSED: &[(&str, &str)] = &[
    // §6: TLC's multiplicity depends on the set object's history.
    ("NxInLazyCup", "lazily represented set"),
    // Stream 2 (plan compiler): ENABLED.
    ("NxEnabled", "ENABLED"),
    // Implied action properties (not yet in the core).
    ("ImpliedActionSeen", "temporal"),
    // A set of an integer and a function: TLC's error depends on when it
    // normalizes the set (C1, C2).
    ("ErBagUnionNonBag", "depends on when it normalizes"),
];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn specs(dir: &str) -> Vec<(String, PathBuf, PathBuf)> {
    let root = repo().join(dir);
    let mut out: Vec<_> = std::fs::read_dir(root.join("specs"))
        .expect("fixture directory")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.is_dir())
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .expect("name")
                .to_owned();
            let transcript = root.join(ORACLE).join(format!("{name}.stdout"));
            (name.clone(), p.join(format!("{name}.tla")), transcript)
        })
        .collect();
    out.sort();
    out
}

fn run(spec: &Path, engine: Engine) -> Value {
    let report = check_file_engine(
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
    .expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    j
}

fn observed(v: &Value) -> Value {
    serde_json::json!({
        "verdict": v["verdict"],
        "property": v["property"],
        "generated": v["statistics"]["generated"],
        "distinct": v["statistics"]["distinct"],
        "depth": v["statistics"]["depth"],
        "queue": v["statistics"]["left_on_queue"],
        "exit": v["exit"],
        "output": v["output"],
        "notes": v["notes"],
        "trace": v["counterexample"]["length"],
    })
}

/// TLC's final counts: generated, distinct, left on queue.
/// Only for a run TLC completes too: where the default profile does better
/// (for example, no overflow, D-008), TLC stops early.
fn tlc_counts(transcript: &Path) -> Option<(u64, u64, u64)> {
    let text = std::fs::read_to_string(transcript).ok()?;
    if !text.contains("No error has been found") {
        return None;
    }
    text.lines()
        .filter_map(|l| {
            let (g, rest) = l.split_once(" states generated, ")?;
            let (d, rest) = rest.split_once(" distinct states found, ")?;
            let q = rest.strip_suffix(" states left on queue.")?;
            let n = |s: &str| s.replace(',', "").parse::<u64>().ok();
            Some((n(g)?, n(d)?, n(q)?))
        })
        .next_back()
}

fn check_dir(dir: &str) -> usize {
    let mut agreed = 0;
    for (name, spec, transcript) in specs(dir) {
        let core = run(&spec, Engine::Core);
        if core["verdict"] == "rejected" {
            // Rejected before any engine runs.
            assert_eq!(
                run(&spec, Engine::Reference)["verdict"],
                "rejected",
                "{name}"
            );
            continue;
        }
        assert_eq!(core["engine"], "core", "{name}");
        if let Some((_, word)) = REFUSED.iter().find(|(n, _)| *n == name) {
            assert_eq!(core["verdict"], "unsupported", "{name}: {core:#}");
            assert!(
                core["message"].as_str().is_some_and(|m| m.contains(word)),
                "{name}: {core:#}"
            );
            continue;
        }
        let reference = run(&spec, Engine::Reference);
        if core["verdict"] == "unsupported" && observed(&core) != observed(&reference) {
            // Only a construct the core names as not implemented yet, or
            // an error in an expression TLC keeps lazy, which the core
            // refuses rather than reports (`04-plan-compiler.md` §4.1).
            let m = core["message"].as_str().unwrap_or("");
            assert!(
                m.contains("(M0 core")
                    || m.contains("(M1 core")
                    || m.starts_with("an evaluation error in an expression TLC evaluates lazily"),
                "{name}: {core:#}"
            );
            continue;
        }
        assert_eq!(observed(&core), observed(&reference), "{name}");
        if core["verdict"] == "success"
            && let Some((g, d, q)) = tlc_counts(&transcript)
        {
            assert_eq!(
                (
                    core["statistics"]["generated"].as_u64(),
                    core["statistics"]["distinct"].as_u64(),
                    core["statistics"]["left_on_queue"].as_u64()
                ),
                (Some(g), Some(d), Some(q)),
                "{name} against TLC"
            );
        }
        agreed += 1;
    }
    agreed
}

#[test]
fn rule_fixtures_match_the_reference_and_tlc() {
    let agreed = check_dir("crates/dive-eval/tests/fixtures/next");
    // 18 fixtures: 14 agree, NxInLazyCup is refused by rule, and
    // NxEnabled and NxInstance wait for stream 2.
    assert!(agreed >= 14, "only {agreed} rule fixtures agreed");
}

#[test]
fn tlc_probes_match_the_reference_and_tlc() {
    let agreed = check_dir("crates/dive-check/tests/fixtures/tlc-probes");
    assert!(agreed >= 18, "only {agreed} probes agreed");
}

/// The error-class fixtures (`crates/dive-check/tests/fixtures/errors/`):
/// the core's error endings (class, exit, counts after TLC's call-stack
/// replay, trace length) equal the reference engine's.
#[test]
fn error_fixtures_match_the_reference() {
    let agreed = check_dir("crates/dive-check/tests/fixtures/errors");
    // 175 fixtures: 85 agree, `ErFpInfinite` among them; the rest use VIEW,
    // ALIAS, POSTCONDITION, properties, Print, Assert, ENABLED, ASSUME
    // errors or recursive functions, which the core does not run yet.
    assert!(agreed >= 85, "only {agreed} error fixtures agreed");
}

/// The standard-library fixtures (`tests/fixtures/core-lib`): one model
/// per operator family of `crates/dive-core/src/stdlib/`, and one per error
/// case, with pinned-TLC transcripts.
#[test]
fn library_fixtures_match_the_reference_and_tlc() {
    let agreed = check_dir("crates/dive-api/tests/fixtures/core-lib");
    // Bags: BagOps and four error endings (ErBagUnionNonBag is refused);
    // TLC: TlcOps, Perm, PermViol and five error endings.
    assert!(agreed >= 13, "only {agreed} library fixtures agreed");
}

/// The M1 generator fixtures (`tests/fixtures/core-m1`): each agrees with
/// the reference engine, or is one of the lazy-union cases both refuse.
/// The `Fp*` fixtures are states holding a set TLC cannot fingerprint.
#[test]
fn m1_generator_fixtures_match_the_reference() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-m1");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("fixtures")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "tla"))
        .collect();
    names.sort();
    assert!(names.len() >= 13);
    for spec in names {
        let name = spec
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_owned();
        let core = run(&spec, Engine::Core);
        let reference = run(&spec, Engine::Reference);
        if name.ends_with("Dup") {
            assert_eq!(core["verdict"], "unsupported", "{name}: {core:#}");
            assert_eq!(reference["verdict"], "unsupported", "{name}");
            assert!(
                core["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("lazily represented set")),
                "{name}: {core:#}"
            );
            continue;
        }
        assert_eq!(observed(&core), observed(&reference), "{name}");
    }
}
