//! The parallel level of the new core (`--engine=core --workers N`,
//! `docs/developer/core/07-engine.md` §3): every observable result is the
//! one-worker result. Every fixture the core runs is checked at 2, 3 and 8
//! workers against 1 worker on its verdict, property, statistics, exit,
//! stop, output (warnings included), and trace document, with and without
//! admission limits.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

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
        Engine::Core,
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
                assert_eq!(many["options"]["workers"]["used"], workers, "{name}");
            }
            checked += 1;
        }
    }
    assert!(checked >= 150, "only {checked} runs were checked");
}
