//! Initial membership is proved by actual complete source emissions.
use dive_eval::{HostControl, Interrupted, Options};
use dive_model::Recipe;
use dive_trace::{Check, Ending, Session, TraceDoc, Validation, ValidationRunError};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn session(
    body: &str,
    variables: &str,
    extra: &str,
    tlc: bool,
) -> Result<Session, Box<dyn std::error::Error>> {
    let source = format!(
        "---- MODULE OrderedInit ----\nEXTENDS Naturals,TLC\nVARIABLES {variables}\n{body}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("OrderedInit.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&format!("INIT Init\nNEXT Next\n{extra}\n"))
        .into_config()
        .ok_or("config")?;
    Ok(Session::new(
        Recipe::new(
            ir,
            config,
            if tlc { Options::tlc() } else { Options::dive() },
            false,
        ),
        None,
    ))
}

fn behavior(state: Value) -> Result<TraceDoc, Box<dyn std::error::Error>> {
    Ok(TraceDoc::behavior(vec![state])?)
}

#[test]
fn empty_product_and_record_branches_preserve_actual_initial_words() -> TestResult {
    for tlc in [false, true] {
        for empty in ["({1, TRUE} \\X {})", "[a : {1, TRUE}, b : {}]"] {
            let model = session(
                &format!(
                    "Init == (x \\in {empty}) \\/ (x = 0)\nNext == UNCHANGED x\nBad == x # x\nGood == x = x"
                ),
                "x",
                "INVARIANT Bad\nINVARIANT Good",
                tlc,
            )?;
            let doc = behavior(json!({"x":0}))?.with_ending(Ending::Invariant("Bad".into()));
            assert_eq!(
                model.try_read(&doc)?.0.validation,
                Validation::Valid,
                "{empty}/{tlc}"
            );
            assert!(matches!(
                model.try_read(&behavior(json!({"x":1}))?.with_ending(Ending::Invariant("Bad".into())))?.0.validation,
                Validation::Invalid(ref failure) if failure.check == Check::Init
            ));
            assert!(matches!(
                model
                    .try_read(&doc.with_ending(Ending::Invariant("Good".into())))?
                    .0
                    .validation,
                Validation::Invalid(_)
            ));
        }
    }
    Ok(())
}

#[test]
fn multiple_emissions_and_normal_exhaustion_are_membership_proofs() -> TestResult {
    for tlc in [false, true] {
        let model = session(
            "Init == (x \\in {0,1}) \\/ (x=2)\nNext == UNCHANGED x",
            "x",
            "",
            tlc,
        )?;
        for x in [0, 1, 2] {
            assert_eq!(
                model.try_read(&behavior(json!({"x":x}))?)?.0.validation,
                Validation::Valid
            );
        }
        assert!(
            matches!(model.try_read(&behavior(json!({"x":3}))?)?.0.validation,
            Validation::Invalid(ref failure) if failure.check == Check::Init)
        );
        // The matching emission completes membership; later source errors are not demanded.
        let after = session(
            "Init == (x=0) \\/ (x=1 \\div 0)\nNext == UNCHANGED x",
            "x",
            "",
            tlc,
        )?;
        assert_eq!(
            after.try_read(&behavior(json!({"x":0}))?)?.0.validation,
            Validation::Valid
        );
    }
    Ok(())
}

#[test]
fn source_errors_partial_candidates_and_effects_never_prove_membership() -> TestResult {
    for tlc in [false, true] {
        for (body, variables, extra, word) in [
            (
                "Init == (x \\in {1} /\\ 1 \\div (x-1)=0) \\/ (x=0)\nNext == UNCHANGED x",
                "x",
                "",
                json!({"x":0}),
            ),
            (
                "Init == (x=0) \\/ (x=1 /\\ y=0)\nNext == UNCHANGED <<x,y>>",
                "x,y",
                "",
                json!({"x":1,"y":0}),
            ),
            (
                "Init == (x=0 /\\ TLCSet(0,42)) \\/ (x=1)\nNext == UNCHANGED x",
                "x",
                "",
                json!({"x":0}),
            ),
            (
                "Leaf(v) == TRUE\nEffect(v) == TLCSet(0,v)\nInit == (x=0 /\\ Leaf(42)) \\/ (x=1)\nNext == UNCHANGED x",
                "x",
                "CONSTANT Leaf <- Effect",
                json!({"x":0}),
            ),
        ] {
            let model = session(body, variables, extra, tlc)?;
            assert!(
                matches!(
                    model.try_read(&behavior(word)?)?.0.validation,
                    Validation::Unsupported(_)
                ),
                "{body}/{tlc}"
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
        if self.calls.fetch_add(1, Ordering::Relaxed) >= self.limit {
            Err(Interrupted::new(490))
        } else {
            Ok(())
        }
    }
}

#[test]
fn ordered_initial_enumeration_keeps_typed_interruption() -> TestResult {
    for tlc in [false, true] {
        let model = session("Init == x \\in 1..10000\nNext == UNCHANGED x", "x", "", tlc)?;
        let doc = behavior(json!({"x":10000}))?;
        let control = Arc::new(StopAfter {
            calls: AtomicUsize::new(0),
            limit: usize::MAX,
        });
        assert_eq!(
            model.read_controlled(&doc, control.clone())?.0.validation,
            Validation::Valid
        );
        let calls = control.calls.load(Ordering::Relaxed);
        assert!(calls > 10000);
        let stop = Arc::new(StopAfter {
            calls: AtomicUsize::new(0),
            limit: calls / 2,
        });
        assert!(matches!(model.read_controlled(&doc, stop),
            Err(ValidationRunError::Interrupted { error, progress })
                if error.interruption().is_some_and(|error| error.token() == 490)
                    && progress.check == Some(Check::Init) && progress.state == Some(0)));
    }
    Ok(())
}
