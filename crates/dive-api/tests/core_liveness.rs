//! Temporal properties on the core engine report the result Core and the
//! retired Reference engine agreed on (`support/agreed.rs`), at every worker
//! count and in both profiles (`docs/developer/core/11-liveness.md` §8), and
//! what the core does not run is a named refusal.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
#[path = "support/refused.rs"]
mod refused;
const AGREED: &str = include_str!("fixtures/agreed/core_liveness.json");

use std::path::PathBuf;

use dive_api::{CheckRequest, PeriodicSchedule, check_file, check_file_core_opts};
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-liveness")
        .join(format!("{name}.tla"))
}

fn run(name: &str, workers: usize, tlc: bool, every_level: bool) -> Value {
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
    check_file(&request).expect("check runs").to_json()
}

#[test]
fn capture_budget_stop_retains_actual_periodic_checks() {
    for (tlc, min_states) in [(false, 2), (false, u64::MAX), (true, 2)] {
        for workers in [1, 4] {
            let file = fixture("CaptureStop");
            let report = check_file(&CheckRequest {
                config: Some(file.with_extension("cfg")),
                file,
                workers: Some(workers),
                seed: Some(0),
                fp: Some(0),
                compat_tlc: tlc,
                periodic: PeriodicSchedule {
                    min_states,
                    growth_factor: u64::MAX,
                },
                time_limit: dive_api::BudgetSetting::Explicit(std::time::Duration::from_millis(
                    500,
                )),
                max_memory: dive_api::BudgetSetting::Unlimited,
                ..CheckRequest::default()
            })
            .expect("budgeted native capture")
            .to_json();
            assert_eq!(report["engine"], "core", "{report:#}");
            assert_eq!(report["verdict"], "stopped", "{report:#}");
            assert_eq!(report["stopped"]["phase"], "graph-capture", "{report:#}");
            assert_eq!(report["execution"]["capture_completed"], false);
            assert!(report["statistics"]["distinct"].as_u64() >= Some(2));
            assert!(report["counterexample"].is_null());
            assert!(report["tlc"].is_null());
            assert_eq!(report["options"]["workers"]["requested"], workers);
            assert_eq!(report["options"]["workers"]["used"], workers);
            if tlc {
                assert_eq!(report["options"]["lncheck"], "final");
                assert!(report["periodic"].is_null());
            } else {
                assert_eq!(report["options"]["lncheck"], "deterministic");
                assert_eq!(report["periodic"]["min_states"], min_states);
                assert_eq!(report["periodic"]["growth_factor"], u64::MAX);
                if min_states == 2 {
                    assert_eq!(report["periodic"]["prefix_checks"], 1);
                    assert_eq!(report["periodic"]["last_size"], 2);
                    assert_eq!(report["periodic"]["last_level"], 1);
                } else {
                    assert_eq!(report["periodic"]["prefix_checks"], 0);
                    assert_eq!(report["periodic"]["last_size"], 0);
                    assert!(report["periodic"]["last_level"].is_null());
                }
                assert_eq!(report["periodic"]["exhausted"], false);
                assert_eq!(report["periodic"]["final_check"], false);
            }
        }
    }
}

/// Assert the intentionally different native capacity policy while every other
/// configuration and observable remains in the reference comparison.
fn native_expected(mut v: Value) -> Value {
    // Ordinary native files intentionally replace inherited count
    // bounds with machine capacity. Assert every changed field,
    // keeping all planning and reference settings compared.
    if v["options"]["temporal_bounds"].is_object() {
        let bounds = &mut v["options"]["temporal_bounds"];
        for field in [
            "states",
            "edges",
            "product_vertices",
            "product_edges",
            "product_steps",
            "acceptance_buffer_bytes",
            "acceptance_materialization_work",
        ] {
            bounds[field] = Value::from(usize::MAX);
        }
        bounds["generated"] = Value::from(u64::MAX);
        bounds["successors_per_unit"] = Value::from(u64::MAX);
        bounds["cell_limits"]["attribution_rows"]["cells"] = Value::from(usize::MAX);
        bounds["cell_limits"]["word_truth"]["cells"] = Value::from(usize::MAX);
        bounds["acceptance_policy"]["bytes"] = Value::from(usize::MAX);
        bounds["acceptance_policy"]["materialization_work"] = Value::from(usize::MAX);
    }
    comparable(v)
}

/// The report without what legitimately differs: timings, memory samples,
/// the engine's own fields, and the retired Reference engine's fingerprint
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
    for name in [
        "Fair",
        "Holds",
        "Wide",
        "Safety",
        "FairUpdate",
        "HoldsUpdate",
    ] {
        for tlc in [false, true] {
            for every_level in [false, true] {
                let reference = run(name, 1, tlc, every_level);
                for workers in [1, 2, 3, 8] {
                    let core = run(name, workers, tlc, every_level);
                    assert_eq!(core["engine"], "core", "{name}: {core:#}");
                    let what = format!("{name} tlc={tlc} every={every_level} w={workers}");
                    let mut expected = native_expected(reference.clone());
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
    let verdict = |name, every| run(name, 1, false, every)["verdict"].clone();
    assert_eq!(verdict("Fair", false), "liveness-violation");
    assert_eq!(verdict("Holds", false), "success");
    assert_eq!(verdict("Safety", false), "invariant-violation");
    let wide = run("Wide", 1, false, true);
    assert_eq!(wide["verdict"], "liveness-violation");
    // Stopped on a prefix: the frontier was not exhausted.
    assert!(
        wide["statistics"]["left_on_queue"].as_u64() > Some(0),
        "{wide:#}"
    );
}

#[test]
fn what_the_core_refuses_is_a_named_refusal_on_the_default_engine() {
    let core = run("Viewed", 1, false, false);
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
    let default = run("Viewed", 1, false, false);
    assert_eq!(default["engine"], "core", "{default:#}");
    assert_eq!(default["verdict"], "unsupported", "{default:#}");
    assert_eq!(default["message"], core["message"], "{default:#}");
    assert!(default.get("engine_fallback").is_none(), "{default:#}");
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
            let reference = check_file(&request).expect("reference check").to_json();
            assert_eq!(reference["verdict"], "success", "{reference:#}");
            for workers in [1, 2, 3, 8] {
                let core = check_file(&CheckRequest {
                    workers: Some(workers),
                    ..request.clone()
                })
                .expect("core check")
                .to_json();
                assert_eq!(core["engine"], "core");
                assert_eq!(
                    core["verdict"], "success",
                    "{property} tlc={tlc} w={workers}: {core:#}"
                );
                let got = comparable(core);
                let mut expected = native_expected(reference.clone());
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
                let reference = check_file(&request).expect("reference").to_json();
                agreed::check_eq(
                    AGREED,
                    "class_controls/0",
                    &(expected),
                    &(reference["verdict"]),
                );
                if tlc && literal {
                    // The retired reference engine named the mixed-shape record
                    // comparison TLC fails on. Core's temporal path refuses at
                    // the failure report instead (#910): still a refusal, but
                    // not yet the comparison's name.
                    assert!(
                        reference["message"]
                            .as_str()
                            .expect("message")
                            .contains("a temporal failure (core): its report is not implemented"),
                        "{reference}"
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
                let default = dive_api::check_file(&request)
                    .expect("default engine")
                    .to_json();
                assert_eq!(
                    default["verdict"], expected,
                    "{case} tlc={tlc} workers={workers}: {default:#}"
                );
                assert_eq!(default["engine"], "core");
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
    // The retired reference engine reported TLC's evaluation error here
    // (#791, #910); Core refuses the comparison by name.
    let core = check_file(&request).expect("core");
    refused::assert_refused(&core.to_json(), core.exit_code(), refused::MIXED_SHAPES);
    let default = dive_api::check_file(&request)
        .expect("default engine")
        .to_json();
    assert_eq!(default["verdict"], "unsupported");
    assert_eq!(default["engine"], "core");
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
                let reference = check_file(&request).expect("reference").to_json();
                agreed::check_eq(
                    AGREED,
                    "captured_source_error_endings_replay_the_canonical_frontier_identity/0",
                    &(verdict),
                    &(reference["verdict"]),
                );
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
                let default = dive_api::check_file(&request)
                    .expect("default engine")
                    .to_json();
                assert_error_ending(&default, &reference, case, tlc, workers, "default");
                assert_eq!(default["engine"], "core");
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

#[test]
fn bounded_flat_cache_reuses_records_without_changing_verified_reports() {
    for tlc in [false, true] {
        let reference = run("FlatCache", 1, tlc, false);
        let core = run("FlatCache", 1, tlc, false);
        assert_eq!(core["verdict"], "liveness-violation");
        assert_eq!(core["property"], "Q");
        assert_eq!(core["counterexample"]["validation"], "valid");
        assert_eq!(core["statistics"]["distinct"], 1101);
        let cache = &core["core"]["metrics"]["liveness"]["values"];
        assert!(cache["cache_hits"].as_u64() > Some(0), "{cache}");
        assert!(cache["cache_direct_hits"].as_u64() > Some(0), "{cache}");
        assert_eq!(core["core"]["metrics"]["liveness"]["retained_rows"], 1101);
        assert!(cache["cache_eligible_misses"].as_u64() > Some(0));
        assert_eq!(cache["cache_uncovered_misses"], 0);
        assert!(cache["cache_bytes"].as_u64() <= cache["cache_limit_bytes"].as_u64());
        agreed::check_eq(
            AGREED,
            "bounded_flat_cache_reuses_records_without_changing_verified_reports/0",
            &(comparable(core)),
            &(native_expected(reference)),
        );
    }
}

#[test]
fn closed_function_reads_keep_nested_reports_and_do_not_reuse_admission() {
    for tlc in [false, true] {
        for workers in [1, 8] {
            for (config, verdict) in [
                ("ClosedFunctionsHolds", "success"),
                ("ClosedFunctionsViolation", "liveness-violation"),
            ] {
                let file = fixture("ClosedFunctions");
                let request = CheckRequest {
                    config: Some(file.with_file_name(format!("{config}.cfg"))),
                    file,
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    compat_tlc: tlc,
                    ..CheckRequest::default()
                };
                let reference = check_file(&request).expect("reference").to_json();
                let core = check_file(&request).expect("core").to_json();
                assert_eq!(core["verdict"], verdict);
                assert_eq!(core["engine"], "core");
                assert_eq!(core["statistics"]["distinct"], 1101);
                if verdict == "liveness-violation" {
                    assert_eq!(core["counterexample"]["validation"], "valid");
                }
                let phases = &core["core"]["metrics"]["liveness"]["analysis_ms"];
                assert!(phases["atoms"].is_number() && phases["product"].is_number());
                assert_eq!(
                    phases["attribution"].is_number(),
                    verdict == "liveness-violation"
                );
                let atoms = &core["core"]["metrics"]["liveness"]["atoms"];
                assert!(atoms["eligible_atoms"].as_u64() > Some(0), "{atoms}");
                assert!(atoms["evaluated"].as_u64() > Some(0), "{atoms}");
                let cache = &core["core"]["metrics"]["liveness"]["values"];
                assert!(cache["cache_hits"].as_u64() > Some(0), "{cache}");
                assert_eq!(
                    cache["cache_uncovered_misses"], 0,
                    "all EWD-shaped closed functions are eligible"
                );
                assert!(cache["cache_bytes"].as_u64() <= cache["cache_limit_bytes"].as_u64());
                // Admission proves these closed integer/string-keyed functions
                // convertible without building reference values (#845).
                assert!(
                    cache["skipped_admission_conversions"].as_u64() > Some(0),
                    "{cache}"
                );
                assert!(
                    cache["skipped_admission_conversions"].as_u64()
                        <= cache["admitted_rows"].as_u64(),
                    "{cache}"
                );
                agreed::check_eq(
                    AGREED,
                    "closed_function_reads_keep_nested_reports_and_do_not_reuse_admission/0",
                    &(comparable(core)),
                    &(native_expected(reference)),
                );
            }
        }
    }
}

#[test]
fn concrete_set_read_cache_preserves_holds_violations_and_independent_words() {
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    for tlc in [false, true] {
        for (config, verdict) in [
            ("ConcreteSetsHolds", "success"),
            ("ConcreteSetsViolation", "liveness-violation"),
        ] {
            let file = fixture("ConcreteSets");
            let request = CheckRequest {
                config: Some(file.with_file_name(format!("{config}.cfg"))),
                file,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                compat_tlc: tlc,
                ..CheckRequest::default()
            };
            let reference = check_file(&request).expect("reference").to_json();
            let core = check_file(&request).expect("core").to_json();
            assert_eq!(core["verdict"], verdict);
            assert_eq!(core["engine"], "core");
            assert_eq!(core["statistics"]["distinct"], 1101);
            // `H`, `P` and the CHOOSE atom of `C` run natively on the
            // set-valued rows (ADR 0154); the `ToString` atom of `S` is
            // refused, so its reference demands read rows through the cache.
            let atoms = &core["core"]["metrics"]["liveness"]["atoms"];
            assert!(atoms["eligible_atoms"].as_u64() >= Some(2), "{atoms}");
            // Each constant-on-these-rows predicate runs once and answers the
            // other 1,100 demands from its exact projection. The violating
            // configuration also has one row-dependent atom per state.
            assert_eq!(atoms["projected_truth"]["state_hits"], 2 * 1100, "{atoms}");
            let extra = if verdict == "liveness-violation" {
                1101
            } else {
                0
            };
            assert_eq!(atoms["evaluated"], 2 + extra, "{atoms}");
            assert_eq!(atoms["reference_evaluations"]["state"], 1101, "{atoms}");
            assert_eq!(atoms["fallback"], 0, "{atoms}");
            assert_eq!(atoms["rejections"]["fresh_string"], 1, "{atoms}");
            let cache = &core["core"]["metrics"]["liveness"]["values"];
            assert!(cache["cache_hits"].as_u64() > Some(0), "{cache}");
            assert_eq!(
                cache["cache_uncovered_misses"], 0,
                "closed materialized sets are eligible"
            );
            assert!(cache["cache_bytes"].as_u64() <= cache["cache_limit_bytes"].as_u64());
            // Fresh `ToString` names are converted at their first admission;
            // later admissions of proved rows convert nothing (#845).
            assert!(cache["converted_values"].as_u64() > Some(0), "{cache}");
            assert!(
                cache["skipped_admission_conversions"].as_u64() > Some(0),
                "{cache}"
            );
            if verdict == "liveness-violation" {
                assert_eq!(core["counterexample"]["validation"], "valid");
                let validator = SpecModel::load(
                    &request.file,
                    request.config.as_ref().expect("cfg"),
                    &TraceOptions {
                        compat_tlc: tlc,
                        run: Some((0, 0)),
                        ..TraceOptions::default()
                    },
                )
                .expect("independent validator");
                let doc = &core["counterexample"]["trace"];
                assert_eq!(
                    validator
                        .validate(&TraceDoc::from_json(doc).expect("word"))
                        .validation,
                    Validation::Valid
                );
                let mut forged = doc.clone();
                forged["states"][0]["values"]["counter"] = serde_json::json!(1);
                assert!(matches!(
                    validator
                        .validate(&TraceDoc::from_json(&forged).expect("changed initial"))
                        .validation,
                    Validation::Invalid(_)
                ));
            }
            agreed::check_eq(
                AGREED,
                "concrete_set_read_cache_preserves_holds_violations_and_independent_words/0",
                &(comparable(core)),
                &(native_expected(reference)),
            );
        }
    }
}

#[test]
fn fixed_native_names_admit_pure_tuple_calls_but_refuse_new_name_demands() {
    for tlc in [false, true] {
        let reference = run("FixedNames/TupleSubSeq", 1, tlc, false);
        assert_eq!(reference["statistics"]["distinct"], 3);
        assert_eq!(reference["statistics"]["generated"], 4);
        assert_eq!(reference["statistics"]["depth"], 3);
        for workers in [1, 8] {
            let actual = run("FixedNames/TupleSubSeq", workers, tlc, false);
            assert_eq!(actual["engine"], "core");
            assert_eq!(actual["options"]["workers"]["used"], workers);
            for field in [
                "verdict",
                "statistics",
                "tlc",
                "property",
                "exit_code",
                "counterexample",
            ] {
                agreed::check_eq(
                    AGREED,
                    "fixed_native_names_admit_pure_tuple_calls_but_refuse_new_name_demands/0",
                    &(actual[field]),
                    &(reference[field]),
                );
            }
            let producer = run("FixedNames/NameProducer", workers, tlc, false);
            assert_eq!(producer["verdict"], "unsupported", "{producer}");
            assert!(producer["counterexample"].is_null());
            if tlc {
                assert!(
                    producer["message"]
                        .as_str()
                        .expect("reason")
                        .contains("after start-up")
                );
            }
            for every in [false, true] {
                let atom = run("FixedNames/NameAtom", workers, tlc, every);
                if tlc {
                    assert_eq!(atom["verdict"], "unsupported", "{atom}");
                    assert!(
                        atom["message"]
                            .as_str()
                            .expect("reason")
                            .contains("outside the proved startup domain")
                    );
                    assert!(atom["counterexample"].is_null());
                    assert!(atom["tlc"].is_null());
                } else {
                    assert_eq!(atom["verdict"], "success", "{atom}");
                }
            }
        }
    }
}

#[test]
fn native_predicate_lane_consumes_certified_scalar_truth() {
    for tlc in [false, true] {
        for name in ["Holds", "Wide"] {
            let core = run(name, 1, tlc, false);
            let reference = run(name, 1, tlc, false);
            agreed::check_eq(
                AGREED,
                "native_predicate_lane_consumes_certified_scalar_truth/0",
                &(core["verdict"]),
                &(reference["verdict"]),
            );
            let atoms = &core["core"]["metrics"]["liveness"]["atoms"];
            assert!(
                atoms["eligible_atoms"]
                    .as_u64()
                    .is_some_and(|count| count > 0),
                "{core:#}"
            );
            assert!(
                atoms["evaluated"].as_u64().is_some_and(|count| count > 0),
                "{core:#}"
            );
        }
    }
}

#[test]
fn native_fairness_words_validate_independently_and_reject_changed_initial_functions() {
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    for tlc in [false, true] {
        let file = fixture("FairUpdate");
        let validator = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("fresh independent validator");
        for workers in [1, 4] {
            let report = run("FairUpdate", workers, tlc, false);
            assert_eq!(report["verdict"], "liveness-violation");
            assert!(
                report["core"]["metrics"]["liveness"]["atoms"]["enabled_evaluations"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
            );
            assert!(
                report["core"]["metrics"]["liveness"]["atoms"]["nonstuttering_evaluations"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
            );
            let proofs = &report["core"]["metrics"]["liveness"]["atoms"]["row_proofs"];
            assert!(proofs["scans"].as_u64() > Some(0), "{proofs}");
            assert!(proofs["hits"].as_u64() > Some(0), "{proofs}");
            assert!(proofs["certified"].as_u64() > Some(0), "{proofs}");
            assert!(
                proofs["peak_storage_bytes"]
                    .as_u64()
                    .is_some_and(|bytes| bytes > 0 && bytes <= 1024 * 1024),
                "{proofs}"
            );
            let reuse = &report["core"]["metrics"]["liveness"]["atoms"]["fairness_reuse"];
            assert_eq!(reuse["stored_columns"], 2, "{reuse}");
            assert!(reuse["state_consumptions"].as_u64() > Some(0), "{reuse}");
            assert!(reuse["action_consumptions"].as_u64() > Some(0), "{reuse}");
            assert!(
                reuse["peak_storage_bytes"]
                    .as_u64()
                    .is_some_and(|n| n > 0 && n <= 16 * 1024 * 1024)
            );
            let doc = &report["counterexample"]["trace"];
            let word = TraceDoc::from_json(doc).expect("trace");
            assert!(matches!(
                validator.validate(&word).validation,
                Validation::Valid
            ));
            let mut wrong = doc.clone();
            wrong["states"][0]["values"]["x"] = serde_json::json!(42);
            let wrong = TraceDoc::from_json(&wrong).expect("well-formed mutation");
            assert!(matches!(
                validator.validate(&wrong).validation,
                Validation::Invalid(_)
            ));
        }
    }
}

fn probe(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs")
        .join(name)
        .join(format!("{name}.tla"))
}

/// The observables both stores must share.
fn store_observables(v: &Value) -> Value {
    serde_json::json!({
        "verdict": v["verdict"],
        "exit_code": v["exit_code"],
        "property": v["property"],
        "statistics": v["statistics"],
        "tlc": v["tlc"],
        "counterexample": v["counterexample"],
        "periodic": v["periodic"],
        "options_workers": v["options"]["workers"],
    })
}

#[test]
fn exact_and_fingerprint_stores_agree_on_temporal_checks() {
    use dive_api::Store;
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    let cases = [
        fixture("Fair"),
        fixture("Holds"),
        fixture("Wide"),
        fixture("FairUpdate"),
        probe("LvNoFair"),
        probe("LvTwoLoopsRev"),
        probe("LvWeakFair"),
        probe("LvPerProcessFair"),
    ];
    let mut violations = 0;
    for file in cases {
        let name = file
            .file_stem()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        for tlc in [false, true] {
            for workers in [1, 4] {
                let request = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file: file.clone(),
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    compat_tlc: tlc,
                    ..CheckRequest::default()
                };
                let fingerprint = check_file(&request).expect("fingerprint check");
                let exact = check_file(&CheckRequest {
                    store: Store::Exact,
                    ..request.clone()
                })
                .expect("exact check");
                let (fp_json, exact_json) = (fingerprint.to_json(), exact.to_json());
                let what = format!("{name} tlc={tlc} w={workers}");
                assert_eq!(exact_json["engine"], "core", "{what}: {exact_json:#}");
                assert_eq!(
                    exact_json["check_kind"], "temporal",
                    "{what}: {exact_json:#}"
                );
                assert_ne!(
                    exact_json["verdict"], "unsupported",
                    "{what}: {exact_json:#}"
                );
                assert_eq!(
                    store_observables(&exact_json),
                    store_observables(&fp_json),
                    "{what}"
                );
                assert_eq!(exact.exit_code(), fingerprint.exit_code(), "{what}");
                assert_eq!(exact_json["options"]["store"], "exact", "{what}");
                assert_eq!(fp_json["options"]["store"], "fingerprint", "{what}");
                assert_eq!(
                    exact_json["core"]["metrics"]["exact_states"],
                    exact_json["statistics"]["distinct"],
                    "{what}: every distinct state keeps its identity"
                );
                assert_eq!(fp_json["core"]["metrics"]["exact_states"], 0, "{what}");
                let exact_human = exact.render_human();
                let fp_human = fingerprint.render_human();
                assert!(
                    exact_human.contains("\nExact full-state storage.\n"),
                    "{what}: {exact_human}"
                );
                assert!(
                    fp_human.contains(
                        "\nFingerprint storage (64-bit); collision probability not computed.\n"
                    ),
                    "{what}: {fp_human}"
                );
                assert!(
                    !fp_human.contains("Exact full-state storage."),
                    "{fp_human}"
                );
                if exact_json["verdict"] == "liveness-violation" {
                    violations += 1;
                    let model = SpecModel::load(
                        &request.file,
                        request.config.as_ref().expect("cfg"),
                        &TraceOptions {
                            compat_tlc: tlc,
                            run: Some((0, 0)),
                            ..TraceOptions::default()
                        },
                    )
                    .expect("independent model");
                    let word = TraceDoc::from_json(&exact_json["counterexample"]["trace"])
                        .expect("exact lasso");
                    assert_eq!(model.read(&word).0.validation, Validation::Valid, "{what}");
                }
            }
        }
    }
    assert!(
        violations >= 8,
        "fairness violations are covered: {violations}"
    );
}

#[test]
fn spilled_or_reclaimed_storage_with_temporal_properties_is_refused_by_name() {
    let file = fixture("Fair");
    let base = CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        seed: Some(0),
        fp: Some(0),
        store: dive_api::Store::Exact,
        ..CheckRequest::default()
    };
    for request in [
        CheckRequest {
            frontier_memory_bytes: Some(0),
            ..base.clone()
        },
        CheckRequest {
            predecessor_memory_bytes: Some(0),
            ..base.clone()
        },
        CheckRequest {
            candidate_memory_bytes: Some(0),
            ..base.clone()
        },
    ] {
        let report = check_file(&request).expect("refusal");
        refused::assert_refused(
            &report.to_json(),
            report.exit_code(),
            refused::TEMPORAL_SPILLED_STORE,
        );
    }
    // An explicit reclamation trigger is refused by its own earlier name.
    let report = check_file(&CheckRequest {
        store_compact_bytes: Some(0),
        ..base
    })
    .expect("refusal");
    refused::assert_refused(
        &report.to_json(),
        report.exit_code(),
        "Store reclamation with runtime effects or native temporal capture",
    );
}

/// A temporal run under a state or depth ceiling.
fn ceiling_run(
    file: &std::path::Path,
    tlc: bool,
    workers: usize,
    store: dive_api::Store,
    ceilings: (Option<u64>, Option<u64>),
) -> dive_api::CheckReport {
    check_file(&CheckRequest {
        config: Some(file.with_extension("cfg")),
        file: file.to_path_buf(),
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        compat_tlc: tlc,
        store,
        max_states: ceilings.0,
        max_depth: ceilings.1,
        periodic: PeriodicSchedule {
            min_states: 2,
            growth_factor: 1,
        },
        ..CheckRequest::default()
    })
    .expect("ceiling run")
}

#[test]
fn state_and_depth_ceilings_stop_temporal_capture_without_a_verdict() {
    use dive_api::{ExitConvention, Store, exit};
    let file = fixture("CaptureStop");
    for tlc in [false, true] {
        for (resource, ceilings, limit) in [
            ("states", (Some(5), None), 5),
            ("depth", (None, Some(3)), 3),
        ] {
            let mut first: Option<Value> = None;
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4] {
                    let report = ceiling_run(&file, tlc, workers, store, ceilings);
                    let json = report.to_json();
                    let what = format!("tlc={tlc} {resource} {store:?} w={workers}");
                    assert_eq!(json["engine"], "core", "{what}: {json:#}");
                    assert_eq!(json["verdict"], "stopped", "{what}: {json:#}");
                    assert_eq!(json["stopped"]["resource"], resource, "{what}");
                    assert_eq!(json["stopped"]["limit"], limit, "{what}");
                    assert_eq!(json["stopped"]["observed"], limit + 1, "{what}");
                    assert_eq!(json["stopped"]["phase"], "graph-capture", "{what}");
                    assert_eq!(json["execution"]["capture_completed"], false, "{what}");
                    assert!(json["counterexample"].is_null(), "{what}");
                    assert!(json["tlc"].is_null(), "{what}");
                    for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                        assert_eq!(report.exit_code_for(convention), exit::STOPPED, "{what}");
                    }
                    if tlc {
                        assert!(json["periodic"].is_null(), "{what}");
                    } else {
                        assert_eq!(json["periodic"]["final_check"], false, "{what}");
                        assert_eq!(json["periodic"]["exhausted"], false, "{what}");
                        assert!(
                            json["periodic"]["prefix_checks"].as_u64() >= Some(1),
                            "{what}"
                        );
                    }
                    let observed = serde_json::json!({
                        "statistics": json["statistics"],
                        "periodic": json["periodic"],
                        "stopped": {
                            "resource": json["stopped"]["resource"],
                            "limit": json["stopped"]["limit"],
                            "observed": json["stopped"]["observed"],
                        },
                    });
                    match &first {
                        None => first = Some(observed),
                        Some(first) => assert_eq!(&observed, first, "{what}"),
                    }
                }
            }
        }
    }
}

#[test]
fn a_ceiling_at_the_model_size_keeps_the_full_temporal_verdict() {
    use dive_api::Store;
    for name in ["Fair", "Holds", "Wide"] {
        let file = fixture(name);
        for tlc in [false, true] {
            let reference = ceiling_run(&file, tlc, 1, Store::Fingerprint, (None, None)).to_json();
            assert_ne!(reference["verdict"], "stopped", "{name}: {reference:#}");
            assert_ne!(reference["verdict"], "unsupported", "{name}: {reference:#}");
            let distinct = reference["statistics"]["distinct"].as_u64();
            let depth = reference["statistics"]["depth"].as_u64();
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4] {
                    for ceilings in [(distinct, None), (None, depth), (distinct, depth)] {
                        let json = ceiling_run(&file, tlc, workers, store, ceilings).to_json();
                        let what = format!("{name} tlc={tlc} {store:?} w={workers} {ceilings:?}");
                        for field in [
                            "verdict",
                            "exit_code",
                            "property",
                            "statistics",
                            "counterexample",
                            "periodic",
                        ] {
                            assert_eq!(json[field], reference[field], "{what}: {field}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_violation_proven_before_the_ceiling_is_still_reported() {
    use dive_api::Store;
    // Checked at every completed level, Wide fails on a prefix (D-014),
    // before the search reaches the rest of its states.
    let file = fixture("Wide");
    let reference = ceiling_run(&file, false, 1, Store::Fingerprint, (None, None)).to_json();
    assert_eq!(reference["verdict"], "liveness-violation", "{reference:#}");
    assert!(reference["statistics"]["left_on_queue"].as_u64() > Some(0));
    let distinct = reference["statistics"]["distinct"].as_u64();
    let full = 60 * 71;
    assert!(distinct < Some(full), "{reference:#}");
    for store in [Store::Fingerprint, Store::Exact] {
        for workers in [1, 4] {
            for ceiling in [distinct, Some(full - 1)] {
                let json = ceiling_run(&file, false, workers, store, (ceiling, None)).to_json();
                for field in ["verdict", "property", "statistics", "counterexample"] {
                    assert_eq!(
                        json[field], reference[field],
                        "{store:?} w={workers} {ceiling:?}: {field}"
                    );
                }
            }
        }
    }
}

#[test]
fn sparse_action_pages_preserve_witness_attribution_across_empty_blocks() {
    let report = run("SparseActionPages", 1, false, false);
    assert_eq!(report["statistics"]["distinct"], 128);
    assert_eq!(report["statistics"]["generated"], 16_385);
    assert_eq!(report["verdict"], "liveness-violation");
    assert_eq!(report["property"], "Outside and Missing");
    assert_eq!(report["counterexample"]["validation"], "valid");
    assert_eq!(report["execution"]["capture_completed"], true);
}
