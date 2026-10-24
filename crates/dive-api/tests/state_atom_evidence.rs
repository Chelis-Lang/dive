//! Typed state failure evidence does not promote an ordinary error verdict.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "failure transport assertions"
)]

use dive_api::temporal;
use dive_check::CheckOptions;
use dive_liveness::analyze::{AnalysisError, Limits, StateEvaluationSite};
use std::sync::Arc;

#[test]
fn detailed_temporal_api_transports_state_cause_without_an_action_error() {
    let source = include_str!("fixtures/state-atom-error/StateCause.tla");
    let analysis =
        dive_sema::analyze_source("StateCause.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(include_str!("fixtures/state-atom-error/StateCause.cfg"))
        .into_config()
        .expect("config");
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let failure =
            temporal::check_final_with_evidence(&ir, &config, &options, Limits::default())
                .expect_err("state atom refusal");
        assert_eq!(failure.error().code(), "DIVE-L0033");
        assert!(matches!(
            failure.error(),
            temporal::Error::Analysis(AnalysisError::Atom { .. })
        ));
        let state = failure.state_evaluation().expect("transported state cause");
        assert_eq!(state.cause().class(), dive_eval::ErrorClass::Eval);
        assert!(state.core_cause().is_some());
        assert!(state.cause().origin().is_some());
        assert_eq!(state.property(), "P");
        assert_eq!(state.site(), StateEvaluationSite::FinalLabels);
        assert!(state.source().location.contains("StateCause"));
        assert!(failure.action_evaluation().is_none());
        assert_eq!(failure.capture_completed(), Some(true));
        let capture = failure.exploration().expect("real completed search");
        assert_eq!(
            (
                capture.stats.distinct,
                capture.stats.generated,
                capture.stats.depth,
                capture.stats.left_on_queue
            ),
            (2, 3, 2, 0)
        );
    }
}

#[test]
fn ordinary_file_reports_keep_the_refusal_schema_and_full_capture_counts() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/state-atom-error/StateCause.tla");
    for compat_tlc in [false, true] {
        let report = dive_api::check_file(&dive_api::CheckRequest {
            file: file.clone(),
            compat_tlc,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..dive_api::CheckRequest::default()
        })
        .expect("ordinary check");
        assert_eq!(report.verdict(), "unsupported");
        assert_eq!(report.exit_code(), 3);
        let json = report.to_json();
        assert_eq!(json["schema_version"], 3);
        assert_eq!(json["verdict"], "unsupported");
        assert_eq!(json["diagnostics"][0]["code"], "DIVE-L0033");
        assert_eq!(json["execution"]["capture_completed"], true);
        assert_eq!(json["statistics"]["generated"], 3);
        assert_eq!(json["statistics"]["distinct"], 2);
        assert_eq!(json["statistics"]["left_on_queue"], 0);
        assert_eq!(json["statistics"]["depth"], 2);
        assert!(json["property"].is_null());
        assert!(json["tlc"].is_null());
        assert!(json["error_site"].is_null());
        assert!(json["counterexample"].is_null());
        assert!(json.get("state_evaluation").is_none());
    }
}
