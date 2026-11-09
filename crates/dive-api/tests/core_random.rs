//! `RandomElement` on the new core (`docs/developer/core/03-value-model.md`
//! §6.1): the core draws from `dive-stdlib`'s streams, the start-up
//! stream for `Init` and, for each expanded state, the stream TLC seeds with
//! the state's fingerprint. So at every seed and in both profiles a run the
//! core completes gives the agreed result (`support/agreed.rs`): verdict, counts,
//! output and the counterexample's states; `--seed` replays it; and under
//! `--compat=tlc` the `Rd*` fixtures (`tests/fixtures/core-lib`) give the
//! pinned TLC's counts and printed lines at `-seed 0`.
//!
//! What the core does not run is refused by name: a draw in `Init` after
//! start-up drew (TLC's trace rebuild restarts that stream, D-002), a
//! `LET`-bound draw used in more than one conjunct (TLC draws it again at
//! each, fixture `RndLetRedraw`) and `RandomSubset`.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_random.json");

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

const ORACLE: &str = "tlc-2026.10.06.014338";

const SEEDS: &[i64] = &[0, 7, -3, i64::MAX];

/// The random fixtures the core refuses, with a word its refusal contains.
const REFUSED: &[(&str, &str)] = &[
    ("RndBfs", "more than one conjunct"),
    ("RndLetRedraw", "more than one conjunct"),
    ("RndNext", "RandomSubset"),
    ("RndStartup", "ASSUMEs drew"),
    ("RndTraceAssume", "ASSUMEs drew"),
    ("TmRandom", "RandomSubset"),
    // The binder refuses random streams under VIEW or SYMMETRY.
    ("RdView", "randomness under VIEW is not supported"),
];

fn refused(name: &str) -> bool {
    REFUSED.iter().any(|(n, _)| *n == name)
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn specs(dir: &str, prefix: &str) -> Vec<(String, PathBuf)> {
    let root = repo().join(dir).join("specs");
    let mut out: Vec<_> = std::fs::read_dir(root)
        .expect("fixture directory")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_owned();
            name.starts_with(prefix)
                .then(|| (name.clone(), p.join(format!("{name}.tla"))))
        })
        .collect();
    out.sort();
    out
}

fn run(spec: &Path, engine: Engine, compat_tlc: bool, seed: i64, workers: usize) -> Value {
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            workers: Some(workers),
            seed: Some(seed),
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
        "message": v["message"],
        "generated": v["statistics"]["generated"],
        "distinct": v["statistics"]["distinct"],
        "depth": v["statistics"]["depth"],
        "queue": v["statistics"]["left_on_queue"],
        "exit": v["exit"],
        "output": v["output"],
        "trace": v["counterexample"]["states"],
    })
}

fn all() -> Vec<(String, PathBuf)> {
    let mut v = specs("crates/dive-api/tests/fixtures/core-lib", "Rd");
    v.extend(specs("crates/dive-stdlib/tests/fixtures/random", "Rnd"));
    v.extend(specs(
        "crates/dive-api/tests/fixtures/checker/tlc-module",
        "TmRandom",
    ));
    v
}

/// Every random fixture, at four seeds and in both profiles: the core's
/// result is the agreed record, or a named refusal.
#[test]
fn draws_match_the_reference_engine() {
    let mut agreed = 0;
    for (name, spec) in all() {
        for &compat in &[false, true] {
            for &seed in SEEDS {
                let core = run(&spec, Engine::Core, compat, seed, 1);
                let at = format!("{name} compat={compat} seed={seed}");
                if let Some((_, word)) = REFUSED.iter().find(|(n, _)| *n == name) {
                    assert_eq!(core["verdict"], "unsupported", "{at}: {core:#}");
                    assert!(
                        core["message"].as_str().is_some_and(|m| m.contains(word)),
                        "{at}: {core:#}"
                    );
                    continue;
                }
                assert_eq!(core["engine"], "core", "{at}");
                let reference = run(&spec, Engine::Core, compat, seed, 1);
                agreed::check_eq(
                    AGREED,
                    "draws_match_the_reference_engine/0",
                    &(observed(&core)),
                    &(observed(&reference)),
                );
                agreed += 1;
            }
        }
    }
    // Rd*: four fixtures; Rnd*: the ten start-up error endings;
    // TmRandomStartup, whose start-up draws `Init` does not continue.
    assert!(agreed >= 15 * 8, "only {agreed} runs agreed");
}

/// The same seed gives the same run, at any requested worker count (the
/// core draws on one worker and reports why).
#[test]
fn a_seed_replays() {
    for (name, spec) in specs("crates/dive-api/tests/fixtures/core-lib", "Rd") {
        if refused(&name) {
            continue;
        }
        for &compat in &[false, true] {
            let first = run(&spec, Engine::Core, compat, 7, 1);
            let again = run(&spec, Engine::Core, compat, 7, 4);
            assert_eq!(observed(&first), observed(&again), "{name}");
            assert_eq!(again["options"]["workers"]["used"], 1, "{name}: {again:#}");
            assert!(
                again["options"]["workers"]["reason"]
                    .as_str()
                    .is_some_and(|r| r.contains("RandomElement") || r.contains("Print")),
                "{name}: {again:#}"
            );
        }
        let a = run(&spec, Engine::Core, false, 0, 1);
        let b = run(&spec, Engine::Core, false, 1, 1);
        if name == "RdBfs" {
            assert_ne!(
                a["output"], b["output"],
                "{name}: the seed changes the draws"
            );
        }
    }
}

/// TLC's printed lines: the transcript's lines outside its tool-mode
/// messages, after SANY's output.
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

fn tlc_counts(transcript: &Path) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(transcript).ok()?;
    text.lines()
        .filter_map(|l| {
            let (g, rest) = l.split_once(" states generated, ")?;
            let (d, _) = rest.split_once(" distinct states found, ")?;
            let n = |s: &str| s.replace(',', "").parse::<u64>().ok();
            Some((n(g)?, n(d)?))
        })
        .next_back()
}

/// Under `--compat=tlc` at `-seed 0`, the core draws what the pinned TLC
/// drew: its counts and, for a run that ends in success, its printed lines.
#[test]
fn compat_draws_match_tlc() {
    let root = repo().join("crates/dive-api/tests/fixtures/core-lib");
    for (name, spec) in specs("crates/dive-api/tests/fixtures/core-lib", "Rd") {
        if refused(&name) {
            continue;
        }
        let transcript = root.join(ORACLE).join(format!("{name}.stdout"));
        let core = run(&spec, Engine::Core, true, 0, 1);
        let (g, d) = tlc_counts(&transcript).expect("TLC counts");
        assert_eq!(
            (
                core["statistics"]["generated"].as_u64(),
                core["statistics"]["distinct"].as_u64()
            ),
            (Some(g), Some(d)),
            "{name}: {core:#}"
        );
        if core["verdict"] == "success" {
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
        }
    }
}
