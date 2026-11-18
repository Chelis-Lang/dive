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
