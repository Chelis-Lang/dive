//! Configured RNG soundness repair reaches the public API before effects.
#![allow(clippy::expect_used, reason = "test assertions")]
#[path = "support/memory_observations.rs"]
mod memory_observations;
#[path = "support/refused.rs"]
mod refused;

use dive_api::{CheckRequest, Store};
#[test]
fn configured_random_corrects_both_wrong_verdicts_and_matches_direct_api_reports() {
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
                        let request = CheckRequest {
                            file: file.clone(),
                            compat_tlc: compat,
                            seed: Some(7),
                            fp: Some(0),
                            workers: Some(width),
                            store,
                            chaos,
                            ..CheckRequest::default()
                        };
                        let report = dive_api::check_file(&request).expect("well-formed request");
                        if chaos.is_some() {
                            // Only the retired reference engine ran --chaos
                            // (#791, #910).
                            refused::assert_refused(
                                &report.to_json(),
                                report.exit_code(),
                                refused::CHAOS,
                            );
                            continue;
                        }
                        let original = std::fs::read_to_string(&file).expect("source");
                        let direct = original.replace("x'=C(", "x'=RandomElement(");
                        std::fs::write(&file, &direct).expect("direct counterpart");
                        let expected = dive_api::check_file(&request).expect("direct request");
                        std::fs::write(&file, &original).expect("restore configured source");
                        let mut j = report.to_json();
                        let mut counterpart = expected.to_json();
                        assert_eq!(report.exit_code(), expected.exit_code());
                        if j["verdict"] == "unsupported" {
                            assert!(compat && inv == "x # 80", "{j}");
                            assert_eq!(
                                j["message"],
                                "TLC initial-predecessor random counterexample replay is not proved"
                            );
                            assert!(j["counterexample"].is_null());
                        }
                        assert_eq!(j["options"]["workers"]["requested"], width);
                        assert_eq!(j["options"]["workers"]["used"], 1);
                        assert_eq!(
                            j["output"],
                            serde_json::json!(["\"must not print\"", "\"must not print\""])
                        );
                        // Elapsed time and process observations vary between runs; the
                        // direct spelling lengthens the Next action location. Preserve
                        // all concrete states, certification, metadata and messages.
                        for doc in [&mut j, &mut counterpart] {
                            memory_observations::normalize(doc);
                            doc["elapsed_ms"] = serde_json::json!(0);
                            if let Some(states) = doc["counterexample"]["states"].as_array_mut() {
                                for state in states {
                                    if state["index"] != 1 {
                                        state["label"] = serde_json::Value::Null;
                                    }
                                }
                            }
                            if let Some(states) =
                                doc["counterexample"]["trace"]["states"].as_array_mut()
                            {
                                for state in states.iter_mut().skip(1) {
                                    assert!(state["action"]["location"].is_string());
                                    state["action"]["location"] = serde_json::Value::Null;
                                }
                            }
                        }
                        assert_eq!(j, counterpart);
                        if compat {
                            assert_eq!(
                                j["verdict"],
                                if inv == "x # 74" {
                                    "success"
                                } else {
                                    "unsupported"
                                }
                            );
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn property_bearing_final_routes_do_not_return_configured_rng_false_verdicts() {
    let dir = std::env::temp_dir().join(format!(
        "dive-configured-final-random-{}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).expect("private directory");
    let file = dir.join("ConfiguredFinal.tla");
    for live in ["<> (n=3)", "[]<> (n>=0)"] {
        for inv in ["x # 74", "x # 80"] {
            std::fs::write(&file,format!("---- MODULE ConfiguredFinal ----\nEXTENDS Naturals,TLC\nCONSTANT C(_)\nVARIABLES n,x\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'=C(1..100)\nInv == {inv}\nLive == {live}\n====\n")).expect("source");
            std::fs::write(dir.join("ConfiguredFinal.cfg"),"INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\nCONSTANT C <- RandomElement\nPROPERTY Live\n").expect("cfg");
            {
                for compat in [false, true] {
                    for store in [Store::Fingerprint, Store::Exact] {
                        for width in [1, 4, 8] {
                            for chaos in [None, Some(19)] {
                                let r = dive_api::check_file(&CheckRequest {
                                    file: file.clone(),
                                    temporal_refusal: Some(refused::FINAL_SHORT.into()),
                                    compat_tlc: compat,
                                    store,
                                    workers: Some(width),
                                    seed: Some(7),
                                    fp: Some(0),
                                    chaos,
                                    ..CheckRequest::default()
                                })
                                .expect("request");
                                let j = r.to_json();
                                assert_eq!(j["verdict"], "unsupported", "{j}");
                                assert_eq!(r.exit_code(), 3);
                                assert!(j["counterexample"].is_null());
                                assert_eq!(j["options"]["workers"]["used"], 0);
                                assert!(j["statistics"].is_null());
                            }
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
