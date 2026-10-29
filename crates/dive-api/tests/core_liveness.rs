//! Temporal properties on the core engine (`--engine=core`) report what the
//! reference engine reports, at every worker count and in both profiles
//! (`docs/developer/core/11-liveness.md` §8), and what the core does not
//! run falls back under `--engine=auto`.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, PeriodicSchedule, check_file_core_opts, check_file_engine};
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

#[test]
fn temporal_atoms_keep_general_functions_distinct_from_record_literals() {
    let file = fixture("Representation/Representation");
    for property in ["Action", "State", "Holds"] {
        for tlc in [false, true] {
            let request = CheckRequest {
                config: Some(file.with_file_name(format!("{property}.cfg"))),
                file: file.clone(),
                seed: Some(0),
                fp: Some(0),
                compat_tlc: tlc,
                ..CheckRequest::default()
            };
            let reference = check_file_engine(&request, Engine::Reference)
                .expect("reference check")
                .to_json();
            assert_eq!(reference["verdict"], "success", "{reference:#}");
            for workers in [1, 2, 3, 8] {
                let core = check_file_engine(
                    &CheckRequest {
                        workers: Some(workers),
                        ..request.clone()
                    },
                    Engine::Core,
                )
                .expect("core check")
                .to_json();
                assert_eq!(core["engine"], "core");
                assert_eq!(
                    core["verdict"], "success",
                    "{property} tlc={tlc} w={workers}: {core:#}"
                );
                let got = comparable(core);
                let mut expected = comparable(reference.clone());
                expected["options"]["workers"] = got["options"]["workers"].clone();
                if let Some(workers) = got.get("workers") {
                    expected["workers"] = workers.clone();
                }
                assert_eq!(got, expected, "{property} tlc={tlc} w={workers}");
            }
        }
    }
}

const CLASS_SWITCHES: &[&str] = &[
    "",
    "-fold",
    "-memo",
    "-memo,-step-memo",
    "-hashcons",
    "-early-probe",
    "-fold,-memo,-step-memo,-hashcons,-early-probe",
];

fn class_controls(module: &str, cases: &[(&str, bool)]) {
    let file = fixture(module);
    for &(case, literal) in cases {
        for tlc in [false, true] {
            for workers in [1, 2, 3, 8] {
                let request = CheckRequest {
                    config: Some(file.with_file_name(format!("{case}.cfg"))),
                    file: file.clone(),
                    seed: Some(0),
                    fp: Some(0),
                    workers: Some(workers),
                    compat_tlc: tlc,
                    ..CheckRequest::default()
                };
                let expected = if tlc && literal {
                    "unsupported"
                } else {
                    "success"
                };
                let reference = check_file_engine(&request, Engine::Reference)
                    .expect("reference")
                    .to_json();
                assert_eq!(reference["verdict"], expected);
                if tlc && literal {
                    assert!(
                        reference["message"]
                            .as_str()
                            .expect("message")
                            .contains("equality of record")
                    );
                }
                for switches in CLASS_SWITCHES {
                    let core = check_file_core_opts(&request, switches)
                        .expect("core")
                        .to_json();
                    assert_eq!(
                        core["verdict"], expected,
                        "{case} tlc={tlc} workers={workers} switches={switches}: {core:#}"
                    );
                }
                let auto = check_file_engine(&request, Engine::Auto)
                    .expect("auto")
                    .to_json();
                assert_eq!(
                    auto["verdict"], expected,
                    "{case} tlc={tlc} workers={workers}: {auto:#}"
                );
                assert_eq!(
                    auto["engine"],
                    if tlc && literal { "reference" } else { "core" }
                );
            }
        }
    }
}

#[test]
fn folded_literal_and_general_classes_do_not_change_temporal_verdicts() {
    class_controls(
        "Representation/Representation",
        &[
            ("State", false),
            ("Literal", true),
            ("NestedGeneral", false),
            ("NestedLiteral", true),
        ],
    );
}

#[test]
fn later_unchanged_states_keep_the_first_discovered_receiver_class() {
    class_controls(
        "DelayedClass/DelayedClass",
        &[
            ("General", false),
            ("Literal", true),
            ("AliasGeneral", false),
            ("AliasLiteral", true),
        ],
    );
}

#[test]
fn ordinary_safety_keeps_its_existing_checked_comparison_refusal() {
    let file = fixture("DelayedClass/DelayedClass");
    let request = CheckRequest {
        config: Some(file.with_file_name("Safety.cfg")),
        file,
        compat_tlc: true,
        seed: Some(0),
        fp: Some(0),
        workers: Some(2),
        ..CheckRequest::default()
    };
    let reference = check_file_engine(&request, Engine::Reference)
        .expect("reference")
        .to_json();
    assert_eq!(reference["verdict"], "evaluation-error");
    assert!(
        reference["message"]
            .as_str()
            .expect("message")
            .contains("equality of record")
    );
    let core = check_file_engine(&request, Engine::Core)
        .expect("core")
        .to_json();
    assert_eq!(core["verdict"], "unsupported");
    let auto = check_file_engine(&request, Engine::Auto)
        .expect("auto")
        .to_json();
    assert_eq!(auto["verdict"], reference["verdict"]);
    assert_eq!(auto["engine"], "reference");
}

/// Error words reconstruct canonical state identities while source evaluation
/// retains original function receiver classes. All three `End` producers and
/// invariant-error call-stack replay must keep that boundary.
#[test]
fn captured_source_error_endings_replay_the_canonical_frontier_identity() {
    let file = fixture("GenError/GenError");
    for (case, verdict) in [
        ("Record", "evaluation-error"),
        ("Tuple", "evaluation-error"),
        ("Nested", "evaluation-error"),
        ("General", "evaluation-error"),
        ("Safety", "evaluation-error"),
        ("Initial", "evaluation-error"),
        ("Deadlock", "deadlock"),
        ("Invariant", "invariant-violation"),
        ("InvariantError", "evaluation-error"),
    ] {
        for tlc in [false, true] {
            for workers in [1, 2, 3, 8] {
                let request = CheckRequest {
                    config: Some(file.with_file_name(format!("{case}.cfg"))),
                    file: file.clone(),
                    seed: Some(0),
                    fp: Some(0),
                    workers: Some(workers),
                    compat_tlc: tlc,
                    ..CheckRequest::default()
                };
                let reference = check_file_engine(&request, Engine::Reference)
                    .expect("reference")
                    .to_json();
                assert_eq!(reference["verdict"], verdict, "{case}: {reference:#}");
                if ["Record", "Tuple", "Nested", "General", "Safety"].contains(&case) {
                    assert_eq!(reference["tlc"]["code"], 2179);
                    assert_eq!(reference["tlc"]["exit_status"], 255);
                    assert_eq!(reference["statistics"]["generated"], 2);
                    assert_eq!(reference["statistics"]["distinct"], 2);
                    assert_eq!(reference["statistics"]["depth"], 2);
                    assert_eq!(reference["counterexample"]["length"], 2);
                }
                for switches in CLASS_SWITCHES {
                    let core = check_file_core_opts(&request, switches)
                        .expect("core")
                        .to_json();
                    assert_error_ending(&core, &reference, case, tlc, workers, switches);
                    assert_eq!(core["engine"], "core");
                }
                let auto = check_file_engine(&request, Engine::Auto)
                    .expect("auto")
                    .to_json();
                assert_error_ending(&auto, &reference, case, tlc, workers, "auto");
                assert_eq!(auto["engine"], "core");
            }
        }
    }
}

fn assert_error_ending(
    actual: &Value,
    reference: &Value,
    case: &str,
    tlc: bool,
    workers: usize,
    switches: &str,
) {
    for field in [
        "verdict",
        "statistics",
        "tlc",
        "exit_code",
        "property",
        "error_site",
        "counterexample",
    ] {
        assert_eq!(
            actual[field], reference[field],
            "{case} tlc={tlc} workers={workers} switches={switches} {field}: {actual:#}"
        );
    }
}
