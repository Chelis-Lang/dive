//! State causes retain their real analysis site without becoming error verdicts.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "typed failure contract assertions"
)]

use super::*;
use std::sync::Arc;

fn bound(property: &str, next: &str, options: &CheckOptions) -> Model {
    let text = format!(
        "---- MODULE StateCause ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == {next}\nP == {property}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("StateCause.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .expect("bound model")
}

fn expected(model: &Model, options: &CheckOptions) -> (dive_eval::EvalError, plan::Source, bool) {
    let control = evaluator_control(options).expect("control");
    let prepared = prepare(model, Limits::default(), &control).expect("plan");
    let state = St::state(Arc::from([Some(dive_value::Value::from(0_i64))]));
    for predicate in &prepared[0].plan.atoms {
        if predicate.kind == AtomKind::State
            && let Err(error) = predicate.evaluate_detailed(model, &state)
        {
            let core = matches!(&error, plan::AtomError::Core(_));
            return (
                error.into_error(),
                plan::Source::new(model, predicate.pred.location),
                core,
            );
        }
    }
    panic!("fixture must have an actual state evaluation failure");
}

#[test]
fn attribution_helper_retains_its_distinct_final_and_periodic_site() {
    let options = CheckOptions::tlc(0, 0);
    let model = bound("[]<>(1 \\div (x - x) = 0)", "x' = 1 - x", &options);
    let control = evaluator_control(&options).expect("control");
    let plans = prepare(&model, Limits::default(), &control).expect("plan");
    let states: [StateVals; 1] = [Arc::from([Some(dive_value::Value::from(0_i64))])];
    let (cause, source, _) = expected(&model, &options);
    for (periodic, site) in [
        (false, StateEvaluationSite::FinalAttribution),
        (true, StateEvaluationSite::PeriodicAttribution),
    ] {
        let mut errors = EvaluationErrors {
            periodic,
            ..EvaluationErrors::default()
        };
        let failure = attribute(
            &model,
            &plans,
            &states,
            0,
            Limits::default().atom_cells,
            (&control, &mut errors),
        )
        .expect_err("direct attribution failure");
        assert_eq!(failure.code(), "DIVE-L0033");
        let state = errors.state.expect("actual attribution helper cause");
        assert_eq!(state.site(), site);
        assert_eq!(state.cause(), &cause);
        assert_eq!(state.source(), &source);
        assert!(errors.action.is_none());
    }
}

#[test]
fn owned_budget_interruption_does_not_become_an_ordinary_state_error_or_foreign_stop() {
    use dive_runtime::budget::{Budget, Phase};
    let mut options = CheckOptions::tlc(0, 0);
    options.budget = Budget::new(Some(std::time::Duration::ZERO), None);
    let (_watchdog, armed) = dive_runtime::budget::arm(&options).expect("armed zero budget");
    let model = bound("[]<>x", "x' = 1 - x", &options);
    let control = evaluator_control(&armed).expect("owned control");
    let interrupted =
        dive_eval::HostControl::check(&*control).expect_err("deterministic expired deadline");
    let cause = dive_eval::EvalError::interrupted(interrupted);
    let clean_options = CheckOptions::tlc(0, 0);
    let clean_control = evaluator_control(&clean_options).expect("other owner");
    let plans = prepare(&model, Limits::default(), &clean_control).expect("plan");
    let pred = plans[0]
        .plan
        .atoms
        .iter()
        .find(|atom| atom.kind == AtomKind::State)
        .expect("state predicate");
    let mut errors = EvaluationErrors::default();
    let mut context = StateContext {
        property: "P",
        errors: &mut errors,
        control: &control,
        attribution: false,
    };
    let failure = state_failure(
        &model,
        pred,
        plan::AtomError::Core(cause.clone()),
        &mut context,
    );
    let AnalysisError::Stopped(stopped) = failure else {
        panic!("owned interruption must stop");
    };
    assert_eq!(stopped.phase, Phase::Temporal);
    assert!(errors.state.is_none());
    assert!(errors.action.is_none());
    let mut foreign_context = StateContext {
        property: "P",
        errors: &mut errors,
        control: &clean_control,
        attribution: false,
    };
    let foreign = state_failure(
        &model,
        pred,
        plan::AtomError::Core(cause.clone()),
        &mut foreign_context,
    );
    assert!(matches!(foreign, AnalysisError::Atom { .. }));
    let state = errors
        .state
        .expect("unowned interruption remains original typed refusal");
    assert_eq!(state.cause(), &cause);
    assert_eq!(state.cause().class(), dive_eval::ErrorClass::Interrupted);
}
