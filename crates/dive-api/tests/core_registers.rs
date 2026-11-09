//! `TLCSet` and `TLCGet` on the new core (`docs/developer/core/03-value-model.md`
//! §6.2). The core reads and writes the reference evaluator's registers,
//! which the `ASSUME`s wrote, in TLC's order with one worker, and reads
//! `TLCGet("level")` from the engine: 0 in `Init`, the checked state's
//! level in state checks, the expanded state's in the next-state relation
//! and action checks.
//!
//! The `Rg*` fixtures (`tests/fixtures/core-lib`) give the agreed result
//! (`support/agreed.rs`) in both profiles at 1 and 4 workers, and the pinned
//! TLC's counts and printed lines. The `Tm*` fixtures in `tests/fixtures/checker` give
//! the agreed result or a named refusal.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_registers.json");

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

const ORACLE: &str = "tlc-2026.10.06.014338";

/// The `Tm*` fixtures the core refuses, with a word its refusal contains.
const REFUSED: &[(&str, &str)] = &[
    ("TmExit", "TLCSet(\"exit\")"),
    ("TmExitInit", "TLCSet(\"exit\")"),
    (
        "TmLevel",
        "Print or PrintT in a run that does not end in success",
    ),
    ("TmRandom", "RandomSubset"),
    (
        "TmRegisters",
        "TLCSet or TLCGet in a run that does not end in success",
    ),
    (
        "TmRegistersAll",
        "TLCSet or TLCGet in a run that does not end in success",
    ),
];

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

fn run(spec: &Path, engine: Engine, compat_tlc: bool, workers: usize) -> Value {
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

#[test]
fn registers_match_the_reference_engine_and_tlc() {
    let root = repo().join("crates/dive-api/tests/fixtures/core-lib");
    let fixtures = specs("crates/dive-api/tests/fixtures/core-lib", "Rg");
    assert!(fixtures.len() >= 4, "{fixtures:?}");
    for (name, spec) in fixtures {
        let transcript = root.join(ORACLE).join(format!("{name}.stdout"));
        for compat in [false, true] {
            let at = format!("{name} compat={compat}");
            let core = run(&spec, Engine::Core, compat, 1);
            assert_eq!(core["engine"], "core", "{at}");
            let reference = run(&spec, Engine::Core, compat, 1);
            agreed::check_eq(
                AGREED,
                "registers_match_the_reference_engine_and_tlc/0",
                &(observed(&core)),
                &(observed(&reference)),
            );
            let four = run(&spec, Engine::Core, compat, 4);
            assert_eq!(observed(&four), observed(&core), "{at} at 4 workers");
            assert_eq!(four["options"]["workers"]["used"], 1, "{at}: {four:#}");
            let (g, d) = tlc_counts(&transcript).expect("TLC counts");
            assert_eq!(
                (
                    core["statistics"]["generated"].as_u64(),
                    core["statistics"]["distinct"].as_u64()
                ),
                (Some(g), Some(d)),
                "{at} against TLC"
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
                assert_eq!(printed, tlc_printed(&transcript), "{at}");
            }
        }
    }
}

#[test]
fn tlc_module_fixtures_match_or_are_refused() {
    for (name, spec) in specs("crates/dive-api/tests/fixtures/checker/tlc-module", "Tm") {
        for compat in [false, true] {
            let at = format!("{name} compat={compat}");
            let core = run(&spec, Engine::Core, compat, 1);
            if let Some((_, word)) = REFUSED.iter().find(|(n, _)| *n == name) {
                assert_eq!(core["verdict"], "unsupported", "{at}: {core:#}");
                assert!(
                    core["message"].as_str().is_some_and(|m| m.contains(word)),
                    "{at}: {core:#}"
                );
                continue;
            }
            let reference = run(&spec, Engine::Core, compat, 1);
            agreed::check_eq(
                AGREED,
                "tlc_module_fixtures_match_or_are_refused/0",
                &(observed(&core)),
                &(observed(&reference)),
            );
        }
    }
}
