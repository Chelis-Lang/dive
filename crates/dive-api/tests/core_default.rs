//! The default engine is the compiled Core (ADR 0153): `check_file` reports
//! exactly what explicit Core reports, and what Core does not implement is a
//! named refusal (exit 3), never a fallback to the reference engine.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file, check_file_engine};
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

fn request(spec: &Path) -> CheckRequest {
    CheckRequest {
        config: Some(spec.with_extension("cfg")),
        file: spec.to_path_buf(),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn run(req: &CheckRequest, engine: Engine) -> Value {
    let report = check_file_engine(req, engine).expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    j["human"] = Value::from(report.render_human());
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

fn run_default(req: &CheckRequest) -> Value {
    let report = check_file(req).expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    j["human"] = Value::from(report.render_human());
    j
}

#[test]
fn the_default_engine_is_core_and_refuses_by_name_without_fallback() {
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
    let (mut on_core, mut refused) = (0, 0);
    for spec in &all {
        let name = spec.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
        let req = request(spec);
        let default = run_default(&req);
        let core = run(&req, Engine::Core);
        assert_eq!(observed(&default), observed(&core), "{name}");
        // A check that ends before the search names no engine.
        assert_eq!(default["engine"], core["engine"], "{name}");
        assert_ne!(default["engine"], "reference", "{name}");
        assert!(default.get("engine_fallback").is_none(), "{name}");
        assert!(default.get("engine_requested").is_none(), "{name}");
        if default["verdict"] == "unsupported" {
            assert_eq!(default["exit"], 3, "{name}");
            assert!(
                default["message"].as_str().is_some_and(|m| !m.is_empty()),
                "{name}: unnamed refusal"
            );
            refused += 1;
        } else if default["engine"] == "core" {
            on_core += 1;
        }
        let human = default["human"].as_str().unwrap_or_default();
        assert!(human.contains("requested engine core"), "{name}: {human}");
        assert!(!human.contains("engine auto"), "{name}: {human}");
    }
    assert!(on_core >= 120, "only {on_core} runs on the core");
    // Refusals stay covered as the core grows.
    assert!(refused >= 1, "no named refusal");
}

/// Exact storage executes on the core and preserves reference observables.
#[test]
fn exact_storage_runs_on_the_core() {
    let spec = repo().join("crates/dive-api/tests/fixtures/core-m0/Mix.tla");
    let mut req = request(&spec);
    req.store = dive_api::Store::Exact;
    let core = run(&req, Engine::Core);
    let default = run_default(&req);
    let reference = run(&req, Engine::Reference);
    assert_eq!(observed(&core), observed(&reference));
    assert_eq!(observed(&default), observed(&reference));
    assert_eq!(default["engine"], "core");
    assert_eq!(default["options"]["store"], "exact");
}
