//! `--engine=auto` (`docs/developer/core/08-correctness.md` §3): where the
//! core runs a check, auto reports the core's result; where the core
//! refuses it, before the search (static) or during it (dynamic), auto
//! reports the reference engine's result for the whole check and names the
//! refusal. Every fixture the core runs is checked both ways.
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

#[test]
fn auto_reports_the_core_or_the_whole_reference_run() {
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
    let (mut on_core, mut fell_static, mut fell_dynamic) = (0, 0, 0);
    for spec in &all {
        let name = spec.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
        let req = request(spec);
        let auto = run(&req, Engine::Auto);
        let core = run(&req, Engine::Core);
        assert_eq!(auto["engine_requested"], "auto", "{name}");
        if core["verdict"] == "unsupported" {
            let reference = run(&req, Engine::Reference);
            assert_eq!(observed(&auto), observed(&reference), "{name}");
            assert_eq!(auto["engine"], "reference", "{name}");
            let fallback = &auto["engine_fallback"];
            assert_eq!(fallback["reason"], core["message"], "{name}");
            match fallback["stage"].as_str() {
                Some("static") => fell_static += 1,
                Some("dynamic") => fell_dynamic += 1,
                other => panic!("{name}: stage {other:?}"),
            }
            let human = auto["human"].as_str().unwrap_or_default();
            assert!(
                human.contains("engine auto: the reference engine ran"),
                "{name}: {human}"
            );
        } else {
            assert_eq!(observed(&auto), observed(&core), "{name}");
            // A check that ends before the search names no engine.
            assert_eq!(auto["engine"], core["engine"], "{name}");
            assert!(auto["engine_fallback"].is_null(), "{name}");
            if core["engine"] == "core" {
                on_core += 1;
            }
        }
    }
    assert!(on_core >= 120, "only {on_core} runs on the core");
    // At this writing: 127 on the core, 92 static and 5 dynamic fallbacks.
    // Both kinds of fallback must stay covered as the core grows.
    assert!(fell_static >= 1, "no static fallback");
    assert!(fell_dynamic >= 1, "no dynamic fallback");
}

/// Run options the core does not implement are static refusals, so auto
/// honours them on the reference engine.
#[test]
fn options_the_core_lacks_run_on_the_reference() {
    let spec = repo().join("crates/dive-api/tests/fixtures/core-m0/Mix.tla");
    let mut req = request(&spec);
    req.store = dive_api::Store::Exact;
    let core = run(&req, Engine::Core);
    assert_eq!(core["verdict"], "unsupported");
    let auto = run(&req, Engine::Auto);
    let reference = run(&req, Engine::Reference);
    assert_eq!(observed(&auto), observed(&reference));
    assert_eq!(auto["engine"], "reference");
    assert_eq!(auto["engine_fallback"]["stage"], "static");
    assert_eq!(auto["options"]["store"], "exact");
}
