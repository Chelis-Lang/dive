//! Actual binding roots retain conservative runtime-name worker restrictions.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "real source binding fixtures"
)]

use std::sync::Arc;

use dive_model::{BindOptions, Model, bind};

fn model(body: &str, extra_config: &str, options: &BindOptions) -> Model {
    let source = format!(
        "---- MODULE Names ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Names.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let text = format!("INIT Init\nNEXT Next\n{extra_config}");
    let config = dive_config::parse(&text).into_config().expect("config");
    bind(ir, &config, options).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn literal_and_recursive_append_calls_remove_only_the_unneeded_name_reason() {
    for source in [
        "Init == x= <<>>\nNext == x'= <<0>> \\o x",
        "RECURSIVE Order(_)\nOrder(S) == IF S={} THEN <<>> ELSE LET e == CHOOSE v \\in S: TRUE IN Append(Order(S\\{e}),e)\nInit == x= <<>>\nNext == x'=x \\o Order({0,1})",
    ] {
        let bound = model(source, "", &BindOptions::tlc());
        assert!(!bound.runtime_name_order, "{:?}", bound.sequential);
        assert_eq!(bound.sequential, Vec::<String>::new());
    }
}

#[test]
fn every_call_and_each_selected_checking_root_must_be_accounted_for() {
    for (body, config) in [
        (
            "Init == x= <<>>\nNext == x'= <<0>> \\o x\nInv == (x \\o x)=x",
            "INVARIANT Inv\n",
        ),
        (
            "Init == x= <<>>\nNext == x'= <<0>> \\o x\nV == x \\o x",
            "VIEW V\n",
        ),
        (
            "Init == x= <<>>\nNext == x'= <<0>> \\o x\nInv == []((x \\o x)=x)",
            "PROPERTY Inv\n",
        ),
        ("Init == x= <<>>\nNext == x'=x \\o x", ""),
    ] {
        let bound = model(body, config, &BindOptions::tlc());
        assert!(bound.runtime_name_order, "{body}: {:?}", bound.sequential);
    }
}

#[test]
fn children_keep_their_own_creators_and_other_sequential_effects() {
    let strings = model(
        "Init == x= <<>>\nNext == x'= <<ToString(x)>> \\o x",
        "",
        &BindOptions::tlc(),
    );
    assert!(strings.runtime_name_order);
    assert!(!strings.sequential.is_empty());
    let registers = model(
        "Init == x= <<>>\nNext == (x'= <<0>> \\o x) /\\ TLCSet(0,x)",
        "",
        &BindOptions::tlc(),
    );
    assert!(!registers.runtime_name_order, "{:?}", registers.sequential);
    assert!(
        registers
            .sequential
            .iter()
            .any(|reason| reason.contains("TLCSet"))
    );
}

#[test]
fn configured_native_origins_and_higher_order_arguments_remain_conservative() {
    let configured = model(
        "CONSTANT C(_,_)\nInit == x= <<>>\nNext == x'= <<0>> \\o C(x,x)",
        "CONSTANT C <- Append\n",
        &BindOptions::tlc(),
    );
    assert!(configured.runtime_name_order, "{:?}", configured.sequential);
    let higher = model(
        "F(Op(_,_),a) == Op(a,a)\nInit == x= <<>>\nNext == x'= <<0>> \\o F(Append,x)",
        "",
        &BindOptions::tlc(),
    );
    assert!(higher.runtime_name_order, "{:?}", higher.sequential);
    let redirected = model(
        "Replacement(a,b) == a\nInit == x= <<>>\nNext == x'=Append(x,x) \\o x",
        "CONSTANT Append <- Replacement\n",
        &BindOptions::tlc(),
    );
    assert!(redirected.runtime_name_order, "{:?}", redirected.sequential);
}

#[test]
fn ungrounded_state_and_except_flows_do_not_inherit_type_invariants() {
    for expression in ["Tail(x)", "SubSeq(x,1,1)", "[x EXCEPT ![1]= <<0>>]"] {
        let bound = model(
            &format!("Init == x= <<>>\nNext == x'={expression} \\o x\nTypeOK == x \\in Seq(Nat)"),
            "INVARIANT TypeOK\n",
            &BindOptions::tlc(),
        );
        assert!(
            bound.runtime_name_order,
            "{expression}: {:?}",
            bound.sequential
        );
    }
}

#[test]
fn default_and_tlc_profiles_keep_the_same_successful_source_evaluation() {
    for options in [BindOptions::dive(), BindOptions::tlc()] {
        let bound = model(
            "Init == x= <<>>\nNext == x'=Append(x,0) \\o <<1>>",
            "",
            &options,
        );
        assert!(!bound.runtime_name_order, "{:?}", bound.sequential);
        let values = Arc::from([Some(dive_value::Value::tuple(Vec::new()))]);
        let successors = Arc::from([Some(dive_value::Value::tuple(vec![
            dive_value::Value::Int(0),
            dive_value::Value::Int(1),
        ]))]);
        let state = dive_eval::St::action(values, successors);
        assert_eq!(
            bound
                .evaluator
                .eval_action(&bound.actions[0], &state)
                .expect("ordinary action"),
            dive_value::Value::TRUE
        );
    }
}
