//! Actual exhausted-queue terminal service, separate from timed requests.
#![allow(
    clippy::expect_used,
    reason = "finite source-owned positive/negative controls"
)]
use dive_api::periodic_candidate::{self, Origin};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::{sync::Arc, time::Duration};
fn inputs(
    init: &str,
    next: &str,
    property: &str,
    hook: Option<&str>,
) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE Terminal ----\nEXTENDS Integers\nVARIABLE x\nInit == {init}\nNext == {next}\nLive == {property}\nHook == {}\n====\n",
        hook.unwrap_or("FALSE")
    );
    let a = dive_sema::analyze_source("Terminal.tla", &source, &dive_sema::Options::default());
    let cfg = format!(
        "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n{}",
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
fn options(w: usize) -> dive_check::CheckOptions {
    dive_check::CheckOptions {
        workers: w,
        chaos: Some(7),
        ..dive_check::CheckOptions::tlc(0, 0)
    }
}
fn requests(force: bool) -> dive_check::progress::Options {
    dive_check::progress::Options {
        interval: Duration::from_secs(3600),
        force,
    }
}
#[test]
fn safe_finished_queue_skips_false_integer_hook_and_confirms_post_service_joins() {
    for w in [1, 3, 4, 8] {
        for hook in [None, Some("TRUE"), Some("FALSE"), Some("1")] {
            let (ir, cfg) = inputs("x=0", "x'=1", "<>(x=0)", hook);
            let r = periodic_candidate::check_optional_terminal(
                ir,
                cfg,
                options(w),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                requests(false),
            )
            .expect("terminal diagnostic");
            let run = r.run().lifecycle();
            let t = run.terminal_service().expect("actual completed frontier");
            assert_eq!(t.stats(), run.safety_eof().expect("postjoin EOF"));
            assert_eq!(t.stats().left_on_queue, 0);
            assert_eq!((t.stats().generated, t.stats().distinct), (3, 2));
            assert_eq!(t.pool().0, w);
            assert!(t.joins_confirmed());
            assert!(t.pending_request().is_none());
            assert_eq!(t.outcome(), &Ok(dive_liveness::periodic::Outcome::Complete));
            assert!(run.service_evaluations().is_empty());
            assert!(run.request_services().is_empty());
            assert_eq!(
                t.suspension_attempted(),
                hook.is_some() || t.runtime_ratio().0 == 0.0
            );
            assert!(!run.final_checks().is_empty());
            assert!(r.witness().is_none());
            assert!(!t.model_proof());
            assert!(!r.model_proof());
        }
    }
}
#[test]
fn deciding_final_words_use_same_independent_replay_and_strict_mutants() {
    for w in [1, 3, 4, 8] {
        let (ir, cfg) = inputs("x=0", "x'=1", "<>(x=2)", Some("FALSE"));
        let r = periodic_candidate::check_optional_terminal(
            ir.clone(),
            cfg.clone(),
            options(w),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(false),
        )
        .expect("real final candidate");
        assert!(
            r.run()
                .lifecycle()
                .terminal_service()
                .expect("terminal")
                .joins_confirmed()
        );
        assert!(r.run().lifecycle().service_evaluations().is_empty());
        let word = r.witness().expect("candidate");
        assert!(matches!(word.origin(), Origin::Eof(_)));
        assert_eq!(
            word.memory_validation().validation,
            dive_trace::Validation::Valid
        );
        assert_eq!(
            word.json_validation().validation,
            dive_trace::Validation::Valid
        );
        for change in 0..4 {
            let fresh = periodic_snapshot::bind_optional(ir.clone(), &cfg).expect("fresh");
            let mut doc = word.document().clone();
            match change {
                0 => {
                    doc.states[0].insert("x".into(), serde_json::json!(99));
                }
                1 => doc.properties = vec!["Unknown".into()],
                2 => {
                    let mut wrong = doc.states[0].clone();
                    wrong.insert("x".into(), serde_json::json!(99));
                    doc.states.push(wrong);
                    doc.labels.push(None);
                    doc.length = Some(doc.states.len());
                    doc.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
                }
                _ => {
                    doc.ending = Some(dive_trace::Ending::Lasso {
                        back_to: doc.states.len(),
                    });
                }
            }
            let t = dive_trace::Trace::decode(fresh.model(), &doc).expect("transport");
            assert!(matches!(
                dive_trace::validate(fresh.model(), &t, None).validation,
                dive_trace::Validation::Invalid(_)
            ));
            let bytes = serde_json::to_string(&doc.to_json()).expect("JSON");
            let parsed = dive_trace::TraceDoc::parse(&bytes).expect("parse");
            let fresh = periodic_snapshot::bind_optional(ir.clone(), &cfg).expect("fresh JSON");
            assert!(matches!(
                dive_trace::validate_doc(fresh.model(), &parsed, &[], None)
                    .0
                    .validation,
                dive_trace::Validation::Invalid(_)
            ));
        }
    }
}
#[test]
fn failed_init_expansion_cut_hook_and_quota_never_create_terminal_authority() {
    for w in [1, 3, 4, 8] {
        for (init, next, hook, forced, states) in [
            ("x=(1 \\div 0)", "x'=1", None, true, None),
            ("x=0", "x'=(1 \\div 0)", None, false, None),
            ("x=0", "x'=1", Some("FALSE"), true, None),
            ("x=0", "x'=x+1", None, false, Some(3)),
        ] {
            let (ir, cfg) = inputs(init, next, "<>(x=2)", hook);
            let opts = options(w);
            let mut limits = phase_graph::Limits::default();
            if let Some(states) = states {
                limits.capture.states = states;
            }
            let r = periodic_candidate::check_optional_terminal(
                ir,
                cfg,
                opts,
                limits,
                periodic_lifecycle::Options::default(),
                requests(forced),
            )
            .expect("actual stopping diagnostic");
            let run = r.run().lifecycle();
            assert!(run.backend().stop().is_some());
            assert!(run.terminal_service().is_none());
            assert!(run.safety_eof().is_none());
            assert!(run.final_checks().is_empty());
            assert!(r.witness().is_none());
            assert!(!r.model_proof());
        }
    }
}
#[test]
fn empty_init_is_genuine_exhaustion_while_profile_and_zero_work_remain_refused() {
    let (ir, cfg) = inputs("x \\in {}", "x'=1", "<>(x=1)", Some("FALSE"));
    let r = periodic_candidate::check_optional_terminal(
        ir.clone(),
        cfg.clone(),
        options(4),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .expect("empty Init diagnostic");
    let t = r
        .run()
        .lifecycle()
        .terminal_service()
        .expect("real empty frontier");
    assert_eq!(
        (
            t.stats().initial_generated,
            t.stats().distinct,
            t.stats().generated
        ),
        (0, 0, 0)
    );
    assert!(t.suspension_attempted());
    assert!(t.joins_confirmed());
    assert!(r.witness().is_none());
    assert!(!r.model_proof());
    let public =
        periodic_snapshot::bind_optional(ir.clone(), &cfg).expect("retained diagnostic model");
    let ordinary = dive_check::capture::explore(
        public.model(),
        &options(4),
        dive_check::capture::Limits::default(),
    );
    assert!(
        matches!(ordinary, dive_check::capture::Exploration::Stopped(stop)
        if matches!(stop.verdict, dive_check::Verdict::Unsupported { .. }))
    );
    let fail = periodic_candidate::check_optional_terminal(
        ir.clone(),
        cfg.clone(),
        dive_check::CheckOptions::dive(0),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .err()
    .expect("default refusal");
    assert!(fail.run.is_none());
    let mut limits = phase_graph::Limits::default();
    limits.graph.steps = 0;
    let r = periodic_candidate::check_optional_terminal(
        ir,
        cfg,
        options(4),
        limits,
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .expect("bounded diagnostic");
    assert!(r.run().lifecycle().terminal_service().is_none());
    assert!(r.witness().is_none());
}
