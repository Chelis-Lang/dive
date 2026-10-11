//! Imported runtime-name tables preserve bounded structural refusal.
#![allow(clippy::expect_used, reason = "coherent archive regression controls")]
use dive_check::{CheckOptions, Verdict, checkpoint};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[test]
fn corrupt_name_tables_refuse_without_continuation_or_a_witness() {
    let source = include_str!(
        "../../../conformance/probes/native-checkpoint-runtime-names/specs/RuntimeStrings/RuntimeStrings.tla"
    );
    let analysis =
        dive_sema::analyze_source("RuntimeStrings.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let cfg=dive_config::parse(include_str!("../../../conformance/probes/native-checkpoint-runtime-names/specs/RuntimeStrings/RuntimeStrings.cfg")).into_config().expect("cfg");
    let dir =
        std::env::temp_dir().join(format!("dive-recover-names-corrupt-{}", std::process::id()));
    std::fs::create_dir(&dir).expect("private directory");
    for profile in [false, true] {
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            let mut opts = if profile {
                CheckOptions::tlc(0, 7)
            } else {
                CheckOptions::dive(7)
            };
            opts.store = store;
            opts.workers = 8;
            opts.chaos = Some(19);
            let cp = checkpoint::Request {
                path: dir.join(format!("{profile}-{store:?}.cp")),
                action: checkpoint::Action::PauseAfterLevels(2),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = checkpoint::check(&ir, &cfg, &opts, &cp);
            assert!(paused.receipt.is_some(), "{:?}", paused.check);
            let raw = std::fs::read(&cp.path).expect("archive");
            let original: serde_json::Value = serde_json::from_slice(&raw[48..]).expect("payload");
            let names = original["names"].as_array().expect("names");
            let runtime = names
                .iter()
                .position(|name| name == "2")
                .expect("new runtime token");
            let initial = checkpoint::check(
                &ir,
                &cfg,
                &opts,
                &checkpoint::Request {
                    path: dir.join(format!("initial-{profile}-{store:?}.cp")),
                    action: checkpoint::Action::PauseAfterLevels(0),
                    ..cp.clone()
                },
            );
            assert!(initial.receipt.is_some());
            let before = std::fs::read(dir.join(format!("initial-{profile}-{store:?}.cp")))
                .expect("initial archive");
            let start: serde_json::Value =
                serde_json::from_slice(&before[48..]).expect("initial payload");
            assert!(
                !start["names"]
                    .as_array()
                    .expect("startup feed")
                    .iter()
                    .any(|name| name == "2")
            );
            for kind in 0..3 {
                let mut bad = original.clone();
                let names = bad["names"].as_array_mut().expect("names");
                match kind {
                    0 => names.swap(0, 1),
                    1 => {
                        let duplicate = names[0].clone();
                        names.push(duplicate);
                    }
                    _ => {
                        names.remove(runtime);
                    }
                }
                let payload = serde_json::to_vec(&bad).expect("payload");
                let mut bytes = b"DIVECP01".to_vec();
                bytes.extend((payload.len() as u64).to_le_bytes());
                bytes.extend(Sha256::digest(&payload));
                bytes.extend(payload);
                let path = dir.join(format!("bad-{profile}-{store:?}-{kind}.cp"));
                std::fs::write(&path, bytes).expect("coherent checksum");
                let result = checkpoint::check(
                    &ir,
                    &cfg,
                    &opts,
                    &checkpoint::Request {
                        path,
                        action: checkpoint::Action::Recover,
                        ..cp.clone()
                    },
                );
                assert!(
                    matches!(
                        result.check.verdict,
                        Verdict::ResourceLimit { .. } | Verdict::Unsupported { .. }
                    ),
                    "{kind}: {:?}",
                    result.check
                );
                assert!(result.receipt.is_none());
                assert!(result.check.counterexample.is_none());
                assert!(result.check.output.is_empty());
                if kind == 2 {
                    // The decoder imported exactly the two valid predecessors
                    // before the missing token: truthful prefix evidence, not
                    // Init/Next continuation or a recovered certificate.
                    let stats = result.check.stats.expect("validated imported prefix");
                    assert_eq!(stats.generated, 0);
                    assert_eq!(stats.initial_generated, 0);
                    assert_eq!(stats.distinct, 2);
                    assert_eq!(stats.depth, 0);
                    assert_eq!(stats.left_on_queue, 0);
                    assert_eq!(result.check.workers.used, 1);
                    assert!(result.check.workers.sequential);
                    assert_eq!(
                        result.check.workers.reason.as_deref(),
                        Some("native checkpoint admission stopped before the worker pool started")
                    );
                } else {
                    assert!(result.check.stats.is_none());
                    assert_eq!(result.check.workers.used, 0);
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
