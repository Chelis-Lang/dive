//! Next error authority comes from fresh ordered replay before the first candidate.
use dive_eval::{ActionComposition, HostControl, Interrupted, Options};
use dive_model::{BindOptions, Recipe};
use dive_trace::{
    Ending, NextStateEndpoint, Session, Trace, TraceDoc, Validation, ValidationRunError,
};
use dive_value::Value;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const DIV: &str = "Init == x=0\nNext == x' = 1 \\div (x-x)";
struct Input {
    ir: Arc<dive_ir::Ir>,
    config: dive_config::ModelConfig,
}
impl Input {
    fn new(body: &str, config: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let text = format!(
            "---- MODULE NextEndpoint ----\nEXTENDS Naturals,TLC\nVARIABLE x\n{body}\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("NextEndpoint.tla", &text, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{analysis:?}");
        Ok(Self {
            ir: Arc::new(dive_ir::lower(&analysis)?),
            config: dive_config::parse(&format!(
                "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n{config}"
            ))
            .into_config()
            .ok_or("config")?,
        })
    }
    fn session(&self, options: Options) -> Session {
        Session::new(
            Recipe::new(self.ir.clone(), self.config.clone(), options, false),
            None,
        )
    }
    fn request(&self, options: Options) -> Result<TraceDoc, Box<dyn std::error::Error>> {
        let model = dive_model::bind(
            self.ir.clone(),
            &self.config,
            &BindOptions {
                eval: options,
                ..BindOptions::tlc()
            },
        )?;
        let state = Arc::from([Some(Value::Int(0))]);
        let error = model
            .evaluator
            .successors(&model.actions, &state, &mut |_| {
                dive_eval::next::Flow::Continue
            })
            .err()
            .ok_or("real Next error")?;
        let endpoint =
            NextStateEndpoint::from_error(&model.ir, &error.error).ok_or("original source")?;
        Ok(
            TraceDoc::behavior(vec![json!({"x":0})])?.with_ending(Ending::NextStateError {
                ec: error.error.ec().name().into(),
                endpoint,
            }),
        )
    }
}

#[test]
fn exact_source_and_cause_require_a_fresh_fallible_session() -> TestResult {
    for body in [
        DIV,
        "Init == x=0\nNext == (x<0 /\\ x'=1) \\/ x'=1 \\div (x-x)",
        "Init == x=0\nNext == x' = (IF ENABLED (x' \\in 1..1000000000 /\\ 1 \\div 0=0) THEN 1 ELSE 2)",
    ] {
        let input = Input::new(body, "")?;
        for options in [Options::dive(), Options::tlc()] {
            let doc = input.request(options)?;
            let wire = doc.to_json();
            assert_eq!(wire["version"], 1);
            assert_eq!(wire["ending"]["endpoint"]["stage"], "next-before-successor");
            assert_eq!(TraceDoc::from_json(&wire)?, doc);
            let session = input.session(options);
            assert!(matches!(
                session.read(&doc).0.validation,
                Validation::Unsupported(_)
            ));
            assert_eq!(session.try_read(&doc)?.0.validation, Validation::Valid);
            assert_eq!(session.try_read(&doc)?.0.validation, Validation::Valid);
            let borrowed = dive_model::bind(
                input.ir.clone(),
                &input.config,
                &BindOptions {
                    eval: options,
                    ..BindOptions::tlc()
                },
            )?;
            assert!(matches!(
                dive_trace::validate_doc(&borrowed, &doc, &[]).0.validation,
                Validation::Unsupported(_)
            ));
            for (pointer, value) in [
                ("/ending/ec", json!("TLC_MODULE_OVERFLOW")),
                ("/ending/endpoint/source/module", json!("Wrong")),
                ("/ending/endpoint/source/instance", json!("Other!Next")),
                ("/states/0/values/x", json!(1)),
            ] {
                let mut bad = wire.clone();
                *bad.pointer_mut(pointer).ok_or("field")? = value;
                assert!(
                    matches!(
                        session.try_read(&TraceDoc::from_json(&bad)?)?.0.validation,
                        Validation::Invalid(_)
                    ),
                    "{bad}"
                );
            }
            let mut bad = wire.clone();
            let offset = bad["ending"]["endpoint"]["source"]["start"]
                .as_u64()
                .ok_or("start")?;
            bad["ending"]["endpoint"]["source"]["start"] = json!(offset + 1);
            assert!(matches!(
                session.try_read(&TraceDoc::from_json(&bad)?)?.0.validation,
                Validation::Invalid(_)
            ));
            for stage in ["constraint", "implied-action", "view", "unknown"] {
                let mut bad = wire.clone();
                bad["ending"]["endpoint"]["stage"] = json!(stage);
                assert!(TraceDoc::from_json(&bad).is_err());
            }
            let mut bad = wire;
            bad["ending"]["endpoint"]["action"] = json!(0);
            assert!(TraceDoc::from_json(&bad).is_err());
        }
    }
    Ok(())
}

#[test]
fn first_candidate_and_earlier_checks_cannot_be_skipped() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        for body in [
            "Init == x=0\nNext == x'=1 \\/ x'=1 \\div (x-x)",
            "Init == x=0\nNext == TRUE \\/ x'=1 \\div (x-x)",
        ] {
            let input = Input::new(body, "")?;
            let doc = input.request(options)?;
            assert!(matches!(
                input.session(options).try_read(&doc)?.0.validation,
                Validation::Unsupported(_)
            ));
        }
        let input = Input::new(DIV, "")?;
        let doc = input.request(options)?;
        // Later alternatives must never run after the first candidate. A
        // stopped replay is different from consuming its enormous query.
        let later = Input::new(
            "Init == x=0\nNext == x' \\in {x+1,x+2} /\\ IF x'=x+1 THEN TRUE ELSE ENABLED (x' \\in (x+1)..(x+1000000000) /\\ FALSE)",
            "",
        )?;
        let bound = dive_model::bind(
            later.ir.clone(),
            &later.config,
            &BindOptions {
                eval: options,
                ..BindOptions::tlc()
            },
        )?;
        assert_eq!(bound.actions.len(), 1);
        let control = Arc::new(StopAfter {
            polls: AtomicUsize::new(0),
            limit: 200,
        });
        assert!(matches!(
            later
                .session(options)
                .read_controlled(&doc, control)?
                .0
                .validation,
            Validation::Unsupported(_)
        ));
        let missing = Input::new("Init == x=0\nNext == x'=x /\\ FALSE", "")?;
        let missing_result = missing.session(options).try_read(&doc)?.0.validation;
        assert!(
            matches!(missing_result, Validation::Invalid(_)),
            "{missing_result:?}"
        );
        let other_site = Input::new("Init == x=0\nBad == 1 \\div (x-x)\nNext == x'=Bad", "")?;
        assert!(matches!(
            other_site.session(options).try_read(&doc)?.0.validation,
            Validation::Invalid(_)
        ));
        for (definition, config) in [
            ("Inv == x#0", "INVARIANT Inv"),
            ("Limit == 1 \\div (x-x)=0", "CONSTRAINT Limit"),
        ] {
            let prior = Input::new(&format!("{DIV}\n{definition}"), config)?;
            assert!(matches!(
                prior.session(options).try_read(&doc)?.0.validation,
                Validation::Invalid(_)
            ));
        }
        let generic = doc.clone().with_ending(Ending::Error {
            ec: Some("TLC_MODULE_DIVISION_BY_ZERO".into()),
        });
        assert!(matches!(
            input.session(options).try_read(&generic)?.0.validation,
            Validation::Unsupported(_)
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct StopAfter {
    polls: AtomicUsize,
    limit: usize,
}

#[test]
fn instantiated_error_origins_remain_outside_the_first_slice() -> TestResult {
    let input = Input::new(
        "---- MODULE Inner ----\nEXTENDS Naturals\nVARIABLE y\nStep == y'=1 \\div y\n====\nI == INSTANCE Inner WITH y<-x\nInit == x=0\nNext == I!Step",
        "",
    )?;
    for options in [Options::dive(), Options::tlc()] {
        let doc = input.request(options)?;
        assert!(doc.to_json()["ending"]["endpoint"]["source"]["instance"].is_string());
        assert!(matches!(
            input.session(options).try_read(&doc)?.0.validation,
            Validation::Unsupported(_)
        ));
    }
    Ok(())
}

#[test]
fn reached_composition_refuses_before_private_collection_in_both_modes() -> TestResult {
    for profile in [Options::dive(), Options::tlc()] {
        for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
            let options = Options {
                action_composition: mode,
                ..profile
            };
            for next in [
                "(x'=1) \\cdot (x'=1 \\div (x-x))",
                "(x'=1) \\cdot ((x'=1) \\cdot (x'=1 \\div (x-x)))",
            ] {
                let input = Input::new(&format!("Init == x=0\nNext == {next}"), "")?;
                // An ordinary allowed host still reaches the original native
                // error. Warm its separate runtime before each fresh proof.
                let model = dive_model::bind(
                    input.ir.clone(),
                    &input.config,
                    &BindOptions {
                        eval: options,
                        ..BindOptions::tlc()
                    },
                )?;
                let doc = input.request(options)?;
                assert_eq!(doc.to_json()["ending"]["ec"], "TLC_MODULE_DIVISION_BY_ZERO");
                let session = input.session(options);
                for _ in 0..3 {
                    let error = model
                        .evaluator
                        .successors(
                            &model.actions,
                            &Arc::from([Some(Value::Int(0))]),
                            &mut |_| dive_eval::next::Flow::Continue,
                        )
                        .err()
                        .ok_or("native composition error")?;
                    assert_eq!(error.error.ec().name(), "TLC_MODULE_DIVISION_BY_ZERO");
                    let validation = session.try_read(&doc)?.0.validation;
                    assert!(
                        matches!(&validation, Validation::Unsupported(reason) if reason.contains("action composition")),
                        "{mode:?}: {validation:?}"
                    );
                }
            }
            // Refusal precedes A-side collection, even if no complete state
            // reaches the outer sink and ordinary search would be enormous.
            let input = Input::new(
                "Init == x=0\nNext == (x' \\in 1..1000000000 /\\ FALSE) \\cdot (x'=x)",
                "",
            )?;
            let doc = Input::new(DIV, "")?.request(options)?;
            let control = Arc::new(StopAfter {
                polls: AtomicUsize::new(0),
                limit: 200,
            });
            assert!(matches!(
                input
                    .session(options)
                    .read_controlled(&doc, control)?
                    .0
                    .validation,
                Validation::Unsupported(_)
            ));
            // Only a reached composition is refused. Source presence in an
            // untaken conditional is not a reason to reject the actual error.
            let untaken = Input::new(
                "Init == x=0\nNext == IF x#0 THEN ((x'=1) \\cdot (x'=x)) ELSE x'=1 \\div (x-x)",
                "",
            )?;
            let doc = untaken.request(options)?;
            assert_eq!(
                untaken.session(options).try_read(&doc)?.0.validation,
                Validation::Valid
            );
        }
    }
    Ok(())
}
impl HostControl for StopAfter {
    fn check(&self) -> Result<(), Interrupted> {
        if self.polls.fetch_add(1, Ordering::SeqCst) >= self.limit {
            Err(Interrupted::new(434))
        } else {
            Ok(())
        }
    }
}

#[test]
fn interruption_resource_and_effects_are_never_completed_validity() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let input = Input::new(DIV, "")?;
        let doc = input.request(options)?;
        let completed = Arc::new(StopAfter {
            polls: AtomicUsize::new(0),
            limit: usize::MAX,
        });
        assert_eq!(
            input
                .session(options)
                .read_controlled(&doc, completed.clone())?
                .0
                .validation,
            Validation::Valid
        );
        let count = completed.polls.load(Ordering::SeqCst);
        assert!(count > 2);
        for limit in [0, count - 1] {
            let control = Arc::new(StopAfter {
                polls: AtomicUsize::new(0),
                limit,
            });
            assert!(matches!(
                input.session(options).read_controlled(&doc, control),
                Err(ValidationRunError::Interrupted { .. })
            ));
        }
        let divergent = Input::new(
            "Init == x=0\nNext == x' = (IF ENABLED (x' \\in 1..1000000000 /\\ FALSE) THEN 1 ELSE 2)",
            "",
        )?;
        let control = Arc::new(StopAfter {
            polls: AtomicUsize::new(0),
            limit: count + 100,
        });
        assert!(matches!(
            divergent.session(options).read_controlled(&doc, control),
            Err(ValidationRunError::Interrupted { .. })
        ));
        let recursive = Input::new(
            "RECURSIVE R(_)\nR(n) == IF n=0 THEN 0 ELSE R(n-1)\nInit == x=0\nNext == x'=R(10000)",
            "",
        )?;
        assert!(matches!(
            recursive
                .session(Options {
                    max_depth: 16,
                    ..options
                })
                .try_read(&doc),
            Err(ValidationRunError::ResourceLimit { .. })
        ));
        for effect in [
            "PrintT(\"no\")",
            "TLCSet(1,42)",
            "RandomElement({TRUE})",
            "TLCGet(1)=42",
        ] {
            let impure = Input::new(
                &format!("Init == x=0\nNext == {effect} /\\ x'=1 \\div (x-x)"),
                "",
            )?;
            assert!(matches!(
                impure.session(options).try_read(&doc)?.0.validation,
                Validation::Unsupported(_)
            ));
        }
        for startup in [
            "ASSUME TLCSet(1,42)",
            "ASSUME PrintT(\"no startup output\")",
        ] {
            let impure = Input::new(&format!("{startup}\n{DIV}"), "")?;
            assert!(matches!(
                impure.session(options).try_read(&doc)?.0.validation,
                Validation::Unsupported(_)
            ));
        }
    }
    Ok(())
}

#[test]
fn caller_cache_and_concurrent_session_warmth_supply_no_truth() -> TestResult {
    let input = Input::new(DIV, "")?;
    for options in [Options::dive(), Options::tlc()] {
        let session = input.session(options);
        let doc = input.request(options)?;
        let model = dive_model::bind(
            input.ir.clone(),
            &input.config,
            &BindOptions {
                eval: options,
                ..BindOptions::tlc()
            },
        )?;
        let trace = Trace::decode(&model, &doc).map_err(|error| error.detail)?;
        for _ in 0..3 {
            let _ = model
                .evaluator
                .successors(&model.actions, &trace.states[0], &mut |_| {
                    dive_eval::next::Flow::Continue
                });
        }
        let mut handles = Vec::new();
        for _ in 0..2 {
            let session = session.clone();
            let doc = doc.clone();
            handles.push(std::thread::spawn(move || {
                session
                    .try_read(&doc)
                    .is_ok_and(|(report, _)| report.validation == Validation::Valid)
            }));
        }
        for handle in handles {
            assert!(handle.join().map_err(|_| "validation thread panicked")?);
        }
    }
    Ok(())
}

#[test]
fn actual_receiver_diagnostic_keeps_control_and_rejects_foreign_cause_authority() -> TestResult {
    let input = Input::new(
        "Init == x=0\nNext == [i \\in 1..2 |-> 1 \\div 0] \\notin {} /\\ x'=1",
        "",
    )?;
    for options in [Options::dive(), Options::tlc()] {
        let request = input.request(options)?;
        let mut wire = request.to_json();
        wire["ending"]["endpoint"]["stage"] = json!("next-ordered-terminal");
        wire["ending"]["endpoint"]["reported"] = json!(2179);
        wire["ending"]["endpoint"]["deciding"] = json!(2147);
        let document = TraceDoc::from_json(&wire)?;
        let complete = Arc::new(StopAfter {
            polls: AtomicUsize::new(0),
            limit: usize::MAX,
        });
        assert_eq!(
            input
                .session(options)
                .read_controlled(&document, complete.clone())?
                .0
                .validation,
            Validation::Valid
        );
        let polls = complete.polls.load(Ordering::SeqCst);
        for limit in [0, 1, polls / 2, polls.saturating_sub(1)] {
            let control = Arc::new(StopAfter {
                polls: AtomicUsize::new(0),
                limit,
            });
            assert!(
                matches!(input.session(options).read_controlled(&document,control),Err(ValidationRunError::Interrupted {error,..}) if error.interruption()==Some(Interrupted::new(434)))
            );
        }
        // A matching numeric primary error is not a receiver proof.
        let direct = Input::new(DIV, "")?;
        let mut plain = direct.request(options)?.to_json();
        plain["ending"]["endpoint"]["stage"] = json!("next-ordered-terminal");
        plain["ending"]["endpoint"]["reported"] = json!(2179);
        plain["ending"]["endpoint"]["deciding"] = json!(2147);
        assert!(matches!(
            direct
                .session(options)
                .try_read(&TraceDoc::from_json(&plain)?)?
                .0
                .validation,
            Validation::Unsupported(_)
        ));
        assert_eq!(
            input.session(options).try_read(&document)?.0.validation,
            Validation::Valid
        );
    }
    Ok(())
}
