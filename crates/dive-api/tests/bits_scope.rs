//! Pinned local-class checking preserves the one-worker reference across worker widths.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "functional file and report assertions"
)]

use dive_api::{
    CheckReport, CheckRequest, SimulateRequest, Store, check_file, check_file_tlc_inputs,
    checkpoint,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const CLASS: &[u8] = include_bytes!("../../dive-model/tests/fixtures/bits/Bits.class");

/// Core, explicitly (the retired Reference engine's former slot);
/// `check_file` and `check_file_tlc_inputs` run the default engine, also Core.
fn reference_engine(req: &CheckRequest) -> Result<CheckReport, dive_api::ApiError> {
    dive_api::check_file(req)
}

struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup(class: bool) -> Files {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let files = loop {
        let path = std::env::temp_dir().join(format!(
            "dive-api-bits-scope-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => break Files(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            other => other.expect("private directory"),
        }
    };
    std::fs::write(
        files.0.join("Bits.tla"),
        "---- MODULE Bits ----\nAnd(x,y,n,m) == 99\n====\n",
    )
    .expect("TLA fallback");
    if class {
        std::fs::write(files.0.join("Bits.class"), CLASS).expect("exact class");
    }
    let answer = if class { 3 } else { 99 };
    std::fs::write(
        files.0.join("Scope.tla"),
        format!(
            "---- MODULE Scope ----\nEXTENDS Naturals, Bits\nVARIABLE x\nInit == x = 0\nNext == x < 3 /\\ x' = x + 1\nInv == And(7,3,0,7) = {answer}\n====\n"
        ),
    )
    .expect("root");
    std::fs::write(
        files.0.join("Scope.cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )
    .expect("config");
    files
}

fn request(files: &Files, compat_tlc: bool, workers: usize, store: Store) -> CheckRequest {
    CheckRequest {
        file: files.0.join("Scope.tla"),
        compat_tlc,
        workers: Some(workers),
        store,
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn reference(report: &CheckReport, workers: usize, baseline: &serde_json::Value) {
    assert_eq!(report.verdict(), "success");
    assert_eq!(report.exit_code(), 0);
    let json = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
    assert_eq!(json["exit_code"], 0);
    assert_eq!(json["options"]["workers"]["requested"], workers);
    assert_eq!(json["statistics"], baseline["statistics"]);
    assert!(json["counterexample"].is_null());
}

#[test]
fn pinned_local_class_matches_reference_at_supported_worker_widths() {
    let files = setup(true);
    for profile in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = request(&files, profile, 1, store);
            let completed = dive_api::check_file(&req).expect("one worker");
            assert_eq!(completed.verdict(), "success", "{}", completed.to_json());
            assert_eq!(completed.exit_code(), 0);
            assert_eq!(completed.to_json()["statistics"]["distinct"], 4);
            let baseline = completed.to_json();
            for workers in [1, 4, 8] {
                req.workers = Some(workers);
                for sequential in [false, true] {
                    req.sequential = sequential;
                    reference(
                        &dive_api::check_file(&req).expect("supported width"),
                        workers,
                        &baseline,
                    );
                }
            }
        }
    }
}

#[test]
fn unknown_class_refuses_before_constant_roles_assumptions_or_fallback_errors() {
    let files = setup(true);
    let mut altered = CLASS.to_vec();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    std::fs::write(files.0.join("Bits.class"), altered).expect("unknown class");
    for (fallback, assumption) in [
        ("99", ""),
        ("99", "ASSUME And(7,3,0,7) = 3\n"),
        ("1 \\div 0", ""),
    ] {
        std::fs::write(
            files.0.join("Bits.tla"),
            format!("---- MODULE Bits ----\nEXTENDS Naturals\nAnd(x,y,n,m) == {fallback}\n====\n"),
        )
        .expect("unlike fallback");
        std::fs::write(
            files.0.join("Scope.tla"),
            format!("---- MODULE Scope ----\nEXTENDS Naturals, Bits\nVARIABLE x\n{assumption}Init == x=0\nNext == x<3 /\\ x'=x+1\nInv == And(7,3,0,7)=3\n====\n"),
        )
        .expect("constant invariant");
        for profile in [false, true] {
            let req = request(&files, profile, 4, Store::Fingerprint);
            for check in [reference_engine, check_file, check_file_tlc_inputs] {
                let report = check(&req).expect("named refusal");
                let json = report.to_json();
                assert_eq!(report.verdict(), "unsupported", "{json}");
                assert!(
                    json["message"]
                        .as_str()
                        .expect("feature")
                        .contains("Bits.class")
                );
                assert_eq!(json["options"]["workers"]["used"], 0);
                assert!(json["statistics"].is_null());
                assert!(json["counterexample"].is_null());
                assert_eq!(json["output"], serde_json::json!([]));
            }
        }
    }
}

#[test]
fn class_refusal_preserves_configuration_binding_priority_and_known_class_execution() {
    let files = setup(true);
    std::fs::write(
        files.0.join("Scope.tla"),
        "---- MODULE Scope ----\nEXTENDS Naturals, Bits\nVARIABLE x\nInit == x=0\nNext == x<3 /\\ x'=x+1\nInv == And(7,3,0,7)=3\n====\n",
    )
    .expect("constant invariant");
    for profile in [false, true] {
        let req = request(&files, profile, 1, Store::Fingerprint);
        for check in [reference_engine, check_file, check_file_tlc_inputs] {
            let json = check(&req).expect("known class").to_json();
            assert_eq!(json["verdict"], "success", "{json}");
            assert_eq!(json["statistics"]["distinct"], 4);
            assert_eq!(json["statistics"]["generated"], 4);
            assert_eq!(json["statistics"]["depth"], 4);
        }
    }
    let mut altered = CLASS.to_vec();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    std::fs::write(files.0.join("Bits.class"), altered).expect("unknown class");
    std::fs::write(
        files.0.join("Scope.cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCONSTANT Missing <- Init\nCHECK_DEADLOCK FALSE\n",
    )
    .expect("invalid configuration binding");
    for profile in [false, true] {
        let req = request(&files, profile, 1, Store::Fingerprint);
        for check in [reference_engine, check_file, check_file_tlc_inputs] {
            let json = check(&req).expect("config error").to_json();
            assert_eq!(json["verdict"], "model-error", "{json}");
            assert_eq!(json["tlc"]["code"], 2226);
            assert!(json["statistics"].is_null());
            assert!(json["counterexample"].is_null());
        }
    }
}

#[test]
fn class_refusal_preserves_configured_constant_operator_arity_errors() {
    let files = setup(true);
    std::fs::write(
        files.0.join("Scope.tla"),
        "---- MODULE Scope ----\nEXTENDS Naturals, Bits\nCONSTANT C(_)\nVARIABLE x\nF == TRUE\nInit == x=0\nNext == x'=x\nInv == And(7,3,0,7)=3\n====\n",
    )
    .expect("operator constant");
    std::fs::write(
        files.0.join("Scope.cfg"),
        "CONSTANT C <- F\nINIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )
    .expect("wrong replacement arity");
    let mut altered = CLASS.to_vec();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    for class in [None, Some(CLASS), Some(altered.as_slice())] {
        if let Some(class) = class {
            std::fs::write(files.0.join("Bits.class"), class).expect("class");
        } else {
            std::fs::remove_file(files.0.join("Bits.class")).expect("no class");
        }
        for profile in [false, true] {
            let req = request(&files, profile, 1, Store::Fingerprint);
            for check in [reference_engine, check_file, check_file_tlc_inputs] {
                let json = check(&req).expect("configuration error").to_json();
                assert_eq!(json["verdict"], "model-error", "{json}");
                assert_eq!(json["tlc"]["code"], 2225);
                assert!(json["statistics"].is_null());
                assert!(json["counterexample"].is_null());
            }
        }
    }
}

#[test]
fn plain_tla_bits_preserves_parallel_checking_and_checkpoint_recovery() {
    let files = setup(false);
    for profile in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let req = request(&files, profile, 1, store);
            let baseline = dive_api::check_file(&req).expect("TLA fallback").to_json();
            let path = files.0.join(format!("fallback-{profile}-{store:?}.cp"));
            let pause = checkpoint::Request {
                path,
                action: checkpoint::Action::PauseAfterLevels(1),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            assert_eq!(
                dive_api::check_file_checkpoint(&req, &pause)
                    .expect("fallback pause")
                    .verdict(),
                "resource-limit"
            );
            for workers in [1, 4, 8] {
                let req = request(&files, profile, workers, store);
                let complete = dive_api::check_file(&req)
                    .expect("fallback width")
                    .to_json();
                assert_eq!(complete["verdict"], "success");
                assert_eq!(complete["statistics"], baseline["statistics"]);
                let resume = checkpoint::Request {
                    action: checkpoint::Action::Recover,
                    ..pause.clone()
                };
                let resumed =
                    dive_api::check_file_checkpoint(&req, &resume).expect("fallback recovery");
                assert_eq!(resumed.verdict(), "success", "{}", resumed.to_json());
                assert_eq!(resumed.to_json()["statistics"], baseline["statistics"]);
            }
        }
    }
}

#[test]
fn local_class_recovery_preserves_identity_across_widths_and_corrupt_bootstrap_priority() {
    let files = setup(true);
    for profile in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let req = request(&files, profile, 1, store);
            let pause = checkpoint::Request {
                path: files.0.join(format!("class-{profile}-{store:?}.cp")),
                action: checkpoint::Action::PauseAfterLevels(1),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let baseline = dive_api::check_file(&req)
                .expect("reference check")
                .to_json();
            let paused = dive_api::check_file_checkpoint(&req, &pause).expect("one-worker pause");
            assert_eq!(paused.verdict(), "resource-limit");
            let bytes = std::fs::read(&pause.path).expect("archive");
            let resume = checkpoint::Request {
                action: checkpoint::Action::Recover,
                ..pause.clone()
            };
            assert_eq!(
                dive_api::check_file_checkpoint(&req, &resume)
                    .expect("one-worker resume")
                    .verdict(),
                "success"
            );
            let corrupt_path = files.0.join(format!("corrupt-{profile}-{store:?}.cp"));
            std::fs::write(&corrupt_path, b"not a checkpoint").expect("corrupt control");
            for workers in [4, 8] {
                let req = request(&files, profile, workers, store);
                let mut width_pause = pause.clone();
                width_pause.path = files
                    .0
                    .join(format!("supported-{profile}-{store:?}-{workers}.cp"));
                let paused_at_width =
                    dive_api::check_file_checkpoint(&req, &width_pause).expect("pause width");
                assert_eq!(paused_at_width.verdict(), "resource-limit");
                assert!(width_pause.path.exists());
                reference(
                    &dive_api::check_file_checkpoint(&req, &resume).expect("recovery width"),
                    workers,
                    &baseline,
                );
                let corrupt_resume = checkpoint::Request {
                    path: corrupt_path.clone(),
                    ..resume.clone()
                };
                assert!(matches!(
                    dive_api::check_file_checkpoint(&req, &corrupt_resume),
                    Err(dive_api::ApiError::Rejected { message }) if message.contains("DIVE-S0400")
                ));
                assert_eq!(std::fs::read(&pause.path).expect("untouched"), bytes);
            }
        }
    }
}

#[test]
fn finite_simulation_keeps_its_separate_worker_contract() {
    let files = setup(true);
    for compat_tlc in [false, true] {
        let mut req = SimulateRequest {
            file: files.0.join("Scope.tla"),
            compat_tlc,
            seed: Some(7),
            depth: Some(2),
            traces: 1,
            workers: Some(1),
            ..SimulateRequest::default()
        };
        let sampled = dive_api::simulate_file(&req).expect("sample").to_json();
        assert_eq!(sampled["result"], "sampled");
        assert_eq!(sampled["model_proved"], false);
        assert_eq!(sampled["traces"][0]["validation"]["result"], "valid");
        // Workers do not change a sample (ADR 0162 §7).
        req.workers = Some(4);
        let parallel = dive_api::simulate_file(&req).expect("sample").to_json();
        assert_eq!(parallel["traces"], sampled["traces"]);
        assert_eq!(parallel["statistics"], sampled["statistics"]);
        assert_eq!(parallel["options"]["workers_used"], 4);
    }
}
