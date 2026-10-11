//! Ordered worker error remains authoritative across stopped service and joins.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite owned-token controls"
)]
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, capture::WorkerStop};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::{sync::Arc, time::Duration};
fn inputs(
    init: &str,
    next: &str,
    role: &str,
    hook: Option<&str>,
) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE WorkerStopped ----\nEXTENDS Integers\nVARIABLES x,y\nInit == {init}\nNext == {next}\nLive == <>(x=0)\nInv == x=0\nAction == [][x'=x]_<<x,y>>\nHook == {}\n====\n",
        hook.unwrap_or("TRUE")
    );
    let a = dive_sema::analyze_source("WorkerStopped.tla", &source, &dive_sema::Options::default());
    let cfg = format!(
        "INIT Init\nNEXT Next\nPROPERTY Live\n{role}\n{}",
        if hook.is_some() {
            "_PERIODIC Hook\n"
        } else {
            ""
        }
    );
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(&cfg).into_config().expect("cfg"),
    )
}
fn options(w: usize) -> CheckOptions {
    CheckOptions {
        workers: w,
        chaos: Some(7),
        ..CheckOptions::tlc(0, 0)
    }
}
fn requests() -> dive_check::progress::Options {
    dive_check::progress::Options {
        interval: Duration::from_secs(3600),
        force: false,
    }
}
#[test]
fn source_error_skips_hook_partial_final_and_preserves_original_result() {
    for width in [1, 3, 4, 8] {
        for hook in [None, Some("TRUE"), Some("FALSE")] {
            for (next, role, kind, code) in [
                ("x'=1 \\div 0 /\\ y'=y", "", WorkerStop::Evaluation, 2179),
                (
                    "x'=1 /\\ y'=y",
                    "INVARIANT Inv",
                    WorkerStop::Invariant,
                    2110,
                ),
                (
                    "x'=1 /\\ y'=y",
                    "PROPERTY Action",
                    WorkerStop::ActionProperty,
                    2112,
                ),
                ("x'=1", "", WorkerStop::Incomplete, 2109),
                ("x<0 /\\ x'=x /\\ y'=y", "", WorkerStop::Deadlock, 2114),
            ] {
                let (ir, cfg) = inputs("x=0 /\\ y=0", next, role, hook);
                let old = periodic_candidate::check_optional_terminal(
                    ir.clone(),
                    cfg.clone(),
                    options(width),
                    phase_graph::Limits::default(),
                    periodic_lifecycle::Options::default(),
                    requests(),
                )
                .expect("old route");
                let fresh = periodic_candidate::check_optional_stopped(
                    ir,
                    cfg,
                    options(width),
                    phase_graph::Limits::default(),
                    periodic_lifecycle::Options::default(),
                    requests(),
                )
                .expect("stopped diagnostic");
                let old = old.run().lifecycle();
                let run = fresh.run().lifecycle();
                let receipt = run.stopped_service().expect("actual source stop");
                assert_eq!(
                    (receipt.kind(), receipt.code(), receipt.source()),
                    (kind, code, 0)
                );
                assert_eq!(receipt.pool().0, width);
                assert_eq!(receipt.pool().1 == 0, width == 1);
                assert_eq!(
                    receipt.joined_stats(),
                    run.backend().stop().expect("original winner").stats
                );
                assert!(receipt.pending_request().is_none());
                assert!(receipt.graph_source().is_none());
                assert_eq!(
                    receipt.outcome(),
                    &Ok(dive_liveness::periodic::Outcome::Complete)
                );
                assert_eq!(receipt.suspension_attempted(), hook.is_some());
                assert_eq!(receipt.force(), (false, false));
                let phase_graph::Error::Capture(original) =
                    &old.backend().stop().expect("old source winner").error
                else {
                    unreachable!("old source")
                };
                let phase_graph::Error::Capture(result) =
                    &run.backend().stop().expect("source winner").error
                else {
                    unreachable!("source")
                };
                assert_eq!(original.verdict, result.verdict);
                assert_eq!(original.stats, result.stats);
                assert_eq!(original.output, result.output);
                assert_eq!(
                    run.backend().stop().expect("stop").stats,
                    old.backend().stop().expect("old").stats
                );
                assert!(run.terminal_service().is_none());
                assert!(run.safety_eof().is_none());
                assert!(run.final_checks().is_empty());
                assert!(run.service_evaluations().is_empty());
                assert!(run.request_services().is_empty());
                assert_eq!(run.stopped_faults().count(), 0);
                assert!(fresh.witness().is_none());
                assert!(!fresh.model_proof());
                assert!(!receipt.model_proof());
            }
        }
    }
}
#[test]
fn outside_eligible_stops_and_default_profile_never_create_stopped_receipt() {
    for width in [1, 3, 4, 8] {
        for (init, next, role, limit) in [
            ("x=1 \\div 0 /\\ y=0", "x'=x /\\ y'=y", "", None),
            ("x=1 /\\ y=0", "x'=x /\\ y'=y", "INVARIANT Inv", None),
            ("x=0 /\\ y=0", "x'=x+1 /\\ y'=y", "", Some(1)),
        ] {
            let (ir, cfg) = inputs(init, next, role, Some("FALSE"));
            let mut limits = phase_graph::Limits::default();
            if let Some(n) = limit {
                limits.capture.states = n;
            }
            let r = periodic_candidate::check_optional_stopped(
                ir,
                cfg,
                options(width),
                limits,
                periodic_lifecycle::Options::default(),
                requests(),
            )
            .expect("honest stop");
            let run = r.run().lifecycle();
            assert!(run.stopped_service().is_none());
            assert!(run.terminal_service().is_none());
            assert!(run.safety_eof().is_none());
            assert!(run.final_checks().is_empty());
            assert!(r.witness().is_none());
        }
    }
    let (ir, cfg) = inputs("x=0 /\\ y=0", "x'=1 /\\ y'=y", "INVARIANT Inv", None);
    let f = periodic_candidate::check_optional_stopped(
        ir.clone(),
        cfg.clone(),
        CheckOptions::dive(0),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(),
    )
    .err()
    .expect("default refused");
    assert!(f.run.is_none());
    let snapshot = periodic_snapshot::bind_optional(ir, &cfg).expect("snapshot");
    assert!(
        matches!(dive_check::capture::explore(snapshot.model(),&options(4),dive_check::capture::Limits::default()),dive_check::capture::Exploration::Stopped(r) if matches!(r.verdict,dive_check::Verdict::Unsupported{..}))
    );
}

#[test]
fn diagnostic_meter_failure_cannot_replace_actual_source_winner_or_mint_completion() {
    for width in [1, 3, 4, 8] {
        let (ir, cfg) = inputs(
            "x=0 /\\ y=0",
            "x'=1 /\\ y'=y",
            "INVARIANT Inv",
            Some("FALSE"),
        );
        let baseline = periodic_candidate::check_optional_terminal(
            ir.clone(),
            cfg.clone(),
            options(width),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(),
        )
        .expect("baseline");
        let mut limits = phase_graph::Limits::default();
        limits.graph.steps = baseline.run().lifecycle().backend().graph_inspections();
        let r = periodic_candidate::check_optional_stopped(
            ir,
            cfg,
            options(width),
            limits,
            periodic_lifecycle::Options::default(),
            requests(),
        )
        .expect("retained source failure");
        let run = r.run().lifecycle();
        let phase_graph::Error::Capture(result) = &run.backend().stop().expect("winner").error
        else {
            unreachable!("original winner")
        };
        assert!(matches!(
            result.verdict,
            dive_check::Verdict::InvariantViolated { initial: false, .. }
        ));
        assert_eq!(
            result.stats,
            baseline
                .run()
                .lifecycle()
                .backend()
                .stop()
                .expect("baseline")
                .stats
        );
        assert!(run.stopped_service().is_none());
        assert_eq!(
            run.stopped_faults().collect::<Vec<_>>(),
            [dive_check::capture::StoppedDiagnosticFault::Callback]
        );
        assert!(run.safety_eof().is_none());
        assert!(run.final_checks().is_empty());
        assert!(r.witness().is_none());
        assert!(!r.model_proof());
    }
}

#[test]
fn forced_stopped_queue_attempts_false_suspension_without_running_false_hook() {
    for width in [1, 3, 4, 8] {
        for hook in [None, Some("FALSE")] {
            let (ir, cfg) = inputs("x=0 /\\ y=0", "x'=1 /\\ y'=y", "INVARIANT Inv", hook);
            let policy = periodic_lifecycle::Options {
                force: true,
                ..periodic_lifecycle::Options::default()
            };
            let r = periodic_candidate::check_optional_stopped(
                ir,
                cfg,
                options(width),
                phase_graph::Limits::default(),
                policy,
                requests(),
            )
            .expect("forced stopped diagnostic");
            let run = r.run().lifecycle();
            let receipt = run.stopped_service().expect("actual transaction");
            assert!(receipt.suspension_attempted());
            assert_eq!(receipt.force(), (true, true));
            assert_eq!(
                receipt.outcome(),
                &Ok(dive_liveness::periodic::Outcome::Complete)
            );
            assert!(receipt.joined_stats().is_some());
            assert!(run.service_evaluations().is_empty());
            assert!(run.request_services().is_empty());
            assert!(run.final_checks().is_empty());
            assert!(run.safety_eof().is_none());
            assert!(r.witness().is_none());
        }
    }
}
