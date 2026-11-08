//! Native actual recovery preserves safety output and independently checked witnesses.
#![allow(clippy::expect_used, reason = "captured safety regression controls")]
#[path = "support/memory_observations.rs"]
mod memory_observations;
#[path = "support/refused.rs"]
mod refused;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, TemporalMode, checkpoint};
use std::path::Path;

fn request(name: &str) -> CheckRequest {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../dive-check/tests/fixtures/tlc-probes/specs/{name}/{name}.tla"
    ));
    CheckRequest {
        file: file.clone(),
        config: Some(file.with_extension("cfg")),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn dir() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "dive-native-api-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).expect("private directory");
    p
}
fn observable(mut j: serde_json::Value) -> serde_json::Value {
    memory_observations::normalize(&mut j);
    let object = j.as_object_mut().expect("object");
    object.remove("elapsed_ms");
    object.remove("checkpoint");
    let options = object
        .get_mut("options")
        .expect("options")
        .as_object_mut()
        .expect("object");
    options.remove("checkpoint");
    options.remove("workers");
    options.remove("profile");
    j
}

fn verify_frontier_recovery_cases(names: &[&str]) {
    let dir = dir();
    let mut serial = 0;
    for &name in names {
        for compat in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(13)] {
                        let mut req = request(name);
                        req.compat_tlc = compat;
                        req.store = store;
                        req.workers = Some(workers);
                        req.chaos = chaos;
                        let report = dive_api::check_file_engine(&req, dive_api::Engine::Core)
                            .expect("reference");
                        let reference = report.to_json();
                        if chaos.is_some() {
                            // Only the retired reference engine ran --chaos (#791, #910).
                            refused::assert_refused(&reference, report.exit_code(), refused::CHAOS);
                            continue;
                        }
                        assert_ne!(reference["verdict"], "unsupported");
                        assert_ne!(reference["verdict"], "internal-error");
                        let checkpoint = checkpoint::Request {
                            path: dir.join(format!("{serial}.cp")),
                            action: checkpoint::Action::PauseAfterLevels(0),
                            counters: checkpoint::Counters::Retain,
                            config_source_sha256: None,
                        };
                        serial += 1;
                        let paused = dive_api::check_file_checkpoint_engine(
                            &req,
                            &checkpoint,
                            dive_api::Engine::Core,
                        )
                        .expect("pause")
                        .to_json();
                        assert_eq!(paused["verdict"], "resource-limit", "{name}: {paused}");
                        assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
                        assert_eq!(paused["options"]["workers"]["requested"], workers);
                        assert_eq!(paused["options"]["workers"]["used"], 1);
                        assert!(paused["counterexample"].is_null());
                        let resumed = dive_api::check_file_checkpoint_engine(
                            &req,
                            &checkpoint::Request {
                                action: checkpoint::Action::Recover,
                                ..checkpoint
                            },
                            dive_api::Engine::Core,
                        )
                        .expect("resume")
                        .to_json();
                        assert_eq!(
                            observable(resumed.clone()),
                            observable(reference),
                            "{name} profile{compat},store{store:?},width{workers},chaos{chaos:?}"
                        );
                        assert_eq!(resumed["options"]["workers"]["used"], workers);
                        assert_eq!(resumed["checkpoint"]["status"], "recovered");
                        assert_eq!(
                            resumed["options"]["checkpoint"]["independent_certificate"],
                            false
                        );
                        if compat {
                            assert_eq!(resumed["options"]["profile"], "tlc+overrides");
                        }
                        if ["MidLevelViolation", "ConstrainedViolation"].contains(&name) {
                            let model = SpecModel::load(
                                &req.file,
                                req.config.as_ref().expect("config"),
                                &TraceOptions {
                                    compat_tlc: compat,
                                    run: Some((0, 7)),
                                    ..TraceOptions::default()
                                },
                            )
                            .expect("independent loaded model");
                            let trace = TraceDoc::from_json(&resumed).expect("resumed witness");
                            let controls = model.controls(&trace);
                            assert_eq!(controls.positive.validation, Validation::Valid);
                            assert!(controls.failures().is_empty(), "{:?}", controls.failures());
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn frontier_recovery_duplicate_successors() {
    verify_frontier_recovery_cases(&["DuplicateSuccessors"]);
}

#[test]
fn frontier_recovery_mid_level_violation() {
    verify_frontier_recovery_cases(&["MidLevelViolation"]);
}

#[test]
fn frontier_recovery_deadlock_mid_level() {
    verify_frontier_recovery_cases(&["DeadlockMidLevel"]);
}

#[test]
fn frontier_recovery_eval_error_mid_level() {
    verify_frontier_recovery_cases(&["EvalErrorMidLevel"]);
}

#[test]
fn frontier_recovery_constrained_violation() {
    verify_frontier_recovery_cases(&["ConstrainedViolation"]);
}

#[test]
fn frontier_recovery_action_constraint_no_deadlock() {
    verify_frontier_recovery_cases(&["ActionConstraintNoDeadlock"]);
}

#[test]
fn recovered_defaults_are_reported_and_source_byte_drift_refuses() {
    let dir = dir();
    let mut req = request("DuplicateSuccessors");
    req.compat_tlc = true;
    let cp = checkpoint::Request {
        path: dir.join("snapshot"),
        action: checkpoint::Action::PauseAfterLevels(1),
        counters: checkpoint::Counters::Tlc,
        config_source_sha256: None,
    };
    assert_eq!(
        dive_api::check_file_checkpoint_engine(&req, &cp, dive_api::Engine::Core)
            .expect("pause")
            .to_json()["checkpoint"]["status"],
        "paused-incomplete"
    );
    req.seed = None;
    req.fp = None;
    let resume = checkpoint::Request {
        action: checkpoint::Action::Recover,
        ..cp
    };
    let json = dive_api::check_file_checkpoint_engine(&req, &resume, dive_api::Engine::Core)
        .expect("resume")
        .to_json();
    assert_eq!(json["verdict"], "success");
    assert_eq!(json["options"]["seed"], 7);
    assert_eq!(json["options"]["seed_source"], "checkpoint");
    assert_eq!(
        json["options"]["fingerprint"]["polynomial_source"],
        "checkpoint"
    );
    let cfg = dir.join("same.cfg");
    let original = std::fs::read_to_string(req.config.as_ref().expect("config")).expect("cfg");
    std::fs::write(&cfg, format!("{original}\n\\* Changed bytes only\n")).expect("alternate cfg");
    req.config = Some(cfg);
    let drift = dive_api::check_file_checkpoint_engine(&req, &resume, dive_api::Engine::Core)
        .expect("identity rejection")
        .to_json();
    assert_eq!(drift["verdict"], "resource-limit");
    assert_eq!(drift["options"]["workers"]["used"], 0);
    assert!(drift["statistics"].is_null());
    assert!(drift["checkpoint"].is_null());
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn reference_engine_archives_are_rejected_by_name_and_damage_by_framing() {
    let dir = dir();
    let req = request("DuplicateSuccessors");
    let resume = checkpoint::Request {
        path: dir.join("reference.cp"),
        action: checkpoint::Action::Recover,
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    dive_runtime::checkpoint::write_payload(&resume.path, b"DIVECP02", b"{}")
        .expect("framed reference archive");
    assert!(matches!(
        dive_api::check_file_checkpoint_engine(&req, &resume, dive_api::Engine::Core),
        Err(dive_api::ApiError::Rejected { message })
            if message.starts_with("DIVE-S0400") && message.contains("reference engine")
    ));
    let mut bytes = std::fs::read(&resume.path).expect("archive bytes");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&resume.path, bytes).expect("damaged archive");
    assert!(matches!(
        dive_api::check_file_checkpoint_engine(&req, &resume, dive_api::Engine::Core),
        Err(dive_api::ApiError::Rejected { message })
            if message.starts_with("DIVE-S0400") && message.contains("checksum")
    ));
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn unsupported_mode_combinations_refuse_before_file_access() {
    let cp = checkpoint::Request {
        path: "nonexistent.cp".into(),
        action: checkpoint::Action::PauseAfterLevels(0),
        counters: checkpoint::Counters::Tlc,
        config_source_sha256: None,
    };
    let mut req = CheckRequest {
        file: "no-model.tla".into(),
        temporal: TemporalMode::FinalShort,
        ..CheckRequest::default()
    };
    assert!(matches!(
        dive_api::check_file_checkpoint_engine(&req, &cp, dive_api::Engine::Core),
        Err(dive_api::ApiError::Unsupported { .. })
    ));
    req.temporal = TemporalMode::Ordinary;
    req.frontier_directory = Some("orphan-frontier".into());
    assert!(matches!(
        dive_api::check_file_checkpoint_engine(&req, &cp, dive_api::Engine::Core),
        Err(dive_api::ApiError::Rejected { message }) if message.contains("DIVE-A0100")
    ));
    req.frontier_directory = None;
    req.predecessor_directory = Some("orphan-directory".into());
    assert!(matches!(
        dive_api::check_file_checkpoint_engine(&req, &cp, dive_api::Engine::Core),
        Err(dive_api::ApiError::Rejected { message }) if message.contains("DIVE-A0101")
    ));
}
