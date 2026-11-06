//! Budgets of the new core (`--engine=core`): `--max-states` and
//! `--max-depth` stop it before the admission they would exceed, and
//! `--time-limit` and `--max-memory` stop it at the next poll, with the
//! reference engine's structured stop (verdict `stopped`, the resource,
//! limit and observation, the statistics at the stop and the exit code;
//! ADR 0107, `docs/developer/core/07-engine.md` §8).
//!
//! The admission limits are deterministic, so each fixture runs under
//! several of them on both engines, and the reports must agree. The time
//! and memory limits are checked on a model too large to finish.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::{BudgetSetting, CheckRequest, Engine, check_file_engine};
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

fn run(spec: &Path, engine: Engine, edit: impl FnOnce(&mut CheckRequest)) -> Value {
    let mut req = CheckRequest {
        config: Some(spec.with_extension("cfg")),
        file: spec.to_path_buf(),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    };
    edit(&mut req);
    let report = check_file_engine(&req, engine).expect("check runs");
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
        "resource": v["stopped"]["resource"],
        "limit": v["stopped"]["limit"],
        "observed": v["stopped"]["observed"],
        "trace": v["counterexample"]["length"],
    })
}

#[test]
fn admission_limits_stop_as_the_reference_engine_does() {
    let mut all = Vec::new();
    for dir in [
        "crates/dive-check/tests/fixtures/tlc-probes/specs",
        "crates/dive-api/tests/fixtures/core-m0",
        "crates/dive-api/tests/fixtures/core-m1",
    ] {
        specs(&repo().join(dir), &mut all);
    }
    let mut stopped = 0;
    for spec in &all {
        let name = spec.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
        for (states, depth) in [
            (Some(1), None),
            (Some(2), None),
            (Some(5), None),
            (None, Some(1)),
            (None, Some(2)),
            (Some(4), Some(2)),
        ] {
            let limits = |r: &mut CheckRequest| {
                r.max_states = states;
                r.max_depth = depth;
            };
            let core = run(spec, Engine::Core, limits);
            if core["verdict"] == "unsupported" {
                break;
            }
            let reference = run(spec, Engine::Reference, limits);
            if reference["verdict"] == "unsupported" {
                assert_eq!(name, "LazyCupDup", "unavailable reference for {name}");
                // This graph has r=0,1,2 and exactly two canonical successors
                // per state (D-028). Pin its admission prefix independently of
                // the frozen reference's construction-history refusal.
                let stopped_prefix = match (states, depth) {
                    (Some(1), None) => Some(("states", 1, 2, 1, 2, 0, 1)),
                    (Some(2), None) => Some(("states", 2, 3, 2, 3, 1, 2)),
                    (None, Some(1)) => Some(("depth", 1, 2, 1, 2, 0, 1)),
                    _ => None,
                };
                if let Some((resource, limit, observation, distinct, generated, queued, depth)) =
                    stopped_prefix
                {
                    assert_eq!(core["verdict"], "stopped");
                    assert_eq!(core["exit"], 5);
                    assert_eq!(core["stopped"]["resource"], resource);
                    assert_eq!(core["stopped"]["limit"], limit);
                    assert_eq!(core["stopped"]["observed"], observation);
                    for (field, expected) in [
                        ("distinct", distinct),
                        ("generated", generated),
                        ("left_on_queue", queued),
                        ("depth", depth),
                        ("initial_generated", 1),
                    ] {
                        assert_eq!(core["statistics"][field], expected, "{field}/{core:#}");
                    }
                    assert_eq!(core["statistics"]["exhaustive"], false);
                    stopped += 1;
                } else {
                    assert_eq!(core["verdict"], "success", "{core:#}");
                    assert_eq!(core["exit"], 0);
                    assert_eq!(core["statistics"]["distinct"], 3);
                    assert_eq!(core["statistics"]["generated"], 7);
                    assert_eq!(core["statistics"]["depth"], 2);
                }
                assert!(core["counterexample"].is_null());
                continue;
            }
            assert_eq!(
                observed(&core),
                observed(&reference),
                "{name} with --max-states {states:?} --max-depth {depth:?}"
            );
            stopped += usize::from(core["verdict"] == "stopped");
        }
    }
    assert!(stopped >= 40, "only {stopped} runs stopped");
}

/// A model of about 10^12 states: no run finishes it.
fn endless() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dive-core-budgets-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let spec = dir.join("Endless.tla");
    std::fs::write(
        &spec,
        "---- MODULE Endless ----\nEXTENDS Naturals\nVARIABLES x, y\n\
         Init == x = 0 /\\ y = 0\n\
         Next == (x < 1000000 /\\ x' = x + 1 /\\ y' = y) \\/ (y < 1000000 /\\ y' = y + 1 /\\ x' = x)\n\
         ====\n",
    )
    .expect("spec");
    std::fs::write(spec.with_extension("cfg"), "INIT Init\nNEXT Next\n").expect("cfg");
    spec
}

#[test]
fn time_and_memory_limits_stop_the_core() {
    let spec = endless();
    let core = run(&spec, Engine::Core, |r| {
        r.time_limit = BudgetSetting::Explicit(Duration::from_millis(300));
    });
    assert_eq!(core["verdict"], "stopped", "{core:#}");
    assert_eq!(core["stopped"]["resource"], "time", "{core:#}");
    assert_eq!(core["stopped"]["limit"], 300, "{core:#}");
    assert_eq!(core["engine"], "core");
    assert!(
        core["statistics"]["distinct"].as_u64() > Some(0),
        "{core:#}"
    );
    let reference = run(&spec, Engine::Reference, |r| {
        r.time_limit = BudgetSetting::Explicit(Duration::from_millis(300));
    });
    assert_eq!(core["exit"], reference["exit"]);
    // A ceiling just above what the process holds now: the search grows
    // past it.
    let rss = dive_check::budget::resident_bytes().expect("resident memory is readable");
    let ceiling = rss + (64 << 20);
    let core = run(&spec, Engine::Core, |r| {
        r.max_memory = BudgetSetting::Explicit(ceiling);
        r.time_limit = BudgetSetting::Explicit(Duration::from_secs(120));
    });
    assert_eq!(core["verdict"], "stopped", "{core:#}");
    assert_eq!(core["stopped"]["resource"], "memory", "{core:#}");
    assert_eq!(core["stopped"]["limit"], ceiling, "{core:#}");
    let _ = std::fs::remove_dir_all(spec.parent().expect("dir"));
}
