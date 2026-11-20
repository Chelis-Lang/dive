//! Core temporal runs under the default whole-run unreduced symmetry policy
//! (D-005), and temporal runs whose only `RandomElement` is fixed at start-up
//! (D-002): the core gives the agreed result (`support/agreed.rs`) and
//! reports the policy and the seed; what Core does not prove still refuses
//! by name.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_symmetry_random_temporal.json");

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, SymmetryLiveness, check_file};
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

fn run(spec: &Path, cfg: &Path, policy: Option<SymmetryLiveness>, seed: i64) -> Value {
    let report = check_file(&CheckRequest {
        config: Some(cfg.to_path_buf()),
        file: spec.to_path_buf(),
        workers: Some(1),
        seed: Some(seed),
        fp: Some(0),
        symmetry_liveness: policy,
        ..CheckRequest::default()
    })
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
        let core = run(&spec, &cfg, None, 0);
        let reference = run(&spec, &cfg, None, 0);
        assert_eq!(core["engine"], "core", "{}: {core}", spec.display());
        assert_ne!(core["verdict"], "unsupported", "{}: {core}", spec.display());
        agreed::check_eq(
            AGREED,
            "declared_symmetry_with_temporal_properties_runs_unreduced_on_the_core/0",
            &(observed(&core)),
            &(observed(&reference)),
        );
        assert_eq!(core["symmetry_liveness"]["effective"], "unreduced");
        assert_eq!(core["symmetry_liveness"]["scope"], "whole-run");
        assert_eq!(core["options"]["symmetry_liveness"], "unreduced");
    }
    // SymLiveMissed's violation, hidden by TLC's reduced graph, is found and
    // independently validated.
    let spec = probe("SymLiveMissed");
    let core = run(&spec, &spec.with_extension("cfg"), None, 0);
    assert_eq!(core["verdict"], "liveness-violation", "{core}");
    assert!(core["counterexample"]["states"].is_array());
}

#[test]
fn refused_and_tlc_symmetry_liveness_policies_still_refuse_on_the_core() {
    let spec = probe("SymLiveMissed");
    let cfg = spec.with_extension("cfg");
    for policy in [SymmetryLiveness::Refuse, SymmetryLiveness::Tlc] {
        let core = run(&spec, &cfg, Some(policy), 0);
        assert_eq!(core["verdict"], "unsupported", "{policy:?}: {core}");
        assert!(core["counterexample"].is_null());
    }
}

#[test]
fn a_startup_fixed_random_definition_checks_temporal_properties_and_replays_by_seed() {
    let spec = fixture("StartupEdges");
    let cfg = spec.with_extension("cfg");
    for seed in [0, 3, 7, -11] {
        let core = run(&spec, &cfg, None, seed);
        let reference = run(&spec, &cfg, None, seed);
        assert_eq!(core["engine"], "core", "seed {seed}: {core}");
        assert_eq!(core["verdict"], "success", "seed {seed}: {core}");
        agreed::check_eq(
            AGREED,
            "a_startup_fixed_random_definition_checks_temporal_properties_and_replays_by_seed/0",
            &(observed(&core)),
            &(observed(&reference)),
        );
        // The seed that fixed the start-up draws is reported and replays them.
        assert_eq!(core["options"]["seed"], seed);
        let again = run(&spec, &cfg, None, seed);
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
        None,
        0,
    );
    assert_eq!(core["verdict"], "unsupported", "{core}");
    assert!(core["counterexample"].is_null());
    // Draws while checking, directly, through a parameterized operator, or
    // through a configured redirect (`Op <- D`, `C <- D`) whose target's
    // body each use evaluates.
    for name in ["NextDraw", "PickDraw", "RedirectDraw", "ConstDraw"] {
        let spec = fixture(name);
        for seed in [0, 1, 5] {
            let core = run(&spec, &spec.with_extension("cfg"), None, seed);
            assert_eq!(core["verdict"], "unsupported", "{name} seed {seed}: {core}");
            assert!(core["counterexample"].is_null(), "{name} seed {seed}");
            let text = core.to_string();
            assert!(
                text.contains("random operators"),
                "{name} seed {seed}: {core}"
            );
        }
    }
}

#[test]
fn a_redirected_random_definition_is_never_read_as_its_startup_value_on_the_safety_route() {
    // Each use of `Op <- D` evaluates D's body; Core never answers with D's
    // single start-up draw, so it refuses the draws by name rather than
    // claim the invariant holds.
    let spec = fixture("RedirectInvariant");
    for seed in [0, 1, 2, 3] {
        let core = run(&spec, &spec.with_extension("cfg"), None, seed);
        assert_eq!(core["verdict"], "unsupported", "seed {seed}: {core}");
        assert!(
            core.to_string().contains("RandomElement"),
            "seed {seed}: {core}"
        );
    }
}

#[test]
fn a_lazy_startup_value_that_draws_at_each_membership_test_refuses_on_every_engine() {
    // `S` is precomputed, but as a deferred filter whose predicate each
    // membership test evaluates: it draws while checking, so it is not fixed
    // at start-up and no seed may report the temporal property as proved.
    let spec = fixture("LazyDraw");
    let cfg = spec.with_extension("cfg");
    for seed in 0..10 {
        let core = run(&spec, &cfg, None, seed);
        assert_eq!(core["verdict"], "unsupported", "seed {seed}: {core}");
        assert!(core["counterexample"].is_null(), "seed {seed}");
        assert!(
            core.to_string().contains("RandomElement"),
            "seed {seed}: {core}"
        );
    }
}

fn run_at(spec: &Path, cfg: &Path, seed: i64, workers: usize) -> (Value, String) {
    let report = check_file(&CheckRequest {
        config: Some(cfg.to_path_buf()),
        file: spec.to_path_buf(),
        workers: Some(workers),
        seed: Some(seed),
        fp: Some(0),
        ..CheckRequest::default()
    })
    .expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    (j, report.render_human())
}

#[test]
fn effectful_temporal_captures_run_with_one_worker_at_any_requested_width() {
    // SpanTreeRandom's class: a constant drawn at start-up (D-002). MCKVsnap's
    // class: declared SYMMETRY, run unreduced (D-005), with an unused
    // `Print` definition. Both used to refuse above one worker; they now
    // capture with one worker and give exactly the one-worker result.
    let dir = std::env::temp_dir().join(format!("dive-effect-width-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let p4 = repo().join("conformance/corpus/probes/specifications/P4SymLive/P4SymLive");
    let source = std::fs::read_to_string(p4.with_extension("tla")).expect("probe");
    let unused = dir.join("P4SymLive.tla");
    std::fs::write(
        &unused,
        source.replace("====", "Test(v) == Print(v, FALSE)\n===="),
    )
    .expect("write probe");
    std::fs::copy(p4.with_extension("cfg"), dir.join("P4SymLive.cfg")).expect("cfg");
    let edges = fixture("StartupEdges");
    for (spec, seeds) in [(edges, &[0_i64, 7][..]), (unused, &[0][..])] {
        let cfg = spec.with_extension("cfg");
        for &seed in seeds {
            let (one, _) = run_at(&spec, &cfg, seed, 1);
            assert_eq!(one["engine"], "core", "{one}");
            assert_eq!(one["verdict"], "success", "{one}");
            assert_eq!(one["options"]["workers"]["used"], 1);
            assert!(one["options"]["workers"]["reason"].is_null(), "{one}");
            for workers in [4, 8] {
                let (wide, human) = run_at(&spec, &cfg, seed, workers);
                assert_eq!(observed(&wide), observed(&one), "{workers}: {wide}");
                assert_eq!(wide["symmetry_liveness"], one["symmetry_liveness"]);
                let w = &wide["options"]["workers"];
                assert_eq!(w["requested"], workers);
                assert_eq!(w["used"], 1);
                assert_eq!(w["sequential"], false);
                let reason = w["reason"].as_str().expect("one-worker reason");
                assert!(reason.contains("with one worker"), "{reason}");
                let mut lines = human.lines();
                let header = lines.next().unwrap_or_default();
                assert!(
                    header.ends_with(&format!("{workers} worker(s) requested, 1 used")),
                    "{human}"
                );
                assert!(lines.any(|line| line == reason), "{human}");
            }
        }
    }
    // What one worker refuses still refuses at every width.
    let edges = fixture("StartupEdges");
    let violated = edges.with_file_name("StartupEdgesViolated.cfg");
    for workers in [1, 4] {
        let (core, _) = run_at(&edges, &violated, 0, workers);
        assert_eq!(core["verdict"], "unsupported", "{core}");
        assert!(core["counterexample"].is_null());
    }
    let _ = std::fs::remove_dir_all(&dir);
}
