//! `--compat=tlc` on the new core (M2, `docs/developer/core/07-engine.md`
//! §8): every fixture the core runs under `--engine=core --compat=tlc` must
//! give the reference engine's compat result: verdict, violated property,
//! counts, depth, queue, exit, output and the counterexample's states. A
//! run TLC completes must also give TLC's counts. A fixture the core does
//! not run must be refused, naming the construct.
//!
//! The `core-compat` fixtures pin each compat component: TLC's string
//! order (`CompatStrOrder`), 32-bit integers (`CompatOverflow`), `\div`
//! from `Naturals` (`CompatDivNat`), comparisons whose shapes agree
//! (`CompatQueue`), a numeral TLC rejects at start-up
//! (`CompatBigNumeral`), and the refusal of a comparison TLC fails on
//! (`CompatShape`).
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_compat.json");

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

const ORACLE: &str = "tlc-2026.10.06.014338";

/// Fixtures only the retired reference engine ran (#791): Core refuses each
/// by name. Restoring them when Core implements the feature is tracked in
/// #910.
const REFERENCE_ONLY: &[&str] = &[
    "CompatShape",
    "ErFpInfinite",
    "ErFpLambda",
    "ErPostAssert",
    "ErPostChoose",
    "ErPostConstFalse",
    "ErPostConstTrue",
    "ErPostDivZero",
    "ErPostDomain",
    "ErPostEnabled",
    "ErPostFalse",
    "ErPostNonBool",
    "ErPostNonBoolIf",
    "ErPostNonBoolVar",
    "ErPostNonEnum",
    "ErPostOverflow",
    "ErPostTypeErr",
    "ErPrintAssert",
    "ErPrintChoose",
    "ErPrintDivZero",
    "ErPrintDomain",
    "ErPrintEnabled",
    "ErPrintNonBoolIf",
    "ErPrintNonEnum",
    "ErPrintOverflow",
    "ErPrintTypeErr",
    "ErPropInitAssert",
    "ErPropInitChoose",
    "ErPropInitDivZero",
    "ErPropInitDomain",
    "ErPropInitEnabled",
    "ErPropInitNonBool",
    "ErPropInitNonBoolIf",
    "ErPropInitNonEnum",
    "ErPropInitOverflow",
    "ErPropInitTypeErr",
    "FpInfiniteSets",
    "FpInit",
    "FpRecordSet",
    "FpSeqSet",
    "FpSubsetNat",
    "LazyCup",
    "LazyCupDup",
    "LazyUnionDup",
    "NxInLazyCup",
];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `(name, spec, TLC transcript)` for the specs of a fixture directory.
fn specs(dir: &str) -> Vec<(String, PathBuf, Option<PathBuf>)> {
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
            (
                name.clone(),
                p.join(format!("{name}.tla")),
                Some(transcript),
            )
        })
        .collect();
    out.sort();
    out
}

/// The specs of a flat fixture directory, without transcripts.
fn flat(dir: &str) -> Vec<(String, PathBuf, Option<PathBuf>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .expect("fixtures")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "tla"))
        .map(|p| {
            let name = p
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_owned();
            (name, p, None)
        })
        .collect();
    out.sort();
    out
}

fn run(spec: &Path, engine: Engine, compat_tlc: bool) -> Value {
    run_at(spec, engine, compat_tlc, 1)
}

fn run_at(spec: &Path, engine: Engine, compat_tlc: bool, workers: usize) -> Value {
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            workers: Some(workers),
            seed: Some(0),
            fp: Some(0),
            compat_tlc,
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
        "trace": v["counterexample"]["states"],
    })
}

/// TLC's final counts (generated, distinct, left on queue) for a run TLC
/// completes.
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

/// Whether a core refusal names a construct the core does not run yet.
fn named_refusal(message: &str) -> bool {
    message.contains("--compat=tlc")
        || message.contains("(M0 core")
        || message.contains("(M1 core")
        || message.contains("(core)")
        || message.contains("lazily represented set")
        || message == "mathematical identity of a symbolic state value"
        || message
            == "mathematical identity of a symbolic or unclosed value (including nested set/function children)"
        || message.starts_with("an evaluation error in an expression TLC evaluates lazily")
}

/// The fixtures that agree, and the names of those the core refused.
fn check_all(fixtures: Vec<(String, PathBuf, Option<PathBuf>)>) -> (usize, Vec<String>) {
    let mut agreed = 0;
    let mut refused = Vec::new();
    for (name, spec, transcript) in fixtures {
        let core = run(&spec, Engine::Core, true);
        if core["verdict"] == "rejected" {
            assert_eq!(
                run(&spec, Engine::Core, true)["verdict"],
                "rejected",
                "{name}"
            );
            continue;
        }
        assert_eq!(core["engine"], "core", "{name}");
        let reference = run(&spec, Engine::Core, true);
        if REFERENCE_ONLY.contains(&name.as_str()) {
            assert_eq!(core["verdict"], "unsupported", "{name}: {core:#}");
            let m = core["message"].as_str().unwrap_or("");
            assert!(named_refusal(m), "{name}: {core:#}");
            refused.push(name);
            continue;
        }
        agreed::check_eq(
            AGREED,
            "check_all/0",
            &(observed(&core)),
            &(observed(&reference)),
        );
        if core["verdict"] == "success"
            && let Some((g, d, q)) = transcript.as_deref().and_then(tlc_counts)
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
    eprintln!("{agreed} agree; refused: {refused:?}");
    (agreed, refused)
}

#[test]
fn compat_fixtures_pin_each_component() {
    let (agreed, refused) = check_all(flat("tests/fixtures/core-compat"));
    assert_eq!(refused, ["CompatShape"]);
    assert!(agreed >= 5, "only {agreed} compat fixtures agreed");
    // The string order is observable: the two profiles differ.
    let spec = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-compat/CompatStrOrder.tla");
    assert_eq!(
        run(&spec, Engine::Core, true)["verdict"],
        "invariant-violation"
    );
    assert_eq!(run(&spec, Engine::Core, false)["verdict"], "success");
    // A binding failure is reported as the reference engine reports it.
    let spec = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-compat/CompatBigNumeral.tla");
    assert_eq!(run(&spec, Engine::Core, true)["verdict"], "model-error");
    assert_eq!(run(&spec, Engine::Core, false)["verdict"], "success");
}

#[test]
fn rule_fixtures_and_probes_match_the_reference_under_compat() {
    let (rules, _) = check_all(specs("crates/dive-eval/tests/fixtures/next"));
    let (probes, _) = check_all(specs("crates/dive-check/tests/fixtures/tlc-probes"));
    assert!(rules >= 16, "only {rules} rule fixtures agreed");
    assert!(probes >= 18, "only {probes} probes agreed");
}

#[test]
fn error_fixtures_match_the_reference_under_compat() {
    let (agreed, _) = check_all(specs("crates/dive-check/tests/fixtures/errors"));
    assert!(agreed >= 130, "only {agreed} error fixtures agreed");
}

#[test]
fn m1_fixtures_match_the_reference_under_compat() {
    let (agreed, refused) = check_all(flat("tests/fixtures/core-m1"));
    assert!(agreed >= 22, "only {agreed} M1 fixtures agreed");
    // `SYMMETRY` runs under `--compat=tlc` too, with TLC's comparison in
    // TLC's variable order.
    assert!(refused.iter().all(|n| !n.starts_with("Sym")), "{refused:?}");
}

/// Parallel compat runs give the one-worker result: every fixture, at 2, 3
/// and 8 workers.
#[test]
fn parallel_compat_runs_give_the_one_worker_result() {
    let mut all = flat("tests/fixtures/core-compat");
    all.extend(flat("tests/fixtures/core-m1"));
    all.extend(specs("crates/dive-eval/tests/fixtures/next"));
    all.extend(specs("crates/dive-check/tests/fixtures/tlc-probes"));
    all.extend(specs("crates/dive-check/tests/fixtures/errors"));
    let mut ran = 0;
    for (name, spec, _) in all {
        let one = run_at(&spec, Engine::Core, true, 1);
        if one["verdict"] == "unsupported" || one["verdict"] == "rejected" {
            continue;
        }
        for workers in [2, 3, 8] {
            let many = run_at(&spec, Engine::Core, true, workers);
            assert_eq!(
                observed(&many),
                observed(&one),
                "{name} at {workers} workers"
            );
        }
        ran += 1;
    }
    assert!(ran >= 130, "only {ran} fixtures ran");
}

/// The default engine runs `--compat=tlc` on the core, and refuses by name,
/// without a Reference fallback, where the core refuses.
#[test]
fn the_default_engine_runs_compat_on_the_core() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-compat");
    let j = run(&dir.join("CompatQueue.tla"), Engine::default(), true);
    assert_eq!(j["engine"], "core", "{j:#}");
    assert_eq!(j["verdict"], "invariant-violation");
    let j = run(&dir.join("CompatShape.tla"), Engine::default(), true);
    assert_eq!(j["engine"], "core", "{j:#}");
    assert_eq!(j["verdict"], "unsupported", "{j:#}");
    assert!(
        j["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{j:#}"
    );
    assert!(j.get("engine_fallback").is_none(), "{j:#}");
}
