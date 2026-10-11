//! Actual graph finalization, final-before-post ordering and terminal negatives.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "explicit adversarial assertions"
)]
use dive_api::periodic_candidate::{self, Origin, Terminal};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn input(live: &str, post: &str, init: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE FinalOrder ----\nEXTENDS Naturals\nVARIABLE x\nInit == {init}\nNext == x'=1\nLive == {live}\nPost == {post}\n====\n"
    );
    let a = dive_sema::analyze_source("FinalOrder.tla", &source, &dive_sema::Options::default());
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nPOSTCONDITION Post\n")
            .into_config()
            .expect("cfg"),
    )
}
fn run(
    live: &str,
    post: &str,
    init: &str,
    width: usize,
    limits: phase_graph::Limits,
) -> periodic_candidate::Report<dive_liveness::periodic_snapshot::OptionalRun> {
    let (ir, cfg) = input(live, post, init);
    periodic_candidate::check_optional_stopped(
        ir,
        cfg,
        dive_check::CheckOptions {
            workers: width,
            chaos: Some(7),
            ..dive_check::CheckOptions::tlc(0, 0)
        },
        limits,
        periodic_lifecycle::Options::default(),
        dive_check::progress::Options {
            interval: Duration::from_secs(3600),
            force: false,
        },
    )
    .expect("owned diagnostic")
}
#[test]
fn actual_final_candidate_precedes_postcondition_and_replays_independently() {
    for width in [1, 3, 4, 8] {
        for post in ["x=x", "FALSE", "TRUE"] {
            let report = run(
                "<>(x=2)",
                post,
                "x=0",
                width,
                phase_graph::Limits::default(),
            );
            assert_eq!(report.terminal(), &Terminal::EofWitness);
            let word = report.witness().expect("independently admitted word");
            assert!(matches!(word.origin(), Origin::Eof(_)));
            assert_eq!(
                word.memory_validation().validation,
                dive_trace::Validation::Valid
            );
            assert_eq!(
                word.json_validation().validation,
                dive_trace::Validation::Valid
            );
            let actual = report.run().lifecycle();
            assert_eq!(
                actual
                    .finalization()
                    .expect("actual graph admission")
                    .capture()
                    .entries(),
                (2, 4, 2, 1)
            );
            assert!(actual.postcondition_failure().is_none());
            assert!(!actual.postconditions_passed());
            assert!(!report.model_proof());
        }
    }
}
#[test]
fn postcondition_failure_requires_full_final_selection_and_preserves_actual_counters() {
    for width in [1, 3, 4, 8] {
        for (post, code) in [("x=x", 2405), ("FALSE", 1000), ("TRUE", 1000)] {
            let report = run(
                "<>(x=0)",
                post,
                "x=0",
                width,
                phase_graph::Limits::default(),
            );
            assert_eq!(
                report.terminal(),
                &Terminal::PostconditionFailure { code: Some(code) }
            );
            let actual = report.run().lifecycle();
            let finalization = actual.finalization().expect("source-owned full graph");
            assert!(finalization.all_checked());
            assert!(!actual.final_checks().is_empty());
            let deciding = actual
                .postcondition_failure()
                .expect("actual deciding post");
            assert_eq!(deciding.stats.expect("actual stats").distinct, 2);
            assert_eq!(deciding.workers.used, width);
            assert!(report.witness().is_none());
            assert!(!report.model_proof());
        }
    }
}
#[test]
fn failed_init_and_original_capture_bound_cannot_mint_finalization_or_posts() {
    for width in [1, 3, 4, 8] {
        for (init, limits) in [
            ("x=1 \\div 0", phase_graph::Limits::default()),
            (
                "x=0",
                phase_graph::Limits {
                    capture: dive_check::capture::Limits {
                        states: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        ] {
            let report = run("<>(x=2)", "FALSE", init, width, limits);
            let actual = report.run().lifecycle();
            assert_eq!(report.terminal(), &Terminal::Incomplete);
            assert!(actual.safety_eof().is_none());
            assert!(actual.finalization().is_none());
            assert!(actual.postcondition_failure().is_none());
            assert!(!actual.postconditions_passed());
            assert!(actual.final_checks().is_empty());
            assert!(report.witness().is_none());
        }
    }
}
