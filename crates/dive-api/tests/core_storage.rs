//! Exact membership and predecessor spill through the actual native safety route.
#![allow(clippy::expect_used, reason = "storage regression assertions")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_storage.json");

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, Store, check_file_engine};
use serde_json::{Value, json};

fn request(name: &str) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-m0")
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observable(document: &Value) -> Value {
    json!({
        "verdict": document["verdict"],
        "statistics": document["statistics"],
        "tlc": document["tlc"],
        "property": document["property"],
        "message": document["message"],
        "counterexample": document["counterexample"],
    })
}

#[test]
fn exact_and_spilled_predecessors_preserve_ordered_native_results() {
    for name in ["Mix", "Dead", "CompoundTrace", "Big", "Bag", "AsFalse"] {
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                let mut req = request(name);
                req.compat_tlc = compat_tlc;
                req.workers = Some(workers);
                let reference = check_file_engine(&req, Engine::Core).expect("reference");
                assert_ne!(reference.to_json()["verdict"], "unsupported", "{name}");
                for store in [Store::Fingerprint, Store::Exact] {
                    req.store = store;
                    for predecessor_memory_bytes in [None, Some(0), Some(64)] {
                        req.predecessor_memory_bytes = predecessor_memory_bytes;
                        let result = check_file_engine(&req, Engine::Core).expect("native check");
                        let document = result.to_json();
                        assert_eq!(document["engine"], "core", "{name}: {document:#}");
                        agreed::check_eq(
                            AGREED,
                            "exact_and_spilled_predecessors_preserve_ordered_native_results/0",
                            &(observable(&document)),
                            &(observable(&reference.to_json())),
                        );
                        agreed::check_eq(
                            AGREED,
                            "exact_and_spilled_predecessors_preserve_ordered_native_results/1",
                            &(result.exit_code_for(ExitConvention::Tlc)),
                            &(reference.exit_code_for(ExitConvention::Tlc)),
                        );
                        if let Some(budget) = predecessor_memory_bytes {
                            if name == "AsFalse" {
                                // The false assumption ends binding before a
                                // search or storage owner is created.
                                assert_eq!(
                                    document["predecessor_storage"]["cleanup_completed"],
                                    false
                                );
                                assert_eq!(document["predecessor_storage"]["committed_records"], 0);
                                continue;
                            }
                            assert_eq!(document["predecessor_storage"]["cleanup_completed"], true);
                            assert_eq!(document["predecessor_storage"]["failures"], json!([]));
                            assert!(
                                document["predecessor_storage"]["peak_retained_bytes"]
                                    .as_u64()
                                    .expect("retained bytes")
                                    <= budget as u64
                            );
                        }
                        if name == "CompoundTrace" {
                            let trace = TraceDoc::from_json(&document).expect("native word");
                            let model = SpecModel::load(
                                &req.file,
                                req.config.as_ref().expect("cfg"),
                                &TraceOptions {
                                    compat_tlc,
                                    run: Some((0, 0)),
                                    ..TraceOptions::default()
                                },
                            )
                            .expect("independent model");
                            assert_eq!(
                                model.try_validate(&trace).expect("replay").validation,
                                Validation::Valid
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn predecessor_write_failure_cannot_certify_a_native_result() {
    let mut req = request("Mix");
    req.predecessor_memory_bytes = Some(0);
    req.predecessor_directory = Some(req.file.clone());
    for store in [Store::Fingerprint, Store::Exact] {
        req.store = store;
        let report = check_file_engine(&req, Engine::Core).expect("storage result");
        let document = report.to_json();
        assert_eq!(document["verdict"], "resource-limit");
        assert!(document["counterexample"].is_null());
        assert_ne!(
            document["predecessor_storage"]["failures"]
                .as_array()
                .expect("failures")
                .as_slice(),
            [] as [Value; 0]
        );
        assert_ne!(report.exit_code_for(ExitConvention::Tlc), 0);
    }
}

#[test]
fn exact_keys_preserve_view_symmetry_and_diagnostic_replay_membership() {
    for relative in [
        "tests/fixtures/core-m1/ViewMerge.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "tests/fixtures/core-m1/SymSets.tla",
        "../dive-check/tests/fixtures/errors/specs/ErInvReplayViolation/ErInvReplayViolation.tla",
        "../dive-check/tests/fixtures/errors/specs/ErInvReplayError/ErInvReplayError.tla",
        "../dive-check/tests/fixtures/errors/specs/ErInvConstrained/ErInvConstrained.tla",
    ] {
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
                let mut req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file,
                    compat_tlc,
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let reference = check_file_engine(&req, Engine::Core).expect("reference");
                assert_ne!(reference.to_json()["verdict"], "unsupported", "{relative}");
                req.store = Store::Exact;
                req.predecessor_memory_bytes = Some(0);
                let actual = check_file_engine(&req, Engine::Core).expect("exact native");
                agreed::check_eq(
                    AGREED,
                    "exact_keys_preserve_view_symmetry_and_diagnostic_replay_membership/0",
                    &(observable(&actual.to_json())),
                    &(observable(&reference.to_json())),
                );
                agreed::check_eq(
                    AGREED,
                    "exact_keys_preserve_view_symmetry_and_diagnostic_replay_membership/1",
                    &(actual.exit_code_for(ExitConvention::Tlc)),
                    &(reference.exit_code_for(ExitConvention::Tlc)),
                );
            }
        }
    }
}
