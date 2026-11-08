//! Actual completed-level Store reclamation, pure safety and owned recovery.
#![allow(clippy::expect_used, reason = "retention regression assertions")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{
    CheckRequest, Engine, Store, check_file_checkpoint_engine, check_file_engine, checkpoint,
};
use serde_json::{Value, json};

fn request(relative: &str, tlc: bool, store: Store, workers: usize) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        seed: Some(0),
        fp: Some(0),
        compat_tlc: tlc,
        store,
        workers: Some(workers),
        ..CheckRequest::default()
    }
}
fn chain(tlc: bool, store: Store, workers: usize) -> CheckRequest {
    request(
        "tests/fixtures/core-retention/RetainedChain.tla",
        tlc,
        store,
        workers,
    )
}
fn observed(doc: &Value) -> Value {
    json!({"verdict":doc["verdict"], "statistics":doc["statistics"], "tlc":doc["tlc"],
        "property":doc["property"], "message":doc["message"], "counterexample":doc["counterexample"]})
}
fn validate(req: &CheckRequest, doc: &Value) {
    let trace = TraceDoc::from_json(doc).expect("actual word");
    let model = SpecModel::load(
        &req.file,
        req.config.as_ref().expect("cfg"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model");
    assert_eq!(
        model.try_validate(&trace).expect("validation").validation,
        Validation::Valid
    );
    if req.file.ends_with("RetainedChain.tla") {
        let controls = model.controls(&trace);
        assert!(controls.positive_ok());
        assert_ne!(controls.mutants.len(), 0);
        assert_eq!(controls.failures(), Vec::<String>::new());
    }
}

#[test]
fn many_forced_collections_release_expanded_payloads_and_keep_valid_words() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1, 4, 8] {
                let mut req = chain(tlc, store, workers);
                let baseline = check_file_engine(&req, Engine::Core)
                    .expect("resident")
                    .to_json();
                assert_eq!(baseline["verdict"], "invariant-violation", "{baseline}");
                assert_eq!(
                    baseline["core"]["metrics"]["store_reclamation"]["collections"],
                    0
                );
                req.store_compact_bytes = Some(0);
                req.frontier_memory_bytes = Some(0);
                req.predecessor_memory_bytes = Some(0);
                req.candidate_memory_bytes = Some(0);
                let actual = check_file_engine(&req, Engine::Core)
                    .expect("reclaimed")
                    .to_json();
                assert_eq!(
                    observed(&actual),
                    observed(&baseline),
                    "{tlc}/{store:?}/W{workers}: {actual}"
                );
                let usage = &actual["core"]["metrics"]["store_reclamation"];
                assert!(
                    usage["collections"].as_u64().expect("collections") >= 64,
                    "{usage}"
                );
                assert!(
                    usage["reclaimed_nodes"].as_u64().expect("released") >= 800,
                    "{usage}"
                );
                assert!(
                    usage["peak_after_nodes"].as_u64().expect("live") < 64,
                    "{usage}"
                );
                assert_eq!(actual["frontier_storage"]["retained_bytes"], 0);
                validate(&req, &actual);
            }
        }
    }
}

#[test]
fn reclamation_preserves_history_views_symmetry_errors_and_disk_keys() {
    for relative in [
        "tests/fixtures/core-m0/Bag.tla",
        "tests/fixtures/core-m0/Dead.tla",
        "tests/fixtures/core-m1/ViewMerge.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "tests/fixtures/core-m1/SymSets.tla",
        "tests/fixtures/storage-tuple-function/specs/Apply/TupleStorage.tla",
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
        "../../conformance/corpus/construction-set-history/specifications/RepeatedCandidateNested/RepeatedCandidateNested.tla",
    ] {
        for tlc in [false, true] {
            for store in [Store::Fingerprint, Store::Exact] {
                let mut req = request(relative, tlc, store, 4);
                let baseline = check_file_engine(&req, Engine::Core)
                    .expect("resident")
                    .to_json();
                assert!(
                    !matches!(
                        baseline["verdict"].as_str(),
                        Some("unsupported" | "internal-error")
                    ),
                    "{relative}: {baseline}"
                );
                req.store_compact_bytes = Some(0);
                req.frontier_memory_bytes = Some(0);
                req.predecessor_memory_bytes = Some(0);
                req.seen_disk = Some(dive_api::SeenOptions::default());
                let actual = check_file_engine(&req, Engine::Core)
                    .expect("reclaimed")
                    .to_json();
                assert_eq!(
                    observed(&actual),
                    observed(&baseline),
                    "{relative}/{tlc}/{store:?}: {actual}"
                );
                if !actual["counterexample"].is_null() {
                    validate(&req, &actual);
                }
            }
        }
    }
}

#[test]
fn dynamic_names_and_high_trigger_preserve_existing_behavior() {
    for tlc in [false, true] {
        let mut req = request(
            "tests/fixtures/core-retention/DynamicNames.tla",
            tlc,
            Store::Exact,
            1,
        );
        let baseline = check_file_engine(&req, Engine::Core)
            .expect("resident")
            .to_json();
        for trigger in [0, usize::MAX] {
            req.store_compact_bytes = Some(trigger);
            let actual = check_file_engine(&req, Engine::Core)
                .expect("names")
                .to_json();
            assert_eq!(observed(&actual), observed(&baseline), "{tlc}: {actual}");
            if trigger == usize::MAX {
                assert_eq!(
                    actual["core"]["metrics"]["store_reclamation"]["collections"],
                    0
                );
            }
        }
    }
}

#[test]
fn successful_search_releases_payloads_and_large_live_roots_are_never_discarded() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let mut req = chain(tlc, store, 4);
            req.config = Some(req.file.with_file_name("RetainedChainSafe.cfg"));
            let full = check_file_engine(&req, Engine::Core)
                .expect("resident success")
                .to_json();
            assert_eq!(full["verdict"], "success");
            let nodes = full["core"]["metrics"]["store"]["nodes"]
                .as_u64()
                .expect("nodes");
            for trigger in [0, 1, 1 << 20] {
                req.store_compact_bytes = Some(trigger);
                let actual = check_file_engine(&req, Engine::Core)
                    .expect("reclaimed success")
                    .to_json();
                assert_eq!(observed(&actual), observed(&full));
                assert!(
                    actual["core"]["metrics"]["store"]["nodes"]
                        .as_u64()
                        .expect("nodes")
                        + 800
                        < nodes
                );
                if trigger == 1 {
                    assert!(actual["core"]["metrics"]["store_reclamation"]["peak_after_bytes_lower_bound"].as_u64().expect("live bytes") > 1);
                }
            }
        }
    }
}

#[test]
fn remapped_compound_words_preserve_hashcons_and_early_probe_ablations() {
    for relative in [
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/core-m1/SymView.tla",
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
    ] {
        for tlc in [false, true] {
            for workers in [1, 4] {
                let mut req = request(relative, tlc, Store::Exact, workers);
                let base = check_file_engine(&req, Engine::Core)
                    .expect("baseline")
                    .to_json();
                req.store_compact_bytes = Some(0);
                req.frontier_memory_bytes = Some(0);
                req.predecessor_memory_bytes = Some(0);
                let actual = dive_api::check_file_core_opts(&req, "-hashcons,-early-probe")
                    .expect("remapped ablation")
                    .to_json();
                assert_eq!(
                    observed(&actual),
                    observed(&base),
                    "{relative}/{tlc}/W{workers}"
                );
                if !actual["counterexample"].is_null() {
                    validate(&req, &actual);
                }
            }
        }
    }
}

fn temp() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "dive-retention-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn subprocess_recovery_worker() {
    let Some(archive) = std::env::var_os("DIVE_RETENTION_ARCHIVE") else {
        return;
    };
    let output = std::env::var_os("DIVE_RETENTION_OUTPUT").expect("output");
    let tlc = std::env::var_os("DIVE_RETENTION_TLC").expect("profile") == "1";
    let exact = std::env::var_os("DIVE_RETENTION_EXACT").expect("store") == "1";
    let mut req = chain(
        tlc,
        if exact {
            Store::Exact
        } else {
            Store::Fingerprint
        },
        8,
    );
    req.store_compact_bytes = Some(0);
    req.frontier_memory_bytes = Some(0);
    req.predecessor_memory_bytes = Some(0);
    req.seen_disk = Some(dive_api::SeenOptions::default());
    let cp = checkpoint::Request {
        path: archive.into(),
        action: checkpoint::Action::Recover,
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let doc = check_file_checkpoint_engine(&req, &cp, Engine::Core)
        .expect("fresh recovery")
        .to_json();
    std::fs::write(output, serde_json::to_vec(&doc).expect("json")).expect("write");
}

#[test]
fn fresh_process_recovery_reclaims_without_replaying_init_as_search() {
    for tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let req = chain(tlc, store, 4);
            let baseline = check_file_engine(&req, Engine::Core)
                .expect("baseline")
                .to_json();
            let archive = temp();
            let output = temp();
            let cp = checkpoint::Request {
                path: archive.clone(),
                action: checkpoint::Action::PauseAfterLevels(8),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = check_file_checkpoint_engine(&req, &cp, Engine::Core)
                .expect("pause")
                .to_json();
            assert_eq!(
                paused["checkpoint"]["status"], "paused-incomplete",
                "{paused}"
            );
            let status =
                std::process::Command::new(std::env::current_exe().expect("test executable"))
                    .args(["--exact", "subprocess_recovery_worker"])
                    .env("DIVE_RETENTION_ARCHIVE", &archive)
                    .env("DIVE_RETENTION_OUTPUT", &output)
                    .env("DIVE_RETENTION_TLC", if tlc { "1" } else { "0" })
                    .env(
                        "DIVE_RETENTION_EXACT",
                        if store == Store::Exact { "1" } else { "0" },
                    )
                    .status()
                    .expect("fresh process");
            assert!(status.success());
            let actual: Value =
                serde_json::from_slice(&std::fs::read(&output).expect("read")).expect("json");
            assert_eq!(observed(&actual), observed(&baseline), "{actual}");
            assert_eq!(actual["checkpoint"]["status"], "recovered");
            assert!(
                actual["core"]["metrics"]["store_reclamation"]["collections"]
                    .as_u64()
                    .expect("collections")
                    >= 50
            );
            validate(&req, &actual);
            std::fs::remove_file(archive).expect("remove archive");
            std::fs::remove_file(output).expect("remove output");
        }
    }
}

#[test]
fn unsupported_owners_refuse_before_opening_source() {
    let mut req = chain(false, Store::Fingerprint, 1);
    req.store_compact_bytes = Some(0);
    req.file = PathBuf::from("missing-source.tla");
    let cp = checkpoint::Request {
        path: temp(),
        action: checkpoint::Action::PauseAfterLevels(0),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    assert!(matches!(
        check_file_checkpoint_engine(&req, &cp, Engine::Core),
        Err(dive_api::ApiError::Unsupported { .. })
    ));
    req.temporal = dive_api::TemporalMode::FinalShort;
    assert!(matches!(
        check_file_engine(&req, Engine::Core),
        Err(dive_api::ApiError::Unsupported { .. })
    ));
    req.temporal = dive_api::TemporalMode::Ordinary;
    req.core_sample = Some(1);
    assert!(matches!(
        check_file_engine(&req, Engine::Core),
        Err(dive_api::ApiError::Unsupported { .. })
    ));
    for tlc in [false, true] {
        let mut effect = request(
            "tests/fixtures/core-retention/RetentionEffect.tla",
            tlc,
            Store::Fingerprint,
            4,
        );
        effect.store_compact_bytes = Some(0);
        let doc = check_file_engine(&effect, Engine::Core)
            .expect("effect refusal")
            .to_json();
        assert_eq!(doc["verdict"], "unsupported");
        assert!(doc["statistics"].is_null());
        assert_eq!(doc["output"], json!([]));
    }
}
