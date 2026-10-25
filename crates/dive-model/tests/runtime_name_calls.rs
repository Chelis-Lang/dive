//! Actual binding roots retain conservative runtime-name worker restrictions.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "real source binding fixtures"
)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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

fn concat_reason(model: &Model) -> bool {
    model
        .sequential
        .iter()
        .any(|reason| reason.starts_with(&dive_ir::StdOp::SequencesConcat.qualified_name()))
}

#[test]
fn closed_scalar_comparison_assumptions_preserve_inventory_and_allow_workers() {
    let bound = model(
        "Ordered(Op(_,_),S) == \\A a,b \\in S: Op(a,b) \\/ Op(b,a) \\/ a=b\nASSUME Ordered(<,{0,1})\nInit == x= <<>>\nNext == x'= <<0>> \\o x",
        "",
        &BindOptions::tlc(),
    );
    assert!(bound.runtime_name_order);
    assert!(bound.sequential_names_only);
    assert_eq!(bound.sequential, Vec::<String>::new());
}

#[test]
fn configured_literal_definitions_use_the_stored_value_without_evaluating_replaced_bodies() {
    let bound = model(
        "D == x \\o x\nInit == x= <<>>\nNext == D=D /\\ x'= <<0>> \\o x",
        "CONSTANT D=sentinel\n",
        &BindOptions::tlc(),
    );
    assert!(bound.runtime_name_order);
    assert!(bound.sequential_names_only);
    assert_eq!(bound.sequential, Vec::<String>::new());
}

#[test]
fn literal_and_recursive_append_calls_remove_only_the_unneeded_name_reason() {
    for source in [
        "Init == x= <<>>\nNext == x'= <<0>> \\o x",
        "RECURSIVE Order(_)\nOrder(S) == IF S={} THEN <<>> ELSE LET e == CHOOSE v \\in S: TRUE IN Append(Order(S\\{e}),e)\nInit == x= <<>>\nNext == x'=x \\o Order({0,1})",
    ] {
        let bound = model(source, "", &BindOptions::tlc());
        // Replay and checkpoint consumers keep their original source inventory.
        assert!(bound.runtime_name_order);
        assert!(bound.sequential_names_only);
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
        assert!(concat_reason(&bound), "{body}: {:?}", bound.sequential);
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
    assert_ne!(strings.sequential, Vec::<String>::new());
    let registers = model(
        "Init == x= <<>>\nNext == (x'= <<0>> \\o x) /\\ TLCSet(0,x)",
        "",
        &BindOptions::tlc(),
    );
    assert!(registers.runtime_name_order);
    assert!(!registers.sequential_names_only);
    assert!(!concat_reason(&registers), "{:?}", registers.sequential);
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
    assert!(concat_reason(&configured));
    let higher = model(
        "F(Op(_,_),a) == Op(a,a)\nInit == x= <<>>\nNext == x'= <<0>> \\o F(Append,x)",
        "",
        &BindOptions::tlc(),
    );
    assert!(higher.runtime_name_order, "{:?}", higher.sequential);
    assert!(concat_reason(&higher));
    let redirected = model(
        "Replacement(a,b) == a\nInit == x= <<>>\nNext == x'=Append(x,x) \\o x",
        "CONSTANT Append <- Replacement\n",
        &BindOptions::tlc(),
    );
    assert!(redirected.runtime_name_order, "{:?}", redirected.sequential);
    assert!(concat_reason(&redirected));
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
        assert!(concat_reason(&bound));
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
        let ordered = options.eval.ctx.order == dive_value::ValueOrder::Tlc
            || options.eval.exploration == dive_eval::ExplorationOrder::Tlc;
        assert_eq!(bound.runtime_name_order, ordered);
        assert_eq!(bound.sequential, Vec::<String>::new());
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

#[derive(Debug)]
struct Polls {
    calls: AtomicUsize,
    stop: Option<usize>,
}

impl dive_eval::HostControl for Polls {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.stop == Some(call) {
            Err(dive_eval::Interrupted::new(917))
        } else {
            Ok(())
        }
    }
}

#[test]
fn late_proof_interruption_keeps_actual_startup_output_and_allows_a_fresh_retry() {
    let source = "---- MODULE Names ----\nEXTENDS Sequences, TLC\nVARIABLE x\nD == IF PrintT(\"before-one\") THEN PrintT(\"before-two\") ELSE FALSE\nInit == x = <<>>\nNext == x' = x \\o <<>>\n====\n";
    let analysis = dive_sema::analyze_source("Names.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    // Fresh ordinary threads also reset the evaluator's existing TLS loop
    // cadence. Calibrate the actual complete walk; no hardcoded proof count or
    // production hook determines the cancellation point.
    let run = |stop| {
        let ir = ir.clone();
        let config = config.clone();
        std::thread::Builder::new()
            .name("tuple-name-control".into())
            .spawn(move || {
                let control = Arc::new(Polls {
                    calls: AtomicUsize::new(0),
                    stop,
                });
                let result =
                    dive_model::bind_controlled(ir, &config, &BindOptions::tlc(), control.clone());
                (result, control.calls.load(Ordering::SeqCst))
            })
            .expect("fixture thread")
            .join()
            .expect("binding completed")
    };
    let (calibration, polls) = run(None);
    let positive = calibration.expect("actual complete proof");
    assert!(positive.runtime_name_order);
    assert_eq!(positive.sequential, Vec::<String>::new());
    let expected = vec![
        dive_model::Message::Print("\"before-one\"".into()),
        dive_model::Message::Print("\"before-two\"".into()),
    ];
    assert_eq!(positive.output, expected);
    assert!(polls > 1);
    // On this successful proved path, the final proof poll is the binder's
    // last controlled boundary, after startup and Model construction.
    let (stopped, actual) = run(Some(polls));
    assert_eq!(actual, polls);
    let dive_model::ControlledBindError::Interrupted { error, output } =
        stopped.expect_err("late proof stop")
    else {
        panic!("original typed interruption");
    };
    assert_eq!(error.class(), dive_eval::ErrorClass::Interrupted);
    assert_eq!(error.interruption().expect("actual identity").token(), 917);
    assert_eq!(output, expected);
    let (retry, actual) = run(None);
    let retry = retry.expect("fresh runtime after interrupted proof");
    assert_eq!(actual, polls);
    assert!(retry.runtime_name_order);
    assert_eq!(retry.sequential, Vec::<String>::new());
    assert_eq!(retry.output, expected);
}
