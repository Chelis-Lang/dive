//! Explicit API admission, failure evidence and independent disk-search witnesses.
#![allow(clippy::expect_used, reason = "test assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{
    CheckRequest, SeenOptions, Store, TemporalMode, check_file, check_file_checkpoint, checkpoint,
};
use std::path::Path;

#[test]
fn malformed_and_unimplemented_requests_refuse_before_input_or_archive_access() {
    for compat_tlc in [false, true] {
        let mut request = CheckRequest {
            file: "missing-seen-model.tla".into(),
            compat_tlc,
            seen_disk: Some(SeenOptions::default()),
            ..CheckRequest::default()
        };
        request.seen_disk.as_mut().expect("policy").limits.entries = 0;
        assert!(
            matches!(check_file(&request),Err(dive_api::ApiError::Rejected{message}) if message.contains("DIVE-S0700"))
        );
        request.seen_disk.as_mut().expect("policy").limits.entries = 16;
        for mode in [TemporalMode::FinalShort, TemporalMode::FinalTlc] {
            request.temporal = mode;
            assert!(
                matches!(check_file(&request),Err(dive_api::ApiError::Unsupported{feature}) if feature.contains("disk seen-state"))
            );
        }
        request.temporal = TemporalMode::Ordinary;
        let archive = checkpoint::Request {
            path: "missing-seen-archive.cp".into(),
            action: checkpoint::Action::Recover,
            counters: checkpoint::Counters::Tlc,
            config_source_sha256: None,
        };
        assert!(
            matches!(check_file_checkpoint(&request,&archive),Err(dive_api::ApiError::Unsupported{feature}) if feature.contains("disk seen-state"))
        );
    }
}

fn witness_case(name: &str, root: &Path) {
    let file = root.join(name).join(format!("{name}.tla"));
    let config = file.with_extension("cfg");
    for compat_tlc in [false, true] {
        let validator = SpecModel::load(
            &file,
            &config,
            &TraceOptions {
                compat_tlc,
                run: Some((0, 7)),
                ..TraceOptions::default()
            },
        )
        .expect("independent model");
        for store in [Store::Fingerprint, Store::Exact] {
            for width in [1, 4, 8] {
                let report = check_file(&CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    seen_disk: Some(SeenOptions::default()),
                    compat_tlc,
                    store,
                    workers: Some(width),
                    seed: Some(7),
                    fp: Some(0),
                    chaos: Some(7),
                    frontier_memory_bytes: Some(0),
                    predecessor_memory_bytes: Some(0),
                    ..CheckRequest::default()
                })
                .expect("check");
                let json = report.to_json();
                assert_eq!(json["options"]["workers"]["requested"], width);
                assert_eq!(json["options"]["workers"]["used"], 1);
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["cleanup_completed"],
                    true
                );
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["expected_disk_bytes"],
                    0
                );
                assert_eq!(
                    json["seen_storage"]["first_failure"],
                    serde_json::Value::Null
                );
                let doc = TraceDoc::from_json(&json).expect("trace transport");
                let controls = validator.controls(&doc);
                assert_eq!(
                    controls.positive.validation,
                    Validation::Valid,
                    "{name} profile{compat_tlc} store{store:?} width{width}: {:?}",
                    controls.positive
                );
                assert!(controls.failures().is_empty(), "{:?}", controls.failures());
                assert_ne!(controls.mutants, [] as [(String, Validation); 0]);
            }
        }
    }
}
#[test]
fn view_counterexample_has_independent_original_and_mutant_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs");
    witness_case("SymOrder", &root);
}
#[test]
fn symmetry_counterexample_has_independent_original_and_mutant_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs");
    witness_case("SymViolation", &root);
}
#[test]
fn invariant_counterexample_has_independent_original_and_mutant_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/disk-seen");
    witness_case("EarlyInvariant", &root);
}
#[test]
fn creation_failure_reports_no_initial_search_or_success() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-check/tests/fixtures/disk-seen/DuplicateNext/DuplicateNext.tla");
    let report = check_file(&CheckRequest {
        file: file.clone(),
        seen_disk: Some(SeenOptions {
            directory: Some(file),
            ..SeenOptions::default()
        }),
        workers: Some(8),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    })
    .expect("typed result");
    let json = report.to_json();
    assert_eq!(json["verdict"], "resource-limit");
    assert_eq!(json["options"]["workers"]["used"], 0);
    assert_eq!(json["seen_storage"]["first_failure"]["phase"], "Creation");
    assert_eq!(json["seen_storage"]["first_failure"]["operation"], "Create");
    assert_eq!(
        json["seen_storage"]["before_cleanup"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["seen_storage"]["after_cleanup"],
        serde_json::Value::Null
    );
}

#[test]
fn low_level_temporal_facades_refuse_before_startup_prints_or_assumptions() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nUnused == PrintT(\"must not bind\")\nASSUME PrintT(\"must not assume\")\nInit == x=0\nNext == x'=x\nP == []<>(x=0)\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    for mut options in [
        dive_check::CheckOptions::tlc(0, 7),
        dive_check::CheckOptions::dive(7),
    ] {
        options.seen_disk = Some(SeenOptions::default());
        let ordinary = dive_check::check(&ir, &config, &options);
        assert!(matches!(
            ordinary.verdict,
            dive_check::Verdict::Unsupported { .. }
        ));
        assert_eq!(ordinary.output, [] as [String; 0]);
        assert!(ordinary.stats.is_none());
        assert!(ordinary.seen.is_none());
        let short = dive_api::temporal::check_final(
            &ir,
            &config,
            &options,
            dive_liveness::analyze::Limits::default(),
        )
        .expect_err("unimplemented combination");
        assert!(matches!(
            short,
            dive_api::temporal::Error::Analysis(
                dive_liveness::analyze::AnalysisError::Unsupported(_)
            )
        ));
        let strict = dive_api::tlc_temporal::check_final(
            &ir,
            &config,
            &options,
            dive_liveness::phase_graph::Limits::default(),
        )
        .expect_err("unimplemented combination");
        assert!(strict.evidence.is_none());
    }
}

#[test]
fn specification_role_refuses_before_binding_even_when_only_safety() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nUnused == PrintT(\"must not bind\")\nASSUME PrintT(\"must not assume\")\nInit == x=0\nNext == x'=x\nSpec == Init /\\ [][Next]_x\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("SPECIFICATION Spec\n")
        .into_config()
        .expect("config");
    for mut options in [
        dive_check::CheckOptions::tlc(0, 7),
        dive_check::CheckOptions::dive(7),
    ] {
        options.seen_disk = Some(SeenOptions::default());
        let result = dive_check::check(&ir, &config, &options);
        assert!(
            matches!(result.verdict, dive_check::Verdict::Unsupported { ref feature } if feature.contains("SPECIFICATION"))
        );
        assert_eq!(result.output, [] as [String; 0]);
        assert!(result.stats.is_none());
        assert!(result.seen.is_none());
    }
}
