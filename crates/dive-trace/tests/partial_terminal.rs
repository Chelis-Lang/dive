//! Missing assignments are separate terminal evidence, never ordinary values.
use dive_eval::{HostControl, Interrupted, Options};
use dive_model::{BindOptions, Recipe};
use dive_trace::{Ending, PartialNextEndpoint, Session, TraceDoc, Validation, ValidationRunError};
use dive_value::Value;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const PARTIAL: &str = "Init == x=0 /\\ y=0\nNext == x'=1";

fn input(
    body: &str,
    options: Options,
) -> Result<(Session, dive_model::Model), Box<dyn std::error::Error>> {
    configured_input(body, "", options)
}

fn configured_input(
    body: &str,
    extra: &str,
    options: Options,
) -> Result<(Session, dive_model::Model), Box<dyn std::error::Error>> {
    let source =
        format!("---- MODULE Partial ----\nEXTENDS Naturals,TLC\nVARIABLES x,y\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("Partial.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n{extra}\n"
    ))
    .into_config()
    .ok_or("config")?;
    let model = dive_model::bind(
        ir.clone(),
        &config,
        &BindOptions {
            eval: options,
            ..BindOptions::tlc()
        },
    )?;
    Ok((
        Session::new(Recipe::new(ir, config, options, false), None),
        model,
    ))
}

fn init_request(model: &dive_model::Model) -> Result<TraceDoc, Box<dyn std::error::Error>> {
    let mut endpoint = None;
    model
        .evaluator
        .init_states(&model.init_actions, &mut |candidate| {
            if candidate.complete() {
                return dive_eval::next::Flow::Continue;
            }
            let mut assignments = serde_json::Map::new();
            let mut unassigned = Vec::new();
            for (name, value) in dive_trace::variables(model).iter().zip(candidate.state) {
                if let Some(value) = value {
                    assignments.insert(
                        name.clone(),
                        dive_trace::value_json::to_json(value, model.evaluator.options().ctx)
                            .unwrap_or(json!(null)),
                    );
                } else {
                    unassigned.push(name.clone());
                }
            }
            endpoint = dive_trace::PartialStateEndpoint::from_initial_candidate(
                &model.ir,
                candidate.action.expr(),
                assignments,
                unassigned,
            );
            dive_eval::next::Flow::Stop
        })?;
    Ok(TraceDoc {
        variables: dive_trace::variables(model),
        states: Vec::new(),
        labels: Vec::new(),
        ending: Some(Ending::PartialInit {
            endpoint: endpoint.ok_or("initial partial candidate")?,
        }),
        length: Some(1),
        source: dive_trace::Source::Dive,
        properties: Vec::new(),
        concrete_closure: false,
        witnesses: Vec::new(),
        draws: Vec::new(),
        startup_draws: Vec::new(),
        run: None,
    })
}

#[test]
fn init_partial_completeness_precedes_checks_and_respects_control() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (first, first_model) = configured_input(
            "Init == x=0\nNext == x'=x /\\ y'=y\nInv == x#0",
            "INVARIANT Inv",
            options,
        )?;
        let doc = init_request(&first_model)?;
        assert_eq!(first.try_read(&doc)?.0.validation, Validation::Valid);
        let control = Arc::new(StopAfter {
            calls: AtomicUsize::new(0),
            limit: usize::MAX,
        });
        assert_eq!(
            first.read_controlled(&doc, control.clone())?.0.validation,
            Validation::Valid
        );
        let calls = control.calls.load(Ordering::SeqCst);
        assert!(calls > 2);
        for limit in [0, calls - 1] {
            assert!(matches!(
                first.read_controlled(
                    &doc,
                    Arc::new(StopAfter {
                        calls: AtomicUsize::new(0),
                        limit
                    })
                ),
                Err(ValidationRunError::Interrupted { .. })
            ));
        }
        let mut bad = doc.clone();
        bad.properties.push("Fake".into());
        let report = first.try_read(&bad)?.0;
        assert!(matches!(report.validation, Validation::Invalid(_)));
        assert_eq!(report.length, 1);
        let (earlier, earlier_model) = configured_input(
            "Init == (x=0 /\\ y=0) \\/ x=1\nNext == x'=x /\\ y'=y\nInv == x#0",
            "INVARIANT Inv",
            options,
        )?;
        assert!(
            matches!(earlier.try_read(&init_request(&earlier_model)?)?.0.validation,Validation::Invalid(ref failure) if failure.check==dive_trace::Check::Invariant)
        );
        let (assumption, assumption_model) =
            input("ASSUME FALSE\nInit == x=0\nNext == x'=x /\\ y'=y", options)?;
        assert!(
            matches!(assumption.try_read(&init_request(&assumption_model)?)?.0.validation,Validation::Invalid(ref failure) if failure.check==dive_trace::Check::Assumption)
        );
        let (newness, newness_model) = configured_input(
            "Init == (x \\in {0,1} /\\ y=0) \\/ x=2\nNext == x'=x /\\ y'=y\nInv == x>=0",
            "INVARIANT Inv",
            options,
        )?;
        assert!(
            matches!(newness.try_read(&init_request(&newness_model)?)?.0.validation,Validation::Unsupported(ref feature) if feature.contains("newness"))
        );
        let (effects, effects_model) = input(
            "Init == PrintT(\"unconfirmed\") /\\ x=0\nNext == x'=x /\\ y'=y",
            options,
        )?;
        assert!(matches!(
            effects
                .try_read(&init_request(&effects_model)?)?
                .0
                .validation,
            Validation::Unsupported(_)
        ));
    }
    Ok(())
}

fn request(model: &dive_model::Model) -> Result<TraceDoc, Box<dyn std::error::Error>> {
    let mut endpoint = None;
    model.evaluator.successors(
        &model.actions,
        &Arc::from([Some(Value::Int(0)), Some(Value::Int(0))]),
        &mut |candidate| {
            if candidate.complete() {
                return dive_eval::next::Flow::Continue;
            }
            let mut assignments = serde_json::Map::new();
            let mut unassigned = Vec::new();
            for (name, value) in dive_trace::variables(model).iter().zip(candidate.state) {
                if let Some(value) = value {
                    assignments.insert(
                        name.clone(),
                        dive_trace::value_json::to_json(value, model.evaluator.options().ctx)
                            .unwrap_or(json!(null)),
                    );
                } else {
                    unassigned.push(name.clone());
                }
            }
            endpoint = PartialNextEndpoint::from_candidate(
                &model.ir,
                candidate.action.expr(),
                assignments,
                unassigned,
            );
            dive_eval::next::Flow::Stop
        },
    )?;
    let mut doc =
        TraceDoc::behavior(vec![json!({"x":0,"y":0})])?.with_ending(Ending::PartialNext {
            endpoint: endpoint.ok_or("candidate")?,
        });
    doc.length = Some(2);
    Ok(doc)
}

#[test]
fn exact_partial_assignments_source_and_version_are_required() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (session, model) = input(PARTIAL, options)?;
        let doc = request(&model)?;
        let wire = doc.to_json();
        assert_eq!(wire["version"], 3);
        assert_eq!(doc.logical_length(), 2);
        assert_eq!(wire["states"].as_array().ok_or("states")?.len(), 1);
        assert_eq!(TraceDoc::from_json(&wire)?, doc);
        assert!(matches!(
            session.read(&doc).0.validation,
            Validation::Unsupported(_)
        ));
        assert_eq!(session.read(&doc).0.length, 2);
        assert!(matches!(
            dive_trace::validate_doc(&model, &doc, &[]).0.validation,
            Validation::Unsupported(_)
        ));
        for _ in 0..3 {
            // Warm caller generation never grants fresh replay permission.
            let _ = request(&model)?;
            let (report, _) = session.try_read(&doc)?;
            assert_eq!(report.validation, Validation::Valid);
            assert_eq!(report.length, 2);
        }
        for (pointer, value) in [
            ("/ending/endpoint/assignments/x", json!(2)),
            ("/ending/endpoint/source/module", json!("Wrong")),
            ("/ending/endpoint/source/end", json!(0)),
            ("/states/0/values/x", json!(1)),
            ("/length", json!(1)),
        ] {
            let mut bad = wire.clone();
            *bad.pointer_mut(pointer).ok_or("pointer")? = value;
            if let Ok(doc) = TraceDoc::from_json(&bad) {
                assert!(
                    matches!(session.try_read(&doc)?.0.validation, Validation::Invalid(_)),
                    "{bad}"
                );
            }
        }
        let mut missing = wire.clone();
        missing["ending"]["endpoint"]["assignments"] = json!({});
        assert!(matches!(
            session
                .try_read(&TraceDoc::from_json(&missing)?)?
                .0
                .validation,
            Validation::Invalid(_)
        ));
        for (pointer, value) in [
            ("/version", json!(1)),
            ("/version", json!(2)),
            ("/ending/ec", json!("TLC_NESTED_EXPRESSION")),
            ("/ending/endpoint/stage", json!("initial-partial-state")),
            ("/ending/endpoint/unassigned", json!(["x"])),
            ("/ending/endpoint/unassigned", json!([])),
        ] {
            let mut bad = wire.clone();
            *bad.pointer_mut(pointer).ok_or("pointer")? = value;
            assert!(TraceDoc::from_json(&bad).is_err(), "{bad}");
        }
        let mut bad = wire.clone();
        bad["ending"]["endpoint"]["verified"] = json!(true);
        assert!(TraceDoc::from_json(&bad).is_err());
        let mut normal = TraceDoc::behavior(vec![json!({"x":0,"y":0})])?.to_json();
        assert_eq!(normal["version"], 1);
        normal["version"] = json!(3);
        assert!(TraceDoc::from_json(&normal).is_err());
    }
    Ok(())
}

#[derive(Debug)]
struct StopAfter {
    calls: AtomicUsize,
    limit: usize,
}
impl HostControl for StopAfter {
    fn check(&self) -> Result<(), Interrupted> {
        if self.calls.fetch_add(1, Ordering::SeqCst) >= self.limit {
            Err(Interrupted::new(607))
        } else {
            Ok(())
        }
    }
}

#[test]
fn earlier_events_effects_and_typed_stops_do_not_certify_a_partial_terminal() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (session, model) = input(PARTIAL, options)?;
        let doc = request(&model)?;
        let control = Arc::new(StopAfter {
            calls: AtomicUsize::new(0),
            limit: usize::MAX,
        });
        assert_eq!(
            session.read_controlled(&doc, control.clone())?.0.validation,
            Validation::Valid
        );
        let calls = control.calls.load(Ordering::SeqCst);
        assert!(calls > 2);
        for limit in [0, calls - 1] {
            let control = Arc::new(StopAfter {
                calls: AtomicUsize::new(0),
                limit,
            });
            assert!(matches!(
                session.read_controlled(&doc, control),
                Err(ValidationRunError::Interrupted { .. })
            ));
        }
        let (earlier, _) = input(
            "Init == x=0 /\\ y=0\nNext == (x'=1 /\\ y'=1 \\div x) \\/ x'=1",
            options,
        )?;
        assert!(
            matches!(earlier.try_read(&doc)?.0.validation,Validation::Invalid(ref failure) if failure.detail.contains("evaluation error before"))
        );
        let (complete, complete_model) = input(
            "Init == x=0 /\\ y=0\nNext == (x'=1 /\\ y'=1) \\/ x'=2",
            options,
        )?;
        assert!(matches!(
            complete.try_read(&doc)?.0.validation,
            Validation::Invalid(_)
        ));
        assert_eq!(
            complete.try_read(&request(&complete_model)?)?.0.validation,
            Validation::Valid
        );
        let (missing, _) = input("Init == x=0 /\\ y=0\nNext == x'=x /\\ FALSE", options)?;
        assert!(matches!(
            missing.try_read(&doc)?.0.validation,
            Validation::Invalid(_)
        ));
        let (long, _) = input(
            "Init == x=0 /\\ y=0\nNext == (x' \\in 1..1000000000 /\\ FALSE) \\/ x'=1",
            options,
        )?;
        assert!(matches!(
            long.read_controlled(
                &doc,
                Arc::new(StopAfter {
                    calls: AtomicUsize::new(0),
                    limit: 20
                })
            ),
            Err(ValidationRunError::Interrupted { .. })
        ));
        let (recursive, _) = input(
            "RECURSIVE R(_)\nR(n) == IF n=0 THEN 0 ELSE R(n-1)\nInit == x=0 /\\ y=0\nNext == x'=R(10000)",
            Options {
                max_depth: 16,
                ..options
            },
        )?;
        assert!(matches!(
            recursive.try_read(&doc),
            Err(ValidationRunError::ResourceLimit { .. })
        ));
        for body in [
            "ASSUME PrintT(\"no output\")\nInit == x=0 /\\ y=0\nNext == x'=1",
            "Init == x=0 /\\ y=0\nNext == PrintT(\"no output\") /\\ x'=1",
        ] {
            let (impure, _) = input(body, options)?;
            assert!(matches!(
                impure.try_read(&doc)?.0.validation,
                Validation::Unsupported(_)
            ));
        }
    }
    Ok(())
}

fn level_request(
    model: &dive_model::Model,
    initial: bool,
) -> Result<TraceDoc, Box<dyn std::error::Error>> {
    let action = if initial {
        model.init_actions.last()
    } else {
        model.actions.last()
    }
    .ok_or("action")?;
    // An untrusted request is intentional: only independent execution
    // can establish its source, level-dependent guard and bindings.
    let assignments = serde_json::Map::from_iter([("x".into(), json!(2))]);
    let endpoint = if initial {
        dive_trace::PartialStateEndpoint::from_initial_candidate(
            &model.ir,
            action.expr(),
            assignments,
            vec!["y".into()],
        )
    } else {
        dive_trace::PartialStateEndpoint::from_candidate(
            &model.ir,
            action.expr(),
            assignments,
            vec!["y".into()],
        )
    }
    .ok_or("endpoint")?;
    Ok(TraceDoc {
        variables: dive_trace::variables(model),
        states: if initial {
            Vec::new()
        } else {
            TraceDoc::behavior(vec![json!({"x":0,"y":0})])?.states
        },
        labels: if initial { Vec::new() } else { vec![None] },
        ending: Some(if initial {
            Ending::PartialInit { endpoint }
        } else {
            Ending::PartialNext { endpoint }
        }),
        length: Some(if initial { 1 } else { 2 }),
        source: dive_trace::Source::Dive,
        properties: Vec::new(),
        concrete_closure: false,
        witnesses: Vec::new(),
        draws: Vec::new(),
        startup_draws: Vec::new(),
        run: None,
    })
}

#[test]
fn ordered_candidate_sites_keep_their_real_levels_and_restore_generation() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        for (initial, definitions, configured, expected_check) in [
            (
                true,
                "C == IF TLCGet(\"level\")=1 THEN TRUE ELSE 1 \\div 0=0",
                "CONSTRAINT C",
                dive_trace::Check::Ending,
            ),
            (
                true,
                "C == TLCGet(\"level\")=1",
                "INVARIANT C",
                dive_trace::Check::Invariant,
            ),
            (
                true,
                "C == TLCGet(\"level\")=1",
                "PROPERTY C",
                dive_trace::Check::ImpliedInit,
            ),
            (
                false,
                "C == IF x=0 THEN TRUE ELSE IF TLCGet(\"level\")=2 THEN TRUE ELSE 1 \\div 0=0",
                "CONSTRAINT C",
                dive_trace::Check::Ending,
            ),
            (
                false,
                "C == IF TLCGet(\"level\")=1 THEN TRUE ELSE 1 \\div 0=0",
                "ACTION_CONSTRAINT C",
                dive_trace::Check::Ending,
            ),
            (
                false,
                "C == [][TLCGet(\"level\")=1]_<<x,y>>",
                "PROPERTY C",
                dive_trace::Check::ImpliedAction,
            ),
        ] {
            let generation = if initial {
                "Init == x \\in {0,2} /\\ IF x=0 THEN y=0 ELSE TLCGet(\"level\")=0\nNext == UNCHANGED <<x,y>>"
            } else {
                "Init == x=0 /\\ y=0\nNext == x' \\in {1,2} /\\ IF x'=1 THEN y'=0 ELSE TLCGet(\"level\")=1"
            };
            let body = format!("{generation}\n{definitions}");
            let (session, model) = configured_input(&body, configured, options)?;
            assert_eq!(
                if initial {
                    model.init_actions.len()
                } else {
                    model.actions.len()
                },
                1
            );
            let doc = level_request(&model, initial)?;
            assert_eq!(
                session.try_read(&doc)?.0.validation,
                Validation::Valid,
                "{initial} {configured}"
            );
            let wrong_definition = if initial || configured != "CONSTRAINT C" {
                definitions.replace("=1", "=0")
            } else {
                definitions.replace("=2", "=0")
            };
            let (wrong, _) = configured_input(
                &format!("{generation}\n{wrong_definition}"),
                configured,
                options,
            )?;
            let report = wrong.try_read(&doc)?.0;
            assert!(
                matches!(report.validation, Validation::Invalid(ref failure) if failure.check == expected_check),
                "{initial} {configured}: {report:?}"
            );
        }
    }
    Ok(())
}
