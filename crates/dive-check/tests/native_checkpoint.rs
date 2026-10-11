//! Native snapshot/resume equivalence controls.
#![allow(clippy::expect_used, reason = "test fixture controls")]
use dive_check::{CheckOptions, Verdict, checkpoint};
use std::sync::Arc;

fn model(src: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let analysis = dive_sema::analyze_source("Checkpoint.tla", src, &dive_sema::Options::default());
    assert!(
        analysis.accepted(),
        "sema {:?}, syntax {:?}",
        analysis.diagnostics(),
        analysis.syntax_diagnostics()
    );
    (
        Arc::new(dive_ir::lower(&analysis).expect("lower")),
        dive_config::parse(cfg).into_config().expect("config"),
    )
}
fn directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "dive-native-checkpoint-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).expect("dir");
    path
}

#[test]
fn actual_resume_matches_uninterrupted_and_tlc_reset_is_explicit() {
    let (ir, cfg) = model(
        "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x < 7 /\\ x' = x+1\nInv == x < 6\n====\n",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    );
    let dir = directory();
    for profile in [false, true] {
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            for width in [1, 4, 8] {
                for boundary in [0, 1, 3] {
                    let mut opts = if profile {
                        CheckOptions::tlc(0, 7)
                    } else {
                        CheckOptions::dive(7)
                    };
                    opts.workers = width;
                    opts.store = store;
                    opts.chaos = Some(17);
                    let full = dive_check::check(&ir, &cfg, &opts);
                    assert!(matches!(full.verdict, Verdict::InvariantViolated { .. }));
                    let path = dir.join(format!("{profile}-{store:?}-{width}-{boundary}.cp"));
                    let request = checkpoint::Request {
                        path: path.clone(),
                        action: checkpoint::Action::PauseAfterLevels(boundary),
                        counters: checkpoint::Counters::Retain,
                        config_source_sha256: None,
                    };
                    let pause = checkpoint::check(&ir, &cfg, &opts, &request);
                    assert!(
                        matches!(pause.check.verdict, Verdict::ResourceLimit { .. }),
                        "{:?}",
                        pause.check
                    );
                    assert_eq!(
                        pause.receipt.as_ref().expect("committed").status,
                        "paused-incomplete"
                    );
                    assert_eq!(
                        pause.check.workers.used,
                        if boundary == 0 { 1 } else { width }
                    );
                    assert!(pause.check.counterexample.is_none());
                    let resume = checkpoint::check(
                        &ir,
                        &cfg,
                        &opts,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            ..request.clone()
                        },
                    );
                    assert_eq!(resume.check.verdict, full.verdict, "{:?}", resume.check);
                    assert_eq!(resume.check.stats, full.stats);
                    let trace = resume.check.counterexample.as_ref().expect("resumed trace");
                    let reference = full.counterexample.as_ref().expect("reference trace");
                    assert_eq!(trace.states.len(), reference.states.len());
                    for (a, b) in trace.states.iter().zip(&reference.states) {
                        assert_eq!(a.values, b.values);
                        assert_eq!(a.action, b.action);
                    }
                    let tlc = checkpoint::check(
                        &ir,
                        &cfg,
                        &opts,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            counters: checkpoint::Counters::Tlc,
                            ..request
                        },
                    );
                    let original = full.stats.expect("stats");
                    let partial = pause.check.stats.expect("paused stats");
                    let reset = tlc.check.stats.expect("reset stats");
                    assert_eq!(
                        reset.generated,
                        original.generated - partial.generated + partial.distinct
                    );
                    assert_eq!(reset.distinct, original.distinct);
                    assert_eq!(tlc.check.verdict, full.verdict);
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn native_refuses_effects_and_identity_mismatch_without_a_verdict() {
    let dir = directory();
    for (source, cfg) in [
        (
            "---- MODULE Checkpoint ----\nEXTENDS Naturals, TLC\nVARIABLE x\nU == Print(1,2)\nInit == x=0\nNext == x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "---- MODULE Checkpoint ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=TLCGet(\"config\").worker\nNext == x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "---- MODULE Checkpoint ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=RandomElement({1,2})\nNext == x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
        ),
    ] {
        let (ir, cfg) = model(source, cfg);
        let path = dir.join("effects.cp");
        let request = checkpoint::Request {
            path: path.clone(),
            action: checkpoint::Action::PauseAfterLevels(0),
            counters: checkpoint::Counters::Tlc,
            config_source_sha256: None,
        };
        let r = checkpoint::check(&ir, &cfg, &CheckOptions::tlc(0, 7), &request);
        assert!(
            matches!(r.check.verdict, Verdict::Unsupported { .. }),
            "{:?}",
            r.check
        );
        assert_eq!(r.check.output, Vec::<String>::new());
        assert_eq!(r.check.workers.used, 0);
        assert!(r.check.stats.is_none());
        assert!(!path.exists());
    }
    let (ir, cfg) = model(
        "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<3 /\\ x'=x+1\n====\n",
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    );
    let request = checkpoint::Request {
        path: dir.join("valid.cp"),
        action: checkpoint::Action::PauseAfterLevels(1),
        counters: checkpoint::Counters::Tlc,
        config_source_sha256: None,
    };
    let original = CheckOptions::tlc(0, 7);
    assert!(
        checkpoint::check(&ir, &cfg, &original, &request)
            .receipt
            .is_some()
    );
    let mut variants = Vec::new();
    let mut a = original.clone();
    a.seed = 8;
    variants.push(a);
    let mut a = original.clone();
    a.fp_index = 1;
    variants.push(a);
    variants.push(CheckOptions::dive(7));
    let mut a = original.clone();
    a.no_deadlock = true;
    variants.push(a);
    for opts in variants {
        let r = checkpoint::check(
            &ir,
            &cfg,
            &opts,
            &checkpoint::Request {
                action: checkpoint::Action::Recover,
                ..request.clone()
            },
        );
        assert!(matches!(r.check.verdict, Verdict::ResourceLimit { .. }));
        assert_eq!(r.check.workers.used, 0);
        assert!(r.check.stats.is_none());
        assert_eq!(r.check.output, Vec::<String>::new());
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn coherently_checksummed_bad_state_keys_names_and_frontiers_refuse_without_receipt() {
    use sha2::{Digest, Sha256};
    let (ir, cfg) = model(
        "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<4 /\\ x'=x+1\n====\n",
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    );
    let dir = directory();
    let opts = CheckOptions::tlc(0, 7);
    let pause_request = checkpoint::Request {
        path: dir.join("valid.cp"),
        action: checkpoint::Action::PauseAfterLevels(1),
        counters: checkpoint::Counters::Tlc,
        config_source_sha256: None,
    };
    let pause = checkpoint::check(&ir, &cfg, &opts, &pause_request);
    assert!(pause.receipt.is_some());
    let raw = std::fs::read(&pause_request.path).expect("snapshot");
    let original: serde_json::Value = serde_json::from_slice(&raw[48..]).expect("payload");
    for kind in 0..8 {
        let mut json = original.clone();
        match kind {
            0 => json["discoveries"][0]["state"] = serde_json::json!([8, 255, 255, 255, 255, 15]),
            1 => json["discoveries"][0]["state"] = serde_json::json!([4, 255, 255, 255, 255, 15]),
            2 => json["discoveries"][0]["key"] = serde_json::json!([1, 2, 3]),
            3 => json["names"][0] = serde_json::json!("changed startup token"),
            4 => json["discoveries"][1]["parent"] = serde_json::json!(1),
            5 => json["frontier"] = serde_json::json!([]),
            6 => {
                json["generated"] = serde_json::json!(u64::MAX);
                json["initial_generated"] = serde_json::json!(u64::MAX);
            }
            _ => json["discoveries"][1]["key"] = json["discoveries"][0]["key"].clone(),
        }
        let payload = serde_json::to_vec(&json).expect("serialize");
        let mut mutated = b"DIVECP02".to_vec();
        mutated.extend((payload.len() as u64).to_le_bytes());
        mutated.extend(Sha256::digest(&payload));
        mutated.extend(payload);
        let path = dir.join(format!("bad-{kind}.cp"));
        std::fs::write(&path, mutated).expect("checksummed negative");
        let result = checkpoint::check(
            &ir,
            &cfg,
            &opts,
            &checkpoint::Request {
                path,
                action: checkpoint::Action::Recover,
                ..pause_request.clone()
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
        assert!(
            !format!("{:?}", result.check.verdict).contains("native magic/schema mismatch"),
            "{kind}: coherent current-format corruption must reach payload admission"
        );
        assert!(
            result.receipt.is_none(),
            "{kind}: invalid import cannot have a recovered receipt"
        );
        assert!(result.check.counterexample.is_none());
        assert_eq!(result.check.output, Vec::<String>::new());
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn concrete_compound_model_values_and_name_prefixes_restore_losslessly() {
    let dir = directory();
    for (case, source, config) in [
        (
            "record",
            "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = [a |-> {\"z\", \"a\"}, b |-> 0]\nNext == x.b < 3 /\\ x' = [x EXCEPT !.b = @ + 1]\nInv == x.b<2\n====\n",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        ),
        (
            "model-values",
            "---- MODULE Checkpoint ----\nCONSTANT A, B\nVARIABLE x\nInit == x = <<A, [tag |-> B]>>\nNext == x' = x\n====\n",
            "CONSTANT A=a B=b\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
        ),
    ] {
        let (ir, cfg) = model(source, config);
        for profile in [false, true] {
            let opts = if profile {
                CheckOptions::tlc(0, 7)
            } else {
                CheckOptions::dive(7)
            };
            let original = dive_check::check(&ir, &cfg, &opts);
            let request = checkpoint::Request {
                path: dir.join(format!("{case}-{profile}.cp")),
                action: checkpoint::Action::PauseAfterLevels(0),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let pause = checkpoint::check(&ir, &cfg, &opts, &request);
            assert!(pause.receipt.is_some(), "{:?}", pause.check);
            let resumed = checkpoint::check(
                &ir,
                &cfg,
                &opts,
                &checkpoint::Request {
                    action: checkpoint::Action::Recover,
                    ..request
                },
            );
            assert_eq!(
                resumed.check.verdict, original.verdict,
                "{:?}",
                resumed.check
            );
            assert_eq!(resumed.check.stats, original.stats);
            if let Some(trace) = resumed.check.counterexample {
                let reference = original.counterexample.expect("reference trace");
                assert_eq!(trace.states.len(), reference.states.len());
                for (a, b) in trace.states.iter().zip(reference.states) {
                    assert_eq!(a.values, b.values);
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn retained_near_maximum_generated_counts_refuse_before_continuation() {
    use sha2::{Digest, Sha256};
    let (ir, cfg) = model(
        "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<4 /\\ x'=x+1\n====\n",
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    );
    let dir = directory();
    let opts = CheckOptions::tlc(0, 7);
    let pause_request = checkpoint::Request {
        path: dir.join("valid.cp"),
        action: checkpoint::Action::PauseAfterLevels(1),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    assert!(
        checkpoint::check(&ir, &cfg, &opts, &pause_request)
            .receipt
            .is_some()
    );
    let raw = std::fs::read(&pause_request.path).expect("snapshot");
    let original: serde_json::Value = serde_json::from_slice(&raw[48..]).expect("payload");
    for generated in [u64::MAX, u64::MAX - 1, u64::MAX - 2] {
        let mut value = original.clone();
        value["generated"] = serde_json::json!(generated);
        let payload = serde_json::to_vec(&value).expect("serialize");
        let mut changed = b"DIVECP02".to_vec();
        changed.extend((payload.len() as u64).to_le_bytes());
        changed.extend(Sha256::digest(&payload));
        changed.extend(payload);
        let path = dir.join(format!("bad-{generated}.cp"));
        std::fs::write(&path, changed).expect("coherent checksum");
        let result = checkpoint::check(
            &ir,
            &cfg,
            &opts,
            &checkpoint::Request {
                path,
                action: checkpoint::Action::Recover,
                ..pause_request.clone()
            },
        );
        eprintln!("retained generated {generated}: {:?}", result.check);
        assert!(matches!(
            result.check.verdict,
            Verdict::ResourceLimit { .. }
        ));
        assert!(result.receipt.is_none());
        assert_eq!(result.check.workers.used, 0);
        assert!(result.check.stats.is_none());
        assert_eq!(result.check.output, Vec::<String>::new());
    }
    for generated in [
        checkpoint::MAX_COUNTER,
        checkpoint::MAX_COUNTER - 1,
        checkpoint::MAX_COUNTER - 2,
    ] {
        let mut value = original.clone();
        value["generated"] = serde_json::json!(generated);
        let payload = serde_json::to_vec(&value).expect("serialize");
        let mut changed = b"DIVECP02".to_vec();
        changed.extend((payload.len() as u64).to_le_bytes());
        changed.extend(Sha256::digest(&payload));
        changed.extend(payload);
        let path = dir.join(format!("admitted-{generated}.cp"));
        std::fs::write(&path, changed).expect("coherent checksum");
        let result = checkpoint::check(
            &ir,
            &cfg,
            &opts,
            &checkpoint::Request {
                path,
                action: checkpoint::Action::Recover,
                ..pause_request.clone()
            },
        );
        assert!(matches!(
            result.check.verdict,
            Verdict::ResourceLimit { .. }
        ));
        assert_eq!(result.check.workers.used, 1);
        assert_eq!(
            result.check.stats.expect("partial evidence").generated,
            checkpoint::MAX_COUNTER
        );
        assert_eq!(
            result
                .receipt
                .expect("admitted archive, not a completed result")
                .status,
            "recovered"
        );
        assert_eq!(result.check.output, Vec::<String>::new());
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// Reference inline execution and each speculative width must stop at the same
/// logical successor, even when the remaining count spans expansion units.
fn verify_native_counter_stopping_prefix(profile: bool, store: dive_store::StoreMode) {
    use sha2::{Digest, Sha256};
    let (ir, cfg) = model(
        "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..3\nA == x<12 /\\ x'=x+4\nB == x<12 /\\ x'=x+5\nNext == A \\/ B\n====\n",
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    );
    let dir = directory();
    let mut opts = if profile {
        CheckOptions::tlc(0, 7)
    } else {
        CheckOptions::dive(7)
    };
    opts.store = store;
    opts.sequential = true;
    let request = checkpoint::Request {
        path: dir.join(format!("{profile}-{store:?}.cp")),
        action: checkpoint::Action::PauseAfterLevels(1),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let pause = checkpoint::check(&ir, &cfg, &opts, &request);
    assert!(pause.receipt.is_some(), "{:?}", pause.check);
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&request.path).expect("snapshot")[48..])
            .expect("payload");
    for remaining in [0, 1, 2, 3, 5, 7] {
        let mut value = original.clone();
        value["generated"] = serde_json::json!(checkpoint::MAX_COUNTER - remaining);
        let payload = serde_json::to_vec(&value).expect("serialize");
        let mut bytes = b"DIVECP02".to_vec();
        bytes.extend(
            u64::try_from(payload.len())
                .expect("payload length")
                .to_le_bytes(),
        );
        bytes.extend(Sha256::digest(&payload));
        bytes.extend(payload);
        let recover = checkpoint::Request {
            path: dir.join(format!("{profile}-{store:?}-{remaining}.cp")),
            action: checkpoint::Action::Recover,
            ..request.clone()
        };
        std::fs::write(&recover.path, bytes).expect("coherent checksum");
        let reference = checkpoint::check(&ir, &cfg, &opts, &recover);
        assert!(matches!(
            reference.check.verdict,
            Verdict::ResourceLimit { .. }
        ));
        assert_eq!(
            reference.check.stats.as_ref().expect("stats").generated,
            checkpoint::MAX_COUNTER
        );
        for width in [1, 4, 8] {
            for chaos in [None, Some(19)] {
                let mut parallel = opts.clone();
                parallel.sequential = false;
                parallel.workers = width;
                parallel.chaos = chaos;
                let resumed = checkpoint::check(&ir, &cfg, &parallel, &recover);
                assert_eq!(resumed.check.verdict, reference.check.verdict);
                assert_eq!(
                    resumed.check.stats, reference.check.stats,
                    "remaining{remaining},width{width},chaos{chaos:?}"
                );
                assert_eq!(resumed.check.workers.used, width);
                assert!(!resumed.check.workers.sequential);
                let actual = resumed
                    .check
                    .counterexample
                    .as_ref()
                    .expect("retained resource-stop prefix");
                let expected = reference
                    .check
                    .counterexample
                    .as_ref()
                    .expect("inline prefix");
                assert_eq!(actual.states.len(), expected.states.len());
                for (a, b) in actual.states.iter().zip(&expected.states) {
                    assert_eq!(a.values, b.values);
                    assert_eq!(a.action, b.action);
                }
                assert_eq!(
                    resumed.receipt.as_ref().expect("admitted").status,
                    "recovered"
                );
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn native_counter_prefix_fingerprint_dive() {
    verify_native_counter_stopping_prefix(false, dive_store::StoreMode::Fingerprint);
}

#[test]
fn native_counter_prefix_fingerprint_tlc() {
    verify_native_counter_stopping_prefix(true, dive_store::StoreMode::Fingerprint);
}

#[test]
fn native_counter_prefix_exact_dive() {
    verify_native_counter_stopping_prefix(false, dive_store::StoreMode::Exact);
}

#[test]
fn native_counter_prefix_exact_tlc() {
    verify_native_counter_stopping_prefix(true, dive_store::StoreMode::Exact);
}

#[test]
fn recovery_can_change_pool_width_and_resume_an_actual_later_level() {
    let (ir, cfg) = model(
        "---- MODULE Checkpoint ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..2\nA == x<20 /\\ x'=x+3\nB == x<20 /\\ x'=x+4\nNext == A \\/ B\nInv == x<17\n====\n",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    );
    let dir = directory();
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
            opts.sequential = true;
            let reference = dive_check::check(&ir, &cfg, &opts);
            assert!(matches!(
                reference.verdict,
                Verdict::InvariantViolated { .. }
            ));
            for pause_width in [1, 4, 8] {
                opts.sequential = false;
                opts.workers = pause_width;
                opts.chaos = Some(31);
                let request = checkpoint::Request {
                    path: dir.join(format!("{profile}-{store:?}-{pause_width}.cp")),
                    action: checkpoint::Action::PauseAfterLevels(2),
                    counters: checkpoint::Counters::Retain,
                    config_source_sha256: None,
                };
                let pause = checkpoint::check(&ir, &cfg, &opts, &request);
                assert_eq!(pause.receipt.as_ref().expect("committed").next_level, 3);
                assert_eq!(pause.check.workers.used, pause_width);
                for resume_width in [1, 4, 8] {
                    opts.workers = resume_width;
                    opts.chaos = Some(19);
                    let resumed = checkpoint::check(
                        &ir,
                        &cfg,
                        &opts,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            ..request.clone()
                        },
                    );
                    assert_eq!(resumed.check.workers.used, resume_width);
                    assert!(!resumed.check.workers.sequential);
                    assert_eq!(resumed.check.verdict, reference.verdict);
                    assert_eq!(resumed.check.stats, reference.stats);
                    let actual = resumed.check.counterexample.as_ref().expect("actual");
                    let expected = reference.counterexample.as_ref().expect("reference");
                    assert_eq!(actual.states.len(), expected.states.len());
                    for (a, b) in actual.states.iter().zip(&expected.states) {
                        assert_eq!(a.values, b.values);
                        assert_eq!(a.action, b.action);
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
