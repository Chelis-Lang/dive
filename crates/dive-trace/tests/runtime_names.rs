//! A witness must not establish the string encounter order that proves itself.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_model::Model;
use dive_trace::{Ending, Trace, TraceDoc, Validation, validate, validate_doc};
use serde_json::json;
use std::sync::Arc;

const SPEC: &str = include_str!("fixtures/runtime-names/ReplayNames.tla");
const CFG: &str = include_str!("fixtures/runtime-names/ReplayNames.cfg");

fn tuple_replay_source(definitions: &str, expression: &str) -> String {
    format!(
        "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{definitions}\nInit == x=0\nNext == x'=IF Len({expression})=1 THEN 1 ELSE 2\nInv == x=0\n====\n"
    )
}

fn tuple_replay_model(source: &str, extra_config: &str) -> Model {
    let analysis =
        dive_sema::analyze_source("TupleReplay.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower tuple replay"));
    let cfg = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\nINVARIANT Inv\n{extra_config}"
    ))
    .into_config()
    .expect("tuple config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("tuple binding")
}

fn tuple_replay_doc(value: i64, lossy: bool) -> TraceDoc {
    if lossy {
        TraceDoc::from_json(&json!({
            "vars":["x"],"counterexample":{"action":[],"state":[[1,{"x":0}],[2,{"x":value}]]}
        }))
        .expect("lossy tuple word")
        .with_ending(Ending::Invariant("Inv".to_owned()))
    } else {
        TraceDoc::behavior(vec![json!({"x":0}), json!({"x":value})])
            .expect("tuple behavior")
            .with_ending(Ending::Invariant("Inv".to_owned()))
    }
}

#[test]
fn strict_whole_words_admit_proved_tuple_calls_and_reject_forged_steps() {
    for (definitions, expression, actual) in [
        ("", "Tail(<<0,1>>)", 1),
        ("Pair == <<0,1>>", "Tail(Pair)", 1),
        ("", "Tail(IF x=0 THEN <<0,1>> ELSE <<2,3>>)", 1),
        ("", "<<0>> \\o <<1>>", 2),
        ("", "SubSeq(<<0,1>>,2,2)", 1),
        ("Drop(v) == Tail(v)", "Drop(<<0,1>>)", 1),
        ("", "LET Pair == <<0,1>> IN Tail(Pair)", 1),
    ] {
        let source = tuple_replay_source(definitions, expression);
        for lossy in [false, true] {
            let model = tuple_replay_model(&source, "");
            // The checker retains its existing conservative sequential policy.
            assert!(model.runtime_name_order, "{expression}");
            let actual_word = tuple_replay_doc(actual, lossy);
            assert_eq!(
                validate_doc(&model, &actual_word, &[], None).0.validation,
                Validation::Valid,
                "{expression}; lossy={lossy}"
            );
            // Bind anew: the actual word cannot seed a forged word's namespace.
            let fresh = tuple_replay_model(&source, "");
            let forged = tuple_replay_doc(if actual == 1 { 2 } else { 1 }, lossy);
            assert!(matches!(
                validate_doc(&fresh, &forged, &[], None).0.validation,
                Validation::Invalid(_)
            ));
        }
    }
}

#[test]
fn strict_tuple_admission_keeps_string_branches_elements_and_assumptions_refused() {
    for (definitions, expression) in [
        ("", "Tail(\"ab\")"),
        ("", "Tail(IF x=0 THEN <<0,1>> ELSE \"ab\")"),
        ("", "Tail(<<0,ToString(x)>>)"),
        ("ASSUME ToString(0)=\"0\"", "Tail(<<0,1>>)"),
        (
            "Unknown == IF x=0 THEN <<0,1>> ELSE \"ab\"",
            "Tail(Unknown)",
        ),
    ] {
        let source = tuple_replay_source(definitions, expression);
        for lossy in [false, true] {
            let model = tuple_replay_model(&source, "");
            assert!(model.runtime_name_order);
            assert_eq!(
                validate_doc(&model, &tuple_replay_doc(1, lossy), &[], None)
                    .0
                    .validation,
                Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()),
                "{definitions}; {expression}; lossy={lossy}"
            );
        }
    }
}

#[test]
fn configured_sequence_aliases_require_the_same_proof() {
    for (expression, admitted) in [("Alias(<<0,1>>)", true), ("Alias(\"ab\")", false)] {
        let source = tuple_replay_source("Alias(v) == v", expression);
        let model = tuple_replay_model(&source, "CONSTANT Alias <- Tail\n");
        assert!(model.runtime_name_order);
        let actual = validate_doc(&model, &tuple_replay_doc(1, false), &[], None)
            .0
            .validation;
        assert_eq!(
            actual,
            if admitted {
                Validation::Valid
            } else {
                Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
            }
        );
    }
}

#[test]
fn tuple_guard_does_not_trust_pure_native_string_constructors() {
    // Json is classified as pure and is not in the old runtime-name marker.
    // The tuple call makes this model require the new complete admission walk.
    let source = tuple_replay_source("ASSUME ToJson(0)=\"0\"", "Tail(<<0,1>>)")
        .replace("Sequences, TLC", "Sequences, TLC, Json");
    for lossy in [false, true] {
        let model = tuple_replay_model(&source, "");
        assert!(model.runtime_name_order);
        assert_eq!(
            validate_doc(&model, &tuple_replay_doc(1, lossy), &[], None)
                .0
                .validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
    }
}

#[test]
fn inductive_tuple_history_admits_actual_steps_but_never_uses_witness_shape_as_proof() {
    for (init, next, admits) in [
        ("<<0,1>>", "x'=1 /\\ s'=Tail(s)", true),
        (
            "<<0,1>>",
            "x'=1 /\\ s'=IF x=0 THEN Tail(s) ELSE <<2>>",
            true,
        ),
        (
            "<<0,1>>",
            "(x=0 /\\ x'=1 /\\ s'=Tail(s)) \\/ (x#0 /\\ x'=x /\\ UNCHANGED s)",
            true,
        ),
        (
            "<<0,1>>",
            "x'=1 /\\ s'=IF x=0 THEN Tail(s) ELSE \"b\"",
            false,
        ),
        (
            "<<0,1>>",
            "x'=1 /\\ s'=IF x=0 THEN Tail(s) ELSE <<ToString(x)>>",
            false,
        ),
        ("\"ab\"", "x'=1 /\\ s'=Tail(s)", false),
        ("<<0,1>>", "x'=1 /\\ Len(Tail(s))=1", false),
    ] {
        let source = format!(
            "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\nInit == x=0 /\\ s = {init}\nNext == {next}\nInv == x=0\n====\n"
        );
        for final_x in [1, 99] {
            let model = tuple_replay_model(&source, "");
            let doc = TraceDoc::behavior(vec![
                json!({"x":0,"s":{"seq":[0,1]}}),
                json!({"x":final_x,"s":{"seq":[1]}}),
            ])
            .expect("tuple stack word")
            .with_ending(Ending::Invariant("Inv".to_owned()));
            let result = validate_doc(&model, &doc, &[], None).0.validation;
            if admits && final_x == 1 {
                assert_eq!(result, Validation::Valid, "{next}");
            } else if admits {
                assert!(matches!(result, Validation::Invalid(_)), "{result:?}");
            } else {
                assert_eq!(
                    result,
                    Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()),
                    "{next}"
                );
            }
        }
    }
}

#[test]
fn inductive_history_requires_every_initial_branch_and_retains_public_api_refusal() {
    for (definitions, init, next, admitted) in [
        (
            "Drop(v) == Tail(v)",
            "x=0 /\\ s = <<0,1>>",
            "x'=1 /\\ s'=Drop(s)",
            true,
        ),
        (
            "Drop(v) == LET Pair == v IN Tail(Pair)",
            "x=0 /\\ s = <<0,1>>",
            "x'=1 /\\ s'=Drop(s)",
            true,
        ),
        (
            "",
            "(x=0 /\\ s = <<0,1>>) \\/ (x=2 /\\ s = \"ab\")",
            "x'=1 /\\ s'=Tail(s)",
            false,
        ),
        (
            "",
            "x=0 /\\ s = <<0,1>>",
            "(x=0 /\\ x'=1 /\\ s'=Tail(s)) \\/ (x#0 /\\ x'=1)",
            false,
        ),
        (
            "",
            "x=0 /\\ s = <<0,1>>",
            "x'=1 /\\ s'=(IF x=1 THEN s ELSE <<0>>)' /\\ Len(Tail(s))=1",
            false,
        ),
    ] {
        let source = format!(
            "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\n{definitions}\nInit == {init}\nNext == {next}\nInv == x=0\n====\n"
        );
        let model = tuple_replay_model(&source, "");
        let doc = TraceDoc::behavior(vec![
            json!({"x":0,"s":{"seq":[0,1]}}),
            json!({"x":1,"s":{"seq":[1]}}),
        ])
        .expect("stack history")
        .with_ending(Ending::Invariant("Inv".into()));
        let result = validate_doc(&model, &doc, &[], None).0.validation;
        if admitted {
            assert_eq!(result, Validation::Valid);
        } else {
            assert!(matches!(result, Validation::Unsupported(_)), "{result:?}");
        }
        let trace = Trace::decode(&model, &doc).expect("decode stack");
        // This API is allowed to receive any state, without a validated Init
        // or predecessor chain. A model invariant cannot license that call.
        assert!(matches!(
            dive_trace::exists::exists_successor(
                &model,
                model.next.as_ref().expect("next"),
                &trace.states[0],
                None,
                dive_trace::exists::Limits::default()
            ),
            Err(dive_trace::exists::ExistsError::Unsupported(_))
        ));
    }
}

#[test]
fn tuple_hypotheses_are_jointly_inductive_and_failed_dependencies_are_removed() {
    for (next_t, admitted) in [("s", true), ("IF x=0 THEN s ELSE \"ab\"", false)] {
        let source = format!(
            "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences\nVARIABLES x,s,t\nInit == x=0 /\\ s = <<0,1>> /\\ t = <<2,3>>\nNext == x'=1 /\\ s'=Tail(t) /\\ t'={next_t}\nInv == x=0\n====\n"
        );
        let model = tuple_replay_model(&source, "");
        let doc = TraceDoc::behavior(vec![
            json!({"x":0,"s":{"seq":[0,1]},"t":{"seq":[2,3]}}),
            json!({"x":1,"s":{"seq":[3]},"t":{"seq":[0,1]}}),
        ])
        .expect("joint history")
        .with_ending(Ending::Invariant("Inv".into()));
        let result = validate_doc(&model, &doc, &[], None).0.validation;
        if admitted {
            assert_eq!(result, Validation::Valid);
        } else {
            assert_eq!(
                result,
                Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
            );
        }
    }
}

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
    dive_model::Pred::new(
        m.ir.def(id).body,
        m.evaluator.scope(id).expect("valid scope"),
    )
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

#[test]
fn configured_operator_constants_follow_authoritative_native_tuple_bindings() {
    for (init, target, admitted) in [
        ("<<0,1>>", "Tail", true),
        ("\"ab\"", "Tail", false),
        ("<<0,1>>", "ToString", false),
    ] {
        let source = format!(
            "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nCONSTANT Drop(_)\nVARIABLES x,s\nInit == x=0 /\\ s = {init}\nNext == x'=1 /\\ s'=Drop(s)\nInv == x=0\n====\n"
        );
        for final_x in [1, 99] {
            let model = tuple_replay_model(&source, &format!("CONSTANT Drop <- {target}\n"));
            let word = TraceDoc::behavior(vec![
                json!({"x":0,"s":{"seq":[0,1]}}),
                json!({"x":final_x,"s":{"seq":[1]}}),
            ])
            .expect("configured native word")
            .with_ending(Ending::Invariant("Inv".to_owned()));
            let actual = validate_doc(&model, &word, &[], None).0.validation;
            if admitted && final_x == 1 {
                assert_eq!(actual, Validation::Valid);
            } else if admitted {
                assert!(matches!(actual, Validation::Invalid(_)), "{actual:?}");
            } else {
                assert_eq!(
                    actual,
                    Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
                );
            }
        }
    }
}

#[test]
fn inductive_shape_admission_does_not_make_forged_initial_or_successor_values_valid() {
    // Necessary Init facts justify accepted histories. They do not promise
    // that no native call can run while a malformed input is being rejected.
    let source = "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\nInit == Len(Tail(s))=1 /\\ x=0 /\\ s = <<0,1>>\nNext == x'=1 /\\ s'=Tail(s)\nInv == x=0\n====\n";
    for (initial, successor, admitted) in [
        (json!({"seq":[0,1]}), json!({"seq":[1]}), true),
        (json!("ab"), json!({"seq":[1]}), false),
        (json!({"seq":[0,1]}), json!("b"), false),
    ] {
        let model = tuple_replay_model(source, "");
        let word = TraceDoc::behavior(vec![
            json!({"x":0,"s":initial}),
            json!({"x":1,"s":successor}),
        ])
        .expect("adversarial concrete input")
        .with_ending(Ending::Invariant("Inv".to_owned()));
        let actual = validate_doc(&model, &word, &[], None).0.validation;
        if admitted {
            assert_eq!(actual, Validation::Valid);
        } else {
            assert!(matches!(actual, Validation::Invalid(_)), "{actual:?}");
        }
    }
}

#[test]
fn fixed_unchanged_tuple_projections_keep_actual_variable_identity() {
    for definitions in [
        "vars == <<x,s>>",
        "vars == <<x,<<s>>>>",
        "Pack(v) == <<x,v>>\nvars == Pack(s)",
        "vars == LET Pack(v) == <<x,v>> IN Pack(s)",
    ] {
        let source = format!(
            "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\n{definitions}\nInit == x=0 /\\ s = <<0,1>>\nNext == (x=0 /\\ x'=1 /\\ s'=Tail(s)) \\/ (x#0 /\\ UNCHANGED vars)\nInv == x=0\n====\n"
        );
        let model = tuple_replay_model(&source, "");
        for final_x in [1, 99] {
            let word = TraceDoc::behavior(vec![
                json!({"x":0,"s":{"seq":[0,1]}}),
                json!({"x":final_x,"s":{"seq":[1]}}),
            ])
            .expect("actual tuple states")
            .with_ending(Ending::Invariant("Inv".to_owned()));
            let actual = validate_doc(&model, &word, &[], None).0.validation;
            if final_x == 1 {
                assert_eq!(actual, Validation::Valid, "{definitions}");
            } else {
                assert!(matches!(actual, Validation::Invalid(_)), "{actual:?}");
            }
        }
    }
}

#[test]
fn unchanged_conditional_permutation_is_not_a_variable_invariant() {
    // Equality of this tuple can swap a tuple variable with a string. The
    // actual supplied word satisfies Next, but cannot justify a tuple-only
    // whole-history certificate. No inferred invariant may trust the word.
    let source = "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s,t\nvars == IF x=1 THEN <<s,t>> ELSE <<t,s>>\nInit == x=0 /\\ s = <<0,1>> /\\ t=\"text\"\nNext == (x=0 /\\ x'=1 /\\ s'=Tail(s) /\\ t'=t) \\/ (x=1 /\\ x'=2 /\\ UNCHANGED vars)\nInv == x<2\n====\n";
    let word = TraceDoc::behavior(vec![
        json!({"x":0,"s":{"seq":[0,1]},"t":"text"}),
        json!({"x":1,"s":{"seq":[1]},"t":"text"}),
        json!({"x":2,"s":"text","t":{"seq":[1]}}),
    ])
    .expect("concrete conditional permutation")
    .with_ending(Ending::Invariant("Inv".to_owned()));
    let model = tuple_replay_model(source, "");
    assert_eq!(
        validate_doc(&model, &word, &[], None).0.validation,
        Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
    );
    let analysis =
        dive_sema::analyze_source("TupleReplay.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let canonical = dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n")
            .into_config()
            .expect("cfg"),
        &dive_model::BindOptions::default(),
    )
    .expect("canonical binding");
    assert_eq!(
        validate_doc(&canonical, &word, &[], None).0.validation,
        Validation::Valid
    );
}

#[test]
fn unchanged_projection_rejects_wrong_parameter_identity_and_excessive_depth() {
    use std::fmt::Write as _;
    let mut deep = String::from("P0 == <<x,s>>\n");
    for index in 1..80 {
        writeln!(deep, "P{index} == P{}", index - 1).expect("writing to String");
    }
    deep.push_str("vars == P79");
    for definitions in ["Pack(v) == <<x,v>>\nvars == Pack(x)".to_owned(), deep] {
        let source = format!(
            "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\n{definitions}\nInit == x=0 /\\ s = <<0,1>>\nNext == (x=0 /\\ x'=1 /\\ s'=Tail(s)) \\/ (x#0 /\\ UNCHANGED vars)\nInv == x=0\n====\n"
        );
        let model = tuple_replay_model(&source, "");
        let word = TraceDoc::behavior(vec![
            json!({"x":0,"s":{"seq":[0,1]}}),
            json!({"x":1,"s":{"seq":[1]}}),
        ])
        .expect("successful prefix is not evidence of all-branch preservation")
        .with_ending(Ending::Invariant("Inv".to_owned()));
        assert_eq!(
            validate_doc(&model, &word, &[], None).0.validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
    }
}
