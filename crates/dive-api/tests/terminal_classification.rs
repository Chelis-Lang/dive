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
        "---- MODULE FinalOrder ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == {init}\nNext == x'=1\nLive == {live}\nPost == {post}\n====\n"
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
        let post = "x=x";
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
        assert_eq!(
            actual
                .counterexample_postcondition_failure()
                .expect("nondetermining render error")
                .verdict
                .tlc()
                .expect("render class")
                .code,
            2405
        );
        assert!(!actual.postconditions_passed());
        assert!(!report.model_proof());
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

#[test]
fn constant_post_render_failure_refuses_witness_while_retaining_actual_final_selection() {
    for width in [1, 3, 4, 8] {
        for post in ["TRUE", "FALSE"] {
            let report = run(
                "<>(x=2)",
                post,
                "x=0",
                width,
                phase_graph::Limits::default(),
            );
            assert_eq!(report.terminal(), &Terminal::Incomplete);
            assert!(report.witness().is_none());
            let actual = report.run().lifecycle();
            assert!(
                actual
                    .finalization()
                    .expect("actual graph")
                    .candidate()
                    .is_some()
            );
            assert!(!actual.postconditions_passed());
            assert!(actual.postcondition_failure().is_none());
            assert!(matches!(
                actual.backend().stop().expect("render refusal").error,
                phase_graph::Error::Input(_)
            ));
        }
    }
}

#[test]
fn original_graph_quota_cannot_mint_final_success_or_post_result() {
    for steps in [0, 1, 16] {
        let report = run(
            "<>(x=0)",
            "x=x",
            "x=0",
            4,
            phase_graph::Limits {
                graph: dive_liveness::product::Limits {
                    steps,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let actual = report.run().lifecycle();
        assert_eq!(report.terminal(), &Terminal::Incomplete);
        assert!(actual.postcondition_failure().is_none());
        assert!(!actual.postconditions_passed());
        assert!(report.witness().is_none());
        assert!(actual.finalization().is_none_or(|f| !f.all_checked()));
        assert!(!report.model_proof());
    }
}

#[test]
fn actual_partial_candidate_with_deferred_post_remains_named_refusal() {
    for width in [1, 3, 4, 8] {
        let (ir, cfg) = input("<>(x=2)", "x=x", "x=0");
        let bound =
            dive_liveness::periodic_snapshot::bind_optional(ir, &cfg).expect("owned snapshot");
        // Public candidate facade uses its own large-stack thread; its thread-local
        // test rendezvous is intentionally not inherited. Exercise the existing
        // actual snapshot service directly at this separately labelled seam.
        let (run, returned) = dive_check::progress::testing::with_returned_prefix_request(|| {
            bound
                .execute_stopped_candidate_source(
                    &dive_check::CheckOptions {
                        workers: width,
                        ..dive_check::CheckOptions::tlc(0, 0)
                    },
                    phase_graph::Limits::default(),
                    periodic_lifecycle::Options {
                        force: true,
                        ratio_limit: f64::INFINITY,
                        progress_interval: i32::MAX,
                        ..Default::default()
                    },
                    dive_check::progress::Options {
                        interval: Duration::from_millis(1),
                        force: false,
                    },
                )
                .expect("owned partial request")
        });
        assert!(returned, "actual stored returned-output rendezvous");
        let actual = run.lifecycle();
        let service = actual.request_services().last().expect("real request");
        assert!(!service.committed().is_empty());
        assert!(!service.resume_requested());
        assert!(
            actual
                .backend()
                .checks()
                .iter()
                .any(|check| check.selection().candidate().is_some())
        );
        assert!(
            matches!(&actual.backend().stop().expect("typed refusal").error, phase_graph::Error::Input(message) if message.contains("partial authority"))
        );
        assert!(actual.safety_eof().is_none());
        assert!(actual.finalization().is_none());
        assert!(!actual.model_proof());
    }
}

#[test]
fn native_effectful_post_remains_preexecution_refused() {
    for post in ["PrintT(TRUE)", "TLCSet(0,TRUE)", "TLCGet(0)=TRUE"] {
        let (ir, cfg) = input("<>(x=2)", post, "x=0");
        assert!(
            periodic_candidate::check_optional_stopped(
                ir,
                cfg,
                dive_check::CheckOptions::tlc(0, 0),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                dive_check::progress::Options {
                    interval: Duration::from_secs(3600),
                    force: false
                }
            )
            .is_err()
        );
    }
}

#[test]
fn actual_short_circuit_posts_preserve_positive_false_and_scalar_paths() {
    for width in [1, 3, 4, 8] {
        for (post, code) in [
            ("TRUE \\/ (x=x)", None),
            ("FALSE /\\ (x=x)", Some(2404)),
            ("IF TRUE THEN 1 ELSE x", Some(2405)),
        ] {
            let safe = run(
                "<>(x=0)",
                post,
                "x=0",
                width,
                phase_graph::Limits::default(),
            );
            let expected = code.map_or(Terminal::FinalSelectionsComplete, |code| {
                Terminal::PostconditionFailure { code: Some(code) }
            });
            assert_eq!(safe.terminal(), &expected);
            let lifecycle = safe.run().lifecycle();
            assert!(lifecycle.finalization().expect("final graph").all_checked());
            assert_eq!(lifecycle.postconditions_passed(), code.is_none());
            assert!(safe.witness().is_none());
            assert!(!safe.model_proof());
            let violating = run(
                "<>(x=2)",
                post,
                "x=0",
                width,
                phase_graph::Limits::default(),
            );
            assert_eq!(violating.terminal(), &Terminal::EofWitness);
            let witness = violating.witness().expect("freshly admitted word");
            assert_eq!(
                witness.memory_validation().validation,
                dive_trace::Validation::Valid
            );
            assert_eq!(
                witness.json_validation().validation,
                dive_trace::Validation::Valid
            );
            assert!(
                violating
                    .run()
                    .lifecycle()
                    .postcondition_failure()
                    .is_none()
            );
            assert_eq!(
                violating
                    .run()
                    .lifecycle()
                    .counterexample_postcondition_failure()
                    .map(|v| v.verdict.tlc().expect("render return").code),
                code
            );
        }
    }
}
