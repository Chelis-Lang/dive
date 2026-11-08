//! The new core's successor generator against the rule fixtures of
//! `docs/developer/core/05-successor-generation.md` §7 (the `Nx*` specs of
//! `crates/dive-eval/tests/fixtures/next/`) and the TLC probes of
//! `crates/dive-api/tests/fixtures/checker/tlc-probes/`.
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
//! construct, never answered wrongly. Canonical lazy-union enumeration has
//! an exact registered generated-count difference (D-028).
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_rules.json");

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

const ORACLE: &str = "tlc-2026.10.06.014338";

/// Fixtures the core refuses, with a word its refusal must contain.
const REFUSED: &[(&str, &str)] = &[
    // A symbolic infinite child cannot acquire a structural state identity.
    // Refuse before the legacy fingerprint error, without an error certificate.
    ("ErFpInfinite", "mathematical identity"),
    // TLC cannot apply an operator with a constant body passed as an
    // `OpValue`; the reference engine applies it (`04-plan-compiler.md` §8).
    ("ErHoConstBagOfAll", "body is constant"),
    ("ErHoConstFoldLeft", "body is constant"),
    // A set of consecutive integers may be TLC's interval, which TLC prints
    // as `1..2`: a stored interval, `DOMAIN` of a sequence
    // (`crates/dive-core/src/print.rs`).
    ("TsStored", "consecutive integers"),
    ("TsDomain", "consecutive integers"),
    ("PrAmbig", "consecutive integers"),
    // TLC prints again while it rebuilds the trace (`PrViol`), and may
    // evaluate a function constructor's body lazily (`PrLazy`).
    ("PrViol", "does not end in success"),
    ("PrLazy", "function constructor"),
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
    // The CommunityModules the reference engine's fixtures use.
    let community = repo().join("crates/dive-eval/tests/fixtures/community-modules");
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            library_paths: vec![community],
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

/// Fixtures only the retired reference engine ran (#791): Core refuses each
/// by name. Restoring them when Core implements the feature is tracked in
/// #910.
const REFERENCE_ONLY: &[&str] = &[
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
];

fn check_dir(dir: &str) -> usize {
    let mut agreed = 0;
    for (name, spec, transcript) in specs(dir) {
        let core = run(&spec, Engine::Core);
        if core["verdict"] == "rejected" {
            // Rejected before any engine runs.
            assert_eq!(run(&spec, Engine::Core)["verdict"], "rejected", "{name}");
            continue;
        }
        assert_eq!(core["engine"], "core", "{name}");
        if name == "NxInLazyCup" {
            // D-028 canonical enumeration visits the repeated record once.
            // The unchanged TLC transcript proves the precise 10 -> 7 delta.
            agreed::check_eq(
                AGREED,
                "check_dir/0",
                &(tlc_counts(&transcript)),
                &(Some((10, 3, 0))),
            );
            assert_eq!(core["verdict"], "success");
            assert_eq!(core["statistics"]["generated"], 7);
            assert_eq!(core["statistics"]["distinct"], 3);
            assert_eq!(core["statistics"]["depth"], 2);
            assert_eq!(core["statistics"]["left_on_queue"], 0);
            continue;
        }
        if let Some((_, word)) = REFUSED.iter().find(|(n, _)| *n == name) {
            assert_eq!(core["verdict"], "unsupported", "{name}: {core:#}");
            assert!(
                core["message"].as_str().is_some_and(|m| m.contains(word)),
                "{name}: {core:#}"
            );
            continue;
        }
        let reference = run(&spec, Engine::Core);
        if REFERENCE_ONLY.contains(&name.as_str()) {
            assert_eq!(core["verdict"], "unsupported", "{name}: {core:#}");
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
        agreed::check_eq(
            AGREED,
            "check_dir/1",
            &(observed(&core)),
            &(observed(&reference)),
        );
        // The default profile draws from dive's streams, not TLC's (D-002):
        // `tests/core_random.rs` checks the `Rd*` fixtures against TLC under
        // `--compat=tlc`.
        if core["verdict"] == "success"
            && !name.starts_with("Rd")
            && let Some((g, d, q)) = tlc_counts(&transcript)
        {
            agreed::check_eq(
                AGREED,
                "check_dir/2",
                &(
                    core["statistics"]["generated"].as_u64(),
                    core["statistics"]["distinct"].as_u64(),
                    core["statistics"]["left_on_queue"].as_u64(),
                ),
                &(Some(g), Some(d), Some(q)),
            );
        }
        agreed += 1;
    }
    agreed
}

#[test]
fn rule_fixtures_match_the_reference_and_tlc() {
    let agreed = check_dir("crates/dive-eval/tests/fixtures/next");
    // Native agreements exclude NxInLazyCup's exact registered D-028
    // generated-count difference; it is checked separately above.
    assert!(agreed >= 15, "only {agreed} rule fixtures agreed");
}

#[test]
fn tlc_probes_match_the_reference_and_tlc() {
    let agreed = check_dir("crates/dive-api/tests/fixtures/checker/tlc-probes");
    assert!(agreed >= 18, "only {agreed} probes agreed");
}

/// The error-class fixtures (`crates/dive-api/tests/fixtures/checker/errors/`):
/// the core's error endings (class, exit, counts after TLC's call-stack
/// replay, trace length) equal the reference engine's.
#[test]
fn error_fixtures_match_the_reference() {
    let agreed = check_dir("crates/dive-api/tests/fixtures/checker/errors");
    // 175 fixtures: 130 or more agree, `ErFpInfinite`, the implied action
    // errors (`ErProp*`), the `ErAlias*` and the `ErView*` fixtures among
    // them; the rest use POSTCONDITION, Print, ENABLED, non-constant split
    // arguments, ASSUME errors or recursive functions, which the core does
    // not run yet.
    assert!(agreed >= 130, "only {agreed} error fixtures agreed");
}

/// The standard-library fixtures (`tests/fixtures/core-lib`): one model
/// per operator family of `crates/dive-core/src/stdlib/`, and one per error
/// case, with pinned-TLC transcripts.
#[test]
fn library_fixtures_match_the_reference_and_tlc() {
    let agreed = check_dir("crates/dive-api/tests/fixtures/core-lib");
    // Bags: BagOps and four error endings (ErBagUnionNonBag is refused);
    // TLC: TlcOps, TsOps, TsLazy, PrOps, PrRange, Perm, PermViol and five
    // error endings (TsStored, TsDomain, PrAmbig, PrViol and PrLazy are
    // refused);
    // CommunityModules: CmOps, CmSeqsViol, CmSubSeqs, CmSubSeqsViol and six
    // error endings;
    // operator arguments: HoOps, FoldOps and six error endings;
    // RandomElement: RdBfs, RdShapes, RdViol, RdErr and RdView (refused
    // by both engines);
    // TLCSet and TLCGet: RgCount, RgAll, RgLevel and RgLevelViol.
    assert!(agreed >= 43, "only {agreed} library fixtures agreed");
}

/// What TLC printed (`Print`, `PrintT`): the transcript's lines outside
/// its tool-mode messages, after SANY's own output.
fn tlc_printed(transcript: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(transcript).expect("transcript");
    let mut out = Vec::new();
    let mut in_message = false;
    let mut started = false;
    for line in text.lines() {
        if line.starts_with("@!@!@STARTMSG") {
            in_message = true;
        } else if line.starts_with("@!@!@ENDMSG") {
            in_message = false;
        } else if line.starts_with("Linting of module") {
            started = true;
        } else if started && !in_message {
            out.push(line.to_owned());
        }
    }
    out
}

/// The `Print` fixtures (`core-lib/specs/Pr*`) that the core runs print
/// exactly what TLC printed, in TLC's order.
#[test]
fn print_fixtures_print_as_tlc() {
    let mut checked = 0;
    for (name, spec, transcript) in specs("crates/dive-api/tests/fixtures/core-lib") {
        if !name.starts_with("Pr") || REFUSED.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let core = run(&spec, Engine::Core);
        assert_eq!(core["verdict"], "success", "{name}: {core:#}");
        let printed: Vec<String> = core["output"]
            .as_array()
            .expect("output")
            .iter()
            .flat_map(|l| {
                l.as_str()
                    .unwrap_or("")
                    .lines()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(printed, tlc_printed(&transcript), "{name}");
        // More workers print the same: a plan that prints runs one.
        let community = repo().join("crates/dive-eval/tests/fixtures/community-modules");
        let four = check_file_engine(
            &CheckRequest {
                config: Some(spec.with_extension("cfg")),
                file: spec.clone(),
                library_paths: vec![community],
                workers: Some(4),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            },
            Engine::Core,
        )
        .expect("check runs")
        .to_json();
        assert_eq!(four["output"], core["output"], "{name} at 4 workers");
        // And under `--compat=tlc`.
        let compat = check_file_engine(
            &CheckRequest {
                config: Some(spec.with_extension("cfg")),
                file: spec.clone(),
                library_paths: vec![
                    repo().join("crates/dive-eval/tests/fixtures/community-modules"),
                ],
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                compat_tlc: true,
                ..CheckRequest::default()
            },
            Engine::Core,
        )
        .expect("check runs")
        .to_json();
        assert_eq!(
            compat["verdict"], "success",
            "{name} under --compat=tlc: {compat:#}"
        );
        assert_eq!(
            compat["output"], core["output"],
            "{name} under --compat=tlc"
        );
        checked += 1;
    }
    assert!(checked >= 1);
}

/// The M1 generator fixtures (`tests/fixtures/core-m1`): each agrees with
/// the reference engine, or has an exact D-028 canonical-enumeration control.
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
        let reference = run(&spec, Engine::Core);
        if matches!(
            name.as_str(),
            "FpInfiniteSets" | "FpRecordSet" | "FpSeqSet" | "FpSubsetNat"
        ) {
            assert_eq!(core["verdict"], "unsupported");
            assert!(
                core["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("mathematical identity"))
            );
            assert!(core["tlc"].is_null());
            continue;
        }
        if name == "FpInit" {
            // Both reject unproved infinite identity; Core now rejects at
            // construction rather than assigning meaningless partial counts.
            assert_eq!(core["verdict"], "unsupported", "{name}");
            assert!(core["tlc"].is_null());
            assert!(
                core["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("mathematical identity"))
            );
            continue;
        }
        if let Some((generated, distinct)) = match name.as_str() {
            "LazyCupDup" => Some((7, 3)),
            "LazyUnionDup" => Some((5, 2)),
            _ => None,
        } {
            // D-028 removes raw duplicate enumeration. Shared finite UNION
            // construction also supports the duplicate-union case for replay.
            assert_eq!(core["verdict"], "success");
            assert_eq!(core["statistics"]["generated"], generated);
            assert_eq!(core["statistics"]["distinct"], distinct);
            assert_eq!(core["statistics"]["depth"], 2);
            // The retired reference engine refused LazyCupDup, so only
            // LazyUnionDup has an agreed record.
            if name != "LazyCupDup" {
                agreed::check_eq(
                    AGREED,
                    "m1_generator_fixtures_match_the_reference/0",
                    &(observed(&core)),
                    &(observed(&reference)),
                );
            }
            continue;
        }
        agreed::check_eq(
            AGREED,
            "m1_generator_fixtures_match_the_reference/1",
            &(observed(&core)),
            &(observed(&reference)),
        );
    }
}
