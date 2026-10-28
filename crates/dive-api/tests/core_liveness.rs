//! Temporal properties on the core engine (`--engine=core`) report what the
//! reference engine reports, at every worker count and in both profiles
//! (`docs/developer/core/11-liveness.md` §8), and what the core does not
//! run falls back under `--engine=auto`.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, PeriodicSchedule, check_file_engine};
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-liveness")
        .join(format!("{name}.tla"))
}

fn run(name: &str, engine: Engine, workers: usize, tlc: bool, every_level: bool) -> Value {
    let file = fixture(name);
    let mut request = CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        compat_tlc: tlc,
        ..CheckRequest::default()
    };
    if every_level {
        request.periodic = PeriodicSchedule {
            min_states: 2,
            growth_factor: 1,
        };
    }
    check_file_engine(&request, engine)
        .expect("check runs")
        .to_json()
}

/// The report without what legitimately differs: timings, memory samples,
/// the engine's own fields, and the reference engine's fingerprint
/// collision estimate, which the core does not report (on safety runs
/// either).
fn comparable(mut v: Value) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(map) => {
                map.retain(|k, _| {
                    !(k.contains("elapsed")
                        || k.ends_with("_ms")
                        || k.contains("memory")
                        || k == "time"
                        || k == "engine"
                        || k == "core"
                        || k == "collision")
                });
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut v);
    v
}

#[test]
fn temporal_reports_on_the_core_are_the_reference_reports() {
    for name in ["Fair", "Holds", "Wide", "Safety"] {
        for tlc in [false, true] {
            for every_level in [false, true] {
                let reference = run(name, Engine::Reference, 1, tlc, every_level);
                for workers in [1, 2, 3, 8] {
                    let core = run(name, Engine::Core, workers, tlc, every_level);
                    assert_eq!(core["engine"], "core", "{name}: {core:#}");
                    let what = format!("{name} tlc={tlc} every={every_level} w={workers}");
                    let mut expected = comparable(reference.clone());
                    let got = comparable(core);
                    // The worker request is the run's own.
                    expected["options"]["workers"] = got["options"]["workers"].clone();
                    if let Some(w) = got.get("workers") {
                        expected["workers"] = w.clone();
                    }
                    assert_eq!(got, expected, "{what}");
                }
            }
        }
    }
}

#[test]
fn every_kind_of_ending_is_covered() {
    let verdict = |name, every| run(name, Engine::Core, 1, false, every)["verdict"].clone();
    assert_eq!(verdict("Fair", false), "liveness-violation");
    assert_eq!(verdict("Holds", false), "success");
    assert_eq!(verdict("Safety", false), "invariant-violation");
    let wide = run("Wide", Engine::Core, 1, false, true);
    assert_eq!(wide["verdict"], "liveness-violation");
    // Stopped on a prefix: the frontier was not exhausted.
    assert!(
        wide["statistics"]["left_on_queue"].as_u64() > Some(0),
        "{wide:#}"
    );
}

#[test]
fn what_the_core_refuses_falls_back_under_auto() {
    let core = run("Viewed", Engine::Core, 1, false, false);
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
    let auto = run("Viewed", Engine::Auto, 1, false, false);
    assert_eq!(auto["engine"], "reference", "{auto:#}");
    assert_eq!(auto["engine_fallback"]["stage"], "static", "{auto:#}");
    let reference = run("Viewed", Engine::Reference, 1, false, false);
    assert_eq!(auto["verdict"], reference["verdict"]);
}
