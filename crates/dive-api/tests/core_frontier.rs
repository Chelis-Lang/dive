//! Ordered live-Store frontier spill preserves actual native safety behavior.
#![allow(clippy::expect_used, reason = "native frontier regression assertions")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{
    CheckRequest, Engine, ExitConvention, Store, check_file_core_opts, check_file_engine,
};
use serde_json::{Value, json};

fn request(relative: &str, tlc: bool, workers: usize, store: Store) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc: tlc,
        workers: Some(workers),
        store,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(document: &Value) -> Value {
    json!({ "verdict":document["verdict"], "statistics":document["statistics"],
        "tlc":document["tlc"], "property":document["property"], "message":document["message"],
        "counterexample":document["counterexample"] })
}

#[test]
fn spilled_frontiers_preserve_both_profiles_stores_worker_widths_and_words() {
    for relative in [
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/core-m0/Bag.tla",
        "tests/fixtures/core-m0/Dead.tla",
        "tests/fixtures/core-m1/ViewMerge.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "tests/fixtures/core-m1/SymSets.tla",
        "tests/fixtures/checker/errors/specs/ErInvReplayError/ErInvReplayError.tla",
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
        "../../conformance/corpus/constant-retry-lazy/specifications/WholeFunctionError/WholeFunctionError.tla",
        "../../conformance/corpus/constant-retry-lazy/specifications/Named/Named.tla",
        "../../conformance/corpus/constant-retry-lazy/specifications/Inline/Inline.tla",
    ] {
        for tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    let mut req = request(relative, tlc, workers, store);
                    let baseline = check_file_engine(&req, Engine::Core).expect("ordinary core");
                    assert!(
                        !matches!(
                            baseline.to_json()["verdict"].as_str(),
                            Some("unsupported" | "internal-error")
                        ),
                        "{relative}: {}",
                        baseline.to_json()
                    );
                    for budget in [0, 96, 65_536] {
                        req.frontier_memory_bytes = Some(budget);
                        req.predecessor_memory_bytes = Some(0);
                        let actual = check_file_engine(&req, Engine::Core).expect("spilled native");
                        let doc = actual.to_json();
                        assert_eq!(
                            observed(&doc),
                            observed(&baseline.to_json()),
                            "{relative}, {tlc}, {store:?}, W{workers}, {budget}"
                        );
                        assert_eq!(
                            actual.exit_code_for(ExitConvention::Tlc),
                            baseline.exit_code_for(ExitConvention::Tlc)
                        );
                        assert_eq!(doc["engine"], "core");
                        // Init runs on one worker; a run that ends there
                        // starts no parallel level and generated no successor.
                        let ended_in_init = doc["options"]["workers"]["reason"]
                            .as_str()
                            .is_some_and(|r| r.contains("ended in Init"));
                        if ended_in_init {
                            assert_eq!(
                                doc["statistics"]["generated"],
                                doc["statistics"]["initial_generated"]
                            );
                        }
                        assert_eq!(
                            doc["options"]["workers"]["used"],
                            if ended_in_init { 1 } else { workers }
                        );
                        assert_eq!(doc["frontier_storage"]["failures"], json!([]));
                        assert_eq!(doc["frontier_storage"]["retained_bytes"], 0);
                        assert_eq!(doc["frontier_storage"]["write_accounting_complete"], true);
                        assert!(
                            doc["frontier_storage"]["peak_retained_bytes"]
                                .as_u64()
                                .expect("retained")
                                <= 2 * budget as u64
                        );
                        assert!(doc["frontier_storage"]["max_codec_nodes_per_value"].is_null());
                        assert_eq!(doc["predecessor_storage"]["cleanup_completed"], true);
                        if relative.ends_with("CompoundTrace.tla") {
                            let model = SpecModel::load(
                                &req.file,
                                req.config.as_ref().expect("cfg"),
                                &TraceOptions {
                                    compat_tlc: tlc,
                                    run: Some((0, 0)),
                                    ..TraceOptions::default()
                                },
                            )
                            .expect("independent model");
                            let trace = TraceDoc::from_json(&doc).expect("native word");
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
fn frontier_write_failure_never_reports_certification() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1, 4, 8] {
                let mut req = request(
                    "tests/fixtures/core-m0/CompoundTrace.tla",
                    tlc,
                    workers,
                    store,
                );
                req.frontier_memory_bytes = Some(0);
                req.frontier_directory = Some(req.file.clone());
                let actual = check_file_engine(&req, Engine::Core).expect("storage result");
                let doc = actual.to_json();
                assert_eq!(doc["verdict"], "resource-limit");
                assert!(doc["counterexample"].is_null());
                assert_eq!(doc["frontier_storage"]["write_accounting_complete"], false);
                assert_ne!(
                    doc["frontier_storage"]["failures"]
                        .as_array()
                        .expect("failures")
                        .as_slice(),
                    [] as [Value; 0]
                );
                assert_ne!(actual.exit_code_for(ExitConvention::Tlc), 0);
            }
        }
    }
}

#[test]
fn a_late_multichunk_violation_keeps_global_frontier_positions_and_predecessors() {
    for (tlc, store, workers) in [
        (false, Store::Fingerprint, 1),
        (true, Store::Exact, 4),
        (false, Store::Exact, 8),
    ] {
        let mut req = request("tests/fixtures/core-m1/WideLevel.tla", tlc, workers, store);
        let baseline = check_file_engine(&req, Engine::Core).expect("ordinary wide frontier");
        assert_eq!(baseline.to_json()["verdict"], "invariant-violation");
        req.frontier_memory_bytes = Some(0);
        req.predecessor_memory_bytes = Some(0);
        let actual = check_file_engine(&req, Engine::Core).expect("spilled wide frontier");
        assert_eq!(observed(&actual.to_json()), observed(&baseline.to_json()));
        assert_eq!(actual.to_json()["options"]["workers"]["used"], workers);
        assert_eq!(actual.to_json()["frontier_storage"]["failures"], json!([]));
    }
}

#[test]
fn spill_preserves_exact_rows_when_hashconsing_and_early_probe_are_disabled() {
    for relative in [
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
    ] {
        for tlc in [false, true] {
            for workers in [1, 4, 8] {
                let mut req = request(relative, tlc, workers, Store::Exact);
                let baseline = check_file_engine(&req, Engine::Core).expect("ordinary exact core");
                req.frontier_memory_bytes = Some(0);
                req.predecessor_memory_bytes = Some(0);
                for switches in ["-hashcons", "-hashcons,-early-probe"] {
                    let actual =
                        check_file_core_opts(&req, switches).expect("ablated native spill");
                    assert_eq!(
                        observed(&actual.to_json()),
                        observed(&baseline.to_json()),
                        "{relative}/{tlc}/{workers}/{switches}"
                    );
                }
            }
        }
    }
}
