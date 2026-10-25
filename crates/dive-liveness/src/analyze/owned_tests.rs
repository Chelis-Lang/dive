//! Real source binding and completed-prefix cache controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual owned-driver controls"
)]

use super::*;
use dive_check::budget::Phase;

fn input(body: &str, properties: &str) -> (Arc<Ir>, ModelConfig) {
    let text =
        format!("---- MODULE OwnedRow ----\nEXTENDS Naturals,TLC\nVARIABLE x\n{body}\n====\n");
    let source = dive_sema::analyze_source("OwnedRow.tla", &text, &dive_sema::Options::default());
    assert!(source.accepted(), "{source:?}");
    (
        Arc::new(dive_ir::lower(&source).expect("IR")),
        dive_config::parse(&format!("INIT Init\nNEXT Next\nPROPERTY {properties}\n"))
            .into_config()
            .expect("config"),
    )
}

fn control(options: &CheckOptions) -> Arc<EvaluationControl> {
    EvaluationControl::new(&options.budget, Phase::Binding).expect("binding owner")
}

fn demand_admitted(runtime: &OwnedTemporal) {
    let control = super::super::evaluator_control(&runtime.options).expect("analysis control");
    let plans =
        super::super::prepare(&runtime.model, Limits::default(), &control).expect("actual plans");
    let authority = Authority {
        model: &runtime.model,
    };
    assert!(
        super::super::row_demand::prepare(
            &runtime.model,
            &plans,
            &runtime.options,
            runtime.model.temporals.len(),
            Limits::default().formula,
            GraphLabelPolicy::NoCellLimit,
            (Some(&authority), &control),
        )
        .expect("proof inspection")
        .is_some(),
        "the actual certified route must be reached"
    );
}

#[test]
fn one_binding_and_frozen_constructor_options_survive_the_consuming_run() {
    let (ir, config) = input(
        "C == Print(0,0)\nInit == x=C\nNext == x'=1-x\nP == []<>(x>=0)",
        "P",
    );
    let mut options = CheckOptions::tlc(0, 7);
    let Dispatch::Temporal(runtime) =
        dispatch_controlled(&ir, &config, &options, control(&options))
            .expect("eligible")
            .expect("bind")
    else {
        panic!("temporal")
    };
    options.seed = 99;
    options.workers = 8;
    let super::super::Analysis::Holds(complete) = runtime
        .final_analysis(
            Limits::default(),
            AcceptancePolicy::Legacy,
            GraphLabelPolicy::NoCellLimit,
        )
        .expect("holding predicate")
    else {
        panic!("holding")
    };
    assert_eq!(complete.options.seed, 7);
    assert_eq!(complete.workers.requested, 1);
    assert_eq!(
        complete.output.len(),
        1,
        "startup Print must not be evaluated again"
    );
    assert_eq!((options.seed, options.workers), (99, 8));
}

#[test]
fn no_bind_fallback_and_the_two_binding_failure_contracts_stay_distinct() {
    let (ir, config) = input("Init == x=0\nNext == UNCHANGED x\nP == []<>(x=0)", "P");
    let mut options = CheckOptions::tlc(0, 0);
    options.store = dive_store::StoreMode::Exact;
    assert!(dispatch_controlled(&ir, &config, &options, control(&options)).is_none());
    assert!(dispatch_ordinary_controlled(&ir, &config, &options, control(&options)).is_none());
    let options = CheckOptions::tlc(0, 0);
    let invalid = dive_config::parse("INIT Missing\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config grammar");
    assert!(matches!(
        dispatch_controlled(&ir, &invalid, &options, control(&options)),
        Some(Err(ControlledBindError::Binding(_)))
    ));
    let Some(Ok(Dispatch::Safety(result))) =
        dispatch_ordinary_controlled(&ir, &invalid, &options, control(&options))
    else {
        panic!("ordinary binding failure is still a safety result")
    };
    assert!(result.binding_model_error);
    assert_ne!(result.verdict, dive_check::Verdict::Success);
}

fn legacy_model(ir: &Arc<Ir>, config: &ModelConfig, options: &CheckOptions) -> Model {
    dive_model::bind(
        ir.clone(),
        config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .expect("independent legacy model")
}

fn assert_legacy_prefix(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    candidate: &periodic::PrefixCandidate,
    checks: periodic::Checks,
) {
    let model = legacy_model(ir, config, options);
    let (legacy, legacy_checks) = periodic::analyze_periodic_with_policies(
        &model,
        options,
        Limits::default(),
        AcceptancePolicy::Legacy,
        GraphLabelPolicy::NoCellLimit,
        checks.schedule,
    )
    .expect("borrowed legacy periodic route");
    let periodic::PeriodicAnalysis::Prefix(legacy) = legacy else {
        panic!("legacy nonexhausted prefix")
    };
    assert_eq!(checks, legacy_checks, "all existing D-014 callback facts");
    assert_eq!(candidate.target_property, legacy.target_property);
    assert_eq!(candidate.properties, legacy.properties);
    assert_eq!(candidate.states, legacy.states);
    assert_eq!(candidate.edges, legacy.edges);
    assert_eq!(candidate.back_to, legacy.back_to);
    assert_eq!(candidate.exploration.stats, legacy.exploration.stats);
}

fn assert_legacy_capture(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    complete: &dive_check::capture::Complete,
) {
    let model = legacy_model(ir, config, options);
    let legacy =
        dive_check::capture::explore_with_evidence(&model, options, Limits::default().capture)
            .expect("legacy capture thread");
    let dive_check::capture::Exploration::Complete(legacy) = legacy else {
        panic!("legacy completed capture")
    };
    assert_eq!(complete.stats, legacy.stats);
    assert_eq!(complete.graph.states, legacy.graph.states);
    assert_eq!(complete.graph.edges, legacy.graph.edges);
    assert_eq!(complete.graph.roots, legacy.graph.roots);
    assert_eq!(complete.graph.concrete.len(), legacy.graph.concrete.len());
    for (actual, original) in complete.graph.concrete.iter().zip(&legacy.graph.concrete) {
        assert_eq!(actual.len(), original.len());
        for (actual, original) in actual.iter().zip(original) {
            assert_eq!(actual.state, original.state);
            assert_eq!(actual.ordinal, original.ordinal);
            assert_eq!(actual.label, original.label);
        }
    }
}

#[test]
fn expanded_prefix_maps_and_all_property_caches_feed_fresh_lasso_replay() {
    for second in [0, 5] {
        let (ir, config) = input(
            &format!(
                "Init == x=0\nNext == IF x<3 THEN x'=x+1 ELSE UNCHANGED x\nP == []<>(x=4)\nQ == []<>(x={second})"
            ),
            "P Q",
        );
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let Dispatch::Temporal(runtime) =
                dispatch_controlled(&ir, &config, &options, control(&options))
                    .expect("eligible")
                    .expect("bind")
            else {
                panic!("temporal")
            };
            demand_admitted(&runtime);
            let (result, checks) = runtime
                .periodic_analysis(
                    Limits::default(),
                    AcceptancePolicy::Legacy,
                    GraphLabelPolicy::NoCellLimit,
                    periodic::Schedule {
                        min_states: 1,
                        growth_factor: 2,
                    },
                )
                .expect("periodic analysis");
            let periodic::PeriodicAnalysis::Prefix(candidate) = result else {
                panic!("real nonexhausted prefix")
            };
            assert_legacy_prefix(&ir, &config, &options, &candidate, checks);
            assert_eq!(checks.prefixes, 1, "Init emits no completed-level callback");
            assert_eq!(checks.last_size, 2);
            assert_eq!(checks.last_level, Some(1));
            assert!(!checks.exhausted);
            assert_eq!(candidate.states.len(), 1);
            assert_eq!(candidate.edges.len(), 1);
            assert!(
                candidate.edges[0].ordinal.is_none(),
                "original implicit stutter occurrence"
            );
            assert_eq!(
                candidate.properties,
                if second == 0 {
                    vec!["P".to_owned()]
                } else {
                    vec!["P".to_owned(), "Q".to_owned()]
                }
            );
            let model = dive_model::bind(
                ir.clone(),
                &config,
                &dive_model::BindOptions {
                    eval: options.eval,
                    symmetry_liveness: options.symmetry_liveness,
                    no_deadlock: options.no_deadlock,
                    host: None,
                },
            )
            .expect("independent model");
            let trace = dive_trace::Trace {
                length: Some(candidate.states.len()),
                labels: vec![None; candidate.states.len()],
                states: candidate.states,
                ending: dive_trace::Ending::Lasso {
                    back_to: candidate.back_to,
                },
                properties: candidate.properties,
            };
            assert_eq!(
                dive_trace::validate(&model, &trace).validation,
                dive_trace::Validation::Valid
            );
            let document = trace.to_doc(&model).expect("document");
            assert_eq!(
                dive_trace::validate_doc(&model, &document, &config.model_values)
                    .0
                    .validation,
                dive_trace::Validation::Valid
            );
        }
    }
}

#[test]
fn actual_init_membership_and_quantified_cartesian_generation_preserve_canonical_yields() {
    let (ir, config) = input(
        "Init == x \\in {0,1}\nNext == \\E a \\in ({0,1} \\X {2,3}): x'=a\nP == []<>(x=x)",
        "P",
    );
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let Dispatch::Temporal(runtime) =
            dispatch_controlled(&ir, &config, &options, control(&options))
                .expect("eligible")
                .expect("bind")
        else {
            panic!("temporal")
        };
        demand_admitted(&runtime);
        // Source eligibility alone is not the induction: inspect the actual
        // values emitted through Gen's In and quantified Cartesian paths.
        let super::super::Analysis::Holds(complete) = runtime
            .final_analysis(
                Limits::default(),
                AcceptancePolicy::Legacy,
                GraphLabelPolicy::NoCellLimit,
            )
            .expect("canonical producer")
        else {
            panic!("holds")
        };
        assert_legacy_capture(&ir, &config, &options, &complete);
        let actual = complete
            .graph
            .states
            .iter()
            .map(|state| state[0].clone().expect("complete field"))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                dive_value::Value::Int(0),
                dive_value::Value::Int(1),
                dive_value::Value::tuple(vec![0.into(), 2.into()]),
                dive_value::Value::tuple(vec![0.into(), 3.into()]),
                dive_value::Value::tuple(vec![1.into(), 2.into()]),
                dive_value::Value::tuple(vec![1.into(), 3.into()]),
            ]
        );
        for value in actual.iter().skip(2) {
            let dive_value::Value::Fcn(tuple) = value else {
                panic!("actual product tuple")
            };
            assert!(tuple.is_tuple_literal());
        }
        assert_eq!(complete.stats.distinct, 6);
    }
}

fn assert_same_atom_refusal(actual: &DetailedFailure, legacy: &DetailedFailure) {
    assert!(matches!(
        actual.failure().error,
        super::super::AnalysisError::Atom { .. }
    ));
    assert_eq!(actual.failure().stage, super::super::Stage::Atoms);
    assert_eq!(actual.failure().stage, legacy.failure().stage);
    assert_eq!(
        actual.failure().error.to_string(),
        legacy.failure().error.to_string()
    );
    assert_eq!(
        actual.failure().exploration.as_ref().map(|e| e.stats),
        legacy.failure().exploration.as_ref().map(|e| e.stats)
    );
    let actual = actual.state_evaluation().expect("actual legacy atom error");
    let legacy = legacy
        .state_evaluation()
        .expect("independent legacy atom error");
    assert_eq!(actual.cause(), legacy.cause());
    assert_eq!(actual.site(), legacy.site());
    assert!(actual.capture_row().is_none());
    assert!(legacy.capture_row().is_none());
}

#[test]
fn due_frontier_nonbooleans_keep_legacy_refusals_instead_of_prefix_words() {
    for property in [
        "<>[](IF x=0 THEN TRUE ELSE x)",
        "[]<>(IF x=0 THEN FALSE ELSE x)",
    ] {
        let (ir, config) = input(
            &format!("Init == x=0\nNext == IF x<2 THEN x'=x+1 ELSE UNCHANGED x\nP == {property}"),
            "P",
        );
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let Dispatch::Temporal(runtime) =
                dispatch_controlled(&ir, &config, &options, control(&options))
                    .expect("eligible")
                    .expect("bind")
            else {
                panic!("temporal")
            };
            demand_admitted(&runtime);
            let schedule = periodic::Schedule {
                min_states: 2,
                growth_factor: 1,
            };
            let actual = runtime
                .periodic_analysis(
                    Limits::default(),
                    AcceptancePolicy::Legacy,
                    GraphLabelPolicy::NoCellLimit,
                    schedule,
                )
                .expect_err("no prefix word before frontier error");
            let model = legacy_model(&ir, &config, &options);
            let legacy = periodic::analyze_periodic_with_policies(
                &model,
                &options,
                Limits::default(),
                AcceptancePolicy::Legacy,
                GraphLabelPolicy::NoCellLimit,
                schedule,
            )
            .expect_err("legacy frontier error");
            assert_same_atom_refusal(&actual.failure, &legacy.failure);
            assert_eq!(actual.checks, legacy.checks);
            assert_eq!(actual.capture_completed, Some(false));
            assert_eq!(actual.capture_completed, legacy.capture_completed);
            assert_eq!(
                actual.prefix.as_ref().map(|e| e.stats),
                legacy.prefix.as_ref().map(|e| e.stats)
            );
            assert_eq!(actual.checks.prefixes, 1);
            assert_eq!(actual.checks.last_size, 2);
            assert!(!actual.checks.final_check && !actual.checks.exhausted);
            assert_eq!(
                actual
                    .failure
                    .state_evaluation()
                    .expect("typed cause")
                    .site(),
                super::super::StateEvaluationSite::PeriodicLabels
            );
        }
    }
}

#[test]
fn completed_semantic_fallback_keeps_the_independent_legacy_atom_failure() {
    for property in [
        "<>[]x",
        "<>[](IF x=0 THEN TRUE ELSE x)",
        "[]<>((1 \\div (x=0))=0)",
    ] {
        let (ir, config) = input(
            &format!("Init == x=0\nNext == x'=1-x\nP == {property}"),
            "P",
        );
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let Dispatch::Temporal(runtime) =
                dispatch_controlled(&ir, &config, &options, control(&options))
                    .expect("eligible")
                    .expect("bind")
            else {
                panic!("temporal")
            };
            demand_admitted(&runtime);
            let actual = runtime
                .final_analysis(
                    Limits::default(),
                    AcceptancePolicy::Legacy,
                    GraphLabelPolicy::NoCellLimit,
                )
                .expect_err("completed legacy semantic refusal");
            let model = legacy_model(&ir, &config, &options);
            let legacy = super::super::analyze_final_with_policies(
                &model,
                &options,
                Limits::default(),
                AcceptancePolicy::Legacy,
                GraphLabelPolicy::NoCellLimit,
            )
            .expect_err("legacy atom refusal");
            assert_same_atom_refusal(&actual, &legacy);
            let completed = actual
                .failure()
                .exploration
                .as_ref()
                .expect("actual full capture");
            assert_eq!(completed.stats.distinct, 2);
            assert_eq!(completed.stats.left_on_queue, 0);
            assert_eq!(
                actual.state_evaluation().expect("typed atom").site(),
                super::super::StateEvaluationSite::FinalLabels
            );
            if property.contains("div") {
                assert_eq!(
                    actual
                        .state_evaluation()
                        .expect("real Core")
                        .core_cause()
                        .expect("actual Java override error")
                        .ec(),
                    dive_eval::Ec::JavaMethodOverride
                );
            }
        }
    }
}

#[test]
fn semantic_fallback_continues_the_same_capture_to_later_next_and_invariant_failures() {
    for (next, invariant, properties) in [
        ("IF x=0 THEN x'=1 ELSE x'=1 \\div 0", "", "P"),
        ("x'=x+1", "Inv == x<2", "P\nINVARIANT Inv"),
    ] {
        let (ir, config) = input(
            &format!("Init == x=0\nNext == {next}\n{invariant}\nP == <>[]x"),
            properties,
        );
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let Dispatch::Temporal(runtime) =
                dispatch_controlled(&ir, &config, &options, control(&options))
                    .expect("eligible")
                    .expect("bind")
            else {
                panic!("temporal")
            };
            demand_admitted(&runtime);
            let actual = runtime
                .final_analysis(
                    Limits::default(),
                    AcceptancePolicy::Legacy,
                    GraphLabelPolicy::NoCellLimit,
                )
                .expect_err("later safety event wins");
            let model = legacy_model(&ir, &config, &options);
            let legacy = super::super::analyze_final_with_policies(
                &model,
                &options,
                Limits::default(),
                AcceptancePolicy::Legacy,
                GraphLabelPolicy::NoCellLimit,
            )
            .expect_err("legacy safety event");
            assert_eq!(actual.failure().stage, super::super::Stage::Capture);
            assert_eq!(actual.failure().stage, legacy.failure().stage);
            let (
                super::super::AnalysisError::Exploration(actual),
                super::super::AnalysisError::Exploration(legacy),
            ) = (&actual.failure().error, &legacy.failure().error)
            else {
                panic!("original capture failure")
            };
            assert_eq!(actual.verdict, legacy.verdict);
            assert_eq!(actual.stats, legacy.stats);
            assert_eq!(actual.output, legacy.output);
            assert_eq!(
                actual.counterexample.as_ref().map(|c| c
                    .states
                    .iter()
                    .map(|s| &s.values)
                    .collect::<Vec<_>>()),
                legacy.counterexample.as_ref().map(|c| c
                    .states
                    .iter()
                    .map(|s| &s.values)
                    .collect::<Vec<_>>())
            );
            assert!(
                actual
                    .counterexample
                    .as_ref()
                    .is_some_and(|c| !c.states.is_empty())
            );
            if invariant.is_empty() {
                assert!(matches!(
                    actual.verdict,
                    dive_check::Verdict::EvaluationError { code: 2179, .. }
                ));
            } else {
                assert!(
                    matches!(&actual.verdict, dive_check::Verdict::InvariantViolated { property, initial: false } if property == "Inv")
                );
            }
        }
    }
}

// A cached startup Print is still an effectful producer in the original
// source inspection. Keeping its output does not grant a row certificate.
fn host_fixture_demand_declines(runtime: &OwnedTemporal) {
    let control = super::super::evaluator_control(&runtime.options).expect("analysis control");
    let plans = super::super::prepare(&runtime.model, Limits::default(), &control)
        .expect("actual effectful-source plans");
    let authority = Authority {
        model: &runtime.model,
    };
    assert!(
        super::super::row_demand::prepare(
            &runtime.model,
            &plans,
            &runtime.options,
            runtime.model.temporals.len(),
            Limits::default().formula,
            GraphLabelPolicy::NoCellLimit,
            (Some(&authority), &control),
        )
        .expect("original producer proof inspection")
        .is_none(),
        "effectful Init-to-C producer must decline the certified row route"
    );
}

#[test]
fn operative_host_selection_retains_fresh_owned_final_and_periodic_graphs() {
    for effectful in [true, false] {
        let constant = if effectful { "Print(0,0)" } else { "0" };
        let (ir, config) = input(
            &format!("C == {constant}\nInit == x=1+C\nNext == x'=x\nP == []<>(x=1)"),
            "P",
        );
        for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for labels in [GraphLabelPolicy::Cells(4096), GraphLabelPolicy::NoCellLimit] {
                for periodic in [false, true] {
                    // The ordinary callback capture runs FIRST on its own binding.
                    let reference = legacy_model(&ir, &config, &options);
                    let dive_check::capture::Exploration::Complete(reference) =
                        dive_check::capture::explore_with_evidence(
                            &reference,
                            &options,
                            Limits::default().capture,
                        )
                        .expect("callback reference")
                    else {
                        panic!("complete callback reference")
                    };
                    let Dispatch::Temporal(runtime) =
                        dispatch_controlled(&ir, &config, &options, control(&options))
                            .expect("source eligible")
                            .expect("fresh bind")
                    else {
                        panic!("genuine temporal roles")
                    };
                    assert_eq!(
                        runtime.model.evaluator.host_binding_stamp().native_hooks(),
                        dive_eval::NativeHooks::Callback
                    );
                    if labels == GraphLabelPolicy::NoCellLimit {
                        if effectful {
                            host_fixture_demand_declines(&runtime);
                        } else {
                            demand_admitted(&runtime);
                        }
                    }
                    let result = if periodic {
                        let (analysis, checks) = runtime
                            .periodic_analysis(
                                Limits::default(),
                                AcceptancePolicy::Legacy,
                                labels,
                                periodic::Schedule::DEFAULT,
                            )
                            .expect("owned periodic analysis");
                        assert!(checks.exhausted);
                        let periodic::PeriodicAnalysis::Final(analysis) = analysis else {
                            panic!("complete periodic graph")
                        };
                        analysis
                    } else {
                        runtime
                            .final_analysis(Limits::default(), AcceptancePolicy::Legacy, labels)
                            .expect("owned final analysis")
                    };
                    let super::super::Analysis::Holds(actual) = result else {
                        panic!("actual holding graph")
                    };
                    assert_eq!(actual.stats, reference.stats);
                    assert_eq!(actual.graph.states, reference.graph.states);
                    assert_eq!(actual.graph.edges, reference.graph.edges);
                    assert_eq!(actual.graph.roots, reference.graph.roots);
                    assert_eq!(actual.output, reference.output);
                    assert_eq!(
                        actual.output.len(),
                        usize::from(effectful),
                        "effectful binding Print executes exactly once; pure binding prints nothing"
                    );
                }
            }
        }
    }
}
