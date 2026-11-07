//! Pure runtime names retain recovery order, not an independent TLC certificate.
#![allow(clippy::expect_used, reason = "bounded recovery regression assertions")]
#[path = "support/memory_observations.rs"]
mod memory_observations;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, checkpoint};
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
    memory_observations::normalize(&mut v);
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
fn run(name: &str, compat_tlc: bool, selected_store: Store, selected_workers: usize) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/fixtures/native-checkpoint-runtime-names/{name}/{name}.tla"
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
    // Preserve the original store/width position: it selects the same four
    // frontier/predecessor spill families even across independently run tests.
    let mut serial = usize::from(selected_store == Store::Exact) * 6
        + [1, 4, 8]
            .iter()
            .position(|width| *width == selected_workers)
            .expect("selected matrix width")
            * 2;
    // References must span stores and widths, not restart inside the split
    // wrapper. These are actual full runs of the same source/profile at width1.
    let fingerprint_reference = dive_api::check_file_engine(
        &CheckRequest {
            store: Store::Fingerprint,
            workers: Some(1),
            ..base.clone()
        },
        dive_api::Engine::Reference,
    )
    .expect("full fingerprint reference")
    .to_json();
    assert_eq!(fingerprint_reference["verdict"], "invariant-violation");
    metadata(&fingerprint_reference, Store::Fingerprint, compat_tlc);
    let mut semantic_reference = Some(cross_store(observable(fingerprint_reference.clone())));
    for store in [selected_store] {
        let store_reference = if store == Store::Fingerprint {
            fingerprint_reference.clone()
        } else {
            dive_api::check_file_engine(
                &CheckRequest {
                    store,
                    workers: Some(1),
                    ..base.clone()
                },
                dive_api::Engine::Reference,
            )
            .expect("full store reference")
            .to_json()
        };
        assert_eq!(store_reference["verdict"], "invariant-violation");
        metadata(&store_reference, store, compat_tlc);
        let mut reference = Some(observable(store_reference));
        for workers in [selected_workers] {
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
                let full = dive_api::check_file_engine(&req, dive_api::Engine::Reference)
                    .expect("full")
                    .to_json();
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
                    let paused = dive_api::check_file_checkpoint_engine(
                        &req,
                        &cp,
                        dive_api::Engine::Reference,
                    )
                    .expect("pause")
                    .to_json();
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
                    let resumed = dive_api::check_file_checkpoint_engine(
                        &recovery,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            ..cp.clone()
                        },
                        dive_api::Engine::Reference,
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
                    let reset = dive_api::check_file_checkpoint_engine(
                        &recovery,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            counters: checkpoint::Counters::Tlc,
                            ..cp
                        },
                        dive_api::Engine::Reference,
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
// Partition independently by store/width; every original chaos/boundary and
// report/replay assertion remains in run. CI timeout bounds are unchanged.
macro_rules! controls {
    ($($test:ident => ($source:literal,$tlc:expr,$store:expr,$workers:expr)),* $(,)?) => {$(#[test] fn $test(){run($source,$tlc,$store,$workers);})*};
}
controls! {
    string_dive_fingerprint_w1 => ("RuntimeStrings", false, Store::Fingerprint, 1),
    string_dive_fingerprint_w4 => ("RuntimeStrings", false, Store::Fingerprint, 4),
    string_dive_fingerprint_w8 => ("RuntimeStrings", false, Store::Fingerprint, 8),
    string_dive_exact_w1 => ("RuntimeStrings", false, Store::Exact, 1),
    string_dive_exact_w4 => ("RuntimeStrings", false, Store::Exact, 4),
    string_dive_exact_w8 => ("RuntimeStrings", false, Store::Exact, 8),
    string_tlc_fingerprint_w1 => ("RuntimeStrings", true, Store::Fingerprint, 1),
    string_tlc_fingerprint_w4 => ("RuntimeStrings", true, Store::Fingerprint, 4),
    string_tlc_fingerprint_w8 => ("RuntimeStrings", true, Store::Fingerprint, 8),
    string_tlc_exact_w1 => ("RuntimeStrings", true, Store::Exact, 1),
    string_tlc_exact_w4 => ("RuntimeStrings", true, Store::Exact, 4),
    string_tlc_exact_w8 => ("RuntimeStrings", true, Store::Exact, 8),
    duplicates_dive_fingerprint_w1 => ("RuntimeDuplicates", false, Store::Fingerprint, 1),
    duplicates_dive_fingerprint_w4 => ("RuntimeDuplicates", false, Store::Fingerprint, 4),
    duplicates_dive_fingerprint_w8 => ("RuntimeDuplicates", false, Store::Fingerprint, 8),
    duplicates_dive_exact_w1 => ("RuntimeDuplicates", false, Store::Exact, 1),
    duplicates_dive_exact_w4 => ("RuntimeDuplicates", false, Store::Exact, 4),
    duplicates_dive_exact_w8 => ("RuntimeDuplicates", false, Store::Exact, 8),
    duplicates_tlc_fingerprint_w1 => ("RuntimeDuplicates", true, Store::Fingerprint, 1),
    duplicates_tlc_fingerprint_w4 => ("RuntimeDuplicates", true, Store::Fingerprint, 4),
    duplicates_tlc_fingerprint_w8 => ("RuntimeDuplicates", true, Store::Fingerprint, 8),
    duplicates_tlc_exact_w1 => ("RuntimeDuplicates", true, Store::Exact, 1),
    duplicates_tlc_exact_w4 => ("RuntimeDuplicates", true, Store::Exact, 4),
    duplicates_tlc_exact_w8 => ("RuntimeDuplicates", true, Store::Exact, 8),
    choose_dive_fingerprint_w1 => ("RuntimeChoose", false, Store::Fingerprint, 1),
    choose_dive_fingerprint_w4 => ("RuntimeChoose", false, Store::Fingerprint, 4),
    choose_dive_fingerprint_w8 => ("RuntimeChoose", false, Store::Fingerprint, 8),
    choose_dive_exact_w1 => ("RuntimeChoose", false, Store::Exact, 1),
    choose_dive_exact_w4 => ("RuntimeChoose", false, Store::Exact, 4),
    choose_dive_exact_w8 => ("RuntimeChoose", false, Store::Exact, 8),
    choose_tlc_fingerprint_w1 => ("RuntimeChoose", true, Store::Fingerprint, 1),
    choose_tlc_fingerprint_w4 => ("RuntimeChoose", true, Store::Fingerprint, 4),
    choose_tlc_fingerprint_w8 => ("RuntimeChoose", true, Store::Fingerprint, 8),
    choose_tlc_exact_w1 => ("RuntimeChoose", true, Store::Exact, 1),
    choose_tlc_exact_w4 => ("RuntimeChoose", true, Store::Exact, 4),
    choose_tlc_exact_w8 => ("RuntimeChoose", true, Store::Exact, 8),
    sequence_dive_fingerprint_w1 => ("RuntimeSequence", false, Store::Fingerprint, 1),
    sequence_dive_fingerprint_w4 => ("RuntimeSequence", false, Store::Fingerprint, 4),
    sequence_dive_fingerprint_w8 => ("RuntimeSequence", false, Store::Fingerprint, 8),
    sequence_dive_exact_w1 => ("RuntimeSequence", false, Store::Exact, 1),
    sequence_dive_exact_w4 => ("RuntimeSequence", false, Store::Exact, 4),
    sequence_dive_exact_w8 => ("RuntimeSequence", false, Store::Exact, 8),
    sequence_tlc_fingerprint_w1 => ("RuntimeSequence", true, Store::Fingerprint, 1),
    sequence_tlc_fingerprint_w4 => ("RuntimeSequence", true, Store::Fingerprint, 4),
    sequence_tlc_fingerprint_w8 => ("RuntimeSequence", true, Store::Fingerprint, 8),
    sequence_tlc_exact_w1 => ("RuntimeSequence", true, Store::Exact, 1),
    sequence_tlc_exact_w4 => ("RuntimeSequence", true, Store::Exact, 4),
    sequence_tlc_exact_w8 => ("RuntimeSequence", true, Store::Exact, 8),
    symmetry_view_dive_fingerprint_w1 => ("RuntimeSymView", false, Store::Fingerprint, 1),
    symmetry_view_dive_fingerprint_w4 => ("RuntimeSymView", false, Store::Fingerprint, 4),
    symmetry_view_dive_fingerprint_w8 => ("RuntimeSymView", false, Store::Fingerprint, 8),
    symmetry_view_dive_exact_w1 => ("RuntimeSymView", false, Store::Exact, 1),
    symmetry_view_dive_exact_w4 => ("RuntimeSymView", false, Store::Exact, 4),
    symmetry_view_dive_exact_w8 => ("RuntimeSymView", false, Store::Exact, 8),
    symmetry_view_tlc_fingerprint_w1 => ("RuntimeSymView", true, Store::Fingerprint, 1),
    symmetry_view_tlc_fingerprint_w4 => ("RuntimeSymView", true, Store::Fingerprint, 4),
    symmetry_view_tlc_fingerprint_w8 => ("RuntimeSymView", true, Store::Fingerprint, 8),
    symmetry_view_tlc_exact_w1 => ("RuntimeSymView", true, Store::Exact, 1),
    symmetry_view_tlc_exact_w4 => ("RuntimeSymView", true, Store::Exact, 4),
    symmetry_view_tlc_exact_w8 => ("RuntimeSymView", true, Store::Exact, 8),
}
