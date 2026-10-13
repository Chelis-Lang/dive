//! User-facing disk-frontier evidence and independent concrete witnesses.
#![allow(clippy::expect_used, reason = "test assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file};

#[test]
fn a_frontier_directory_without_a_budget_rejects_before_file_access() {
    for compat_tlc in [false, true] {
        for temporal in [
            dive_api::TemporalMode::Ordinary,
            dive_api::TemporalMode::FinalShort,
        ] {
            let error = check_file(&CheckRequest {
                file: std::path::PathBuf::from("missing-model-for-invalid-frontier-request.tla"),
                frontier_directory: Some(std::path::PathBuf::from("unused-frontier-directory")),
                compat_tlc,
                temporal,
                ..CheckRequest::default()
            })
            .expect_err("the directory must not be silently ignored");
            assert!(matches!(error, dive_api::ApiError::Rejected { message }
                if message == "DIVE-A0100: frontier_directory requires frontier_memory_bytes"));
        }
    }
}

#[test]
fn spilled_counterexamples_validate_and_corruptions_are_rejected() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs");
    for name in ["SymOrder", "SymViolation"] {
        let file = root.join(name).join(format!("{name}.tla"));
        let config = file.with_extension("cfg");
        for compat_tlc in [false, true] {
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 7)),
                    ..TraceOptions::default()
                },
            )
            .expect("model");
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    let r = check_file(&CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        compat_tlc,
                        store,
                        workers: Some(workers),
                        seed: Some(7),
                        fp: Some(0),
                        chaos: Some(13),
                        frontier_memory_bytes: Some(0),
                        ..CheckRequest::default()
                    })
                    .expect("check");
                    let json = r.to_json();
                    assert_eq!(json["frontier_storage"]["retained_bytes"], 0);
                    assert!(
                        json["frontier_storage"]["committed_spill_bytes"]
                            .as_u64()
                            .is_some_and(|n| n > 0)
                    );
                    assert_eq!(json["options"]["frontier_memory_bytes_per_queue"], 0);
                    let doc = TraceDoc::from_json(&json).expect("trace");
                    let controls = model.controls(&doc);
                    assert_eq!(controls.positive.validation, Validation::Valid);
                    assert!(controls.failures().is_empty(), "{:?}", controls.failures());
                }
            }
        }
    }
}

#[test]
fn disk_creation_failure_has_no_success_and_truthful_worker_evidence() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../dive-check/tests/fixtures/tlc-probes/specs/DuplicateSuccessors/DuplicateSuccessors.tla",
    );
    let r = check_file(&CheckRequest {
        frontier_memory_bytes: Some(0),
        frontier_directory: Some(file.clone()),
        file,
        compat_tlc: true,
        workers: Some(8),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    })
    .expect("check");
    let j = r.to_json();
    assert_eq!(j["verdict"], "resource-limit");
    assert_eq!(j["options"]["workers"]["requested"], 8);
    assert_eq!(j["options"]["workers"]["used"], 1);
    assert_ne!(
        j["frontier_storage"]["failures"]
            .as_array()
            .expect("failures")
            .as_slice(),
        [] as [serde_json::Value; 0]
    );
    assert_eq!(j["frontier_storage"]["write_accounting_complete"], false);
    assert_eq!(j["counterexample"], serde_json::Value::Null);
}
