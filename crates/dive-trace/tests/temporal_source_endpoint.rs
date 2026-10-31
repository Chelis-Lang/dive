//! Post-Next error authority comes from fresh source and ordered row replay.

use std::sync::Arc;

use dive_eval::{HostControl, Interrupted, Options, St};
use dive_model::{BindOptions, Recipe};
use dive_trace::{Ending, Session, TemporalSourceEndpoint, TraceDoc, Validation};
use dive_value::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FAIL_NEXT: &str =
    include_str!("../../dive-api/tests/fixtures/fairness-primary/MinChooseFair.tla");
const SAFE_NEXT: &str =
    include_str!("../../dive-api/tests/fixtures/fairness-primary/MinChooseFairSafeNext.tla");

fn input(
    source: &str,
    options: Options,
) -> Result<(Session, TraceDoc), Box<dyn std::error::Error>> {
    input_request(source, options, "", "Pick")
}

fn input_request(
    source: &str,
    options: Options,
    extra_config: &str,
    error_name: &str,
) -> Result<(Session, TraceDoc), Box<dyn std::error::Error>> {
    let name = source.split_whitespace().nth(2).ok_or("module name")?;
    let parsed = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(parsed.accepted(), "{parsed:?}");
    let ir = Arc::new(dive_ir::lower(&parsed)?);
    let config = dive_config::parse(&format!(
        "SPECIFICATION Spec\nPROPERTY Live\n{extra_config}\n"
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
    let pick = ir.root_def(error_name).ok_or("error definition")?;
    let scope = model.evaluator.scope(pick)?;
    let state = Arc::from([Some(Value::Int(0)), Some(Value::Int(0))]);
    let error = if error_name == "Pick" {
        model
            .evaluator
            .enabled_scoped(&scope, ir.def(pick).body, None, state)
    } else {
        model
            .evaluator
            .eval_def(pick, &St::state(state))
            .map(|_| false)
    }
    .err()
    .ok_or("observed enabled error")?;
    let endpoint = TemporalSourceEndpoint::from_error(&model, &error).ok_or("source")?;
    let document = TraceDoc::behavior(vec![serde_json::json!({"s":0,"x":0})])?.with_ending(
        Ending::TemporalSourceError {
            ec: error.ec().name().into(),
            endpoint,
        },
    );
    Ok((
        Session::new(Recipe::new(ir, config, options, false), None),
        document,
    ))
}

#[test]
fn next_failure_and_completed_self_row_are_independently_replayed() -> TestResult {
    for source in [FAIL_NEXT, SAFE_NEXT] {
        for options in [Options::dive(), Options::tlc()] {
            let (session, document) = input(source, options)?;
            assert_eq!(document.to_json()["version"], 5);
            assert!(matches!(
                session.read(&document).0.validation,
                Validation::Unsupported(_)
            ));
            let (report, plain) = session.try_read(&document)?;
            assert_eq!(report.validation, Validation::Valid, "{source}: {report:?}");
            let plain = plain.ok_or("plain trace")?;
            assert_eq!(plain.to_json()["version"], 5);
            assert_eq!(session.try_read(&plain)?.0.validation, Validation::Valid);

            let mut wrong = document.to_json();
            wrong["ending"]["endpoint"]["source"]["module"] = serde_json::json!("Wrong");
            assert!(matches!(
                session
                    .try_read(&TraceDoc::from_json(&wrong)?)?
                    .0
                    .validation,
                Validation::Invalid(_)
            ));
            let mut wrong = document.to_json();
            wrong["states"][0]["values"]["x"] = serde_json::json!(1);
            assert!(matches!(
                session
                    .try_read(&TraceDoc::from_json(&wrong)?)?
                    .0
                    .validation,
                Validation::Invalid(_)
            ));
            for version in [1, 2, 3, 4, 6] {
                let mut wrong = document.to_json();
                wrong["version"] = serde_json::json!(version);
                assert!(TraceDoc::from_json(&wrong).is_err());
            }
            let mut wrong = document.to_json();
            wrong["ending"]["endpoint"]["suppresses_next_diagnostic"] = serde_json::json!(true);
            assert!(TraceDoc::from_json(&wrong).is_err());
            let mut wrong = document.to_json();
            wrong["ending"]["ec"] = serde_json::json!("TLC_MODULE_DIVISION_BY_ZERO");
            assert!(matches!(
                session
                    .try_read(&TraceDoc::from_json(&wrong)?)?
                    .0
                    .validation,
                Validation::Invalid(_)
            ));
        }
    }
    Ok(())
}

#[test]
fn actual_self_emission_and_earlier_candidate_errors_keep_their_priority() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let source = FAIL_NEXT.replace("Next == Pick", "Next == UNCHANGED <<s,x>> \\/ Pick");
        let (session, document) = input(&source, options)?;
        assert_eq!(session.try_read(&document)?.0.validation, Validation::Valid);
        let source = FAIL_NEXT.replace("Next == Pick", "Next == s'=1 /\\ x'=1 \\div (x-x)");
        let (session, document) = input(&source, options)?;
        assert!(matches!(
            session.try_read(&document)?.0.validation,
            Validation::Unsupported(_)
        ));
        let source = SAFE_NEXT.replace("====", "Bad == (CHOOSE v \\in {1,2}: v>9)=0\n====");
        let (session, document) = input_request(&source, options, "ACTION_CONSTRAINT Bad", "Pick")?;
        assert!(matches!(
            session.try_read(&document)?.0.validation,
            Validation::Invalid(_)
        ));
    }
    Ok(())
}

#[test]
fn action_bins_precede_state_bins_and_quiet_actions_do_not_skip_state_errors() -> TestResult {
    let source = SAFE_NEXT.replace(
        "Live == <>(s = 1)",
        "BadState == (CHOOSE v \\in {1,2}: v>9+x)=0\nLive == <>[]BadState",
    );
    for options in [Options::dive(), Options::tlc()] {
        let (session, action) = input(&source, options)?;
        assert_eq!(session.try_read(&action)?.0.validation, Validation::Valid);
        let (_, state) = input_request(&source, options, "", "BadState")?;
        assert!(matches!(
            session.try_read(&state)?.0.validation,
            Validation::Invalid(_)
        ));
        let quiet = source.replace("x' = CHOOSE v \\in {1, 2} : v > 5", "x'=0");
        let (session, state) = input_request(&quiet, options, "", "BadState")?;
        assert_eq!(session.try_read(&state)?.0.validation, Validation::Valid);
    }
    Ok(())
}

#[derive(Debug)]
struct StopAfter {
    polls: std::sync::atomic::AtomicUsize,
    limit: usize,
}

impl HostControl for StopAfter {
    fn check(&self) -> Result<(), Interrupted> {
        if self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= self.limit {
            Err(Interrupted::new(502))
        } else {
            Ok(())
        }
    }
}

#[test]
fn owned_replay_interruptions_never_become_valid_error_words() -> TestResult {
    let (session, document) = input(SAFE_NEXT, Options::tlc())?;
    for limit in [0, 1, 20, 100, 300] {
        let control = Arc::new(StopAfter {
            polls: std::sync::atomic::AtomicUsize::new(0),
            limit,
        });
        let result = session.read_controlled(&document, control);
        assert!(
            matches!(
                result,
                Err(dive_trace::ValidationRunError::Interrupted { .. })
            ),
            "limit {limit}: {result:?}"
        );
    }
    assert_eq!(session.try_read(&document)?.0.validation, Validation::Valid);
    let counter = Arc::new(StopAfter {
        polls: std::sync::atomic::AtomicUsize::new(0),
        limit: usize::MAX,
    });
    assert_eq!(
        session
            .read_controlled(&document, counter.clone())?
            .0
            .validation,
        Validation::Valid
    );
    let total = counter.polls.load(std::sync::atomic::Ordering::SeqCst);
    for limit in [total / 2, total.saturating_sub(10), total.saturating_sub(1)] {
        let control = Arc::new(StopAfter {
            polls: std::sync::atomic::AtomicUsize::new(0),
            limit,
        });
        assert!(matches!(
            session.read_controlled(&document, control),
            Err(dive_trace::ValidationRunError::Interrupted { .. })
        ));
    }
    Ok(())
}

#[test]
fn distinct_destination_and_irrelevant_fairness_never_certify_the_claim() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let source = SAFE_NEXT.replace("Next == UNCHANGED <<s,x>>", "Next == s'=1 /\\ x'=0");
        let (session, document) = input(&source, options)?;
        assert!(matches!(
            session.try_read(&document)?.0.validation,
            Validation::Unsupported(_)
        ));
        let source = FAIL_NEXT.replace("WF_<<s, x>>(Pick)", "WF_<<s, x>>(UNCHANGED <<s,x>>)");
        let (session, document) = input(&source, options)?;
        assert_ne!(session.try_read(&document)?.0.validation, Validation::Valid);
        let source = SAFE_NEXT.replace("WF_<<s, x>>(Pick)", "TRUE");
        let (session, document) = input(&source, options)?;
        assert_ne!(session.try_read(&document)?.0.validation, Validation::Valid);
        let source = SAFE_NEXT.replace("Next == UNCHANGED <<s,x>>", "Next == FALSE");
        let (session, document) = input(&source, options)?;
        assert_ne!(session.try_read(&document)?.0.validation, Validation::Valid);
        let source = SAFE_NEXT.replace("Init == s = 0 /\\ x = 0", "Init == s=0 /\\ x \\in {0,1}");
        let (session, document) = input(&source, options)?;
        assert!(matches!(
            session.try_read(&document)?.0.validation,
            Validation::Unsupported(_)
        ));
        let (session, document) = input(SAFE_NEXT, options)?;
        let mut longer = document.to_json();
        let row = longer["states"][0].clone();
        longer["states"].as_array_mut().ok_or("states")?.push(row);
        longer["length"] = serde_json::json!(2);
        assert!(matches!(
            session
                .try_read(&TraceDoc::from_json(&longer)?)?
                .0
                .validation,
            Validation::Unsupported(_)
        ));
    }
    Ok(())
}
