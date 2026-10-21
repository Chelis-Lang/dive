//! Semantic admission controls for recursive evaluator leaves.
#![allow(clippy::expect_used, reason = "test assertions")]
use std::sync::Arc;

use dive_eval::{Evaluator, Model, Options};

fn model(body: &str) -> Model {
    let source = format!(
        "---- MODULE Leaf ----\nEXTENDS Integers, Sequences, TLC\nVARIABLE v\n{body}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Leaf.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR")))
}

fn admits(body: &str, name: &str) -> bool {
    let model = model(body);
    let def = model.ir.root_def(name).expect("definition");
    Evaluator::new(model, Options::tlc())
        .expect("evaluator")
        .recursive_leaf(def)
}

#[test]
fn state_level_binders_and_local_recursive_values_are_not_syntax_exclusions() {
    assert!(admits(
        r"RECURSIVE R(_)
R(n) == IF n = 0 THEN TRUE
        ELSE LET f[i \in 0..n] == IF i = 0 THEN 0 ELSE f[i-1] + 1
                 g(x) == x + v
                 values == {g(x) : x \in {f[n]}}
             IN (\A x \in {y \in values : y >= v} : x >= v) /\ R(n-1)",
        "R"
    ));
    // Evaluation errors are handled by the existing evaluator, not by an
    // admission whitelist (nor by a proof that evaluation terminates).
    assert!(admits(
        r"RECURSIVE R(_)
R(n) == IF n = 0 THEN CHOOSE x : x = 0 ELSE R(n-1)",
        "R"
    ));
}

#[test]
fn semantic_dependencies_cannot_hide_temporal_enabled_or_impure_work() {
    for dependency in ["v' = v", "ENABLED (v' = v)", "[] (v = v)", "PrintT(v = v)"] {
        let body = format!(
            "Hidden == {dependency}\nRECURSIVE R(_)\nR(n) == IF n = 0 THEN Hidden ELSE R(n-1)"
        );
        assert!(!admits(&body, "R"), "{dependency}");
        // Even unused LET definitions are analysed: no hidden dynamic work
        // is admitted merely because the current small input skips it.
        let body = format!(
            "RECURSIVE R(_)\nR(n) == LET Hidden == {dependency} IN IF n = 0 THEN TRUE ELSE R(n-1)"
        );
        assert!(!admits(&body, "R"), "unused LET {dependency}");
    }
}

#[test]
fn callbacks_configured_operators_substitutions_and_overrides_remain_unproved() {
    assert!(!admits(
        "RECURSIVE R(_,_)\nR(Op(_),n) == IF n = 0 THEN Op(v) ELSE R(Op,n-1)",
        "R"
    ));
    assert!(!admits(
        "Id(x) == x\nApply(Op(_), x) == Op(x)\nRECURSIVE R(_)\nR(n) == IF n = 0 THEN Apply(Id,v) ELSE R(n-1)",
        "R"
    ));
    assert!(!admits(
        "CONSTANT C\nRECURSIVE R(_)\nR(n) == IF n = 0 THEN C ELSE R(n-1)",
        "R"
    ));
    assert!(!admits(
        "---- MODULE Inner ----\nCONSTANT C\nGet == C\n====\nI == INSTANCE Inner WITH C <- v\nRECURSIVE R(_)\nR(n) == IF n = 0 THEN I!Get ELSE R(n-1)",
        "R"
    ));
    let mut model = model(
        "RECURSIVE R(_)\nR(n) == IF n = 0 THEN TRUE ELSE R(n-1)\nRECURSIVE S(_)\nS(n) == IF n = 0 THEN FALSE ELSE S(n-1)",
    );
    let original = model.ir.root_def("R").expect("R");
    let replacement = model.ir.root_def("S").expect("S");
    model.redirects.insert(original, replacement);
    let ev = Evaluator::new(model, Options::tlc()).expect("evaluator");
    assert!(!ev.recursive_leaf(original));
    assert!(ev.recursive_leaf(replacement));
    assert!(!ev.recursive_leaf(dive_ir::DefId(u32::MAX)));
}

#[test]
fn value_bindings_do_not_certify_operator_bindings_or_value_overrides() {
    let source = "CONSTANT C\nD == TRUE\nRECURSIVE R(_)\nR(n) == IF n = 0 THEN C ELSE R(n-1)";
    let mut bound = model(source);
    let def = bound.ir.root_def("R").expect("R");
    let replacement = bound.ir.root_def("D").expect("D");
    let constant = bound
        .ir
        .consts()
        .iter()
        .position(|c| bound.ir.decl(*c).name == "C")
        .expect("C");
    bound.constants[constant] = Some(dive_eval::ConstBinding::Value(true.into()));
    assert!(
        Evaluator::new(bound.clone(), Options::tlc())
            .expect("value binding")
            .recursive_leaf(def)
    );
    bound.constants[constant] = Some(dive_eval::ConstBinding::Def(replacement));
    assert!(
        !Evaluator::new(bound, Options::tlc())
            .expect("operator binding")
            .recursive_leaf(def)
    );

    let mut bound = model("RECURSIVE R\nR == IF TRUE THEN TRUE ELSE R");
    let def = bound.ir.root_def("R").expect("R");
    bound.def_values.insert(def, true.into());
    assert!(
        !Evaluator::new(bound, Options::tlc())
            .expect("value override")
            .recursive_leaf(def)
    );
}

#[test]
fn long_helper_chains_do_not_consume_the_native_call_stack() {
    use std::fmt::Write as _;

    let mut body = String::from("D0 == TRUE\n");
    for index in 1..4096 {
        writeln!(body, "D{index} == D{}", index - 1).expect("source");
    }
    body.push_str("RECURSIVE R(_)\nR(n) == IF n = 0 THEN D4095 ELSE R(n-1)\n");
    // Front-end analysis has its own documented stack requirement. Only
    // the new dependency proof runs with the deliberately small stack below.
    let (ev, def) = std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || {
            let bound = model(&body);
            let def = bound.ir.root_def("R").expect("R");
            let ev = Evaluator::new(bound, Options::tlc()).expect("evaluator");
            (ev, def)
        })
        .expect("analysis thread")
        .join()
        .expect("analysis completed");
    assert!(
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(move || ev.recursive_leaf(def))
            .expect("small-stack proof thread")
            .join()
            .expect("proof completed")
    );
}
