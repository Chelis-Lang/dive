//! Post-Next error authority comes from fresh source and ordered row replay.

use std::sync::Arc;

use dive_eval::Options;
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
    let name = source.split_whitespace().nth(2).ok_or("module name")?;
    let parsed = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(parsed.accepted(), "{parsed:?}");
    let ir = Arc::new(dive_ir::lower(&parsed)?);
    let config = dive_config::parse("SPECIFICATION Spec\nPROPERTY Live\n")
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
    let pick = ir.root_def("Pick").ok_or("Pick")?;
    let scope = model.evaluator.scope(pick)?;
    let error = model
        .evaluator
        .enabled_scoped(
            &scope,
            ir.def(pick).body,
            None,
            Arc::from([Some(Value::Int(0)), Some(Value::Int(0))]),
        )
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
    }
    Ok(())
}
