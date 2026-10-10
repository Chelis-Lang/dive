//! A witness must not establish the string encounter order that proves itself.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_model::Model;
use dive_trace::{Ending, Trace, TraceDoc, Validation, validate, validate_doc};
use serde_json::json;
use std::sync::Arc;

const SPEC: &str = include_str!("fixtures/runtime-names/ReplayNames.tla");
const CFG: &str = include_str!("fixtures/runtime-names/ReplayNames.cfg");

fn model(tlc: bool, source: &str) -> Model {
    let a = dive_sema::analyze_source("ReplayNames.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("lower"));
    let cfg = dive_config::parse(CFG).into_config().expect("cfg");
    dive_model::bind(
        ir,
        &cfg,
        &if tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::dive()
        },
    )
    .expect("bind")
}

#[test]
fn forged_lossless_and_lossy_witnesses_cannot_prove_their_own_namespace() {
    let doc = TraceDoc::parse(include_str!("fixtures/runtime-names/bad.json")).expect("doc");
    let m = model(true, SPEC);
    assert!(m.runtime_name_order);
    let trace = Trace::decode(&m, &doc).expect("decode");
    let refused = Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned());
    assert_eq!(validate(&m, &trace, None).validation, refused);
    assert_eq!(validate_doc(&m, &doc, &[], None).0.validation, refused);
    let lossy =
        TraceDoc::from_json(&json!({"vars":["x","s"],"counterexample":{"action":[],"state":[[1,{"x":6,"s":"unset"}],[2,{"x":6,"s":"7"}]]}}))
            .expect("TLC document")
            .with_ending(Ending::Invariant("Inv".to_owned()));
    assert_eq!(validate_doc(&m, &lossy, &[], None).0.validation, refused);
}

#[test]
fn canonical_validation_still_distinguishes_actual_and_forged_steps() {
    let m = model(false, SPEC);
    assert!(!m.runtime_name_order);
    let actual = TraceDoc::behavior(vec![json!({"x":6,"s":"unset"}), json!({"x":6,"s":"6"})])
        .expect("actual");
    assert_eq!(
        validate_doc(&m, &actual, &[], None).0.validation,
        Validation::Valid
    );
    let forged = TraceDoc::parse(include_str!("fixtures/runtime-names/bad.json")).expect("forged");
    assert!(matches!(
        validate_doc(&m, &forged, &[], None).0.validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn literal_names_remain_supported_in_tlc_order() {
    let source = SPEC
        .replace("ToString(x), ToString(x+1)", "\"6\", \"7\"")
        .replace("s = ToString(x)", "s = \"6\"");
    let m = model(true, &source);
    assert!(!m.runtime_name_order);
    let actual = TraceDoc::behavior(vec![json!({"x":6,"s":"unset"}), json!({"x":6,"s":"6"})])
        .expect("actual");
    assert_eq!(
        validate_doc(&m, &actual, &[], None).0.validation,
        Validation::Valid
    );
}
