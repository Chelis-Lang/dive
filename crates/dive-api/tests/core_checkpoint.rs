//! End-to-end owned Core completed-level recovery.
#![allow(clippy::expect_used, reason = "checkpoint regression assertions")]
use dive_api::{
    CheckRequest, Engine, Store, check_file_checkpoint_engine, check_file_engine, checkpoint,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dive-core-cp-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}
fn req() -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/user/examples/CheckpointCounter.tla");
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
#[test]
fn pause_fresh_owner_resume_matches_uninterrupted() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = req();
            req.compat_tlc = tlc;
            req.store = store;
            req.workers = Some(4);
            let full = check_file_engine(&req, Engine::Core)
                .expect("baseline")
                .to_json();
            let file = path();
            let mut cp = checkpoint::Request {
                path: file.clone(),
                action: checkpoint::Action::PauseAfterLevels(2),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = check_file_checkpoint_engine(&req, &cp, Engine::Core)
                .expect("pause")
                .to_json();
            assert_eq!(
                paused["checkpoint"]["status"], "paused-incomplete",
                "{paused}"
            );
            cp.action = checkpoint::Action::Recover;
            req.workers = Some(8);
            let resumed = check_file_checkpoint_engine(&req, &cp, Engine::Core)
                .expect("resume")
                .to_json();
            assert_eq!(resumed["verdict"], full["verdict"], "{resumed}");
            assert_eq!(resumed["statistics"], full["statistics"], "{resumed}");
            assert_eq!(resumed["checkpoint"]["status"], "recovered", "{resumed}");
            std::fs::remove_file(file).expect("cleanup");
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "fixture matrix includes actual-word replay and storage assertions together"
)]
fn transformed_spilled_and_disk_recovery_preserves_actual_words() {
    for (relative, tlc, store, disk) in [
        (
            "tests/fixtures/core-m1/PropStep.tla",
            true,
            Store::Fingerprint,
            false,
        ),
        (
            "tests/fixtures/core-m1/PropRefine.tla",
            false,
            Store::Exact,
            false,
        ),
        (
            "tests/fixtures/storage-tuple-function/specs/Apply/TupleStorage.tla",
            true,
            Store::Exact,
            false,
        ),
        (
            "tests/fixtures/storage-tuple-function/specs/Witness/TupleStorage.tla",
            false,
            Store::Exact,
            true,
        ),
        (
            "tests/fixtures/core-m0/CompoundTrace.tla",
            true,
            Store::Exact,
            false,
        ),
        (
            "tests/fixtures/core-m1/ViewMerge.tla",
            false,
            Store::Fingerprint,
            false,
        ),
        (
            "tests/fixtures/core-m1/SymView.tla",
            true,
            Store::Exact,
            false,
        ),
        (
            "tests/fixtures/core-m1/SymSets.tla",
            false,
            Store::Exact,
            true,
        ),
        (
            "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
            true,
            Store::Exact,
            true,
        ),
    ] {
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
        let mut req = CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            compat_tlc: tlc,
            store,
            workers: Some(4),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let full = check_file_engine(&req, Engine::Core)
            .expect("baseline")
            .to_json();
        req.frontier_memory_bytes = Some(0);
        req.predecessor_memory_bytes = Some(0);
        if disk {
            req.seen_disk = Some(dive_api::SeenOptions::default());
        }
        let file = path();
        let mut cp = checkpoint::Request {
            path: file.clone(),
            action: checkpoint::Action::PauseAfterLevels(0),
            counters: checkpoint::Counters::Retain,
            config_source_sha256: None,
        };
        let paused = check_file_checkpoint_engine(&req, &cp, Engine::Core)
            .expect("pause")
            .to_json();
        assert_eq!(
            paused["checkpoint"]["status"], "paused-incomplete",
            "{relative}: {paused}"
        );
        cp.action = checkpoint::Action::Recover;
        req.workers = Some(8);
        // The default engine recovers the Core archive.
        let resumed = dive_api::check_file_checkpoint(&req, &cp)
            .expect("resume")
            .to_json();
        for field in ["verdict", "statistics", "counterexample", "property", "tlc"] {
            assert_eq!(resumed[field], full[field], "{relative}/{field}: {resumed}");
        }
        assert_eq!(resumed["engine"], "core");
        if !resumed["counterexample"].is_null() {
            use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
            let model = SpecModel::load(
                &req.file,
                req.config.as_ref().expect("cfg"),
                &TraceOptions {
                    compat_tlc: tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )
            .expect("fresh validator");
            let trace = TraceDoc::from_json(&resumed).expect("actual word");
            assert_eq!(
                model.try_validate(&trace).expect("replay").validation,
                Validation::Valid
            );
            if relative.ends_with("CompoundTrace.tla") {
                let controls = model.controls(&trace);
                assert!(controls.positive_ok());
                assert_ne!(controls.mutants.len(), 0);
                assert_eq!(controls.failures(), Vec::<String>::new());
            }
        }
        std::fs::remove_file(file).expect("cleanup");
    }
}

#[test]
fn subprocess_resume_worker() {
    let Some(file) = std::env::var_os("DIVE_CORE_CP_CHILD_ARCHIVE") else {
        return;
    };
    let output = std::env::var_os("DIVE_CORE_CP_CHILD_OUTPUT").expect("output");
    let cp = checkpoint::Request {
        path: file.into(),
        action: checkpoint::Action::Recover,
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let result = dive_api::check_file_checkpoint(&req(), &cp)
        .expect("fresh process resume")
        .to_json();
    std::fs::write(output, serde_json::to_vec(&result).expect("json")).expect("result");
}
#[test]
fn fresh_process_resumes_and_tlc_counter_reset_is_explicit() {
    let req = req();
    let file = path();
    let output = path();
    let mut cp = checkpoint::Request {
        path: file.clone(),
        action: checkpoint::Action::PauseAfterLevels(2),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let paused = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("pause")
        .to_json();
    assert_eq!(
        paused["checkpoint"]["status"], "paused-incomplete",
        "{paused}"
    );
    let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "subprocess_resume_worker"])
        .env("DIVE_CORE_CP_CHILD_ARCHIVE", &file)
        .env("DIVE_CORE_CP_CHILD_OUTPUT", &output)
        .status()
        .expect("child");
    assert!(status.success());
    let resumed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).expect("read child")).expect("json");
    assert_eq!(resumed["verdict"], "success", "{resumed}");
    assert_eq!(resumed["statistics"]["generated"], 6);
    cp.action = checkpoint::Action::Recover;
    cp.counters = checkpoint::Counters::Tlc;
    let reset = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("reset")
        .to_json();
    assert_eq!(reset["statistics"]["generated"], 6);
    assert_eq!(reset["statistics"]["initial_generated"], 3, "{reset}");
    std::fs::remove_file(file).expect("cleanup");
    std::fs::remove_file(output).expect("cleanup output");
}

fn section_ranges(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut at = 0;
    let mut ranges = Vec::new();
    for _ in 0..4 {
        let mut n = [0; 8];
        n.copy_from_slice(&bytes[at..at + 8]);
        at += 8;
        let size = usize::try_from(u64::from_le_bytes(n)).expect("section");
        ranges.push(at..at + size);
        at += size;
    }
    ranges
}
#[test]
fn malformed_archive_identity_no_replace_and_storage_failures_never_certify() {
    let req = req();
    let file = path();
    let cp = checkpoint::Request {
        path: file.clone(),
        action: checkpoint::Action::PauseAfterLevels(2),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let paused = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("pause")
        .to_json();
    assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
    let before = std::fs::read(&file).expect("bytes");
    let duplicate = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("no replace")
        .to_json();
    assert_ne!(duplicate["verdict"], "success");
    assert!(duplicate["checkpoint"].is_null());
    assert_eq!(std::fs::read(&file).expect("still exists"), before);
    let (payload, _, _) = checkpoint::read_payload(&file, b"DIVECC01").expect("envelope");
    let ranges = section_ranges(&payload);
    // Alter real search facts while recomputing the enclosing checksum: framing
    // integrity alone cannot admit a bad parent/frontier/key/owned coordinate.
    for (offset, word) in [(16, 99u64), (80, 0), (72, 123), (96, 1), (112, 123)] {
        let bad = path();
        let mut bytes = payload.clone();
        let at = ranges[3].start + offset;
        bytes[at..at + 8].copy_from_slice(&word.to_le_bytes());
        checkpoint::write_payload(&bad, b"DIVECC01", &bytes).expect("negative envelope");
        let request = checkpoint::Request {
            path: bad.clone(),
            action: checkpoint::Action::Recover,
            ..cp.clone()
        };
        let report = check_file_checkpoint_engine(&req, &request, Engine::Core)
            .expect("named reject")
            .to_json();
        assert_ne!(report["verdict"], "success", "offset{offset}: {report}");
        assert!(report["counterexample"].is_null());
        assert!(
            report["checkpoint"].is_null(),
            "malformed data is not an imported search: {report}"
        );
        std::fs::remove_file(bad).expect("cleanup");
    }
    let mut changed = req.clone();
    changed.compat_tlc = true;
    let recover = checkpoint::Request {
        action: checkpoint::Action::Recover,
        ..cp.clone()
    };
    let mismatch = check_file_checkpoint_engine(&changed, &recover, Engine::Core)
        .expect("identity refusal")
        .to_json();
    assert!(
        mismatch["message"]
            .as_str()
            .expect("reason")
            .contains("identity mismatch")
    );
    assert!(mismatch["checkpoint"].is_null());
    let mut disk = req.clone();
    let mut seen = dive_api::SeenOptions::default();
    seen.limits.entries = 1;
    disk.seen_disk = Some(seen);
    let failed = check_file_checkpoint_engine(&disk, &recover, Engine::Core)
        .expect("fallible restore")
        .to_json();
    assert_eq!(failed["verdict"], "resource-limit", "{failed}");
    assert_eq!(
        failed["seen_storage"]["first_failure"]["phase"], "Insertion",
        "{failed}"
    );
    assert!(failed["counterexample"].is_null());
    let corrupt = path();
    let mut bytes = before;
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&corrupt, bytes).expect("corrupt");
    let bad = checkpoint::Request {
        path: corrupt.clone(),
        action: checkpoint::Action::Recover,
        ..cp
    };
    assert!(dive_api::check_file_checkpoint(&req, &bad).is_err());
    std::fs::remove_file(corrupt).expect("cleanup");
    std::fs::remove_file(file).expect("cleanup");
}

#[test]
fn configured_spec_counter_policy_and_prebinding_refusals() {
    let directory = path();
    std::fs::create_dir(&directory).expect("directory");
    let file = directory.join("Recovery.tla");
    let config = file.with_extension("cfg");
    std::fs::write(&file,"---- MODULE Recovery ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in {0,1}\nNext == x' = IF x<3 THEN x+1 ELSE x\nSpec == Init /\\ [][Next]_x\n====\n").expect("source");
    std::fs::write(&config, "SPECIFICATION Spec\n").expect("config");
    let mut req = CheckRequest {
        file,
        config: Some(config.clone()),
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..CheckRequest::default()
    };
    let full = check_file_engine(&req, Engine::Core)
        .expect("baseline")
        .to_json();
    assert_eq!(full["statistics"]["generated"], 6, "{full}");
    let mut cp = checkpoint::Request {
        path: directory.join("saved"),
        action: checkpoint::Action::PauseAfterLevels(2),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let paused = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("pause")
        .to_json();
    assert_eq!(
        paused["checkpoint"]["status"], "paused-incomplete",
        "{paused}"
    );
    cp.action = checkpoint::Action::Recover;
    let retained = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("retain")
        .to_json();
    assert_eq!(retained["statistics"], full["statistics"]);
    cp.counters = checkpoint::Counters::Tlc;
    let reset = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("reset")
        .to_json();
    assert_eq!(reset["statistics"]["generated"], 5, "{reset}");
    assert_eq!(reset["statistics"]["initial_generated"], 4);
    std::fs::write(
        &config,
        "SPECIFICATION Spec\n\\* comment changes identity\n",
    )
    .expect("config comment");
    let rejected = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("source identity")
        .to_json();
    assert!(rejected["checkpoint"].is_null());
    assert_ne!(rejected["verdict"], "success");
    req.time_limit = dive_api::BudgetSetting::Explicit(std::time::Duration::ZERO);
    let stopped = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("budget before startup")
        .to_json();
    assert_ne!(stopped["verdict"], "success");
    assert!(stopped["checkpoint"].is_null());
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn core_effect_and_temporal_refusals_do_not_execute_or_commit() {
    let directory = path();
    std::fs::create_dir(&directory).expect("dir");
    for (case, definition, config) in [
        ("Effect", "Unused == PrintT(1)", "INIT Init\nNEXT Next\n"),
        (
            "Temporal",
            "Property == []<>(x=1)",
            "INIT Init\nNEXT Next\nPROPERTY Property\n",
        ),
    ] {
        let file = directory.join(format!("{case}.tla"));
        let cfg = file.with_extension("cfg");
        let archive = directory.join(format!("{case}.cp"));
        std::fs::write(&file,format!("---- MODULE {case} ----\nEXTENDS Naturals,TLC\nVARIABLE x\n{definition}\nInit == x=0\nNext == x'=1-x\n====\n")).expect("source");
        std::fs::write(&cfg, config).expect("config");
        let req = CheckRequest {
            file,
            config: Some(cfg),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let cp = checkpoint::Request {
            path: archive.clone(),
            action: checkpoint::Action::PauseAfterLevels(0),
            counters: checkpoint::Counters::Retain,
            config_source_sha256: None,
        };
        let report = check_file_checkpoint_engine(&req, &cp, Engine::Core)
            .expect("refusal")
            .to_json();
        assert_eq!(report["verdict"], "unsupported", "{report}");
        assert_eq!(report["output"], serde_json::json!([]));
        assert!(report["checkpoint"].is_null());
        assert!(!archive.exists());
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn backend_disk_policy_admits_before_archive_or_model_loading() {
    use dive_api::ApiError;
    // The retired reference engine refused disk seen-state with checkpoints
    // and wrote the archives Core rejected (#791). Core validates the disk
    // policy, then opens the archive, before the model loads.
    let missing = path();
    let mut request = CheckRequest {
        file: missing.with_extension("tla"),
        seen_disk: Some(dive_api::SeenOptions::default()),
        ..CheckRequest::default()
    };
    let cp = checkpoint::Request {
        path: missing.clone(),
        action: checkpoint::Action::Recover,
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    assert!(matches!(
        check_file_checkpoint_engine(&request, &cp, Engine::Core),
        Err(ApiError::Rejected { message }) if message.contains("DIVE-S0400")
    ));
    request.seen_disk.as_mut().expect("policy").limits.entries = 0;
    assert!(matches!(
        check_file_checkpoint_engine(&request, &cp, Engine::Core),
        Err(ApiError::Rejected { message }) if message.contains("DIVE-S0700")
    ));
    assert!(!missing.exists());
}
