//! Ordered errors need actual primary and diagnostic execution, not metadata.
use dive_eval::{HostControl, Interrupted, Options};
use dive_model::{BindOptions, Recipe};
use dive_trace::{Ending, NextStateEndpoint, Session, TraceDoc, Validation, ValidationRunError};
use dive_value::Value;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const BODY: &str = "Init == x=0 /\\ y=0\nNext == (x'=1 /\\ y'=0) \\/ x'=1 \\div (x-x)";
fn input(
    body: &str,
    config: &str,
    options: Options,
) -> Result<(Session, dive_model::Model), Box<dyn std::error::Error>> {
    let source =
        format!("---- MODULE Ordered ----\nEXTENDS Naturals,TLC\nVARIABLES x,y\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("Ordered.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n{config}"
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
        .evaluator
        .successors(
            &model.actions,
            &Arc::from([Some(Value::Int(0)), Some(Value::Int(0))]),
            &mut |_| dive_eval::next::Flow::Continue,
        )
        .err()
        .ok_or("actual generator error")?
        .error;
    let reported = if error.is_eval_exception() {
        error.ec().code()
    } else {
        1000
    };
    let deciding = if error.is_eval_exception() {
        error.ec().code()
    } else {
        2103
    };
    Ok(
        TraceDoc::behavior(vec![json!({"x":0,"y":0})])?.with_ending(Ending::NextStateError {
            ec: error.ec().name().into(),
            endpoint: NextStateEndpoint::from_ordered_error(&model.ir, &error, reported, deciding)
                .ok_or("source")?,
        }),
    )
}
#[test]
fn exact_ordered_source_cause_reported_and_deciding_context_are_required() -> TestResult {
    for (options, default_profile) in [(Options::dive(), true), (Options::tlc(), false)] {
        let (session, model) = input(BODY, "", options)?;
        let doc = request(&model)?;
        let wire = doc.to_json();
        assert_eq!(wire["version"], 1);
        assert_eq!(session.try_read(&doc)?.0.validation, Validation::Valid);
        for (pointer, value) in [
            ("/ending/ec", json!("GENERAL")),
            ("/ending/endpoint/reported", json!(1000)),
            ("/ending/endpoint/deciding", json!(2103)),
            ("/ending/endpoint/source/module", json!("Forged")),
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
        bad["ending"]["endpoint"]["stage"] = json!("constraint");
        assert!(TraceDoc::from_json(&bad).is_err());
        let mut bad = wire.clone();
        bad["ending"]["endpoint"]
            .as_object_mut()
            .ok_or("endpoint")?
            .remove("deciding");
        assert!(TraceDoc::from_json(&bad).is_err());
        let (wrong_site, _) = input(
            &format!("{BODY}\nLimit == IF x=0 THEN TRUE ELSE 1 \\div (x-x)=0"),
            "CONSTRAINT Limit",
            options,
        )?;
        assert!(
            matches!(wrong_site.try_read(&doc)?.0.validation,Validation::Invalid(ref failure) if failure.detail.contains("different evaluation site"))
        );
        let (newness, _) = input(&format!("{BODY}\nInv == x>=0"), "INVARIANT Inv", options)?;
        assert!(
            matches!(newness.try_read(&doc)?.0.validation,Validation::Unsupported(ref feature) if feature.contains("newness"))
        );
        let (excluded, excluded_model) = input(
            &format!("{BODY}\nInside == x=0\nAC == 1 \\div (x-x)=0\nInv == x>=0"),
            "CONSTRAINT Inside\nACTION_CONSTRAINT AC\nINVARIANT Inv",
            options,
        )?;
        assert_eq!(
            excluded.try_read(&request(&excluded_model)?)?.0.validation,
            Validation::Valid
        );
        let (prior_invariant, prior_model) = input(
            &format!("{BODY}\nInside == x=0\nInv == x=0"),
            "CONSTRAINT Inside\nINVARIANT Inv",
            options,
        )?;
        assert!(
            matches!(prior_invariant.try_read(&request(&prior_model)?)?.0.validation,Validation::Invalid(ref failure) if failure.check==dive_trace::Check::Invariant)
        );
        let (property, property_model) = input(
            &format!("{BODY}\nAct == [][x'=x]_<<x,y>>"),
            "PROPERTY Act",
            options,
        )?;
        assert!(
            matches!(property.try_read(&request(&property_model)?)?.0.validation,Validation::Invalid(ref failure) if failure.check==dive_trace::Check::ImpliedAction)
        );
        let (raw, raw_model) = input(
            "Init == x=0 /\\ y=0\nNext == (x'={1,TRUE} /\\ y'=0) \\/ x'=1 \\div (x-x)",
            "",
            options,
        )?;
        let doc = request(&raw_model)?;
        if default_profile {
            // D-028 accepts the first mixed-set successor; the later genuine
            // division-by-zero error is still a source-qualified v1 word.
            let wire = doc.to_json();
            assert_eq!(wire["version"], 1);
            assert_eq!(wire["ending"]["endpoint"]["deciding"], 2179);
            assert_eq!(raw.try_read(&doc)?.0.validation, Validation::Valid);
            for (pointer, value) in [
                ("/ending/ec", json!("GENERAL")),
                ("/ending/endpoint/source/module", json!("Forged")),
            ] {
                let mut bad = wire.clone();
                *bad.pointer_mut(pointer).ok_or("field")? = value;
                assert!(matches!(
                    raw.try_read(&TraceDoc::from_json(&bad)?)?.0.validation,
                    Validation::Invalid(_)
                ));
            }
        } else {
            assert!(
                matches!(raw.try_read(&doc)?.0.validation,Validation::Unsupported(ref feature) if feature.contains("materialized"))
            );
        }
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
            Err(Interrupted::new(125))
        } else {
            Ok(())
        }
    }
}
#[test]
fn diagnostic_execution_is_controlled_and_cannot_be_replaced_by_primary_metadata() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (session, model) = input(
            "Init == x=0 /\\ y=0\nNext == IF ENABLED (x' \\in (x+1)..(x+1048576) /\\ FALSE) THEN x'=x ELSE x'=1 \\div (x-x)",
            "",
            options,
        )?;
        let doc = request(&model)?;
        let one = Arc::new(StopAfter {
            calls: AtomicUsize::new(0),
            limit: usize::MAX,
        });
        {
            let _scope = model.evaluator.host_control(Some(one.clone()))?;
            let _ = request(&model)?;
        }
        let primary = one.calls.load(Ordering::SeqCst);
        assert!(primary > 512, "{primary}");
        // This allows one whole source generation plus a generous fixed/fractional
        // binding margin, but interrupts real work during the diagnostic generation.
        let limit = primary + primary / 4;
        assert!(matches!(
            session.read_controlled(
                &doc,
                Arc::new(StopAfter {
                    calls: AtomicUsize::new(0),
                    limit
                })
            ),
            Err(ValidationRunError::Interrupted { .. })
        ));
        assert_eq!(session.try_read(&doc)?.0.validation, Validation::Valid);
    }
    Ok(())
}
