//! Native recovery with bounded transient frontiers/logs preserves concrete witnesses.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "native checkpoint regression controls"
)]
#[path = "support/memory_observations.rs"]
mod memory_observations;

use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file, check_file_checkpoint, checkpoint};
use std::path::Path;

fn request(name: &str) -> CheckRequest {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/fixtures/native-checkpoint-keys/{name}/{name}.tla"
    ));
    CheckRequest {
        file: file.clone(),
        config: Some(file.with_extension("cfg")),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let p = std::env::temp_dir().join(format!(
            "dive-native-spill-api-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        // A recycled PID can meet a directory left by an interrupted run.
        // Claim a fresh path atomically; never reuse another run's files.
        match std::fs::create_dir(&p) {
            Ok(()) => return p,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            result => result.expect("private directory"),
        }
    }
}
fn observable(mut value: serde_json::Value) -> serde_json::Value {
    memory_observations::normalize(&mut value);
    let object = value.as_object_mut().expect("object");
    object.remove("elapsed_ms");
    object.remove("checkpoint");
    object.remove("frontier_storage");
    object.remove("predecessor_storage");
    let options = object
        .get_mut("options")
        .expect("options")
        .as_object_mut()
        .expect("options object");
    for key in [
        "checkpoint",
        "workers",
        "profile",
        "frontier_memory_bytes_per_queue",
        "frontier_record_bytes_limit",
        "frontier_codec_nodes_per_value_limit",
        "predecessor_memory_bytes",
        "predecessor_path_steps_limit",
    ] {
        options.remove(key);
    }
    value
}

#[allow(
    clippy::too_many_lines,
    reason = "one complete recovery/independent-witness matrix per source case"
)]
fn recovery_case(
    name: &str,
    profiles: &[bool],
    family: u8,
    stores: &[Store],
    widths: &[usize],
    schedules: &[Option<u64>],
) {
    assert!((1..=3).contains(&family));
    let directory = directory();
    let mut serial = 0;
    let frontier = directory.join("frontier");
    let predecessor = directory.join("predecessor");
    std::fs::create_dir(&frontier).expect("frontier parent");
    std::fs::create_dir(&predecessor).expect("predecessor parent");
    {
        for &compat_tlc in profiles {
            let base = request(name);
            let spec = SpecModel::load(
                &base.file,
                base.config.as_ref().expect("config"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 7)),
                    ..TraceOptions::default()
                },
            )
            .expect("independent model");
            for &store in stores {
                for &workers in widths {
                    for &chaos in schedules {
                        let req = CheckRequest {
                            compat_tlc,
                            store,
                            workers: Some(workers),
                            chaos,
                            frontier_memory_bytes: (family & 1 != 0).then_some(0),
                            frontier_directory: (family & 1 != 0).then(|| frontier.clone()),
                            predecessor_memory_bytes: (family & 2 != 0).then_some(0),
                            predecessor_directory: (family & 2 != 0).then(|| predecessor.clone()),
                            ..base.clone()
                        };
                        let full = check_file(&req).expect("full").to_json();
                        assert_eq!(full["verdict"], "invariant-violation");
                        let memory = CheckRequest {
                            frontier_memory_bytes: None,
                            frontier_directory: None,
                            predecessor_memory_bytes: None,
                            predecessor_directory: None,
                            ..req.clone()
                        };
                        assert_eq!(
                            observable(full.clone()),
                            observable(check_file(&memory).expect("memory counterpart").to_json())
                        );
                        storage(&full, family, &frontier, &predecessor);
                        for boundary in [0, 1] {
                            let cp = checkpoint::Request {
                                path: directory.join(format!("{serial}.cp")),
                                action: checkpoint::Action::PauseAfterLevels(boundary),
                                counters: checkpoint::Counters::Retain,
                                config_source_sha256: None,
                            };
                            serial += 1;
                            let paused = check_file_checkpoint(&req, &cp).expect("pause").to_json();
                            assert_eq!(paused["verdict"], "resource-limit", "{paused}");
                            assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
                            assert_eq!(
                                paused["options"]["workers"]["used"],
                                if boundary == 0 { 1 } else { workers }
                            );
                            assert!(paused["counterexample"].is_null());
                            storage(&paused, family, &frontier, &predecessor);
                            let resumed = check_file_checkpoint(
                                &req,
                                &checkpoint::Request {
                                    action: checkpoint::Action::Recover,
                                    ..cp.clone()
                                },
                            )
                            .expect("resume")
                            .to_json();
                            assert_eq!(
                                observable(resumed.clone()),
                                observable(full.clone()),
                                "{name}, profile{compat_tlc}, store{store:?}, width{workers}, chaos{chaos:?}, boundary{boundary}"
                            );
                            assert_eq!(resumed["options"]["workers"]["requested"], workers);
                            assert_eq!(resumed["options"]["workers"]["used"], workers);
                            assert_eq!(resumed["options"]["checkpoint"]["parallel_recovery"], true);
                            assert_eq!(
                                resumed["options"]["checkpoint"]["independent_certificate"],
                                false
                            );
                            storage(&resumed, family, &frontier, &predecessor);
                            let doc = TraceDoc::from_json(&resumed).expect("concrete witness");
                            assert_eq!(spec.validate(&doc).validation, Validation::Valid, "{name}");
                            if name == "CpSymConcrete" {
                                for state in &doc.states {
                                    assert_eq!(state["owner"], serde_json::json!({"model":"p2"}));
                                }
                            }
                            if name != "CpSymView" {
                                for at in [0, 1, doc.states.len() - 1] {
                                    let mut bad = doc.clone();
                                    let (variable, value) = if name == "CpView" {
                                        ("n", serde_json::json!(99))
                                    } else {
                                        ("owner", serde_json::json!({"model":"p1"}))
                                    };
                                    bad.states[at].insert(variable.into(), value);
                                    let Validation::Invalid(failure) =
                                        spec.validate(&bad).validation
                                    else {
                                        panic!("equivalent-key witness accepted: {name} state{at}");
                                    };
                                    assert_eq!(
                                        failure.check,
                                        if at == 0 { Check::Init } else { Check::Next }
                                    );
                                }
                            }
                            let reset = check_file_checkpoint(
                                &req,
                                &checkpoint::Request {
                                    action: checkpoint::Action::Recover,
                                    counters: checkpoint::Counters::Tlc,
                                    ..cp
                                },
                            )
                            .expect("TLC reset")
                            .to_json();
                            storage(&reset, family, &frontier, &predecessor);
                            assert_eq!(reset["verdict"], full["verdict"]);
                            assert_eq!(reset["counterexample"], full["counterexample"]);
                            for key in ["distinct", "depth", "left_on_queue"] {
                                assert_eq!(reset["statistics"][key], full["statistics"][key]);
                            }
                            assert_eq!(
                                reset["statistics"]["generated"]
                                    .as_u64()
                                    .expect("generated"),
                                full["statistics"]["generated"]
                                    .as_u64()
                                    .expect("full generated")
                                    - paused["statistics"]["generated"]
                                        .as_u64()
                                        .expect("paused generated")
                                    + paused["statistics"]["distinct"]
                                        .as_u64()
                                        .expect("paused distinct")
                            );
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}

fn storage(v: &serde_json::Value, family: u8, frontier: &Path, predecessor: &Path) {
    if !v["checkpoint"].is_null() {
        assert_eq!(v["checkpoint"]["transient_storage_cleanup_completed"], true);
    }
    if family & 1 != 0 {
        let evidence = &v["frontier_storage"];
        assert_eq!(evidence["mode"], "encoded-spill");
        assert_eq!(evidence["failures"], serde_json::json!([]));
        assert_eq!(evidence["peak_retained_bytes"], 0);
        assert_eq!(evidence["retained_bytes"], 0);
        assert!(evidence["committed_spill_bytes"].as_u64().expect("bytes") > 0);
    }
    if family & 2 != 0 {
        let evidence = &v["predecessor_storage"];
        assert_eq!(evidence["cleanup_completed"], true);
        assert_eq!(evidence["failures"], serde_json::json!([]));
        assert_eq!(evidence["peak_retained_bytes"], 0);
        assert_eq!(evidence["retained_bytes_before_cleanup"], 0);
        assert_eq!(evidence["spilled"], true);
        assert!(evidence["committed_records"].as_u64().expect("records") > 0);
    }
    assert_eq!(
        std::fs::read_dir(frontier)
            .expect("frontier cleanup")
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(predecessor).expect("log cleanup").count(),
        0
    );
}

// Each test owns one unchanged store/width/schedule slice of the original matrix.
macro_rules! slice {
    ($test:ident, $source:expr, $compat:expr, $family:expr, $store:expr, $width:expr, $chaos:expr) => {
        #[test]
        fn $test() {
            recovery_case(
                $source,
                &[$compat],
                $family,
                &[$store],
                &[$width],
                &[$chaos],
            );
        }
    };
}
macro_rules! widths {
    ($source:expr, $compat:expr, $family:expr, $store:expr) => {
        slice!(w1_plain, $source, $compat, $family, $store, 1, None);
        slice!(w1_chaos, $source, $compat, $family, $store, 1, Some(19));
        slice!(w4_plain, $source, $compat, $family, $store, 4, None);
        slice!(w4_chaos, $source, $compat, $family, $store, 4, Some(19));
        slice!(w8_plain, $source, $compat, $family, $store, 8, None);
        slice!(w8_chaos, $source, $compat, $family, $store, 8, Some(19));
    };
}
macro_rules! controls {
    ($parent:ident, $source:expr, $compat:expr, $family:expr) => {
        mod $parent {
            use super::*;
            mod fingerprint {
                use super::*;
                widths!($source, $compat, $family, Store::Fingerprint);
            }
            mod exact {
                use super::*;
                widths!($source, $compat, $family, Store::Exact);
            }
        }
    };
}
controls!(cpview_dive_frontier_spill_recovery, "CpView", false, 1);
controls!(cpview_dive_predecessor_spill_recovery, "CpView", false, 2);
controls!(cpview_dive_both_spill_recovery, "CpView", false, 3);
controls!(cpview_tlc_frontier_spill_recovery, "CpView", true, 1);
controls!(cpview_tlc_predecessor_spill_recovery, "CpView", true, 2);
controls!(cpview_tlc_both_spill_recovery, "CpView", true, 3);
controls!(
    cpsymview_dive_frontier_spill_recovery,
    "CpSymView",
    false,
    1
);
controls!(
    cpsymview_dive_predecessor_spill_recovery,
    "CpSymView",
    false,
    2
);
controls!(cpsymview_dive_both_spill_recovery, "CpSymView", false, 3);
controls!(cpsymview_tlc_frontier_spill_recovery, "CpSymView", true, 1);
controls!(
    cpsymview_tlc_predecessor_spill_recovery,
    "CpSymView",
    true,
    2
);
controls!(cpsymview_tlc_both_spill_recovery, "CpSymView", true, 3);
controls!(
    cpsymconcrete_dive_frontier_spill_recovery,
    "CpSymConcrete",
    false,
    1
);
controls!(
    cpsymconcrete_dive_predecessor_spill_recovery,
    "CpSymConcrete",
    false,
    2
);
controls!(
    cpsymconcrete_dive_both_spill_recovery,
    "CpSymConcrete",
    false,
    3
);
controls!(
    cpsymconcrete_tlc_frontier_spill_recovery,
    "CpSymConcrete",
    true,
    1
);
controls!(
    cpsymconcrete_tlc_predecessor_spill_recovery,
    "CpSymConcrete",
    true,
    2
);
controls!(
    cpsymconcrete_tlc_both_spill_recovery,
    "CpSymConcrete",
    true,
    3
);

#[test]
fn storage_failures_do_not_create_a_pause_or_certify_recovery() {
    let dir = directory();
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"not a directory").expect("block spill parent");
    for compat_tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let req = CheckRequest {
                compat_tlc,
                store,
                workers: Some(8),
                ..request("CpSymView")
            };
            let good = checkpoint::Request {
                path: dir.join(format!("good-{compat_tlc}-{store:?}.cp")),
                action: checkpoint::Action::PauseAfterLevels(1),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = check_file_checkpoint(&req, &good)
                .expect("reference pause")
                .to_json();
            assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
            for family in [1, 2, 3] {
                let bad = CheckRequest {
                    frontier_memory_bytes: (family & 1 != 0).then_some(0),
                    frontier_directory: (family & 1 != 0).then(|| blocked.clone()),
                    predecessor_memory_bytes: (family & 2 != 0).then_some(0),
                    predecessor_directory: (family & 2 != 0).then(|| blocked.clone()),
                    ..req.clone()
                };
                let fresh = checkpoint::Request {
                    path: dir.join(format!("failed-{compat_tlc}-{store:?}-{family}.cp")),
                    ..good.clone()
                };
                for cp in [
                    fresh.clone(),
                    checkpoint::Request {
                        action: checkpoint::Action::Recover,
                        ..good.clone()
                    },
                ] {
                    let result = check_file_checkpoint(&bad, &cp)
                        .expect("typed storage failure")
                        .to_json();
                    assert_eq!(result["verdict"], "resource-limit", "{result}");
                    assert!(result["checkpoint"].is_null(), "{result}");
                    assert!(result["counterexample"].is_null());
                    assert!(result["collision"].is_null());
                    assert_eq!(result["output"], serde_json::json!([]));
                    assert_eq!(result["options"]["workers"]["used"], 1);
                    let frontier = result["frontier_storage"]["failures"]
                        .as_array()
                        .is_some_and(|v| !v.is_empty());
                    let predecessor = result["predecessor_storage"]["failures"]
                        .as_array()
                        .is_some_and(|v| !v.is_empty());
                    assert!(frontier || predecessor, "failure evidence: {result}");
                }
                assert!(!fresh.path.exists());
            }
            let corrupt = dir.join(format!("corrupt-{compat_tlc}-{store:?}.cp"));
            let mut bytes = std::fs::read(&good.path).expect("archive");
            let last = bytes.len() - 1;
            bytes[last] ^= 1;
            std::fs::write(&corrupt, bytes).expect("corrupt archive");
            let recovery = checkpoint::Request {
                path: corrupt,
                action: checkpoint::Action::Recover,
                ..good
            };
            let spill = CheckRequest {
                frontier_memory_bytes: Some(0),
                predecessor_memory_bytes: Some(0),
                ..req
            };
            assert!(matches!(
                check_file_checkpoint(&spill, &recovery),
                Err(dive_api::ApiError::Rejected { .. })
            ));
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn recovery_can_change_transient_budgets_and_worker_width() {
    let dir = directory();
    for compat_tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let reference = CheckRequest {
                compat_tlc,
                store,
                workers: Some(1),
                ..request("CpSymView")
            };
            let full = check_file(&reference).expect("memory reference").to_json();
            let pause = CheckRequest {
                workers: Some(4),
                frontier_memory_bytes: Some(0),
                ..reference.clone()
            };
            let cp = checkpoint::Request {
                path: dir.join(format!("{compat_tlc}-{store:?}.cp")),
                action: checkpoint::Action::PauseAfterLevels(1),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            assert_eq!(
                check_file_checkpoint(&pause, &cp)
                    .expect("spill pause")
                    .to_json()["checkpoint"]["status"],
                "paused-incomplete"
            );
            for workers in [1, 4, 8] {
                for budget in [None, Some(0), Some(64), Some(512)] {
                    let req = CheckRequest {
                        workers: Some(workers),
                        chaos: Some(19),
                        frontier_memory_bytes: budget,
                        predecessor_memory_bytes: budget,
                        ..reference.clone()
                    };
                    let result = check_file_checkpoint(
                        &req,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            ..cp.clone()
                        },
                    )
                    .expect("cross-budget recovery")
                    .to_json();
                    assert_eq!(observable(result.clone()), observable(full.clone()));
                    assert_eq!(result["options"]["workers"]["used"], workers);
                    if let Some(budget) = budget {
                        assert!(
                            result["frontier_storage"]["peak_retained_bytes"]
                                .as_u64()
                                .expect("bytes")
                                <= (2 * budget) as u64
                        );
                        assert_eq!(result["frontier_storage"]["retained_bytes"], 0);
                        assert!(
                            result["predecessor_storage"]["peak_retained_bytes"]
                                .as_u64()
                                .expect("bytes")
                                <= budget as u64
                        );
                        assert_eq!(result["predecessor_storage"]["cleanup_completed"], true);
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
