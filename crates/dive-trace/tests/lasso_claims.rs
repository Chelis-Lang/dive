//! Independent periodic behavior and named-property claims.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_model::Model;
use dive_trace::{Check, Ending, TraceDoc, Validation, validate_doc};
use serde_json::json;

const SPEC: &str = "---- MODULE Claims ----\nVARIABLE x\nInit == x = FALSE\n\
    Next == x' = ~x\nSpec == Init /\\ [][Next]_x\n\
    P == <>[]x\nQ == <>[]~x\nChanges == x' # x\nA == [][x' = TRUE]_x\n====\n";

fn bound(source: &str, cfg: &str) -> Model {
    let name = source.split_whitespace().nth(2).expect("name");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("lower"));
    let cfg = dive_config::parse(cfg).into_config().expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}

fn word() -> TraceDoc {
    TraceDoc::behavior(vec![json!({"x":false}), json!({"x":true})])
        .expect("word")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["P".into()])
}

fn result(model: &Model, doc: &TraceDoc) -> Validation {
    validate_doc(model, doc, &[]).0.validation
}

#[test]
fn closing_edge_and_claimed_truth_are_both_checked() {
    let m = bound(SPEC, "SPECIFICATION Spec\nPROPERTY P\n");
    let doc = word();
    assert_eq!(result(&m, &doc), Validation::Valid);
    // Stuttering at TRUE is a permitted behavior, but satisfies P.
    let changed = doc.clone().with_ending(Ending::Stuttering);
    assert!(matches!(result(&m, &changed), Validation::Invalid(f) if f.check == Check::Temporal));
    let unknown = doc.clone().with_properties(vec!["Missing".into()]);
    assert!(matches!(result(&m, &unknown), Validation::Invalid(f) if f.check == Check::Ending));
    let past_end = doc.clone().with_ending(Ending::Lasso { back_to: 2 });
    assert!(matches!(result(&m, &past_end), Validation::Invalid(f) if f.check == Check::Shape));
    // The last transition of the cycle violates the action property A.
    let m = bound(SPEC, "SPECIFICATION Spec\nPROPERTY P A\n");
    assert!(matches!(result(&m, &doc), Validation::Invalid(f) if f.check == Check::ImpliedAction));
}

#[test]
fn named_set_is_complete_and_legacy_claims_stay_unsupported() {
    let m = bound(SPEC, "SPECIFICATION Spec\nPROPERTY P Q\n");
    assert!(
        matches!(result(&m, &word()), Validation::Invalid(f) if f.check == Check::Temporal && f.detail.contains("omitted"))
    );
    let all = word().with_properties(vec!["P".into(), "Q".into()]);
    assert_eq!(result(&m, &all), Validation::Valid);
    let reversed = word().with_properties(vec!["Q".into(), "P".into()]);
    assert!(
        matches!(result(&m, &reversed), Validation::Invalid(f) if f.check == Check::Ending && f.detail.contains("configured order"))
    );
    let reversed_model = bound(SPEC, "SPECIFICATION Spec\nPROPERTY Q P\n");
    assert_eq!(result(&reversed_model, &reversed), Validation::Valid);
    let duplicate = all.clone().with_properties(vec!["P".into(), "P".into()]);
    assert!(matches!(result(&m, &duplicate), Validation::Invalid(f) if f.check == Check::Ending));
    let legacy = word().with_properties(vec![]);
    assert!(matches!(result(&m, &legacy), Validation::Unsupported(s) if s.contains("liveness")));
    let nonperiodic = all.with_ending(Ending::Behavior);
    assert!(matches!(result(&m, &nonperiodic), Validation::Invalid(f) if f.check == Check::Ending));
}

#[test]
fn temporal_behavior_conjuncts_must_hold_and_recurring_action_certifies_fairness() {
    let src = SPEC.replace(
        "Spec == Init /\\ [][Next]_x",
        "Spec == Init /\\ [][Next]_x /\\ []<>x",
    );
    let m = bound(&src, "SPECIFICATION Spec\nPROPERTY P\n");
    let doc = TraceDoc::behavior(vec![json!({"x":false})])
        .expect("word")
        .with_ending(Ending::Stuttering)
        .with_properties(vec!["P".into()]);
    assert!(matches!(result(&m, &doc), Validation::Invalid(f) if f.check == Check::Fairness));
    let src = SPEC.replace(
        "Spec == Init /\\ [][Next]_x",
        "Spec == Init /\\ [][Next]_x /\\ WF_x(Next)",
    );
    let m = bound(&src, "SPECIFICATION Spec\nPROPERTY P\n");
    // The cycle executes <<Next>>_x infinitely often, proving WF without
    // needing to decide whether some other action is enabled.
    assert_eq!(result(&m, &word()), Validation::Valid);
    let stutter = TraceDoc::behavior(vec![json!({"x":false})])
        .expect("word")
        .with_ending(Ending::Stuttering)
        .with_properties(vec!["P".into()]);
    assert!(matches!(result(&m,&stutter),Validation::Invalid(f) if f.check == Check::Fairness));
}

#[test]
fn implicit_stutters_bypass_action_constraints_but_real_edges_do_not() {
    let m = bound(
        SPEC,
        "SPECIFICATION Spec\nPROPERTY P\nACTION_CONSTRAINT Changes\n",
    );
    let doc = TraceDoc::behavior(vec![json!({"x":false}), json!({"x":false})])
        .expect("word")
        .with_ending(Ending::Stuttering)
        .with_properties(vec!["P".into()]);
    assert_eq!(result(&m, &doc), Validation::Valid);
    let src = SPEC.replace("Changes == x' # x", "Changes == x' = x");
    let m = bound(
        &src,
        "SPECIFICATION Spec\nPROPERTY P\nACTION_CONSTRAINT Changes\n",
    );
    assert!(
        matches!(result(&m, &word()), Validation::Invalid(f) if f.check == Check::ActionConstraint)
    );
    // State constraints still apply to every state, including the last.
    let src = SPEC.replace("Changes == x' # x", "Changes == x = FALSE");
    let m = bound(&src, "SPECIFICATION Spec\nPROPERTY P\nCONSTRAINT Changes\n");
    assert!(matches!(result(&m, &word()), Validation::Invalid(f) if f.check == Check::Constraint));
}

#[test]
fn wrapper_claims_cannot_disagree_with_the_inner_document() {
    let doc = word();
    let raw = doc.to_json();
    assert_eq!(TraceDoc::from_json(&raw).expect("round trip"), doc);
    let wrapped = json!({"counterexample":{"properties":["Q"],"trace":raw}});
    assert!(TraceDoc::from_json(&wrapped).is_err());
    let all = word().with_properties(vec!["P".into(), "Q".into()]);
    let wrapped = json!({"counterexample":{"properties":["Q","P"],"trace":all.to_json()}});
    assert!(TraceDoc::from_json(&wrapped).is_err());
    let wrapped = json!({"counterexample":{"properties":["P","Q"],"trace":all.to_json()}});
    assert_eq!(
        TraceDoc::from_json(&wrapped).expect("same ordered names"),
        all
    );
}

#[test]
fn lossy_reader_preflights_before_scoring_effectful_initial_predicates() {
    let src = SPEC
        .replace("VARIABLE x", "EXTENDS TLC\nVARIABLE x")
        .replace(
            "Init == x = FALSE",
            "Init == PrintT(\"must not print\") /\\ x = FALSE",
        );
    let m = bound(&src, "SPECIFICATION Spec\nPROPERTY P\n");
    let _ = m.take_output();
    let doc = TraceDoc::parse(
        r#"{"vars":["x"],"counterexample":{"state":[[1,{"x":false}]],"action":[]}}"#,
    )
    .expect("TLC document")
    .with_ending(Ending::Stuttering)
    .with_properties(vec!["P".into()]);
    assert!(matches!(result(&m, &doc), Validation::Unsupported(s) if s.contains("side effects")));
    assert_eq!(m.take_output(), Vec::<String>::new());
}

#[test]
fn pinned_liveness_words_validate_including_recurring_fair_actions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes");
    let mut valid = 0;
    for name in [
        "LvNoFair",
        "LvEAPrefix",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
        "LvStutterActionConstraint",
        "LvWFIntermittent",
        "LvStutterEnd",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let cfg =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("cfg");
        let m = bound(&src, &cfg);
        let raw =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.trace.json")))
                .expect("trace");
        let j: serde_json::Value = serde_json::from_str(&raw).expect("JSON");
        let actions = j["counterexample"]["action"].as_array().expect("actions");
        let back = usize::try_from(
            actions.last().expect("loop")[2][0]
                .as_u64()
                .expect("target"),
        )
        .expect("index")
            - 1;
        let doc = TraceDoc::parse(&raw)
            .expect("document")
            .with_ending(Ending::Lasso { back_to: back })
            .with_properties(vec!["Prop".into()]);
        let r = result(&m, &doc);
        match r {
            Validation::Valid => valid += 1,
            Validation::Unsupported(s) => {
                panic!("pinned recurring-action control {name} was refused: {s}")
            }
            Validation::Invalid(f) => panic!("{name}: {f:?}"),
            other => panic!("{name}: unexpected validation {other:?}"),
        }
    }
    assert!(
        valid == 10,
        "controls must exercise actual certification, got {valid}"
    );
}

#[test]
fn nested_filter_domains_share_completed_source_checks() {
    use std::fmt::Write as _;

    let mut source = String::from(
        "---- MODULE Nested ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nD0(a) == a\n",
    );
    for level in 1..=12 {
        writeln!(
            source,
            "D{level}(a) == {{z \\in D{}(a) : z \\in a}}",
            level - 1
        )
        .expect("write to string");
    }
    source.push_str("P == <> (2 \\in D12(IF x THEN {0} ELSE {1}))\n====\n");
    let model = bound(&source, "INIT Init\nNEXT Next\nPROPERTY P\n");
    assert_eq!(result(&model, &word()), Validation::Valid);
}
