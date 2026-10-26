//! The M0 core engine (`--engine=core`) agrees with the reference engine on
//! verdicts and TLC's counts, under every strategy switch, and refuses what
//! it does not implement.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-m0")
        .join(format!("{name}.tla"))
}

fn run(name: &str, engine: Engine) -> Value {
    let file = fixture(name);
    let report = check_file_engine(
        &CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        engine,
    )
    .expect("check runs");
    report.to_json()
}

fn counts(v: &Value) -> (Value, Value, Value, Value) {
    (
        v["verdict"].clone(),
        v["statistics"]["generated"].clone(),
        v["statistics"]["distinct"].clone(),
        v["statistics"]["depth"].clone(),
    )
}

#[test]
fn core_matches_reference_counts_and_verdicts() {
    for name in ["Mix", "Viol", "Dead", "Fold"] {
        let reference = run(name, Engine::Reference);
        let core = run(name, Engine::Core);
        assert_eq!(core["engine"], "core", "{name}");
        assert!(reference.get("engine").is_none(), "{name}");
        assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
        assert_eq!(core["property"], reference["property"], "{name}");
    }
}

/// R9: a closed filter beyond the fold budget is left to run time, the
/// abandoned fold is reported, and the counts do not change.
#[test]
fn large_folds_are_bounded_and_reported() {
    let core = run("Fold", Engine::Core);
    let plan = &core["core"]["metrics"]["plan"];
    assert_eq!(plan["fold_abandoned"], 1, "{plan:#}");
    assert!(
        plan["fold_fuel"].as_u64().is_some_and(|f| f <= 1 << 22),
        "{plan:#}"
    );
    assert!(plan["fold_ms"].is_number(), "{plan:#}");
    assert_eq!(counts(&core), counts(&run("Fold", Engine::Reference)));
}

#[test]
fn violations_and_deadlocks_are_reported() {
    assert_eq!(run("Viol", Engine::Core)["verdict"], "invariant-violation");
    assert_eq!(run("Viol", Engine::Core)["property"], "Small");
    assert_eq!(run("Dead", Engine::Core)["verdict"], "deadlock");
}

#[test]
fn unsupported_features_are_named_without_fallback() {
    let core = run("Unsup", Engine::Core);
    assert_eq!(core["verdict"], "unsupported");
    assert_eq!(core["engine"], "core");
    assert!(
        core["message"]
            .as_str()
            .is_some_and(|m| m.contains("temporal properties")),
        "{core:#}"
    );
}

#[test]
fn every_strategy_switch_leaves_counts_unchanged() {
    for name in ["Mix", "Fold"] {
        let file = fixture(name);
        let req = CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let base = counts(&run(name, Engine::Reference));
        let mut all = Vec::new();
        for switch in dive_core_switches() {
            all.push(format!("-{switch}"));
            let r = dive_api::check_file_core_opts(&req, &format!("-{switch}")).expect("runs");
            assert_eq!(counts(&r.to_json()), base, "{name} -{switch}");
        }
        let r = dive_api::check_file_core_opts(&req, &all.join(",")).expect("runs");
        assert_eq!(counts(&r.to_json()), base, "{name} all off");
    }
}

fn dive_core_switches() -> [&'static str; 9] {
    [
        "delta-hash",
        "early-probe",
        "hashcons",
        "symbolic-member",
        "memo",
        "parent-skip",
        "arena",
        "eager",
        "fold",
    ]
}

#[test]
fn unknown_switches_are_refused() {
    let file = fixture("Viol");
    let req = CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(1),
        ..CheckRequest::default()
    };
    let r = dive_api::check_file_core_opts(&req, "-turbo").expect("runs");
    assert_eq!(r.to_json()["verdict"], "unsupported");
}
