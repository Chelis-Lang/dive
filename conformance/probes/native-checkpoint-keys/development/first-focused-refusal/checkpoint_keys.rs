//! Transformed key admission refuses coherent corruption rather than inventing model verdicts.
#![allow(clippy::expect_used, reason = "native archive controls")]
use dive_check::{CheckOptions, Verdict, checkpoint};
use sha2::{Digest, Sha256};
use std::sync::Arc;

fn directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "dive-native-key-check-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).expect("directory");
    p
}
fn write(path: &std::path::Path, value: &serde_json::Value) {
    let payload = serde_json::to_vec(value).expect("payload");
    let mut bytes = b"DIVECP01".to_vec();
    bytes.extend((payload.len() as u64).to_le_bytes());
    bytes.extend(Sha256::digest(&payload));
    bytes.extend(payload);
    std::fs::write(path, bytes).expect("coherent checksum");
}

#[test]
fn configured_view_key_failure_is_import_failure_not_a_model_verdict() {
    let analysis = dive_sema::analyze_source(
        "ViewKey.tla",
        "---- MODULE ViewKey ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<4 /\\ x'=x+1\nView == x+0\n====\n",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nVIEW View\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    let dir = directory();
    for store in [
        dive_store::StoreMode::Fingerprint,
        dive_store::StoreMode::Exact,
    ] {
        for mut options in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
            options.store = store;
            let cp = checkpoint::Request {
                path: dir.join(format!("{store:?}-{:?}.cp", options.profile)),
                action: checkpoint::Action::PauseAfterLevels(1),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = checkpoint::check(&ir, &cfg, &options, &cp);
            assert!(paused.receipt.is_some(), "{:?}", paused.check);
            let raw = std::fs::read(&cp.path).expect("archive");
            let original: serde_json::Value = serde_json::from_slice(&raw[48..]).expect("payload");
            for kind in [0, 1] {
                let mut bad = original.clone();
                if kind == 0 {
                    // TRUE is a correctly encoded single concrete value, but
                    // cannot be evaluated by the configured arithmetic VIEW.
                    bad["discoveries"][0]["state"] = serde_json::json!([1]);
                } else {
                    let key = bad["discoveries"][0]["key"].as_array_mut().expect("key");
                    key[0] = serde_json::json!(key[0].as_u64().expect("byte") ^ 255);
                }
                let path = dir.join(format!("bad-{store:?}-{:?}-{kind}.cp", options.profile));
                write(&path, &bad);
                let recovered = checkpoint::check(
                    &ir,
                    &cfg,
                    &options,
                    &checkpoint::Request {
                        path,
                        action: checkpoint::Action::Recover,
                        ..cp.clone()
                    },
                );
                let Verdict::ResourceLimit { message } = &recovered.check.verdict else {
                    panic!("{:?}", recovered.check);
                };
                assert!(
                    message.contains(if kind == 0 {
                        "configured state/key admission"
                    } else {
                        "state/key mismatch"
                    }),
                    "{message}"
                );
                assert!(recovered.receipt.is_none());
                assert!(recovered.check.counterexample.is_none());
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
