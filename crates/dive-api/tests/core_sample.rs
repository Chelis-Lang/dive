//! The successor sample of the shadow comparison (`--core-sample`,
//! `docs/developer/core/08-correctness.md` §4). On every fixture the core
//! runs, the successors of the sampled states equal the reference
//! evaluator's, action by action and with multiplicities; the same seed
//! samples the same states at any worker count; and sampling changes no
//! result.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, check_file};
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

fn run(spec: &Path, workers: usize, sample: Option<usize>) -> Value {
    let report = check_file(&CheckRequest {
        config: Some(spec.with_extension("cfg")),
        file: spec.to_path_buf(),
        workers: Some(workers),
        seed: Some(7),
        fp: Some(0),
        core_sample: sample,
        ..CheckRequest::default()
    })
    .expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    j
}

fn observed(v: &Value) -> Value {
    serde_json::json!({
        "verdict": v["verdict"],
        "property": v["property"],
        "statistics": v["statistics"],
        "exit": v["exit"],
        "output": v["output"],
        "trace": v["counterexample"],
    })
}

#[test]
fn a_reference_refusal_is_visible_without_comparison_credit() {
    let spec =
        repo().join("crates/dive-eval/tests/fixtures/next/specs/NxInLazyCup/NxInLazyCup.tla");
    let checked = run(&spec, 1, Some(16));
    assert_eq!(checked["verdict"], "success", "{checked:#}");
    for (field, expected) in [("generated", 7), ("distinct", 3), ("depth", 2)] {
        assert_eq!(checked["statistics"][field], expected, "{field}");
    }
    let sample = &checked["core"]["sample"];
    assert_eq!(sample["states"], 3);
    assert_eq!(sample["skipped"], 3);
    assert_eq!(sample["compared"], 0);
    assert_eq!(sample["successors"], 0);
    assert_eq!(sample["mismatches"], 0);
    let shown = sample["shown"].as_array().expect("unavailable comparisons");
    assert_eq!(shown.len(), 3);
    for item in shown {
        assert!(
            item["difference"]
                .as_str()
                .expect("reason")
                .starts_with("reference comparison unavailable: unsupported: enumerating")
        );
    }
}

#[test]
fn sampled_successors_equal_the_reference_evaluators() {
    let mut all = Vec::new();
    for dir in [
        "crates/dive-eval/tests/fixtures/next/specs",
        "crates/dive-api/tests/fixtures/checker/tlc-probes/specs",
        "crates/dive-api/tests/fixtures/checker/errors/specs",
        "crates/dive-api/tests/fixtures/core-m0",
        "crates/dive-api/tests/fixtures/core-m1",
    ] {
        specs(&repo().join(dir), &mut all);
    }
    let mut sampled = 0;
    let mut compared = 0;
    for spec in &all {
        let name = spec.display().to_string();
        let one = run(spec, 1, Some(16));
        if one["verdict"] == "unsupported" || one["verdict"] == "rejected" {
            continue;
        }
        let sample = &one["core"]["sample"];
        if sample.is_null() {
            // Refused before the search.
            continue;
        }
        assert_eq!(sample["mismatches"], 0, "{name}: {sample:#}");
        assert_eq!(observed(&one), observed(&run(spec, 1, None)), "{name}");
        let three = run(spec, 3, Some(16));
        assert_eq!(three["core"]["sample"], *sample, "{name}");
        sampled += sample["states"].as_u64().unwrap_or(0);
        compared += sample["compared"].as_u64().unwrap_or(0);
    }
    assert!(sampled >= 200, "only {sampled} states sampled");
    assert!(compared >= 200, "only {compared} pairs compared");
}
