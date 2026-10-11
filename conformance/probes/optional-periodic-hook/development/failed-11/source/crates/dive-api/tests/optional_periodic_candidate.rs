//! Actual optional configuration execution; no supplied hook or word certificate.
#![allow(clippy::expect_used, reason = "paired optional diagnostic controls")]
use dive_api::periodic_candidate::{self, Origin};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::{sync::Arc, time::Duration};
fn inputs(property: &str, hook: Option<&str>) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE Optional ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit == x=0\nNext == x'=1\nLive == {property}\nHook == {}\n====\n",
        hook.unwrap_or("FALSE")
    );
    let a = dive_sema::analyze_source("Optional.tla", &source, &dive_sema::Options::default());
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
fn genuine_absent_hook_eof_words_are_independently_admitted_and_mutants_rejected() {
    for w in [1, 3, 4, 8] {
        let (ir, cfg) = inputs("<>(x=2)", None);
        let r = periodic_candidate::check_optional(
            ir.clone(),
            cfg.clone(),
            options(w),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(false),
        )
        .expect("actual EOF witness");
        assert_eq!(r.run().configured_name(), None);
        assert_eq!(r.run().definition(), None);
        assert_eq!(r.run().expression(), None);
        assert!(r.run().lifecycle().service_evaluations().is_empty());
        assert!(!r.model_proof());
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
        let fresh = periodic_snapshot::bind_optional(ir, &cfg).expect("fresh owned snapshot");
        for change in 0..4 {
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
            let json = serde_json::to_string(&doc.to_json()).expect("JSON");
            let d = dive_trace::TraceDoc::parse(&json).expect("parse");
            assert!(matches!(
                dive_trace::validate_doc(fresh.model(), &d, &cfg.model_values, None)
                    .0
                    .validation,
                dive_trace::Validation::Invalid(_)
            ));
        }
    }
}
#[test]
fn empty_request_is_not_eof_and_false_hook_still_precedes_expansion() {
    for (hook, forced_policy) in [(None, false), (None, true), (Some("FALSE"), false)] {
        let (ir, cfg) = inputs("<>(x=0)", hook);
        let r = periodic_candidate::check_optional(
            ir,
            cfg,
            options(4),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                force: forced_policy,
                ..periodic_lifecycle::Options::default()
            },
            requests(true),
        )
        .expect("diagnostic");
        assert!(r.witness().is_none());
        assert!(!r.model_proof());
        let services = r.run().lifecycle().request_services();
        assert!(!services.is_empty());
        let first = &services[0];
        assert!(first.committed().is_empty());
        assert_eq!(first.graph_source(), None);
        if hook.is_some() {
            assert!(!first.resume_requested());
            assert!(r.run().lifecycle().safety_eof().is_none());
            assert_eq!(r.run().lifecycle().service_evaluations().len(), 1);
        } else {
            assert_eq!(first.resume_requested(), forced_policy);
            assert!(r.run().lifecycle().safety_eof().is_some());
            assert!(r.run().lifecycle().service_evaluations().is_empty());
        }
    }
}
#[test]
fn no_hook_does_not_relax_profile_startup_or_bounds() {
    let (ir, cfg) = inputs("<>(x=2)", None);
    let e = periodic_candidate::check_optional(
        ir.clone(),
        cfg.clone(),
        dive_check::CheckOptions::dive(0),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .err()
    .expect("profile refusal");
    assert!(e.run.is_none());
    let mut limits = phase_graph::Limits::default();
    limits.graph.steps = 0;
    let r = periodic_candidate::check_optional(
        ir,
        cfg,
        options(4),
        limits,
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .expect("bounded diagnostic");
    assert!(r.witness().is_none());
    assert!(!r.model_proof());
    let source = "---- MODULE Hidden ----\nEXTENDS Integers,TLC\nVARIABLE x\nUnused == PrintT(TRUE)\nInit == x=0\nNext == x'=1\nLive == <>(x=2)\n====\n";
    let a = dive_sema::analyze_source("Hidden.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("effect IR"));
    let (_, cfg) = inputs("<>(x=2)", None);
    let e = periodic_candidate::check_optional(
        ir,
        cfg,
        options(4),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(true),
    )
    .err()
    .expect("prestartup refusal");
    assert!(e.run.is_none());
    assert!(matches!(e.error,periodic_candidate::Error::Binding(ref b) if b.output.is_empty()));
}
#[test]
fn actual_failed_initialization_precedes_the_no_hook_request_lane() {
    let source = "---- MODULE InitError ----\nEXTENDS Integers\nVARIABLE x\nInit == x=(1 \\div 0)\nNext == x'=1\nLive == <>(x=2)\n====\n";
    let a = dive_sema::analyze_source("InitError.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let (_, cfg) = inputs("<>(x=2)", None);
    let r = periodic_candidate::check_optional(
        ir,
        cfg,
        options(4),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(true),
    )
    .expect("actual init error diagnostic");
    let stop = r
        .run()
        .lifecycle()
        .backend()
        .stop()
        .expect("actual failed Init stop");
    assert!(matches!(&stop.error, phase_graph::Error::Capture(f)
        if f.verdict.tlc().is_some_and(|class| class.code == 2179)));
    let workers = stop.workers.as_ref().expect("actual inline Init workers");
    assert_eq!((workers.requested, workers.used), (4, 1));
    assert!(r.witness().is_none());
    assert!(!r.model_proof());
    assert!(r.run().lifecycle().request_services().is_empty());
    assert!(r.run().lifecycle().service_evaluations().is_empty());
    assert!(r.run().lifecycle().safety_eof().is_none());
}
