//! Deterministic binding-driver authority and pre-search evidence.
#![allow(clippy::expect_used, reason = "bounded binding control fixtures")]
use super::*;
use dive_eval::HostControl;
use std::time::Duration;

fn input() -> (Arc<Ir>, ModelConfig) {
    let source = "---- MODULE Binding ----\nVARIABLE x\nInit==x=0\nNext==x'=x\n====\n";
    let sema = dive_sema::analyze_source("Binding.tla", source, &dive_sema::Options::default());
    assert!(sema.accepted());
    (
        Arc::new(dive_ir::lower(&sema).expect("IR")),
        dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("config"),
    )
}

#[test]
fn interrupted_binding_reports_no_search_and_requires_this_armed_owner() {
    let (ir, cfg) = input();
    for mut opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let watchdog = budget::Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        opts.budget = watchdog.budget().clone();
        let control =
            budget::EvaluationControl::new(&opts.budget, budget::Phase::Binding).expect("control");
        let failure = bind_for_run_controlled(&ir, &cfg, &opts, None, control.clone())
            .expect_err("binding entry interrupted");
        let result = controlled_binding_failure(&ir, &opts, &failure, &control);
        assert!(
            matches!(result.verdict,Verdict::Stopped(ref stop) if stop.phase==budget::Phase::Binding && stop.resource==budget::Resource::Time)
        );
        assert!(result.stats.is_none());
        assert_eq!(result.workers.used, 0);
        assert!(result.counterexample.is_none());
        let other =
            budget::EvaluationControl::new(&budget::Budget::unlimited(), budget::Phase::Binding)
                .expect("unarmed foreign owner");
        let foreign = controlled_binding_failure(&ir, &opts, &failure, &other);
        assert!(matches!(foreign.verdict, Verdict::InternalError { .. }));
    }
}

#[test]
fn genuine_binding_failure_retains_priority_and_ordered_output() {
    let (ir, _) = input();
    let opts = CheckOptions::dive(0);
    let watchdog = budget::Budget::new(Some(Duration::ZERO), None)
        .start()
        .expect("thread")
        .expect("watchdog");
    let control =
        budget::EvaluationControl::new(watchdog.budget(), budget::Phase::Binding).expect("owner");
    assert!(control.check().is_err());
    let failure = dive_model::ControlledBindError::Binding(Box::new(dive_model::Failure {
        error: dive_model::BindError::tlc(dive_model::Ec::TlcConfigMissingInit, &[]),
        output: vec![dive_model::Message::Print("before".into())],
    }));
    let result = controlled_binding_failure(&ir, &opts, &failure, &control);
    assert!(matches!(result.verdict, Verdict::ModelError { .. }));
    assert_eq!(result.output, ["before"]);
    assert!(result.binding_model_error);
    assert!(result.stats.is_none());
}
