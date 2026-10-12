//! Runtime names under TLC's encounter order (ADR 0016, amendment 3).
//!
//! Names interned when the model is bound (SANY's encounter order, which
//! includes the identifiers of the standard modules, constants and the
//! configuration) have TLC's tokens. The validator validates as usual and
//! refuses the verdict only if decoding or checking created a name after
//! binding. A witness can therefore never establish the order that proves
//! itself.
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

fn refused() -> Validation {
    Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
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
fn string_operands_refuse_only_when_replay_creates_a_name() {
    // SANY interns every identifier and numeral it reads, so `"b"` (an
    // identifier of the standard modules) and `"9137"` (a numeral of the
    // spec) are start-up names: `Tail("ab")` and `ToString(9137)` create
    // nothing. Branches not taken on the trace create nothing. `"q9"` and
    // `"9137"` built from `9000+137` are created by the replay itself.
    for (definitions, expression, expected) in [
        ("", "Tail(\"ab\")", Validation::Valid),
        ("", "Tail(\"zq9\")", refused()),
        (
            "",
            "Tail(IF x=0 THEN <<0,1>> ELSE \"ab\")",
            Validation::Valid,
        ),
        (
            "",
            "Tail(IF x=0 THEN <<0,1>> ELSE \"zq9\")",
            Validation::Valid,
        ),
        ("", "Tail(<<0,ToString(x+9137)>>)", Validation::Valid),
        ("", "Tail(<<0,ToString(x+9000+137)>>)", refused()),
        (
            "ASSUME ToString(0)=\"0\"",
            "Tail(<<0,1>>)",
            Validation::Valid,
        ),
        (
            "ASSUME ToString(9000+137) # \"x\"",
            "Tail(<<0,1>>)",
            refused(),
        ),
        (
            "Unknown == IF x=0 THEN <<0,1>> ELSE \"ab\"",
            "Tail(Unknown)",
            Validation::Valid,
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
                expected,
                "{definitions}; {expression}; lossy={lossy}"
            );
        }
    }
}

#[test]
fn configured_sequence_aliases_follow_the_same_rule() {
    for (expression, admitted) in [
        ("Alias(<<0,1>>)", true),
        ("Alias(\"ab\")", true),
        ("Alias(\"zq9\")", false),
    ] {
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
fn pure_native_string_constructors_are_not_trusted() {
    // Json is classified as pure. The evaluator does not implement `ToJson`
    // yet, so the ASSUME refuses before any name can be created.
    let source = tuple_replay_source("ASSUME ToJson(0)=\"0\"", "Tail(<<0,1>>)")
        .replace("Sequences, TLC", "Sequences, TLC, Json");
    for lossy in [false, true] {
        let model = tuple_replay_model(&source, "");
        assert!(model.runtime_name_order);
        assert!(matches!(
            validate_doc(&model, &tuple_replay_doc(1, lossy), &[], None)
                .0
                .validation,
            Validation::Unsupported(_)
        ));
    }
}

#[test]
fn tuple_histories_validate_by_replay_without_witness_shape_as_proof() {
    // No case creates a name on this word: the string branches are not
    // taken, and `"ab"` and `"b"` are start-up names. Each word gets the
    // canonical verdict: valid with x=1, invalid with x=99 or a false Init.
    for (init, next, init_holds) in [
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
            true,
        ),
        (
            "<<0,1>>",
            "x'=1 /\\ s'=IF x=0 THEN Tail(s) ELSE <<ToString(x)>>",
            true,
        ),
        ("\"ab\"", "x'=1 /\\ s'=Tail(s)", false),
        ("<<0,1>>", "x'=1 /\\ Len(Tail(s))=1", true),
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
            if init_holds && final_x == 1 {
                assert_eq!(result, Validation::Valid, "{next}");
            } else {
                assert!(
                    matches!(result, Validation::Invalid(_)),
                    "{next}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn tuple_histories_with_string_alternatives_validate_the_actual_word() {
    // The string alternatives are literals and are not taken on this word,
    // so replay creates no name. The public existence API receives an
    // arbitrary state and uses no facts about histories.
    for (definitions, init, next) in [
        (
            "Drop(v) == Tail(v)",
            "x=0 /\\ s = <<0,1>>",
            "x'=1 /\\ s'=Drop(s)",
        ),
        (
            "Drop(v) == LET Pair == v IN Tail(Pair)",
            "x=0 /\\ s = <<0,1>>",
            "x'=1 /\\ s'=Drop(s)",
        ),
        (
            "",
            "(x=0 /\\ s = <<0,1>>) \\/ (x=2 /\\ s = \"ab\")",
            "x'=1 /\\ s'=Tail(s)",
        ),
        (
            "",
            "x=0 /\\ s = <<0,1>>",
            "(x=0 /\\ x'=1 /\\ s'=Tail(s)) \\/ (x#0 /\\ x'=1)",
        ),
        (
            "",
            "x=0 /\\ s = <<0,1>>",
            "x'=1 /\\ s'=(IF x=1 THEN s ELSE <<0>>)' /\\ Len(Tail(s))=1",
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
        assert_eq!(result, Validation::Valid, "{init}; {next}");
        let trace = Trace::decode(&model, &doc).expect("decode stack");
        let decision = dive_trace::exists::exists_successor(
            &model,
            model.next.as_ref().expect("next"),
            &trace.states[0],
            None,
            dive_trace::exists::Limits::default(),
        );
        // The state has a successor: the decision may refuse (a call with a
        // state-dependent argument has no cover rule), never be Disabled.
        assert!(
            !matches!(decision, Ok(dive_trace::exists::Decision::Disabled)),
            "{next}: {decision:?}"
        );
    }
}

#[test]
fn joint_tuple_histories_validate_the_actual_word() {
    // The string branch is not taken and `"ab"` is a literal: both validate.
    for next_t in ["s", "IF x=0 THEN s ELSE \"ab\""] {
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
        assert_eq!(result, Validation::Valid, "{next_t}");
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
    // `P` and `A` create "13" and "14" on state x=13: refused. `Q` and `B`
    // create nothing and are decided, unless a name was already interned
    // after binding (as by a checker sharing the model): then every answer
    // is refused.
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
            // The refusal is about names created, not about the model:
            // fresh models decide `Q` and `B`.
            let temporal = entry_model(ENTRY_SPEC, cfg, true);
            let action = entry_model(ENTRY_SPEC, cfg, true);
            if previous_encounter {
                temporal.names.intern("14");
                action.names.intern("14");
            }
            let q = dive_trace::temporal::evaluate_lasso(
                &temporal,
                &predicate(&temporal, "Q"),
                &[entry_state()],
                0,
            );
            let b = dive_trace::exists::exists_successor(
                &action,
                &predicate(&action, "B"),
                &entry_state(),
                None,
                dive_trace::exists::Limits::default(),
            );
            if previous_encounter {
                assert!(matches!(
                    q,
                    Err(dive_trace::temporal::TemporalError::Unsupported(_))
                ));
                assert!(matches!(
                    b,
                    Err(dive_trace::exists::ExistsError::Unsupported(_))
                ));
            } else {
                assert_eq!(q, Ok(vec![true]));
                assert!(matches!(
                    b,
                    Ok(dive_trace::exists::Decision::Enabled { .. })
                ));
            }
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
    // Start-up strings, untaken string branches and opaque wrappers create
    // no name either.
    for (expression, extra, cfg) in [
        ("Tail(\"ab\")", "", ""),
        ("Tail(IF x=13 THEN <<1,2>> ELSE \"ab\")", "", ""),
        ("Tail(Identity(<<1,2>>))", "Identity(v)==v", ""),
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
fn created_names_and_effects_in_sequence_inputs_stay_refused() {
    for (expression, extra) in [
        ("Tail(\"zq9\")", ""),
        ("Tail(<<ToString(x),ToString(x+1)>>)", ""),
        ("Tail(PrintT(<<1,2>>))", ""),
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
fn configured_operator_constants_follow_their_native_bindings() {
    // A false Init is invalid. `ToString(s)` creates "<<0, 1>>": refused,
    // unless `x'=1` already fails and the conjunction stops before it.
    for (init, target, expected) in [
        ("<<0,1>>", "Tail", None),
        ("\"ab\"", "Tail", Some(false)),
        ("<<0,1>>", "ToString", Some(true)),
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
            match expected {
                None if final_x == 1 => assert_eq!(actual, Validation::Valid),
                None | Some(false) => {
                    assert!(matches!(actual, Validation::Invalid(_)), "{actual:?}");
                }
                Some(true) if final_x == 1 => assert_eq!(actual, refused()),
                Some(true) => assert!(matches!(actual, Validation::Invalid(_)), "{actual:?}"),
            }
        }
    }
}

#[test]
fn forged_initial_or_successor_values_are_never_valid() {
    // Decoding the string `"zab"` creates a name: refused. `"b"` is a start-up
    // name, so the forged successor is refuted by evaluation.
    let source = "---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\nInit == Len(Tail(s))=1 /\\ x=0 /\\ s = <<0,1>>\nNext == x'=1 /\\ s'=Tail(s)\nInv == x=0\n====\n";
    for (initial, successor, admitted) in [
        (json!({"seq":[0,1]}), json!({"seq":[1]}), true),
        (json!("zab"), json!({"seq":[1]}), false),
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
        } else if initial == json!("zab") {
            assert_eq!(actual, refused());
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
fn unchanged_conditional_permutation_validates_by_replay() {
    // Equality of this tuple can swap a tuple variable with a string. The
    // actual supplied word satisfies Next, and replay creates no name
    // (`"text"` is a literal): it validates in both orders.
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
        Validation::Valid
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
fn unchanged_projections_of_any_shape_or_depth_validate_by_replay() {
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
        .expect("actual tuple states")
        .with_ending(Ending::Invariant("Inv".to_owned()));
        assert_eq!(
            validate_doc(&model, &word, &[], None).0.validation,
            Validation::Valid
        );
    }
}

#[test]
fn a_refutation_that_needs_a_created_name_is_refused_not_invalid() {
    // The step's value `"8"` is not a start-up name, and refuting it
    // evaluates `ToString(7)`, which creates another. TLC's order between
    // them is unknown, so the validator refuses rather than judge.
    let source = "---- MODULE ReplayNames ----\nEXTENDS Naturals, TLC\nVARIABLES x, s\nInit == x = 6 /\\ s = \"unset\"\nNext == x' = x /\\ s' = ToString(x + 1)\nInv == TRUE\n====\n";
    let word = TraceDoc::behavior(vec![json!({"x":6,"s":"unset"}), json!({"x":6,"s":"8"})])
        .expect("forged word");
    let tlc = model(true, source);
    assert_eq!(validate_doc(&tlc, &word, &[], None).0.validation, refused());
    let canonical = model(false, source);
    assert!(matches!(
        validate_doc(&canonical, &word, &[], None).0.validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn sequences_of_model_values_validate_in_tlc_order() {
    // The shape of MCCheckpointCoordinationFailure (#242): `SubSeq` and `\o`
    // on a record field of a state variable and an EXCEPT `@`, over
    // sequences of model values. Nothing is created, so the word validates.
    let source = "---- MODULE Logs ----\nEXTENDS Naturals, Sequences\nCONSTANTS a, b\nVARIABLES latest, log\nInit == latest = [log |-> <<a, b>>] /\\ log = <<b>>\nNext == /\\ latest' = latest\n        /\\ log' = [log EXCEPT ![1] = @] \\o SubSeq(latest.log, 1, 1)\nInv == Len(log) < 3\n====\n";
    let analysis = dive_sema::analyze_source("Logs.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let cfg = dive_config::parse("CONSTANTS a = a b = b\nINIT Init\nNEXT Next\nINVARIANT Inv\n")
        .into_config()
        .expect("cfg");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    assert!(model.runtime_name_order);
    let state = |log: &[&str]| {
        json!({
            "latest": {"record": {"log": {"seq": [{"model": "a"}, {"model": "b"}]}}},
            "log": {"seq": log.iter().map(|m| json!({"model": m})).collect::<Vec<_>>()},
        })
    };
    let word = TraceDoc::behavior(vec![
        state(&["b"]),
        state(&["b", "a"]),
        state(&["b", "a", "a"]),
    ])
    .expect("word")
    .with_ending(Ending::Invariant("Inv".to_owned()));
    assert_eq!(
        validate_doc(&model, &word, &cfg.model_values, None)
            .0
            .validation,
        Validation::Valid
    );
    let forged = TraceDoc::behavior(vec![state(&["b"]), state(&["b", "b"])]).expect("forged word");
    assert!(matches!(
        validate_doc(&model, &forged, &cfg.model_values, None)
            .0
            .validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn existing_runtime_names_refuse_malformed_words_too() {
    let m = entry_model(ENTRY_SPEC, "INIT Init\nNEXT Next\n", true);
    m.names.intern("a-fresh-runtime-name");
    assert_eq!(
        dive_trace::temporal::evaluate_lasso(&m, &predicate(&m, "Q"), &[], 0),
        Err(dive_trace::temporal::TemporalError::Unsupported(
            dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()
        ))
    );
    let doc = TraceDoc::from_json(&json!({
        "vars": ["x"], "counterexample": {"action": [], "state": [[1, {"x": 0}]]}
    }))
    .expect("TLC document without an ending");
    assert_eq!(validate_doc(&m, &doc, &[], None).0.validation, refused());
}

#[test]
fn foreign_typed_names_refuse_before_trace_lasso_or_enabledness_evaluation() {
    use dive_trace::exists::{Decision, ExistsError, Limits, exists_successor};
    use dive_trace::temporal::{TemporalError, evaluate_lasso};
    use dive_value::{Interner, Value};

    for tlc in [false, true] {
        for nested in [false, true] {
            let (initial_x, initial_y, select_x, select_y) = if nested {
                ("<<\"a\">>", "<<\"b\">>", "x[1]", "y[1]")
            } else {
                ("\"a\"", "\"b\"", "x", "y")
            };
            let source = format!(
                "---- MODULE EntryNames ----\nVARIABLES x,y\nInit == x = {initial_x} /\\ y = {initial_y}\nNext == UNCHANGED <<x,y>>\nInv == (CHOOSE v \\in {{{select_x},{select_y}}}: TRUE) = {select_x}\nP == []Inv\n====\n"
            );
            let m = entry_model(&source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", tlc);
            let foreign = Interner::new();
            foreign.intern("b");
            let state = |names: &Interner| -> dive_trace::State {
                let wrap = |text| {
                    let value = Value::string(names, text);
                    if nested {
                        Value::tuple(vec![value])
                    } else {
                        value
                    }
                };
                Arc::from([Some(wrap("a")), Some(wrap("b"))])
            };
            for owned in [false, true] {
                let here = state(if owned { &m.names } else { &foreign });
                let trace = Trace {
                    states: vec![here.clone()],
                    labels: vec![None],
                    ending: Ending::Invariant("Inv".to_owned()),
                    length: None,
                    properties: Vec::new(),
                };
                assert!(!m.has_runtime_names());
                let report = validate(&m, &trace, None);
                let temporal =
                    evaluate_lasso(&m, &predicate(&m, "P"), std::slice::from_ref(&here), 0);
                let enabled =
                    exists_successor(&m, &predicate(&m, "Next"), &here, None, Limits::default());
                if owned {
                    let mut behavior = trace.clone();
                    behavior.ending = Ending::Behavior;
                    assert_eq!(validate(&m, &behavior, None).validation, Validation::Valid);
                    // The actual start-up names make Inv true: this claimed
                    // invariant violation is rejected, not a counterexample.
                    assert!(
                        matches!(report.validation, Validation::Invalid(_)),
                        "{report:?}"
                    );
                    assert_eq!(temporal, Ok(vec![true]));
                    assert!(
                        matches!(enabled, Ok(Decision::Enabled { .. })),
                        "{enabled:?}"
                    );
                } else {
                    assert!(
                        matches!(report.validation, Validation::Unsupported(reason) if reason.contains("DIVE-V0041"))
                    );
                    assert!(
                        matches!(temporal, Err(TemporalError::Unsupported(reason)) if reason.contains("DIVE-V0041"))
                    );
                    assert!(
                        matches!(enabled, Err(ExistsError::Unsupported(reason)) if reason.contains("DIVE-V0041"))
                    );
                }
            }
        }
    }
}

#[test]
fn opaque_typed_states_refuse_without_evaluation_or_callbacks() {
    use dive_value::{Ctx, DeferredKind, DeferredSet, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct Opaque(AtomicUsize);
    impl DeferredSet for Opaque {
        fn kind(&self) -> DeferredKind {
            DeferredKind::Map
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            None
        }
    }

    for tlc in [false, true] {
        let source = "---- MODULE EntryNames ----\nEXTENDS TLC\nVARIABLE x\nInit == PrintT(TRUE) /\\ x=x\nNext == UNCHANGED x\nP == []TRUE\n====\n";
        let m = entry_model(source, "INIT Init\nNEXT Next\n", tlc);
        let callback = Arc::new(Opaque::default());
        let state: dive_trace::State =
            Arc::from([Some(Value::deferred(callback.clone(), Ctx::tlc()))]);
        let trace = Trace {
            states: vec![state.clone()],
            labels: vec![None],
            ending: Ending::Behavior,
            length: None,
            properties: Vec::new(),
        };
        assert!(
            matches!(validate(&m, &trace, None).validation, Validation::Unsupported(reason) if reason.contains("DIVE-V0042"))
        );
        assert!(
            matches!(dive_trace::temporal::evaluate_lasso(&m, &predicate(&m, "P"), std::slice::from_ref(&state), 0), Err(dive_trace::temporal::TemporalError::Unsupported(reason)) if reason.contains("DIVE-V0042"))
        );
        assert!(
            matches!(dive_trace::exists::exists_successor(&m, &predicate(&m, "Next"), &state, None, dive_trace::exists::Limits::default()), Err(dive_trace::exists::ExistsError::Unsupported(reason)) if reason.contains("DIVE-V0042"))
        );
        assert_eq!(callback.0.load(Ordering::Relaxed), 0);
        assert_eq!(m.take_output(), Vec::<String>::new());
    }
}
