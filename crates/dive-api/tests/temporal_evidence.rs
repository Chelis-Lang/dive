//! Execution evidence must survive failures after a complete graph capture.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_api::temporal::{self, Error, Outcome, Stage};
use dive_check::{CheckOptions, Verdict};
use dive_liveness::analyze::{self, AnalysisError, Limits};
use dive_store::StoreMode;

fn input(source: &str, config: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    (
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        dive_config::parse(config)
            .into_config()
            .expect("configuration"),
    )
}

const SOURCE: &str =
    "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

#[test]
fn post_capture_product_and_atom_limits_retain_actual_work() {
    let (ir, config) = input(SOURCE, CONFIG);
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        options.workers = 4;
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            options.store = store;
            for stage in [analyze::Stage::Atoms, analyze::Stage::Product] {
                let mut limits = Limits::default();
                if stage == analyze::Stage::Atoms {
                    limits.atom_cells = 0;
                } else {
                    limits.product.acceptance_cells = 0;
                }
                let failure = temporal::check_final_with_evidence(&ir, &config, &options, limits)
                    .expect_err("post-capture limit");
                assert_eq!(failure.stage(), Stage::Analysis(stage));
                assert_eq!(failure.capture_completed(), Some(true));
                assert!(matches!(
                    failure.error(),
                    Error::Analysis(AnalysisError::Limit(_) | AnalysisError::Product(_))
                ));
                let capture = failure.exploration().expect("completed capture facts");
                assert_eq!(
                    (
                        capture.stats.generated,
                        capture.stats.distinct,
                        capture.stats.depth
                    ),
                    (3, 2, 2)
                );
                let workers = failure.workers(4).expect("known execution width");
                assert_eq!((workers.requested, workers.used), (4, 4));
                assert!(!workers.sequential);
                assert!(failure.elapsed() > std::time::Duration::ZERO);
                assert_eq!(capture.collision.is_some(), store == StoreMode::Fingerprint);
                assert!(!failure.to_string().contains("no search ran"));
            }
        }
    }
}

#[test]
fn before_capture_planning_and_binding_failures_prove_zero_work() {
    let (ir, config) = input(SOURCE, CONFIG);
    let limits = Limits {
        properties: 0,
        ..Default::default()
    };
    let failure = temporal::check_final_with_evidence(&ir, &config, &CheckOptions::dive(0), limits)
        .expect_err("planning bound");
    assert_eq!(failure.stage(), Stage::Analysis(analyze::Stage::Planning));
    assert_eq!(failure.capture_completed(), Some(false));
    assert!(failure.exploration().is_none());
    assert_eq!(failure.workers(8).expect("proved no capture").used, 0);
    assert_eq!(failure.error().code(), "DIVE-L0034");
    let config = dive_config::parse("INIT Missing\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("valid config grammar");
    let failure = temporal::check_final_with_evidence(
        &ir,
        &config,
        &CheckOptions::dive(0),
        Limits::default(),
    )
    .expect_err("binding failure");
    assert_eq!(failure.stage(), Stage::Binding);
    assert_eq!(failure.capture_completed(), Some(false));
    assert!(failure.exploration().is_none());
    assert_eq!(failure.workers(8).expect("proved no capture").used, 0);
}

#[test]
fn capture_stop_retains_partial_safety_result() {
    let (ir, config) = input(SOURCE, CONFIG);
    let mut options = CheckOptions::dive(0);
    options.workers = 8;
    let limits = Limits {
        capture: dive_check::capture::Limits {
            states: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let result =
        temporal::check_final_with_evidence(&ir, &config, &options, limits).expect("safety stop");
    let Outcome::Safety(stop) = result.outcome() else {
        panic!("partial capture cannot become a temporal verdict");
    };
    assert!(matches!(stop.verdict, Verdict::ResourceLimit { .. }));
    assert_eq!(stop.workers.used, 8);
    assert_eq!(stop.workers.requested, 8);
    assert!(stop.stats.is_some());
}

#[test]
fn legacy_error_interface_preserves_diagnostic_code() {
    let (ir, config) = input(SOURCE, CONFIG);
    let mut limits = Limits::default();
    limits.product.acceptance_cells = 0;
    let error = temporal::check_final(&ir, &config, &CheckOptions::dive(0), limits)
        .expect_err("legacy product failure");
    assert!(matches!(error, Error::Analysis(AnalysisError::Product(_))));
    assert_eq!(error.code(), "DIVE-L0031");
}

#[test]
fn additive_acceptance_policy_keeps_all_legacy_limit_entrypoints() {
    use dive_liveness::product::{AcceptancePolicy, ProductError};
    let (ir, config) = input(
        "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = x\nP == []<>x\n====\n",
        CONFIG,
    );
    for mut options in [CheckOptions::dive(7), CheckOptions::tlc(7, 0)] {
        for workers in [1, 4] {
            options.workers = workers;
            let mut limits = Limits::default();
            limits.product.acceptance_cells = 0;
            let legacy = temporal::check_final_with_evidence(&ir, &config, &options, limits)
                .expect_err("custom legacy cells");
            assert!(matches!(
                legacy.error(),
                Error::Analysis(AnalysisError::Product(ProductError::Limit(
                    "acceptance cells"
                )))
            ));
            assert!(temporal::check_final(&ir, &config, &options, limits).is_err());
            let model = dive_model::bind(
                ir.clone(),
                &config,
                &dive_model::BindOptions {
                    eval: options.eval,
                    no_deadlock: options.no_deadlock,
                    host: None,
                },
            )
            .expect("model");
            assert!(analyze::analyze_final(&model, &options, limits).is_err());
            assert!(analyze::analyze_final_with_evidence(&model, &options, limits).is_err());
            assert!(analyze::analyze_final_with_action_errors(&model, &options, limits).is_err());
            for (policy, expected) in [
                (
                    AcceptancePolicy::Buffers {
                        bytes: 0,
                        work: 100,
                    },
                    "acceptance buffer bytes",
                ),
                (
                    AcceptancePolicy::Buffers {
                        bytes: 100,
                        work: 0,
                    },
                    "acceptance materialization work",
                ),
            ] {
                let failed =
                    temporal::check_final_with_policy(&ir, &config, &options, limits, policy)
                        .expect_err("explicit byte/work refusal");
                assert_eq!(failed.capture_completed(), Some(true));
                assert!(
                    matches!(failed.error(),Error::Analysis(AnalysisError::Product(ProductError::Limit(actual))) if *actual==expected)
                );
            }
            let report = temporal::check_final_with_policy(
                &ir,
                &config,
                &options,
                limits,
                AcceptancePolicy::compact_default(),
            )
            .expect("explicit buffer policy");
            let Outcome::Violation(word) = report.outcome() else {
                panic!("validated violation")
            };
            assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
            assert_eq!(
                dive_trace::validate_doc(&model, word.document(), &[])
                    .0
                    .validation,
                dive_trace::Validation::Valid
            );
            let mut forged = word.document().clone();
            forged.states[0].insert("x".into(), true.into());
            assert!(matches!(
                dive_trace::validate_doc(&model, &forged, &[]).0.validation,
                dive_trace::Validation::Invalid(_)
            ));
        }
    }
}
