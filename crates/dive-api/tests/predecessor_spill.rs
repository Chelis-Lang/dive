//! Separate predecessor budgets preserve ordered safety results and witnesses.
#![allow(clippy::expect_used, reason = "regression assertions")]
#[path = "support/memory_observations.rs"]
mod memory_observations;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, TemporalMode, check_file};
use std::path::Path;

fn request(corpus: &str, name: &str) -> CheckRequest {
    let fixture = if name.starts_with("Tm") {
        "tlc-module"
    } else {
        "tlc-probes"
    };
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../{corpus}/tests/fixtures/{fixture}/specs/{name}/{name}.tla"
    ));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observable(mut json: serde_json::Value) -> serde_json::Value {
    memory_observations::normalize(&mut json);
    let fields = json.as_object_mut().expect("document");
    fields.remove("elapsed_ms");
    fields.remove("predecessor_storage");
    let options = fields
        .get_mut("options")
        .expect("options")
        .as_object_mut()
        .expect("object");
    options.remove("predecessor_memory_bytes");
    options.remove("predecessor_path_steps_limit");
    // Thread evidence remains compared for each exact request.
    json
}

#[test]
fn zero_tiny_logs_equal_memory_at_all_worker_widths_and_chaos() {
    for (corpus, name) in [
        ("dive-check", "DuplicateSuccessors"),
        ("dive-check", "TmRandom"),
        ("dive-check", "TmRandomStartup"),
        ("dive-check", "MidLevelViolation"),
        ("dive-check", "DeadlockMidLevel"),
        ("dive-check", "EvalErrorMidLevel"),
        ("dive-check", "InitViolation"),
        ("dive-check", "ConstrainedViolation"),
        ("dive-check", "ActionConstraintNoDeadlock"),
        ("dive-check", "ImpliedActionSeen"),
        ("dive-liveness", "SymOrder"),
        ("dive-liveness", "SymViolation"),
        ("dive-liveness", "SymView"),
    ] {
        for compat_tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(13)] {
                        let mut req = request(corpus, name);
                        req.compat_tlc = compat_tlc;
                        req.store = store;
                        req.workers = Some(workers);
                        req.chaos = chaos;
                        let reference = check_file(&req).expect("memory").to_json();
                        assert_ne!(reference["verdict"], "unsupported", "{name}");
                        assert_ne!(reference["verdict"], "internal-error", "{name}");
                        for budget in [0, 24, 256] {
                            req.predecessor_memory_bytes = Some(budget);
                            let actual = check_file(&req).expect("bounded").to_json();
                            assert_eq!(
                                observable(actual.clone()),
                                observable(reference.clone()),
                                "{name}, {compat_tlc}, {store:?}, {workers}, {chaos:?}, {budget}"
                            );
                            let evidence = &actual["predecessor_storage"];
                            assert_eq!(evidence["cleanup_completed"], true);
                            assert_eq!(evidence["failures"], serde_json::json!([]));
                            assert!(
                                evidence["peak_retained_bytes"].as_u64().expect("bytes")
                                    <= budget as u64
                            );
                            if budget == 0
                                && evidence["committed_records"].as_u64().expect("records") > 0
                            {
                                assert_eq!(evidence["retained_bytes_before_cleanup"], 0);
                                assert_eq!(evidence["spilled"], true);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn concrete_spilled_symmetry_witnesses_remain_independently_valid() {
    for name in ["SymOrder", "SymViolation"] {
        for compat_tlc in [false, true] {
            let mut req = request("dive-liveness", name);
            req.compat_tlc = compat_tlc;
            req.predecessor_memory_bytes = Some(0);
            let model = SpecModel::load(
                &req.file,
                req.config.as_ref().expect("config"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 7)),
                    ..TraceOptions::default()
                },
            )
            .expect("independent model");
            for store in [Store::Fingerprint, Store::Exact] {
                req.store = store;
                for workers in [1, 4, 8] {
                    req.workers = Some(workers);
                    req.chaos = Some(13);
                    let json = check_file(&req).expect("check").to_json();
                    let trace = TraceDoc::from_json(&json).expect("witness");
                    let controls = model.controls(&trace);
                    assert_eq!(controls.positive.validation, Validation::Valid);
                    assert!(controls.failures().is_empty(), "{:?}", controls.failures());
                }
            }
        }
    }
}

#[test]
fn early_admission_and_failed_creation_never_certify_success() {
    let mut req = CheckRequest {
        file: "nonexistent-model.tla".into(),
        predecessor_directory: Some("nonexistent-log-parent".into()),
        ..CheckRequest::default()
    };
    let error = check_file(&req)
        .expect_err("orphan directory refused before file access")
        .to_string();
    assert!(error.contains("DIVE-A0101"), "{error}");
    req.predecessor_memory_bytes = Some(0);
    req.temporal = TemporalMode::FinalShort;
    assert!(
        check_file(&req)
            .expect_err("capture refused early")
            .to_string()
            .contains("predecessor spill")
    );
    let mut req = request("dive-check", "DuplicateSuccessors");
    req.compat_tlc = true;
    req.workers = Some(8);
    req.predecessor_memory_bytes = Some(0);
    req.predecessor_directory = Some(req.file.clone());
    let json = check_file(&req).expect("failure reported").to_json();
    assert_eq!(json["verdict"], "resource-limit");
    assert_eq!(json["options"]["workers"]["used"], 1);
    assert_eq!(json["counterexample"], serde_json::Value::Null);
    assert_eq!(
        json["predecessor_storage"]["write_accounting_complete"],
        false
    );
    assert_ne!(
        json["predecessor_storage"]["failures"]
            .as_array()
            .expect("failures")
            .as_slice(),
        [] as [serde_json::Value; 0]
    );
    assert_eq!(json["statistics"]["generated"], 1);
    assert_eq!(json["statistics"]["distinct"], 1);
}

#[test]
fn combined_frontier_and_predecessor_spill_preserves_order_and_witnesses() {
    fn observed(mut j: serde_json::Value) -> serde_json::Value {
        memory_observations::normalize(&mut j);
        let f = j.as_object_mut().expect("document");
        for key in ["elapsed_ms", "frontier_storage", "predecessor_storage"] {
            f.remove(key);
        }
        let opts = f["options"].as_object_mut().expect("options");
        for key in [
            "frontier_memory_bytes_per_queue",
            "frontier_codec_nodes_per_value_limit",
            "frontier_record_bytes_limit",
            "predecessor_memory_bytes",
            "predecessor_path_steps_limit",
        ] {
            opts.remove(key);
        }
        j
    }
    for (corpus, name) in [
        ("dive-check", "DuplicateSuccessors"),
        ("dive-check", "MidLevelViolation"),
        ("dive-liveness", "SymViolation"),
        ("dive-liveness", "SymView"),
    ] {
        for compat_tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    let mut req = request(corpus, name);
                    req.compat_tlc = compat_tlc;
                    req.store = store;
                    req.workers = Some(workers);
                    req.chaos = Some(13);
                    let reference = check_file(&req).expect("memory").to_json();
                    req.frontier_memory_bytes = Some(0);
                    req.predecessor_memory_bytes = Some(0);
                    let bounded = check_file(&req).expect("both bounded").to_json();
                    assert_eq!(
                        observed(bounded.clone()),
                        observed(reference),
                        "{name}/{compat_tlc}/{store:?}/{workers}"
                    );
                    assert_eq!(bounded["predecessor_storage"]["cleanup_completed"], true);
                    assert_eq!(
                        bounded["predecessor_storage"]["failures"],
                        serde_json::json!([])
                    );
                    assert_eq!(
                        bounded["frontier_storage"]["failures"],
                        serde_json::json!([])
                    );
                    if bounded["counterexample"].is_object() {
                        let model = SpecModel::load(
                            &req.file,
                            req.config.as_ref().expect("config"),
                            &TraceOptions {
                                compat_tlc,
                                run: Some((0, 7)),
                                ..TraceOptions::default()
                            },
                        )
                        .expect("independent model");
                        let trace = TraceDoc::from_json(&bounded).expect("witness");
                        let controls = model.controls(&trace);
                        assert_eq!(controls.positive.validation, Validation::Valid);
                        assert!(controls.failures().is_empty(), "{:?}", controls.failures());
                    }
                }
            }
        }
    }
}
