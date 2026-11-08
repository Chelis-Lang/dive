//! Explicit API admission, failure evidence and independent disk-search witnesses.
#![allow(clippy::expect_used, reason = "test assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, SeenOptions, Store, TemporalMode, checkpoint};
use std::path::Path;

/// Only the retired reference engine ran `--chaos` (#791, #910): Core refuses
/// it by name, and the same request without it runs on Core.
fn assert_chaos_refused_then_clear(request: &mut CheckRequest) {
    let chaotic = dive_api::check_file_engine(request, dive_api::Engine::Core).expect("check");
    refused::assert_refused(&chaotic.to_json(), chaotic.exit_code(), refused::CHAOS);
    request.chaos = None;
}

#[test]
fn invariant_error_replay_reports_actual_continuation_and_verified_known_class() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-check/tests/fixtures/disk-seen/ReplayNew/ReplayNew.tla");
    for compat_tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            for width in [1, 4, 8] {
                let mut request = CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    compat_tlc,
                    store,
                    workers: Some(width),
                    seed: Some(7),
                    fp: Some(0),
                    chaos: Some(7),
                    seen_disk: Some(SeenOptions::default()),
                    ..CheckRequest::default()
                };
                assert_chaos_refused_then_clear(&mut request);
                let json = dive_api::check_file_engine(&request, dive_api::Engine::Core)
                    .expect("error report")
                    .to_json();
                assert_eq!(json["statistics"]["generated"], 2);
                assert_eq!(json["statistics"]["distinct"], 4);
                assert_eq!(json["statistics"]["left_on_queue"], 2);
                assert_eq!(json["tlc"]["code"], 2111);
                assert_eq!(json["tlc"]["verified"], true);
                assert_eq!(json["options"]["workers"]["requested"], width);
                assert_eq!(json["options"]["workers"]["used"], 1);
                // Core reports the error from its own search, with no separate
                // replay (the retired reference engine's replay record is gone),
                // and the disk-backed run reports what the in-memory run does.
                assert!(json["seen_storage"]["replay"].is_null(), "{json}");
                request.seen_disk = None;
                let memory = dive_api::check_file_engine(&request, dive_api::Engine::Core)
                    .expect("in-memory report")
                    .to_json();
                for field in ["verdict", "message", "statistics", "tlc", "counterexample"] {
                    assert_eq!(json[field], memory[field], "{field}");
                }
                assert_eq!(json["seen_storage"]["admitted_keys"], 4);
                assert_eq!(
                    json["seen_storage"]["first_failure"],
                    serde_json::Value::Null
                );
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["cleanup_completed"],
                    true
                );
            }
        }
    }
}

#[test]
fn malformed_and_unimplemented_requests_refuse_before_input_or_archive_access() {
    for compat_tlc in [false, true] {
        let mut request = CheckRequest {
            file: "missing-seen-model.tla".into(),
            compat_tlc,
            seen_disk: Some(SeenOptions::default()),
            ..CheckRequest::default()
        };
        request.seen_disk.as_mut().expect("policy").limits.entries = 0;
        assert!(
            matches!(dive_api::check_file_engine(&request, dive_api::Engine::Core),Err(dive_api::ApiError::Rejected{message}) if message.contains("DIVE-S0700"))
        );
        request.seen_disk.as_mut().expect("policy").limits.entries = 16;
        request.temporal = TemporalMode::FinalShort;
        assert!(
            matches!(dive_api::check_file_engine(&request, dive_api::Engine::Core),Err(dive_api::ApiError::Unsupported{feature}) if feature.contains("disk seen-state"))
        );
        request.temporal = TemporalMode::Ordinary;
        let archive = checkpoint::Request {
            path: "missing-seen-archive.cp".into(),
            action: checkpoint::Action::Recover,
            counters: checkpoint::Counters::Tlc,
            config_source_sha256: None,
        };
        assert!(
            matches!(dive_api::check_file_checkpoint_engine(&request,&archive, dive_api::Engine::Core),Err(dive_api::ApiError::Rejected{message}) if message.contains("DIVE-S0400"))
        );
    }
}

fn witness_case(name: &str, root: &Path) {
    let file = root.join(name).join(format!("{name}.tla"));
    let config = file.with_extension("cfg");
    for compat_tlc in [false, true] {
        let validator = SpecModel::load(
            &file,
            &config,
            &TraceOptions {
                compat_tlc,
                run: Some((0, 7)),
                ..TraceOptions::default()
            },
        )
        .expect("independent model");
        for store in [Store::Fingerprint, Store::Exact] {
            for width in [1, 4, 8] {
                let mut request = CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    seen_disk: Some(SeenOptions::default()),
                    compat_tlc,
                    store,
                    workers: Some(width),
                    seed: Some(7),
                    fp: Some(0),
                    chaos: Some(7),
                    frontier_memory_bytes: Some(0),
                    predecessor_memory_bytes: Some(0),
                    ..CheckRequest::default()
                };
                assert_chaos_refused_then_clear(&mut request);
                let report =
                    dive_api::check_file_engine(&request, dive_api::Engine::Core).expect("check");
                let json = report.to_json();
                assert_eq!(json["options"]["workers"]["requested"], width);
                assert_eq!(json["options"]["workers"]["used"], 1);
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["cleanup_completed"],
                    true
                );
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["expected_disk_bytes"],
                    0
                );
                assert_eq!(
                    json["seen_storage"]["first_failure"],
                    serde_json::Value::Null
                );
                let doc = TraceDoc::from_json(&json).expect("trace transport");
                let controls = validator.controls(&doc);
                assert_eq!(
                    controls.positive.validation,
                    Validation::Valid,
                    "{name} profile{compat_tlc} store{store:?} width{width}: {:?}",
                    controls.positive
                );
                assert!(controls.failures().is_empty(), "{:?}", controls.failures());
            }
        }
    }
}
#[test]
fn view_counterexample_has_independent_original_and_mutant_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs");
    witness_case("SymOrder", &root);
}
#[test]
fn symmetry_counterexample_has_independent_original_and_mutant_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs");
    witness_case("SymViolation", &root);
}
#[test]
fn invariant_counterexample_has_independent_original_and_mutant_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/disk-seen");
    witness_case("EarlyInvariant", &root);
}
#[test]
fn creation_failure_reports_no_initial_search_or_success() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-check/tests/fixtures/disk-seen/DuplicateNext/DuplicateNext.tla");
    let report = dive_api::check_file_engine(
        &CheckRequest {
            file: file.clone(),
            seen_disk: Some(SeenOptions {
                directory: Some(file),
                ..SeenOptions::default()
            }),
            workers: Some(8),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        },
        dive_api::Engine::Core,
    )
    .expect("typed result");
    let json = report.to_json();
    assert_eq!(json["verdict"], "resource-limit");
    assert_eq!(json["options"]["workers"]["used"], 0);
    assert_eq!(json["seen_storage"]["first_failure"]["phase"], "Creation");
    assert_eq!(json["seen_storage"]["first_failure"]["operation"], "Create");
    assert_eq!(
        json["seen_storage"]["before_cleanup"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["seen_storage"]["after_cleanup"],
        serde_json::Value::Null
    );
}

/// Core's report for `source` and `cfg` with resident and with disk seen
/// storage, under `opts`.
fn resident_and_disk(
    source: &str,
    cfg: &str,
    opts: &dive_runtime::CheckOptions,
) -> (serde_json::Value, serde_json::Value) {
    let files = core_final::Files::new(source, cfg);
    let resident = files.request(opts);
    let disk = CheckRequest {
        seen_disk: Some(SeenOptions::default()),
        ..resident.clone()
    };
    let run = |request: &CheckRequest| {
        dive_api::check_file_engine(request, dive_api::Engine::Core)
            .expect("report")
            .to_json()
    };
    (run(&resident), run(&disk))
}

#[test]
fn disk_seen_storage_with_temporal_properties_refuses_before_startup_prints_or_assumptions() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nUnused == PrintT(\"must not bind\")\nASSUME PrintT(\"must not assume\")\nInit == x=0\nNext == x'=x\nP == []<>(x=0)\n====\n";
    for options in [
        dive_runtime::CheckOptions::tlc(0, 7),
        dive_runtime::CheckOptions::dive(7),
    ] {
        let (resident, disk) =
            resident_and_disk(source, "INIT Init\nNEXT Next\nPROPERTY P\n", &options);
        // Resident storage binds (start-up output included) and checks P.
        assert_eq!(resident["verdict"], "success", "{resident}");
        assert_eq!(
            resident["output"],
            serde_json::json!(["\"must not bind\"", "\"must not assume\""])
        );
        assert_eq!(resident["statistics"]["generated"], 2);
        assert_eq!(resident["statistics"]["distinct"], 1);
        // Disk storage with a temporal property is refused before start-up.
        assert_eq!(disk["verdict"], "unsupported", "{disk}");
        assert_eq!(
            disk["message"],
            "disk seen-state retention with temporal checking"
        );
        assert_eq!(disk["output"], serde_json::json!([]));
        assert!(disk["statistics"].is_null(), "{disk}");
    }
}

#[test]
fn effectful_specification_role_matches_resident_storage() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nUnused == PrintT(\"must not bind\")\nASSUME PrintT(\"must not assume\")\nInit == x=0\nNext == x'=x\nSpec == Init /\\ [][Next]_x\n====\n";
    for options in [
        dive_runtime::CheckOptions::tlc(0, 7),
        dive_runtime::CheckOptions::dive(7),
    ] {
        let (resident, disk) = resident_and_disk(source, "SPECIFICATION Spec\n", &options);
        assert_eq!(disk["verdict"], "success", "{disk}");
        assert_eq!(disk["verdict"], resident["verdict"]);
        assert_eq!(disk["statistics"], resident["statistics"]);
        assert_eq!(disk["output"], resident["output"]);
        assert_eq!(disk["statistics"]["generated"], 2);
    }
}

#[test]
fn actual_print_events_refuse_unproved_disk_error_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/disk-seen");
    for name in [
        "ReplayPrint",
        "ReplayPrintInit",
        "ReplayPrintEmpty",
        "ReplayPrintInvariant",
        "ReplayPrintWrapper",
        "ReplayPrintAlias",
        "ReplayPrintReplayOnly",
    ] {
        let file = root.join(name).join(format!("{name}.tla"));
        for compat_tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for width in [1, 4, 8] {
                    let mut request = CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc,
                        store,
                        seen_disk: Some(SeenOptions::default()),
                        workers: Some(width),
                        seed: Some(7),
                        fp: Some(0),
                        chaos: Some(7),
                        ..CheckRequest::default()
                    };
                    assert_chaos_refused_then_clear(&mut request);
                    let json = dive_api::check_file_engine(&request, dive_api::Engine::Core)
                        .expect("diagnostic report")
                        .to_json();
                    assert_eq!(json["verdict"], "unsupported", "{name}: {json}");
                    assert!(
                        json["message"]
                            .as_str()
                            .expect("feature")
                            .contains("Print/PrintT")
                    );
                    assert_eq!(json["statistics"]["generated"], 2);
                    assert_eq!(json["tlc"], serde_json::Value::Null);
                    assert_eq!(json["counterexample"], serde_json::Value::Null);
                    assert_eq!(json["options"]["workers"]["requested"], width);
                    assert_eq!(json["options"]["workers"]["used"], 1);
                    assert!(json["seen_storage"]["replay"].is_null(), "{json}");
                    assert_eq!(
                        json["seen_storage"]["first_failure"],
                        serde_json::Value::Null
                    );
                    assert_eq!(
                        json["seen_storage"]["after_cleanup"]["cleanup_completed"],
                        true
                    );
                }
            }
        }
    }
}
