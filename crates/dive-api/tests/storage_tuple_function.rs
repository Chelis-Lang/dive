//! Structural storage keeps function diagnostics and independently checked words.
#![allow(clippy::expect_used, reason = "captured storage regression assertions")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/storage_tuple_function.json");
#[path = "support/memory_observations.rs"]
mod memory_observations;
#[path = "support/refused.rs"]
mod refused;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, checkpoint};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn request(name: &str, compat_tlc: bool) -> CheckRequest {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/storage-tuple-function/specs")
        .join(name)
        .join("TupleStorage.tla");
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc,
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn directory() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "dive-structural-storage-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).expect("private directory");
    p
}
// Only request-specific accounting differs; every remaining concrete observable
// is compared, within each store/profile. Store/fingerprint metadata is retained.
fn observable(mut j: Value) -> Value {
    memory_observations::normalize(&mut j);
    let fields = j.as_object_mut().expect("report");
    for key in [
        "elapsed_ms",
        "checkpoint",
        "frontier_storage",
        "predecessor_storage",
    ] {
        fields.remove(key);
    }
    let opts = fields["options"].as_object_mut().expect("options");
    for key in [
        "checkpoint",
        "profile",
        "workers",
        "frontier_memory_bytes_per_queue",
        "frontier_codec_nodes_per_value_limit",
        "frontier_record_bytes_limit",
        "predecessor_memory_bytes",
        "predecessor_path_steps_limit",
    ] {
        opts.remove(key);
    }
    j
}
fn storage(j: &Value, frontier: bool, predecessor: bool) {
    assert_eq!(j["frontier_storage"]["failures"], serde_json::json!([]));
    if predecessor {
        assert_eq!(j["predecessor_storage"]["failures"], serde_json::json!([]));
    } else {
        assert!(j["predecessor_storage"].is_null());
    }
    if frontier {
        assert_eq!(j["frontier_storage"]["retained_bytes"], 0);
    }
    if predecessor {
        assert_eq!(j["predecessor_storage"]["cleanup_completed"], true);
        assert_eq!(j["predecessor_storage"]["retained_bytes_before_cleanup"], 0);
    }
}
fn witness(model: &SpecModel, report: &Value) {
    let doc = TraceDoc::from_json(report).expect("v1 word");
    let word = doc.to_json();
    assert_eq!(word["version"], 1);
    // V1 represents a function over 1..n by the sequence it equals. Storage
    // still preserves its concrete class; replay checks the supplied values.
    assert_eq!(
        word["states"][0]["values"]["s"],
        serde_json::json!({"seq": [1, 2]})
    );
    assert_eq!(model.validate(&doc).validation, Validation::Valid);
    for at in [0, 1, 3] {
        let mut mutant = doc.clone();
        mutant.states[at].insert("n".into(), serde_json::json!(99));
        assert!(matches!(
            model.validate(&mutant).validation,
            Validation::Invalid(_)
        ));
    }
}
fn independent(name: &str, compat: bool) -> SpecModel {
    let req = request(name, compat);
    SpecModel::load(
        &req.file,
        req.config.as_ref().expect("cfg"),
        &TraceOptions {
            compat_tlc: compat,
            run: Some((7, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model")
}
fn assert_comparison(reference: &Value) {
    // D-009 compares these distinct domains structurally.
    assert_eq!(reference["verdict"], "success");
    assert_eq!(reference["statistics"]["generated"], 3);
    assert_eq!(reference["statistics"]["distinct"], 2);
    assert_eq!(reference["statistics"]["depth"], 2);
    assert_eq!(reference["tlc"]["code"], 2193);
    assert_eq!(reference["tlc"]["exit_status"], 0);
}
#[allow(
    clippy::too_many_lines,
    reason = "one storage matrix against the agreed records"
)]
fn verify(name: &str, compat: bool, stores: &[Store], widths: &[usize], schedules: &[Option<u64>]) {
    let directory = directory();
    let mut serial = 0;
    let model = (name == "Witness").then(|| independent(name, compat));
    for &store in stores {
        for &workers in widths {
            for &chaos in schedules {
                let base = CheckRequest {
                    store,
                    workers: Some(workers),
                    chaos,
                    ..request(name, compat)
                };
                let run = dive_api::check_file(&base).expect("memory");
                let reference = run.to_json();
                // Only the retired reference engine ran these cases (#791).
                if chaos.is_some() {
                    refused::assert_refused(&reference, run.exit_code(), refused::CHAOS);
                    continue;
                }
                if compat && matches!(name, "CompareFunction" | "CompareTuple") {
                    refused::assert_refused(&reference, run.exit_code(), refused::MIXED_SHAPES);
                    continue;
                }
                if name == "Apply" {
                    assert_eq!(reference["verdict"], "evaluation-error");
                    assert!(
                        reference["message"]
                            .as_str()
                            .expect("cause")
                            .ends_with("Attempted to access tuple at a non integral index: \"a\"")
                    );
                    assert_eq!(reference["statistics"]["generated"], 1);
                    assert_eq!(reference["statistics"]["distinct"], 1);
                    assert_eq!(reference["statistics"]["depth"], 1);
                    assert_eq!(reference["tlc"]["code"], 2103);
                    assert_eq!(reference["tlc"]["reported"], 1000);
                    // This ported diagnostic remains an uncertified error site.
                    assert_eq!(reference["tlc"]["verified"], false);
                } else if matches!(name, "CompareFunction" | "CompareTuple") {
                    assert_comparison(&reference);
                } else {
                    assert_eq!(reference["verdict"], "invariant-violation");
                    assert_eq!(reference["statistics"]["generated"], 4);
                    assert_eq!(reference["statistics"]["distinct"], 4);
                    assert_eq!(reference["statistics"]["depth"], 4);
                }
                for (frontier, predecessor) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    let req = CheckRequest {
                        frontier_memory_bytes: frontier.then_some(0),
                        predecessor_memory_bytes: predecessor.then_some(0),
                        ..base.clone()
                    };
                    let bounded = dive_api::check_file(&req).expect("bounded").to_json();
                    agreed::check_eq(
                        AGREED,
                        "verify/0",
                        &(workers),
                        &(bounded["options"]["workers"]["used"]),
                    );
                    storage(&bounded, frontier, predecessor);
                    assert_eq!(
                        observable(bounded.clone()),
                        observable(reference.clone()),
                        "{name}/{compat}/{store:?}/{workers}/{chaos:?}/{frontier}/{predecessor}"
                    );
                    let cp = checkpoint::Request {
                        path: directory.join(format!("{serial}.cp")),
                        action: checkpoint::Action::PauseAfterLevels(0),
                        counters: checkpoint::Counters::Retain,
                        config_source_sha256: None,
                    };
                    serial += 1;
                    let paused = dive_api::check_file_checkpoint(&req, &cp)
                        .expect("pause")
                        .to_json();
                    assert_eq!(paused["verdict"], "resource-limit");
                    assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
                    assert_eq!(paused["options"]["workers"]["used"], 1);
                    assert_eq!(&std::fs::read(&cp.path).expect("archive")[..8], b"DIVECC01");
                    let resumed = dive_api::check_file_checkpoint(
                        &req,
                        &checkpoint::Request {
                            action: checkpoint::Action::Recover,
                            ..cp
                        },
                    )
                    .expect("recover")
                    .to_json();
                    assert_eq!(resumed["checkpoint"]["status"], "recovered");
                    assert_eq!(
                        resumed["checkpoint"]["transient_storage_cleanup_completed"],
                        true
                    );
                    assert_eq!(
                        resumed["options"]["checkpoint"]["independent_certificate"],
                        false
                    );
                    agreed::check_eq(
                        AGREED,
                        "verify/1",
                        &(workers),
                        &(resumed["options"]["workers"]["used"]),
                    );
                    assert_eq!(
                        resumed["options"]["profile"],
                        if compat { "tlc+overrides" } else { "dive" }
                    );
                    storage(&resumed, frontier, predecessor);
                    assert_eq!(observable(resumed.clone()), observable(reference.clone()));
                    if let Some(model) = &model {
                        for report in [&bounded, &resumed] {
                            witness(model, report);
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}
// Each test owns one unchanged store/width/schedule slice of the original matrix.
macro_rules! slice {
    ($test:ident, $source:expr, $compat:expr, $store:expr, $width:expr, $chaos:expr) => {
        #[test]
        fn $test() {
            verify($source, $compat, &[$store], &[$width], &[$chaos]);
        }
    };
}
macro_rules! widths {
    ($source:expr, $compat:expr, $store:expr) => {
        slice!(w1_plain, $source, $compat, $store, 1, None);
        slice!(w1_chaos, $source, $compat, $store, 1, Some(13));
        slice!(w4_plain, $source, $compat, $store, 4, None);
        slice!(w4_chaos, $source, $compat, $store, 4, Some(13));
        slice!(w8_plain, $source, $compat, $store, 8, None);
        slice!(w8_chaos, $source, $compat, $store, 8, Some(13));
    };
}
macro_rules! controls {
    ($parent:ident, $source:expr, $compat:expr) => {
        mod $parent {
            use super::*;
            mod fingerprint {
                use super::*;
                widths!($source, $compat, Store::Fingerprint);
            }
            mod exact {
                use super::*;
                widths!($source, $compat, Store::Exact);
            }
        }
    };
}
controls!(apply_tlc, "Apply", true);
controls!(apply_dive, "Apply", false);
controls!(witness_tlc, "Witness", true);
controls!(witness_dive, "Witness", false);
// Pinned TLC compares both representations through integer-domain functions.
controls!(compare_function_tlc, "CompareFunction", true);
controls!(compare_function_dive, "CompareFunction", false);
controls!(compare_tuple_tlc, "CompareTuple", true);
controls!(compare_tuple_dive, "CompareTuple", false);

#[test]
fn actual_older_ambiguous_archive_refuses_without_search() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/storage-tuple-function/legacy-v01.cp");
    assert_eq!(
        &std::fs::read(&path).expect("captured old archive")[..8],
        b"DIVECP01"
    );
    for compat in [true, false] {
        let result = dive_api::check_file_checkpoint(
            &request("Apply", compat),
            &checkpoint::Request {
                path: path.clone(),
                action: checkpoint::Action::Recover,
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            },
        )
        .expect_err("pre-search archive refusal");
        assert!(
            matches!(result, dive_api::ApiError::Rejected { ref message } if message == "DIVE-S0400: native magic/schema mismatch")
        );
    }
}
