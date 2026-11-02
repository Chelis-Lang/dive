//! Explicit Core disk membership preserves ordinary safety observables.
#![allow(
    clippy::expect_used,
    reason = "native disk storage regression assertions"
)]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, SeenOptions, Store, check_file_engine};
use serde_json::{Value, json};

fn request(relative: &str, tlc: bool, store: Store, workers: usize) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc: tlc,
        store,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(doc: &Value) -> Value {
    json!({"verdict":doc["verdict"],"statistics":doc["statistics"],"property":doc["property"],"tlc":doc["tlc"],"counterexample":doc["counterexample"]})
}

#[test]
fn disk_matches_core_identity_constraints_views_symmetry_and_pure_error_replay() {
    for relative in [
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/core-m0/Bag.tla",
        "tests/fixtures/core-m0/Dead.tla",
        "tests/fixtures/core-m1/ViewMerge.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "tests/fixtures/core-m1/SymSets.tla",
        "../dive-check/tests/fixtures/errors/specs/ErInvReplayError/ErInvReplayError.tla",
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
    ] {
        for tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                let mut req = request(relative, tlc, store, 1);
                let baseline = check_file_engine(&req, Engine::Core).expect("resident Core");
                assert!(
                    !matches!(
                        baseline.to_json()["verdict"].as_str(),
                        Some("unsupported" | "internal-error")
                    ),
                    "{relative}: {}",
                    baseline.to_json()
                );
                for workers in [1, 8] {
                    req.workers = Some(workers);
                    req.seen_disk = Some(SeenOptions::default());
                    req.frontier_memory_bytes = Some(0);
                    req.predecessor_memory_bytes = Some(0);
                    let actual = check_file_engine(&req, Engine::Core).expect("disk Core");
                    let doc = actual.to_json();
                    assert_eq!(
                        observed(&doc),
                        observed(&baseline.to_json()),
                        "{relative}/{tlc}/{store:?}/{workers}"
                    );
                    assert_eq!(actual.exit_code(), baseline.exit_code());
                    assert_eq!(doc["engine"], "core");
                    assert_eq!(doc["options"]["workers"]["requested"], workers);
                    assert_eq!(doc["options"]["workers"]["used"], 1);
                    assert_eq!(
                        doc["seen_storage"]["after_cleanup"]["cleanup_completed"],
                        true
                    );
                    assert_eq!(
                        doc["seen_storage"]["before_cleanup"]["resident_key_collection_bytes"],
                        0
                    );
                    assert!(doc["seen_storage"]["first_failure"].is_null());
                    if relative.ends_with("CompoundTrace.tla") {
                        assert_eq!(doc["statistics"]["generated"], 3);
                        assert_eq!(doc["statistics"]["distinct"], 3);
                        let model = SpecModel::load(
                            &req.file,
                            req.config.as_ref().expect("cfg"),
                            &TraceOptions {
                                compat_tlc: tlc,
                                run: Some((0, 0)),
                                ..TraceOptions::default()
                            },
                        )
                        .expect("fresh independent model");
                        let trace = TraceDoc::from_json(&doc).expect("actual word");
                        assert_eq!(
                            model
                                .try_validate(&trace)
                                .expect("independent replay")
                                .validation,
                            Validation::Valid
                        );
                        if tlc && store == Store::Exact && workers == 1 {
                            let controls = model.controls(&trace);
                            assert!(controls.positive_ok());
                            assert_ne!(
                                controls.mutants.len(),
                                0,
                                "negative controls must be selected"
                            );
                            assert!(controls.failures().is_empty(), "{:?}", controls.failures());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn disk_creation_and_entry_failure_are_resource_stops_without_certificates() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = request("tests/fixtures/core-m0/CompoundTrace.tla", tlc, store, 8);
            req.seen_disk = Some(SeenOptions::default());
            req.seen_disk.as_mut().expect("policy").directory = Some(req.file.clone());
            let doc = check_file_engine(&req, Engine::Core)
                .expect("creation stop")
                .to_json();
            assert_eq!(doc["verdict"], "resource-limit");
            assert_eq!(doc["seen_storage"]["first_failure"]["phase"], "Creation");
            assert_eq!(doc["options"]["workers"]["used"], 0);
            assert!(doc["counterexample"].is_null());
            req.seen_disk.as_mut().expect("policy").directory = None;
            req.seen_disk.as_mut().expect("policy").limits.entries = 1;
            let doc = check_file_engine(&req, Engine::Core)
                .expect("entry stop")
                .to_json();
            assert_eq!(doc["verdict"], "resource-limit");
            assert_eq!(doc["seen_storage"]["first_failure"]["phase"], "Insertion");
            assert_eq!(doc["statistics"]["distinct"], 1);
            assert!(doc["counterexample"].is_null());
            assert_eq!(
                doc["seen_storage"]["after_cleanup"]["cleanup_completed"],
                true
            );
        }
    }
}

#[test]
fn disk_auto_keeps_its_static_whole_reference_backend() {
    let mut req = request(
        "tests/fixtures/core-m0/CompoundTrace.tla",
        true,
        Store::Exact,
        8,
    );
    req.seen_disk = Some(SeenOptions::default());
    let doc = check_file_engine(&req, Engine::Auto)
        .expect("static disk fallback")
        .to_json();
    assert_eq!(doc["engine"], "reference");
    assert_eq!(doc["engine_fallback"]["stage"], "static");
    assert_eq!(doc["options"]["workers"]["used"], 1);
}

#[test]
fn disk_does_not_silently_ignore_configured_temporal_properties() {
    let mut req = request(
        "tests/fixtures/core-liveness/Wide.tla",
        true,
        Store::Exact,
        8,
    );
    req.seen_disk = Some(SeenOptions::default());
    let doc = check_file_engine(&req, Engine::Core)
        .expect("temporal refusal")
        .to_json();
    assert_eq!(doc["verdict"], "unsupported");
    assert!(
        doc["message"]
            .as_str()
            .expect("reason")
            .contains("disk seen-state")
    );
    assert!(doc["counterexample"].is_null());
}

#[test]
fn replay_entry_failure_retains_its_actual_ordinal_and_frozen_source_prefix() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = request(
                "../dive-check/tests/fixtures/disk-seen/ReplayNew/ReplayNew.tla",
                tlc,
                store,
                8,
            );
            req.seen_disk = Some(SeenOptions::default());
            req.seen_disk.as_mut().expect("policy").limits.entries = 3;
            let doc = check_file_engine(&req, Engine::Core)
                .expect("replay storage stop")
                .to_json();
            assert_eq!(doc["verdict"], "resource-limit");
            assert_eq!(
                doc["seen_storage"]["first_failure"]["phase"],
                "ReplayInsertion"
            );
            assert_eq!(doc["seen_storage"]["first_failure"]["successor_ordinal"], 2);
            assert_eq!(doc["seen_storage"]["first_failure"]["generated"], 2);
            assert_eq!(doc["seen_storage"]["admitted_keys"], 3);
            assert_eq!(doc["statistics"]["generated"], 2);
            assert!(doc["counterexample"].is_null());
        }
    }
}
