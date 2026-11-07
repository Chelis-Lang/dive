//! Core temporal runs under the default whole-run unreduced symmetry policy
//! (D-005), and temporal runs whose only `RandomElement` is fixed at start-up
//! (D-002): the core gives the reference engine's result and reports the
//! policy and the seed; what neither engine proves still refuses by name.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, SymmetryLiveness, check_file_engine};
use serde_json::{Value, json};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn probe(name: &str) -> PathBuf {
    repo().join(format!(
        "crates/dive-liveness/tests/fixtures/tlc-probes/specs/{name}/{name}.tla"
    ))
}

fn fixture(name: &str) -> PathBuf {
    repo().join(format!(
        "crates/dive-api/tests/fixtures/core-startup-random/{name}.tla"
    ))
}

fn run(
    spec: &Path,
    cfg: &Path,
    engine: Engine,
    policy: Option<SymmetryLiveness>,
    seed: i64,
) -> Value {
    let report = check_file_engine(
        &CheckRequest {
            config: Some(cfg.to_path_buf()),
            file: spec.to_path_buf(),
            workers: Some(1),
            seed: Some(seed),
            fp: Some(0),
            symmetry_liveness: policy,
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
    json!({
        "verdict": v["verdict"],
        "property": v["property"],
        "generated": v["statistics"]["generated"],
        "distinct": v["statistics"]["distinct"],
        "depth": v["statistics"]["depth"],
        "exit": v["exit"],
        "trace": v["counterexample"]["states"],
    })
}

#[test]
fn declared_symmetry_with_temporal_properties_runs_unreduced_on_the_core() {
    let p4 = repo().join("conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla");
    for spec in [
        p4,
        probe("SymLiveMissed"),
        probe("SymLiveSymProp"),
        probe("SymLiveWarn"),
    ] {
        let cfg = spec.with_extension("cfg");
        let core = run(&spec, &cfg, Engine::Core, None, 0);
        let reference = run(&spec, &cfg, Engine::Reference, None, 0);
        assert_eq!(core["engine"], "core", "{}: {core}", spec.display());
        assert_ne!(core["verdict"], "unsupported", "{}: {core}", spec.display());
        assert_eq!(observed(&core), observed(&reference), "{}", spec.display());
        assert_eq!(core["symmetry_liveness"]["effective"], "unreduced");
        assert_eq!(core["symmetry_liveness"]["scope"], "whole-run");
        assert_eq!(core["options"]["symmetry_liveness"], "unreduced");
    }
    // SymLiveMissed's violation, hidden by TLC's reduced graph, is found and
    // independently validated.
    let spec = probe("SymLiveMissed");
    let core = run(&spec, &spec.with_extension("cfg"), Engine::Core, None, 0);
    assert_eq!(core["verdict"], "liveness-violation", "{core}");
    assert!(core["counterexample"]["states"].is_array());
}

#[test]
fn refused_and_tlc_symmetry_liveness_policies_still_refuse_on_the_core() {
    let spec = probe("SymLiveMissed");
    let cfg = spec.with_extension("cfg");
    for policy in [SymmetryLiveness::Refuse, SymmetryLiveness::Tlc] {
        let core = run(&spec, &cfg, Engine::Core, Some(policy), 0);
        assert_eq!(core["verdict"], "unsupported", "{policy:?}: {core}");
        assert!(core["counterexample"].is_null());
    }
}

#[test]
fn a_startup_fixed_random_definition_checks_temporal_properties_and_replays_by_seed() {
    let spec = fixture("StartupEdges");
    let cfg = spec.with_extension("cfg");
    for seed in [0, 3, 7, -11] {
        let core = run(&spec, &cfg, Engine::Core, None, seed);
        let reference = run(&spec, &cfg, Engine::Reference, None, seed);
        assert_eq!(core["engine"], "core", "seed {seed}: {core}");
        assert_eq!(core["verdict"], "success", "seed {seed}: {core}");
        assert_eq!(observed(&core), observed(&reference), "seed {seed}");
        // The seed that fixed the start-up draws is reported and replays them.
        assert_eq!(core["options"]["seed"], seed);
        let again = run(&spec, &cfg, Engine::Core, None, seed);
        assert_eq!(observed(&core), observed(&again), "seed {seed}");
    }
}

#[test]
fn unvalidated_or_runtime_draws_with_temporal_properties_refuse_by_name() {
    // A violation needs an independently validated counterexample; the
    // validator does not yet replay start-up draws, so no violation is claimed.
    let spec = fixture("StartupEdges");
    let core = run(
        &spec,
        &spec.with_file_name("StartupEdgesViolated.cfg"),
        Engine::Core,
        None,
        0,
    );
    assert_eq!(core["verdict"], "unsupported", "{core}");
    assert!(core["counterexample"].is_null());
    // Draws while checking, directly or through a parameterized operator.
    for name in ["NextDraw", "PickDraw"] {
        let spec = fixture(name);
        let core = run(&spec, &spec.with_extension("cfg"), Engine::Core, None, 0);
        assert_eq!(core["verdict"], "unsupported", "{name}: {core}");
        let text = core.to_string();
        assert!(text.contains("random operators"), "{name}: {core}");
    }
}
