//! A run that ends in Init reports the one worker that evaluated it.
#![allow(clippy::expect_used, reason = "worker report regression assertions")]

use dive_api::{CheckRequest, check_file};
use std::path::Path;

fn request(workers: usize) -> CheckRequest {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/native-checkpoint-keys/CpSymView/CpSymView.tla");
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(workers),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

#[test]
fn a_spill_failure_in_init_reports_one_worker_and_a_full_search_reports_the_width() {
    let dir = std::env::temp_dir().join(format!("dive-init-workers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("private directory");
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"not a directory").expect("block spill parent");
    for compat_tlc in [false, true] {
        let full = check_file(&CheckRequest {
            compat_tlc,
            ..request(8)
        })
        .expect("full search")
        .to_json();
        assert!(
            full["statistics"]["depth"].as_u64().expect("depth") > 1,
            "{full}"
        );
        assert_eq!(full["options"]["workers"]["requested"], 8);
        assert_eq!(full["options"]["workers"]["used"], 8, "{full}");
        for (frontier, predecessor) in [(true, false), (false, true), (true, true)] {
            let failed = check_file(&CheckRequest {
                compat_tlc,
                frontier_memory_bytes: frontier.then_some(0),
                frontier_directory: frontier.then(|| blocked.clone()),
                predecessor_memory_bytes: predecessor.then_some(0),
                predecessor_directory: predecessor.then(|| blocked.clone()),
                ..request(8)
            })
            .expect("storage failure")
            .to_json();
            assert_eq!(failed["verdict"], "resource-limit", "{failed}");
            assert_eq!(
                failed["statistics"]["generated"],
                failed["statistics"]["initial_generated"]
            );
            assert_eq!(failed["options"]["workers"]["requested"], 8);
            // Init runs on one worker; no parallel level started.
            assert_eq!(failed["options"]["workers"]["used"], 1, "{failed}");
            assert!(
                failed["options"]["workers"]["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("ended in Init")),
                "{failed}"
            );
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn temporal_capture_reports_inline_width_only_for_wide_cheap_initial_frontiers() {
    let dir = std::env::temp_dir().join(format!("dive-temporal-width-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("private directory");
    let file = dir.join("Cheap.tla");
    let config = dir.join("Cheap.cfg");
    std::fs::write(&config, "INIT Init\nNEXT Next\nPROPERTY Holds\n").expect("config");
    for (max, next, expected) in [
        (4095, "x' = x", 1),
        (4094, "x' = x", 8),
        (4095, "x' \\in {x, 0}", 8),
    ] {
        std::fs::write(&file, format!("---- MODULE Cheap ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..{max}\nNext == {next}\nHolds == []<>(x >= 0)\n====\n")).expect("module");
        let check = |width| {
            check_file(&CheckRequest {
                file: file.clone(),
                config: Some(config.clone()),
                workers: Some(width),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })
            .expect("finite check")
            .to_json()
        };
        let one = check(1);
        let eight = check(8);
        assert_eq!(eight["check_kind"], "temporal", "{eight}");
        assert_eq!(eight["verdict"], "success", "{eight}");
        assert_eq!(eight["statistics"], one["statistics"]);
        let workers = &eight["options"]["workers"];
        assert_eq!(workers["used"], expected, "{workers}");
        let scope = &eight["core"]["temporal_workers"];
        assert_eq!(scope["width_applies_to"], "source-expansion");
        assert_eq!(
            scope["serial_phases"],
            serde_json::json!([
                "initial-states",
                "ordered-commit",
                "temporal-analysis",
                "trace-validation"
            ])
        );
        if expected == 1 {
            assert_eq!(workers["sequential"], true);
            assert!(
                workers["reason"]
                    .as_str()
                    .is_some_and(|why| why.contains("4096 initial states"))
            );
        } else {
            assert!(workers["reason"].is_null());
        }
    }
    std::fs::remove_dir_all(dir).expect("remove fixture");
}
