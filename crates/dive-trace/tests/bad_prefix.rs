//! `bad-prefix` endings: a finite trace that no infinite extension can turn
//! into a behavior satisfying the named temporal property (ADR 0162 §4.3).
//! The validator checks every state and step as for an invariant ending,
//! then decides the property in three-valued logic on the prefix.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_model::Model;
use dive_trace::{Check, Ending, TraceDoc, Validation, validate_doc};
use serde_json::json;

const SPEC: &str = "---- MODULE Grow ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
    Next == x' = x + 1\nSpec == Init /\\ [][Next]_x\nFairSpec == Spec /\\ WF_x(Next)\n\
    StartsBounded == (x = 0) => [](x < 3)\n\
    EitherBound == [](x < 2) \\/ [](x # 1)\n\
    NestedBox == [](x = 1 => [](x < 3))\n\
    StepDouble == x = 0 => [][x' = x + 1 /\\ x < 2]_x\n\
    Eventually == <>(x = 9)\n\
    Mixed == <>(x = 9) /\\ [](x < 3)\n\
    LeadsBack == (x = 2) ~> (x = 0)\n\
    Bounded == []<>(x = 0) \\/ [](x < 3)\n====\n";

fn bound(cfg: &str) -> Model {
    let a = dive_sema::analyze_source("Grow.tla", SPEC, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("lower"));
    let cfg = dive_config::parse(cfg).into_config().expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::dive()).expect("bind")
}

fn prefix(n: i64, name: &str) -> TraceDoc {
    TraceDoc::behavior((0..n).map(|x| json!({ "x": x })).collect())
        .expect("word")
        .with_ending(Ending::BadPrefix(name.into()))
}

fn result(property: &str, n: i64) -> Validation {
    let model = bound(&format!("SPECIFICATION FairSpec\nPROPERTY {property}\n"));
    validate_doc(&model, &prefix(n, property), &[]).0.validation
}

#[test]
fn refuting_prefixes_validate() {
    for (property, n) in [("StartsBounded", 4), ("EitherBound", 3), ("NestedBox", 4)] {
        assert_eq!(result(property, n), Validation::Valid, "{property} on {n}");
    }
}

#[test]
fn a_prefix_some_extension_may_satisfy_is_invalid() {
    for (property, n) in [
        ("StartsBounded", 3),
        ("EitherBound", 2),
        ("Eventually", 5),
        ("LeadsBack", 5),
        ("Bounded", 4),
    ] {
        assert!(
            matches!(result(property, n), Validation::Invalid(f) if f.check == Check::Temporal),
            "{property} on {n}: {:?}",
            result(property, n)
        );
    }
}

/// The step out of the last state is unknown, so an action property that
/// could fail only on it is not refuted; the step out of `x = 2` fails it.
#[test]
fn the_last_step_is_unknown() {
    assert!(matches!(
        result("StepDouble", 2),
        Validation::Invalid(f) if f.check == Check::Temporal
    ));
    assert_eq!(result("StepDouble", 4), Validation::Valid);
}

/// `[](x < 3)` of `Mixed` is an invariant part, checked on every state as
/// for an invariant ending, before the claim's prefix truth.
#[test]
fn safety_parts_are_checked_first() {
    assert!(matches!(
        result("Mixed", 4),
        Validation::Invalid(f) if f.check == Check::Invariant
    ));
}

#[test]
fn every_step_must_be_a_behavior_step() {
    let model = bound("SPECIFICATION FairSpec\nPROPERTY StartsBounded\n");
    let doc = TraceDoc::behavior(vec![json!({"x": 0}), json!({"x": 2}), json!({"x": 3})])
        .expect("word")
        .with_ending(Ending::BadPrefix("StartsBounded".into()));
    assert!(matches!(
        validate_doc(&model, &doc, &[]).0.validation,
        Validation::Invalid(f) if f.check == Check::Next
    ));
}

#[test]
fn the_name_must_be_a_configured_property() {
    let model = bound("SPECIFICATION FairSpec\nPROPERTY StartsBounded\n");
    assert!(matches!(
        validate_doc(&model, &prefix(4, "Mixed"), &[]).0.validation,
        Validation::Invalid(f) if f.check == Check::Ending
    ));
}

#[derive(Debug)]
struct Continue;
impl dive_eval::HostControl for Continue {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        Ok(())
    }
}

/// A fresh controlled session reads a bad-prefix word as a finite word.
#[test]
fn a_controlled_session_validates_bad_prefixes() {
    let analysis = dive_sema::analyze_source("Grow.tla", SPEC, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let config = dive_config::parse("SPECIFICATION FairSpec\nPROPERTY StartsBounded\n")
        .into_config()
        .expect("config");
    let session = dive_trace::Session::new(
        dive_model::Recipe::new(ir, config, dive_eval::Options::dive(), false),
        None,
    );
    let control: Arc<dyn dive_eval::HostControl> = Arc::new(Continue);
    let read = |n| {
        session
            .read_controlled(&prefix(n, "StartsBounded"), Arc::clone(&control))
            .expect("read")
            .0
            .validation
    };
    assert_eq!(read(4), Validation::Valid);
    assert!(matches!(read(3), Validation::Invalid(f) if f.check == Check::Temporal));
}
