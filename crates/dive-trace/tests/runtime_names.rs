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

const ENTRY_SPEC: &str = r"---- MODULE EntryNames ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = x
A == x' = x /\ ((CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE) = ToString(x))
P == []((CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE) = ToString(x))
B == x' = x
Q == []TRUE
====";

fn entry_model(source: &str, cfg: &str, tlc: bool) -> Model {
    let analysis =
        dive_sema::analyze_source("EntryNames.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower entry predicate"));
    let cfg = dive_config::parse(cfg).into_config().expect("entry config");
    dive_model::bind(
        ir,
        &cfg,
        &if tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::dive()
        },
    )
    .expect("bind entry predicate")
}

fn predicate(m: &Model, name: &str) -> dive_model::Pred {
    let id = m.ir.root_def(name).expect("predicate definition");
    dive_model::Pred::new(m.ir.def(id).body, m.evaluator.scope(id))
}

fn entry_state() -> dive_trace::State {
    Arc::from([Some(dive_value::Value::Int(13))])
}

#[test]
fn predicate_apis_refuse_encounter_sensitive_names_with_or_without_config_roles() {
    for configured in [false, true] {
        let cfg = if configured {
            "INIT Init\nNEXT A\nPROPERTY P\n"
        } else {
            "INIT Init\nNEXT Next\n"
        };
        for previous_encounter in [false, true] {
            let temporal = entry_model(ENTRY_SPEC, cfg, true);
            assert_eq!(temporal.runtime_name_order, configured);
            if previous_encounter {
                temporal.names.intern("14");
            }
            let result = dive_trace::temporal::evaluate_lasso(
                &temporal,
                &predicate(&temporal, "P"),
                &[entry_state()],
                0,
            );
            assert_eq!(
                result,
                Err(dive_trace::temporal::TemporalError::Unsupported(
                    dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()
                ))
            );
            let action = entry_model(ENTRY_SPEC, cfg, true);
            if previous_encounter {
                action.names.intern("14");
            }
            assert!(matches!(
                dive_trace::exists::exists_successor(
                    &action, &predicate(&action, "A"), &entry_state(), None, dive_trace::exists::Limits::default()
                ),
                Err(dive_trace::exists::ExistsError::Unsupported(reason))
                    if reason == dive_trace::RUNTIME_NAMES_UNSUPPORTED
            ));
            // Admission is predicate-scoped, not a blanket rejection of the model.
            assert_eq!(
                dive_trace::temporal::evaluate_lasso(
                    &temporal,
                    &predicate(&temporal, "Q"),
                    &[entry_state()],
                    0
                ),
                Ok(vec![true])
            );
            assert!(matches!(
                dive_trace::exists::exists_successor(
                    &action,
                    &predicate(&action, "B"),
                    &entry_state(),
                    None,
                    dive_trace::exists::Limits::default()
                ),
                Ok(dive_trace::exists::Decision::Enabled { .. })
            ));
        }
    }
}

#[test]
fn canonical_predicate_answers_do_not_depend_on_prior_name_encounters() {
    for prior in [false, true] {
        let model = entry_model(ENTRY_SPEC, "INIT Init\nNEXT A\nPROPERTY P\n", false);
        if prior {
            model.names.intern("14");
        }
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(
                &model,
                &predicate(&model, "P"),
                &[entry_state()],
                0
            ),
            Ok(vec![true])
        );
        assert!(matches!(
            dive_trace::exists::exists_successor(
                &model,
                &predicate(&model, "A"),
                &entry_state(),
                None,
                dive_trace::exists::Limits::default()
            ),
            Ok(dive_trace::exists::Decision::Enabled { .. })
        ));
    }
}

#[test]
fn native_aliases_and_nested_definitions_receive_the_same_scoped_refusal() {
    let nested = ENTRY_SPEC
        .replace("ToString(x)", "Outer")
        .replace("A ==", "Inner == ToString(x)\nOuter == Inner\nA ==");
    let redirected = ENTRY_SPEC
        .replace("ToString", "Alias")
        .replace("A ==", "Alias(v) == v\nA ==");
    let constant = ENTRY_SPEC
        .replace("VARIABLE x", "CONSTANT C\nVARIABLE x")
        .replace("ToString(x)", "C")
        .replace("A ==", "Render == ToString(13)\nA ==");
    for (source, cfg) in [
        (nested.as_str(), "INIT Init\nNEXT Next\n"),
        (
            redirected.as_str(),
            "INIT Init\nNEXT Next\nCONSTANT Alias <- ToString\n",
        ),
        (
            constant.as_str(),
            "INIT Init\nNEXT Next\nCONSTANT C <- Render\n",
        ),
    ] {
        let model = entry_model(source, cfg, true);
        assert!(matches!(
            dive_trace::temporal::evaluate_lasso(
                &model, &predicate(&model, "P"), &[entry_state()], 0
            ),
            Err(dive_trace::temporal::TemporalError::Unsupported(reason))
                if reason == dive_trace::RUNTIME_NAMES_UNSUPPORTED
        ));
        assert!(matches!(
            dive_trace::exists::exists_successor(
                &model, &predicate(&model, "A"), &entry_state(), None, dive_trace::exists::Limits::default()
            ),
            Err(dive_trace::exists::ExistsError::Unsupported(reason))
                if reason == dive_trace::RUNTIME_NAMES_UNSUPPORTED
        ));
    }
}

fn sequence_entry(expression: &str, extra: &str, config: &str) -> Model {
    let source = format!(
        "---- MODULE EntryNames ----\nEXTENDS Naturals, TLC, Sequences\nVARIABLE x\nInit == x=0\nNext == x'=x\n{extra}\nA == x'=x /\\ Len({expression})=1\nP == [](Len({expression})=1)\n====\n"
    );
    entry_model(&source, &format!("INIT Init\nNEXT Next\n{config}"), true)
}

#[test]
fn scoped_tuple_only_sequence_calls_remain_independently_decidable() {
    for (expression, extra, cfg) in [
        ("Tail(<<1,2>>)", "", ""),
        ("SubSeq(IF x=13 THEN <<1,2>> ELSE <<3,4>>,1,1)", "", ""),
        ("<<1>> \\o <<>>", "", ""),
        (
            "Alias(T)",
            "Alias(v)==v\nT == <<1,2>>",
            "CONSTANT Alias <- Tail\n",
        ),
        (
            "Tail(C)",
            "CONSTANT C\nPair == <<1,2>>",
            "CONSTANT C <- Pair\n",
        ),
        ("Tail(LET T == <<1,2>> IN T)", "", ""),
    ] {
        let model = sequence_entry(expression, extra, cfg);
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(
                &model,
                &predicate(&model, "P"),
                &[entry_state()],
                0
            ),
            Ok(vec![true]),
            "{expression}"
        );
        assert!(
            matches!(
                dive_trace::exists::exists_successor(
                    &model,
                    &predicate(&model, "A"),
                    &entry_state(),
                    None,
                    dive_trace::exists::Limits::default()
                ),
                Ok(dive_trace::exists::Decision::Enabled { .. })
            ),
            "{expression}"
        );
    }
}

#[test]
fn string_mixed_opaque_and_effectful_sequence_inputs_stay_refused() {
    for (expression, extra) in [
        ("Tail(\"ab\")", ""),
        ("Tail(IF x=13 THEN <<1,2>> ELSE \"ab\")", ""),
        ("Tail(<<ToString(x),ToString(x+1)>>)", ""),
        ("Tail(PrintT(<<1,2>>))", ""),
        ("Tail(Identity(<<1,2>>))", "Identity(v)==v"),
    ] {
        let model = sequence_entry(expression, extra, "");
        assert!(
            matches!(
                dive_trace::temporal::evaluate_lasso(
                    &model,
                    &predicate(&model, "P"),
                    &[entry_state()],
                    0
                ),
                Err(dive_trace::temporal::TemporalError::Unsupported(_))
            ),
            "{expression}"
        );
        assert!(
            matches!(
                dive_trace::exists::exists_successor(
                    &model,
                    &predicate(&model, "A"),
                    &entry_state(),
                    None,
                    dive_trace::exists::Limits::default()
                ),
                Err(dive_trace::exists::ExistsError::Unsupported(_))
            ),
            "{expression}"
        );
    }
}
