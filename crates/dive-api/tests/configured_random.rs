//! Configured RNG soundness repair reaches the public API before effects.
#![allow(clippy::expect_used, reason = "test assertions")]
use dive_api::{CheckRequest, Store, check_file};
#[test]
fn configured_random_refuses_false_positive_and_false_negative_models_without_effects() {
    let dir = std::env::temp_dir().join(format!("dive-configured-random-{}", std::process::id()));
    std::fs::create_dir(&dir).expect("private directory");
    let file = dir.join("ConfiguredRandom.tla");
    for inv in ["x # 74", "x # 80"] {
        std::fs::write(&file,format!("---- MODULE ConfiguredRandom ----\nEXTENDS Naturals,TLC\nCONSTANT C(_)\nVARIABLES n,x\nASSUME PrintT(\"must not print\")\nStartup == PrintT(\"must not print\")\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'=C(1..100)\nInv == {inv}\n====\n")).expect("source");
        std::fs::write(dir.join("ConfiguredRandom.cfg"),"INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\nCONSTANT C <- RandomElement\n").expect("cfg");
        for compat in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                for width in [1, 4, 8] {
                    for chaos in [None, Some(19)] {
                        let report = check_file(&CheckRequest {
                            file: file.clone(),
                            compat_tlc: compat,
                            seed: Some(7),
                            fp: Some(0),
                            workers: Some(width),
                            store,
                            chaos,
                            ..CheckRequest::default()
                        })
                        .expect("well-formed request");
                        let j = report.to_json();
                        assert_eq!(j["verdict"], "unsupported");
                        assert_eq!(report.exit_code(), 3);
                        assert_eq!(j["options"]["workers"]["requested"], width);
                        assert_eq!(j["options"]["workers"]["used"], 0);
                        assert!(
                            j["statistics"].is_null()
                                && j["counterexample"].is_null()
                                && j["collision"].is_null()
                        );
                        assert_eq!(j["output"], serde_json::json!([]));
                        assert_eq!(j["warnings"], serde_json::json!([]));
                        assert!(report.render_human().contains("configured native random replacements need bound-context stream classification and independent replay"));
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
