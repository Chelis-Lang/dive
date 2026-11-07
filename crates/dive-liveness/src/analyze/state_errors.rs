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
fn final_core_cause_and_legacy_refusal_keep_the_original_error_and_completed_counts() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound("[]<>(1 \\div (x - x) = 0)", "x' = 1 - x", &options);
        let (cause, source, core) = expected(&model, &options);
        assert!(core);
        assert!(cause.origin().is_some());
        let mut detailed = analyze_final_with_action_errors(&model, &options, Limits::default())
            .expect_err("state atom error");
        let evidence = detailed.state_evaluation().expect("typed state cause");
        assert_eq!(evidence.cause(), &cause);
        assert_eq!(evidence.core_cause(), Some(&cause));
        assert_eq!(evidence.source(), &source);
        assert_eq!(evidence.property(), "P");
        assert_eq!(evidence.site(), StateEvaluationSite::FinalLabels);
        assert!(detailed.action_evaluation().is_none());
        assert_eq!(detailed.failure().error.code(), "DIVE-L0033");
        assert_eq!(detailed.failure().stage, Stage::Atoms);
        let captured = detailed
            .failure()
            .exploration
            .as_ref()
            .expect("actual complete capture");
        assert_eq!(
            (
                captured.stats.distinct,
                captured.stats.generated,
                captured.stats.depth,
                captured.stats.left_on_queue
            ),
            (2, 3, 2, 0)
        );
        let state = detailed
            .take_state_evaluation()
            .expect("move state evidence");
        assert!(detailed.state_evaluation().is_none());
        let (legacy, action) = detailed.into_parts();
        assert!(action.is_none());
        assert_eq!(legacy.error.code(), "DIVE-L0033");
        let AnalysisError::Atom {
            property,
            source: outer,
            detail,
        } = legacy.error
        else {
            panic!("legacy atom refusal");
        };
        assert_eq!(property, "P");
        assert_eq!(outer.as_ref(), state.source());
        assert_eq!(detail, cause.to_string());
    }
}

#[test]
fn synthesized_nonboolean_is_distinct_from_the_original_core_cause() {
    let options = CheckOptions::tlc(0, 0);
    let model = bound("[]<>x", "x' = 1 - x", &options);
    let (cause, source, core) = expected(&model, &options);
    assert!(!core);
    let failure = analyze_final_with_action_errors(&model, &options, Limits::default())
        .expect_err("nonboolean state");
    let state = failure
        .state_evaluation()
        .expect("synthesized state failure");
    assert_eq!(state.cause(), &cause);
    assert_eq!(state.source(), &source);
    assert!(state.core_cause().is_none());
    assert_eq!(state.site(), StateEvaluationSite::FinalLabels);
    assert!(failure.action_evaluation().is_none());
    assert_eq!(failure.failure().error.code(), "DIVE-L0033");
}

#[test]
fn due_prefix_state_failure_keeps_periodic_site_and_incomplete_actual_progress() {
    let options = CheckOptions::dive(0);
    let model = bound("[]<>(1 \\div (x - x) = 0)", "x' = x + 1", &options);
    let failure = periodic::analyze_periodic(
        &model,
        &options,
        Limits::default(),
        AcceptancePolicy::Legacy,
        periodic::Schedule {
            min_states: 1,
            growth_factor: 2,
        },
    )
    .expect_err("due prefix state error");
    assert_eq!(failure.capture_completed, Some(false));
    assert_eq!(failure.checks.prefixes, 1);
    assert!(!failure.checks.final_check);
    assert!(failure.failure.failure().exploration.is_none());
    let progress = failure.prefix.as_ref().expect("actual incomplete prefix");
    assert_eq!(
        (
            progress.stats.distinct,
            progress.stats.generated,
            progress.stats.depth,
            progress.stats.left_on_queue
        ),
        (2, 2, 2, 1)
    );
    let state = failure
        .failure
        .state_evaluation()
        .expect("prefix state cause");
    assert_eq!(state.site(), StateEvaluationSite::PeriodicLabels);
    assert!(state.core_cause().is_some());
    assert_eq!(failure.failure.failure().error.code(), "DIVE-L0033");
}

#[test]
fn periodic_final_fallback_is_not_mislabelled_as_a_prefix_failure() {
    let options = CheckOptions::dive(0);
    let model = bound("[]<>(1 \\div (x - x) = 0)", "x' = 1 - x", &options);
    let failure = periodic::analyze_periodic(
        &model,
        &options,
        Limits::default(),
        AcceptancePolicy::Legacy,
        periodic::Schedule::DEFAULT,
    )
    .expect_err("final state failure");
    assert_eq!(failure.capture_completed, Some(true));
    assert_eq!(failure.checks.prefixes, 0);
    assert!(failure.checks.final_check);
    assert!(failure.prefix.is_none());
    assert_eq!(
        failure
            .failure
            .state_evaluation()
            .expect("state cause")
            .site(),
        StateEvaluationSite::FinalLabels
    );
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
            None,
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
