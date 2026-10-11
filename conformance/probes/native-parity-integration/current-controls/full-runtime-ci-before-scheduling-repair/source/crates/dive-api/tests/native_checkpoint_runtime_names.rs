//! Pure runtime names retain recovery order, not an independent TLC certificate.
#![allow(clippy::expect_used, reason = "bounded recovery regression assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file, check_file_checkpoint, checkpoint};
use std::path::{Path, PathBuf};

fn directory() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "dive-checkpoint-names-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).expect("private directory");
    p
}
fn observable(mut v: serde_json::Value) -> serde_json::Value {
    let obj = v.as_object_mut().expect("report");
    for field in [
        "elapsed_ms",
        "checkpoint",
        "frontier_storage",
        "predecessor_storage",
    ] {
        obj.remove(field);
    }
    let opts = obj["options"].as_object_mut().expect("options");
    for field in [
        "checkpoint",
        "workers",
        "profile",
        "frontier_memory_bytes_per_queue",
        "frontier_record_bytes_limit",
        "frontier_codec_nodes_per_value_limit",
        "predecessor_memory_bytes",
        "predecessor_path_steps_limit",
    ] {
        opts.remove(field);
    }
    v
}

// Store selection changes exactly these reported metadata fields. Retain
// every other report field, including diagnostics/output/notes/collision.
fn cross_store(mut v: serde_json::Value) -> serde_json::Value {
    let opts = v["options"].as_object_mut().expect("options");
    opts.remove("store");
    opts.remove("fingerprint");
    v
}
fn metadata(v: &serde_json::Value, store: Store, compat_tlc: bool) {
    assert_eq!(
        v["collision"],
        serde_json::Value::Null,
        "mid-level invariant stop has no completed collision estimate"
    );
    if store == Store::Exact {
        assert_eq!(v["options"]["store"], "exact");
        assert_eq!(v["options"]["fingerprint"], serde_json::Value::Null);
    } else {
        assert_eq!(v["options"]["store"], "fingerprint");
        assert_eq!(
            v["options"]["fingerprint"],
            serde_json::json!({"polynomial_index":0,"polynomial_source":"flag","scheme":if compat_tlc {"tlc"} else {"dive"}})
        );
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "complete source-specific profile recovery and witness matrix"
)]
fn run(name: &str, compat_tlc: bool) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../conformance/probes/native-checkpoint-runtime-names/specs/{name}/{name}.tla"
    ));
    let base = CheckRequest {
        file: file.clone(),
        config: Some(file.with_extension("cfg")),
        compat_tlc,
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    };
    let independent = SpecModel::load(
        &file,
        base.config.as_ref().expect("cfg"),
        &TraceOptions {
            compat_tlc,
            run: Some((0, 7)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model");
    let dir = directory();
    let mut serial = 0;
    let mut semantic_reference = None;
    for store in [Store::Fingerprint, Store::Exact] {
        let mut reference = None;
        for workers in [1, 4, 8] {
            for chaos in [None, Some(19)] {
                let family = serial % 4;
                let req = CheckRequest {
                    store,
                    workers: Some(workers),
                    chaos,
                    frontier_memory_bytes: (family & 1 != 0).then_some(0),
                    predecessor_memory_bytes: (family & 2 != 0).then_some(0),
                    ..base.clone()
                };
                let full = check_file(&req).expect("full").to_json();
                assert_eq!(full["verdict"], "invariant-violation", "{full}");
                metadata(&full, store, compat_tlc);
                let semantic = cross_store(observable(full.clone()));
                if let Some(v) = &semantic_reference {
                    assert_eq!(&semantic, v, "cross-store meaning");
                } else {
                    semantic_reference = Some(semantic);
                }
                let expected = observable(full.clone());
                if let Some(v) = &reference {
                    assert_eq!(&expected, v, "cross width/order");
                } else {
                    reference = Some(expected.clone());
                }
                for boundary in [0, 1, 2] {
                    let cp = checkpoint::Request {
                        path: dir.join(format!("{serial}-{boundary}.cp")),
                        action: checkpoint::Action::PauseAfterLevels(boundary),
                        counters: checkpoint::Counters::Retain,
                        config_source_sha256: None,
                    };
                    let paused = check_file_checkpoint(&req, &cp).expect("pause").to_json();
                    assert_eq!(paused["verdict"], "resource-limit", "{paused}");
                    metadata(&paused, store, compat_tlc);
                    assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
                    assert_eq!(
                        paused["checkpoint"]["transient_storage_cleanup_completed"],
                        true
                    );
                    assert!(paused["counterexample"].is_null());
                    assert_eq!(
                        paused["options"]["workers"]["used"],
                        if compat_tlc || boundary == 0 {
                            1
                        } else {
                            workers
                        }
                    );
                    // Restore under a different requested width: names and search
                    // identity are independent of physical pool configuration.
                    let recovery = CheckRequest {
                        workers: Some(if workers == 8 { 4 } else { 8 }),
                        ..req.clone()
                    };
                    let resumed = check_file_checkpoint(
                        &recovery,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            ..cp.clone()
                        },
                    )
                    .expect("recover")
                    .to_json();
                    metadata(&resumed, store, compat_tlc);
                    assert_eq!(
                        observable(resumed.clone()),
                        expected,
                        "{name}/{compat_tlc}/{store:?}/{workers}/{chaos:?}/{boundary}"
                    );
                    assert_eq!(
                        resumed["options"]["workers"]["used"],
                        if compat_tlc {
                            1
                        } else {
                            recovery.workers.expect("width")
                        }
                    );
                    assert_eq!(
                        resumed["options"]["workers"]["requested"],
                        recovery.workers.expect("width")
                    );
                    assert_eq!(
                        resumed["checkpoint"]["transient_storage_cleanup_completed"],
                        true
                    );
                    let doc = TraceDoc::from_json(&resumed).expect("concrete trace");
                    let proof = independent.validate(&doc).validation;
                    if compat_tlc {
                        assert_eq!(
                            proof,
                            Validation::Unsupported(
                                dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()
                            )
                        );
                    } else {
                        assert_eq!(proof, Validation::Valid);
                        let mut bad = doc.clone();
                        bad.states[0].insert("n".into(), serde_json::json!(99));
                        assert!(matches!(
                            independent.validate(&bad).validation,
                            Validation::Invalid(_)
                        ));
                    }
                    let reset = check_file_checkpoint(
                        &recovery,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            counters: checkpoint::Counters::Tlc,
                            ..cp
                        },
                    )
                    .expect("reset")
                    .to_json();
                    metadata(&reset, store, compat_tlc);
                    assert_eq!(reset["verdict"], full["verdict"]);
                    assert_eq!(reset["counterexample"], full["counterexample"]);
                    assert_eq!(
                        reset["statistics"]["distinct"],
                        full["statistics"]["distinct"]
                    );
                    assert_eq!(
                        reset["statistics"]["generated"]
                            .as_u64()
                            .expect("generated"),
                        full["statistics"]["generated"].as_u64().expect("full")
                            - paused["statistics"]["generated"].as_u64().expect("paused")
                            + paused["statistics"]["distinct"].as_u64().expect("distinct")
                    );
                }
                serial += 1;
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
macro_rules! controls {
    ($($test:ident => ($source:literal,$tlc:expr)),* $(,)?) => {$(#[test] fn $test(){run($source,$tlc);})*};
}
controls! {
    string_dive => ("RuntimeStrings",false), string_tlc => ("RuntimeStrings",true),
    duplicates_dive => ("RuntimeDuplicates",false), duplicates_tlc => ("RuntimeDuplicates",true),
    choose_dive => ("RuntimeChoose",false), choose_tlc => ("RuntimeChoose",true),
    sequence_dive => ("RuntimeSequence",false), sequence_tlc => ("RuntimeSequence",true),
    symmetry_view_dive => ("RuntimeSymView",false), symmetry_view_tlc => ("RuntimeSymView",true),
}
