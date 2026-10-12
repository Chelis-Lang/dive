//! Source-owned range facts must not be inferred from supplied frame values.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_model::Model;
use dive_trace::{Ending, Trace, TraceDoc, TraceEncoding, Validation, validate_doc};
use serde_json::json;
use std::sync::Arc;

fn model(init: &str, next: &str, definitions: &str, compat: bool) -> Model {
    let initial = format!("x=0 /\\ s = {init}");
    model_with_init(&initial, next, definitions, compat)
}

fn model_with_init(init: &str, next: &str, definitions: &str, compat: bool) -> Model {
    let source = format!(
        "---- MODULE IndexedStack ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\n{definitions}\nInit == {init}\nNext == {next}\nInv == x<2\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("IndexedStack.tla", &source, &dive_sema::Options::default());
    assert!(
        analysis.accepted(),
        "{:?}; {:?}; source={source}",
        analysis.diagnostics(),
        analysis.syntax_diagnostics()
    );
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("lower indexed stack")),
        &dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n")
            .into_config()
            .expect("config"),
        &if compat {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::default()
        },
    )
    .expect("bind indexed stack")
}

fn word() -> TraceDoc {
    TraceDoc::behavior(vec![
        json!({"x":0,"s":{"fcn":[[1,{"seq":[0,1]}],[2,{"seq":[0,1]}]]}}),
        json!({"x":1,"s":{"fcn":[[1,{"seq":[1]}],[2,{"seq":[0,1]}]]}}),
        json!({"x":2,"s":{"fcn":[[1,{"seq":[1]}],[2,{"seq":[4,0,1]}]]}}),
    ])
    .expect("stored owner stacks")
    .with_ending(Ending::Invariant("Inv".to_owned()))
}

const NEXT: &str = "(x=0 /\\ x'=1 /\\ s'=[s EXCEPT ![1]=Tail(s[1])]) \\/ (x=1 /\\ x'=2 /\\ s'=[s EXCEPT ![2]= <<4>> \\o s[2]]) \\/ (x>1 /\\ UNCHANGED <<x,s>>)";

#[test]
fn tuple_ranges_preserve_actual_indexed_next_and_ending() {
    for compat in [true, false] {
        let model = model("[i \\in {1,2} |-> <<0,1>>]", NEXT, "", compat);
        let original = word();
        let (report, decoded) = validate_doc(&model, &original, &[], None);
        assert_eq!(report.validation, Validation::Valid, "{report:?}");
        let trace = decoded.expect("decoded original");
        assert_eq!(
            dive_trace::validate(&model, &trace, None).validation,
            Validation::Valid
        );
        let v2 = trace
            .to_doc_with_encoding(&model, TraceEncoding::RepresentationV2)
            .expect("representation-preserving range word");
        let transported = TraceDoc::from_json(&v2.to_json()).expect("v2 transport");
        let (report, decoded_v2) = validate_doc(&model, &transported, &[], None);
        assert_eq!(report.validation, Validation::Valid);
        assert_eq!(
            trace.states,
            decoded_v2.expect("read exact range word").states
        );
        assert_eq!(
            Trace::decode(&model, &transported)
                .expect("direct v2 decode")
                .states,
            trace.states
        );
        assert_eq!(model.take_output(), Vec::<String>::new());
        let mut changed = original.to_json();
        changed["states"][1]["values"]["s"]["fcn"][1][1]["seq"] = json!([1]);
        let changed = TraceDoc::from_json(&changed).expect("wrong other-owner stack");
        assert!(matches!(
            validate_doc(&model, &changed, &[], None).0.validation,
            Validation::Invalid(_)
        ));
        let altered = original
            .clone()
            .with_ending(Ending::Invariant("MissingProperty".to_owned()));
        assert!(matches!(
            validate_doc(&model, &altered, &[], None).0.validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn finite_quantified_and_scoped_range_sources_preserve_original_word() {
    for (definitions, next) in [
        (
            "Owners == {1,2}\nRead(f,k) == f[k]",
            NEXT.replace("s[1]", "Read(s,1)")
                .replace("s[2]", "Read(s,2)"),
        ),
        (
            "Step(k) == (x=0 /\\ x'=1 /\\ s'=[s EXCEPT ![k]=Tail(s[k])])",
            NEXT.replace(
                "(x=0 /\\ x'=1 /\\ s'=[s EXCEPT ![1]=Tail(s[1])])",
                "(\\E owner \\in {1}: Step(owner))",
            ),
        ),
        (
            "",
            NEXT.replace("Tail(s[1])", "LET Dropped == Tail(s[1]) IN Dropped"),
        ),
    ] {
        let model = model("[i \\in {1,2} |-> <<0,1>>]", &next, definitions, true);
        assert_eq!(
            validate_doc(&model, &word(), &[], None).0.validation,
            Validation::Valid,
            "{next}"
        );
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}

#[test]
fn unvisited_disjuncts_cannot_borrow_range_facts_from_the_word() {
    for bad in [
        "s'=[s EXCEPT ![1]=\"text\"]",
        "s'=[s EXCEPT ![1]=IF x=0 THEN <<0>> ELSE \"text\"]",
        "s'=s'",
        "TRUE",
        "s'=[t EXCEPT ![1]= <<0>>]",
    ] {
        let declarations = if bad.contains("[t ") {
            "t == [i \\in {1,2} |-> \"text\"]"
        } else {
            ""
        };
        let next = format!("{NEXT} \\/ (x=99 /\\ x'=100 /\\ {bad})");
        let strict = model("[i \\in {1,2} |-> <<0,1>>]", &next, declarations, true);
        assert_eq!(
            validate_doc(&strict, &word(), &[], None).0.validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()),
            "{bad}"
        );
        assert_eq!(strict.take_output(), Vec::<String>::new());
        let canonical = model("[i \\in {1,2} |-> <<0,1>>]", &next, declarations, false);
        assert_eq!(
            validate_doc(&canonical, &word(), &[], None).0.validation,
            Validation::Valid,
            "{bad}"
        );
    }
}

#[test]
fn supplied_tuple_values_cannot_discharge_string_init_ranges() {
    for init in [
        "[i \\in {1,2} |-> \"text\"]",
        "[i \\in {1,2} |-> IF i=1 THEN <<0,1>> ELSE \"text\"]",
    ] {
        let strict = model(init, NEXT, "", true);
        assert_eq!(
            validate_doc(&strict, &word(), &[], None).0.validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
        assert_eq!(strict.take_output(), Vec::<String>::new());
    }
}

#[test]
fn range_facts_do_not_hide_effects_or_unknown_domains() {
    for (init, next) in [
        ("[i \\in Nat |-> <<0,1>>]", NEXT.to_owned()),
        (
            "[i \\in {1,2} |-> <<0,1>>]",
            format!("PrintT(\"\") /\\ ({NEXT})"),
        ),
    ] {
        let strict = model(init, &next, "", true);
        assert_eq!(
            validate_doc(&strict, &word(), &[], None).0.validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
        assert_eq!(strict.take_output(), Vec::<String>::new());
    }
}

#[test]
fn every_initial_disjunct_is_required_and_word_init_is_independently_checked() {
    let good = "x=0 /\\ s = [i \\in {1,2} |-> <<0,1>>]";
    for bad in ["x=99 /\\ s = [i \\in {1,2} |-> \"text\"]", "x=99 /\\ TRUE"] {
        let initial = format!("({good}) \\/ ({bad})");
        let strict = model_with_init(&initial, NEXT, "", true);
        assert_eq!(
            validate_doc(&strict, &word(), &[], None).0.validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
        assert_eq!(strict.take_output(), Vec::<String>::new());
    }
    let strict = model_with_init(good, NEXT, "", true);
    let mut changed = word().to_json();
    changed["states"][0]["values"]["s"]["fcn"][0][1]["seq"] = json!([1]);
    let changed = TraceDoc::from_json(&changed).expect("forged initial frame");
    assert!(matches!(
        validate_doc(&strict, &changed, &[], None).0.validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn arbitrary_state_public_predicates_do_not_inherit_whole_word_range_facts() {
    let strict = model("[i \\in {1,2} |-> <<0,1>>]", NEXT, "", true);
    let trace = Trace::decode(&strict, &word()).expect("concrete supplied range");
    assert_eq!(
        validate_doc(&strict, &word(), &[], None).0.validation,
        Validation::Valid
    );
    assert!(matches!(
        dive_trace::exists::exists_successor(
            &strict,
            strict.next.as_ref().expect("Next"),
            &trace.states[0],
            None,
            dive_trace::exists::Limits::default()
        ),
        Err(dive_trace::exists::ExistsError::Unsupported(_))
    ));
    assert_eq!(strict.take_output(), Vec::<String>::new());
}

#[test]
fn nested_updates_opaque_functions_and_conditional_projection_do_not_prove_a_range() {
    for bad in [
        "s'=[s EXCEPT ![1][1]=0]",
        "s'=IF x=99 THEN [i \\in {1,2} |-> \"text\"] ELSE s",
        "UNCHANGED IF x=99 THEN <<x,s>> ELSE <<s,x>>",
        "(\\A owner \\in {}: s'=s)",
    ] {
        let next = format!("{NEXT} \\/ (x=99 /\\ {bad})");
        let strict = model("[i \\in {1,2} |-> <<0,1>>]", &next, "", true);
        assert_eq!(
            validate_doc(&strict, &word(), &[], None).0.validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()),
            "{bad}"
        );
        assert_eq!(strict.take_output(), Vec::<String>::new());
    }
}
