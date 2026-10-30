//! Fresh candidate replay must execute primary and secondary demand order.
use dive_eval::{HostControl, Interrupted, Options, St};
use dive_model::{BindOptions, Recipe};
use dive_trace::{
    CandidateErrorSite, CandidateStateEndpoint, Ending, Session, TraceDoc, Validation,
    ValidationRunError,
};
use dive_value::Value;
use serde_json::json;
use std::sync::Arc;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn input(
    next: &str,
    extra: &str,
    config: &str,
    options: Options,
) -> Result<(Session, dive_model::Model), Box<dyn std::error::Error>> {
    let text = format!(
        "---- MODULE Candidate ----\nEXTENDS Naturals,TLC\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == {next}\nC == x#2 \\/ (CHOOSE i \\in {{1}}: FALSE)=0\n{extra}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Candidate.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nCONSTRAINT C\n{config}"
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
fn request(model: &dive_model::Model) -> Result<TraceDoc, Box<dyn std::error::Error>> {
    let error = model
        .eval(
            &model.constraints[0].pred,
            &St::state(Arc::from([Some(Value::Int(2)), Some(Value::Int(0))])),
        )
        .err()
        .ok_or("actual candidate error")?;
    Ok(
        TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":2,"y":0})])?.with_ending(
            Ending::CandidateStateError {
                ec: error.ec().name().into(),
                endpoint: CandidateStateEndpoint::from_error(
                    &model.ir,
                    &error,
                    CandidateErrorSite::Constraint,
                    1000,
                    2103,
                )
                .ok_or("source")?,
            },
        ),
    )
}
#[derive(Debug)]
struct Stop;
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        Err(Interrupted::new(125))
    }
}
#[test]
fn diagnostic_error_priority_history_and_host_refusals_are_executed() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (good, model) = input("x'=2 /\\ y'=0", "", "", options)?;
        let doc = request(&model)?;
        assert_eq!(good.try_read(&doc)?.0.validation, Validation::Valid);
        assert!(matches!(
            good.read_controlled(&doc, Arc::new(Stop)),
            Err(ValidationRunError::Interrupted { .. })
        ));
        let (secondary, model) = input(
            "x' \\in {2,3} /\\ y'=IF x'=2 THEN 0 ELSE 1 \\div 0",
            "",
            "",
            options,
        )?;
        assert_eq!(model.actions.len(), 1);
        let doc = request(&model)?;
        assert!(
            matches!(secondary.try_read(&doc)?.0.validation,Validation::Invalid(ref failure) if failure.detail.contains("deciding wrapper"))
        );
        let mut wire = doc.to_json();
        wire["ending"]["endpoint"]["deciding"] = json!(2179);
        assert_eq!(
            secondary
                .try_read(&TraceDoc::from_json(&wire)?)?
                .0
                .validation,
            Validation::Valid
        );
        let (history, model) = input(
            "x' \\in {1,2} /\\ y'=0",
            "Inv == x>=0",
            "INVARIANT Inv",
            options,
        )?;
        assert!(
            matches!(history.try_read(&request(&model)?)?.0.validation,Validation::Unsupported(ref feature) if feature.contains("newness"))
        );
        let (view, model) = input("x'=2 /\\ y'=0", "V == x", "VIEW V", options)?;
        assert!(
            matches!(view.try_read(&request(&model)?)?.0.validation,Validation::Unsupported(ref feature) if feature.contains("VIEW"))
        );
        let (effects, model) = input("x'=2 /\\ y'=PrintT(0)", "", "", options)?;
        assert!(matches!(
            effects.try_read(&request(&model)?)?.0.validation,
            Validation::Unsupported(_)
        ));
    }
    Ok(())
}
