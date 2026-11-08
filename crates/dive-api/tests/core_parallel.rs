//! The parallel level of the new core (`--engine=core --workers N`,
//! `docs/developer/core/07-engine.md` §3): every observable result is the
//! one-worker result. Every fixture the core runs is checked at 2, 3 and 8
//! workers against 1 worker on its verdict, property, statistics, exit,
//! stop, output (warnings included), and trace document, with and without
//! admission limits.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_parallel.json");

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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

fn run(spec: &Path, workers: usize, max_states: Option<u64>) -> Value {
    run_on(spec, workers, max_states, Engine::Core)
}

fn run_on(spec: &Path, workers: usize, max_states: Option<u64>, engine: Engine) -> Value {
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            workers: Some(workers),
            seed: Some(0),
            fp: Some(0),
            max_states,
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
    let mut stopped = v["stopped"].clone();
    if let Some(o) = stopped.as_object_mut() {
        o.remove("elapsed_ms");
    }
    serde_json::json!({
        "verdict": v["verdict"],
        "property": v["property"],
        "message": v["message"],
        "statistics": v["statistics"],
        "exit": v["exit"],
        "stopped": stopped,
        "output": v["output"],
        "trace": v["counterexample"],
    })
}

#[test]
fn every_worker_count_gives_the_one_worker_result() {
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
    let mut checked = 0;
    for spec in &all {
        let name = spec.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
        for limit in [None, Some(3)] {
            let one = run(spec, 1, limit);
            if one["verdict"] == "unsupported" || one["verdict"] == "rejected" {
                break;
            }
            for workers in [2, 3, 8] {
                let many = run(spec, workers, limit);
                assert_eq!(
                    observed(&many),
                    observed(&one),
                    "{name} at {workers} workers, --max-states {limit:?}"
                );
                // A run that ends at startup (a false or failing ASSUME)
                // starts no worker, in the reference engine too.
                let used = if one["options"]["workers"]["used"] == 0 {
                    let reference = run_on(spec, workers, limit, Engine::Core);
                    agreed::check_eq(
                        AGREED,
                        "every_worker_count_gives_the_one_worker_result/0",
                        &(observed(&one)["verdict"]),
                        &(observed(&reference)["verdict"]),
                    );
                    reference["options"]["workers"]["used"].clone()
                } else if many["options"]["workers"]["reason"]
                    .as_str()
                    .is_some_and(|r| {
                        r.contains("(Print or PrintT)")
                            || r.contains("(RandomElement)")
                            || r.contains("TLCSet or TLCGet")
                    })
                {
                    // A specification that prints or draws runs one worker,
                    // so that its output and draws are in TLC's order.
                    Value::from(1)
                } else {
                    Value::from(workers)
                };
                assert_eq!(many["options"]["workers"]["used"], used, "{name}");
            }
            checked += 1;
        }
    }
    assert!(checked >= 150, "only {checked} runs were checked");
}
