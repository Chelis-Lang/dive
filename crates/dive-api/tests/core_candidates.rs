//! Owned candidate payload spill through actual native safety checking.
#![allow(
    clippy::expect_used,
    reason = "candidate storage regression assertions"
)]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, Store, check_file};
use serde_json::{Value, json};
use std::path::PathBuf;

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
fn observed(doc: &Value) -> Value {
    json!({"verdict":doc["verdict"], "statistics":doc["statistics"], "tlc":doc["tlc"],
        "property":doc["property"], "message":doc["message"], "counterexample":doc["counterexample"]})
}

#[test]
fn candidate_spill_preserves_profiles_stores_widths_history_and_independent_words() {
    for relative in [
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/core-m0/Bag.tla",
        "tests/fixtures/core-m0/Dead.tla",
        "tests/fixtures/core-m1/ViewMerge.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "tests/fixtures/core-m1/SymSets.tla",
        "tests/fixtures/checker/errors/specs/ErInvReplayError/ErInvReplayError.tla",
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
    ] {
        for tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    let mut req = request(relative, tlc, workers, store);
                    let baseline = check_file(&req).expect("resident Core");
                    assert!(
                        !matches!(
                            baseline.to_json()["verdict"].as_str(),
                            Some("unsupported" | "internal-error")
                        ),
                        "{relative}: {}",
                        baseline.to_json()
                    );
                    for bytes in [0, 256, 65_536] {
                        req.candidate_memory_bytes = Some(bytes);
                        req.frontier_memory_bytes = Some(0);
                        req.predecessor_memory_bytes = Some(0);
                        let actual = check_file(&req).expect("candidate check");
                        let doc = actual.to_json();
                        assert_eq!(
                            observed(&doc),
                            observed(&baseline.to_json()),
                            "{relative}, {tlc}, {store:?}, W{workers}, {bytes}"
                        );
                        assert_eq!(
                            actual.exit_code_for(ExitConvention::Tlc),
                            baseline.exit_code_for(ExitConvention::Tlc)
                        );
                        assert_eq!(doc["options"]["workers"]["used"], workers);
                        let usage = &doc["core"]["metrics"]["candidate_storage"];
                        assert_eq!(usage["memory_bytes_per_worker"], bytes);
                        assert_eq!(usage["retained_bytes"], 0);
                        assert_eq!(usage["failures"], json!([]));
                        assert!(
                            usage["peak_retained_bytes"].as_u64().expect("peak")
                                <= bytes as u64 * workers as u64
                        );
                        if relative.ends_with("CompoundTrace.tla") {
                            let trace = TraceDoc::from_json(&doc).expect("word");
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
                            assert_eq!(
                                model.try_validate(&trace).expect("validation").validation,
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
fn candidate_spill_combines_with_disk_seen_and_refuses_other_backends() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = request("tests/fixtures/core-m0/CompoundTrace.tla", tlc, 4, store);
            let baseline = check_file(&req).expect("resident Core");
            req.candidate_memory_bytes = Some(0);
            req.seen_disk = Some(dive_api::SeenOptions::default());
            let actual = check_file(&req).expect("disk candidate check");
            assert_eq!(observed(&actual.to_json()), observed(&baseline.to_json()));
        }
    }
}

#[test]
fn candidate_write_failure_never_certifies_success_and_directory_needs_budget() {
    let mut req = request(
        "tests/fixtures/core-m0/CompoundTrace.tla",
        false,
        4,
        Store::Exact,
    );
    req.candidate_directory = Some(req.file.clone());
    assert!(check_file(&req).is_err());
    req.candidate_memory_bytes = Some(0);
    let actual = check_file(&req).expect("storage failure report");
    let doc = actual.to_json();
    assert_eq!(doc["verdict"], "resource-limit", "{doc:#}");
    assert!(doc["counterexample"].is_null());
    assert_ne!(actual.exit_code_for(ExitConvention::Tlc), 0);
    assert_ne!(
        doc["core"]["metrics"]["candidate_storage"]["failures"]
            .as_array()
            .expect("failures")
            .as_slice(),
        [] as [Value; 0]
    );
}

#[test]
fn candidate_spill_small_smoke_runs_both_profiles_and_actual_widths() {
    for relative in [
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/core-m1/ViewMerge.tla",
    ] {
        for tlc in [false, true] {
            for workers in [1, 4] {
                let mut req = request(relative, tlc, workers, Store::Exact);
                let baseline = check_file(&req).expect("resident");
                req.candidate_memory_bytes = Some(0);
                let actual = check_file(&req).expect("spilled");
                assert_eq!(observed(&actual.to_json()), observed(&baseline.to_json()));
                assert_eq!(
                    actual.to_json()["core"]["metrics"]["candidate_storage"]["retained_bytes"],
                    0
                );
            }
        }
    }
}

#[test]
fn candidate_policy_can_change_across_core_checkpoint_recovery() {
    use dive_api::{check_file_checkpoint, checkpoint};
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = request(
                "../../docs/user/examples/CheckpointCounter.tla",
                tlc,
                4,
                store,
            );
            let baseline = check_file(&req).expect("resident");
            req.candidate_memory_bytes = Some(0);
            req.frontier_memory_bytes = Some(0);
            req.predecessor_memory_bytes = Some(0);
            let path = std::env::temp_dir().join(format!(
                "dive-candidate-checkpoint-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let mut cp = checkpoint::Request {
                path: path.clone(),
                action: checkpoint::Action::PauseAfterLevels(2),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = check_file_checkpoint(&req, &cp).expect("pause").to_json();
            assert_eq!(
                paused["checkpoint"]["status"], "paused-incomplete",
                "{paused:#}"
            );
            req.workers = Some(8);
            req.candidate_memory_bytes = Some(256);
            cp.action = checkpoint::Action::Recover;
            let resumed = check_file_checkpoint(&req, &cp).expect("recover").to_json();
            assert_eq!(observed(&resumed), observed(&baseline.to_json()));
            assert_eq!(resumed["checkpoint"]["status"], "recovered");
            assert_eq!(
                resumed["core"]["metrics"]["candidate_storage"]["retained_bytes"],
                0
            );
            std::fs::remove_file(path).expect("checkpoint cleanup");
        }
    }
}

#[test]
fn candidate_spill_keeps_incomplete_constraint_and_action_endings() {
    for relative in [
        "tests/fixtures/checker/dfid/DfidIncomplete.tla",
        "tests/fixtures/checker/errors/specs/ErConstrAssert/ErConstrAssert.tla",
        "tests/fixtures/checker/tlc-probes/specs/ImpliedActionSeen/ImpliedActionSeen.tla",
    ] {
        for tlc in [false, true] {
            let mut req = request(relative, tlc, 4, Store::Exact);
            let baseline = check_file(&req).expect("resident");
            assert!(
                !matches!(
                    baseline.to_json()["verdict"].as_str(),
                    Some("unsupported" | "internal-error")
                ),
                "{relative}: {}",
                baseline.to_json()
            );
            req.candidate_memory_bytes = Some(0);
            let actual = check_file(&req).expect("spilled");
            assert_eq!(
                observed(&actual.to_json()),
                observed(&baseline.to_json()),
                "{relative}"
            );
            assert_eq!(
                actual.exit_code_for(ExitConvention::Tlc),
                baseline.exit_code_for(ExitConvention::Tlc)
            );
        }
    }
}

#[test]
fn candidate_closure_keeps_an_actual_shared_dag_compact() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1, 4] {
                let mut req = request(
                    "tests/fixtures/core-candidates/SharedDag20.tla",
                    tlc,
                    workers,
                    store,
                );
                req.no_deadlock = true;
                let baseline = check_file(&req).expect("resident shared DAG").to_json();
                assert_eq!(baseline["verdict"], "success");
                req.candidate_memory_bytes = Some(0);
                let actual = check_file(&req).expect("spilled shared DAG").to_json();
                assert_eq!(observed(&actual), observed(&baseline));
                let resident = &baseline["core"]["metrics"]["arena_peak"];
                let spilled = &actual["core"]["metrics"]["arena_peak"];
                for field in ["nodes", "handles"] {
                    assert!(
                        spilled[field].as_u64().expect("spilled arena")
                            <= resident[field].as_u64().expect("resident arena") * 2 + 8,
                        "shared DAG expanded: {resident} -> {spilled}"
                    );
                }
                let usage = &actual["core"]["metrics"]["candidate_storage"];
                assert!(
                    usage["committed_spill_bytes"]
                        .as_u64()
                        .expect("spill bytes")
                        < 4096
                );
                assert_eq!(usage["origin_index_bytes"], 0);
            }
        }
    }
}

#[test]
fn canonical_candidates_need_no_constructor_history_index() {
    for tlc in [false, true] {
        let mut req = request(
            "tests/fixtures/core-candidates/OriginRows.tla",
            tlc,
            4,
            Store::Exact,
        );
        req.no_deadlock = true;
        let resident = check_file(&req).expect("resident origins").to_json();
        assert_eq!(resident["verdict"], "success");
        req.candidate_memory_bytes = Some(0);
        let actual = check_file(&req).expect("spilled origins").to_json();
        assert_eq!(observed(&actual), observed(&resident));
        let usage = &actual["core"]["metrics"]["candidate_storage"];
        assert_eq!(usage["peak_retained_bytes"], 0);
        assert_eq!(usage["origin_index_bytes"], 0);
        let peak = usage["peak_origin_index_bytes"]
            .as_u64()
            .expect("actual origin workspace");
        assert_eq!(
            peak, 0,
            "canonical candidates retain no constructor history"
        );
    }
}
