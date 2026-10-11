//! The public checker exposes the VIEW/RNG no-work refusal in both profiles.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_api::{CheckRequest, Store, check_file};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[test]
fn public_view_random_refusal_never_claims_workers_output_or_witness() {
    let dir = std::env::temp_dir().join(format!(
        "dive-view-random-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).expect("private test directory");
    let source = dir.join("RandomView.tla");
    std::fs::write(&source, "---- MODULE RandomView ----\nEXTENDS Naturals, TLC\nVARIABLES n,x\nASSUME PrintT(\"must not print\")\nStartup == PrintT(\"must not print\")\nInit == n=0 /\\ x=0\nNext == n'=n+1 /\\ x'=RandomElement(1..100)\nInv == n<3\nV == n\n====\n").expect("source");
    std::fs::write(
        dir.join("RandomView.cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW V\n",
    )
    .expect("config");
    for compat in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let report = check_file(&CheckRequest {
                        file: source.clone(),
                        compat_tlc: compat,
                        seed: Some(7),
                        fp: Some(0),
                        workers: Some(workers),
                        store,
                        chaos,
                        ..CheckRequest::default()
                    })
                    .expect("well-formed request");
                    let json = report.to_json();
                    assert_eq!(json["verdict"], "unsupported");
                    assert_eq!(report.exit_code(), 3);
                    assert_eq!(json["options"]["workers"]["used"], 0);
                    assert_eq!(json["options"]["workers"]["requested"], workers);
                    assert!(
                        json["statistics"].is_null()
                            && json["counterexample"].is_null()
                            && json["collision"].is_null()
                    );
                    assert_eq!(json["output"], serde_json::json!([]));
                    assert_eq!(json["warnings"], serde_json::json!([]));
                    assert!(report.render_human().contains("randomness under VIEW needs transformed-fingerprint seeding and independent replay"));
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
