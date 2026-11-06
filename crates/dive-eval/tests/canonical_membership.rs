//! Mathematical membership must canonicalize explicit raw children before equality.
#![allow(clippy::expect_used, reason = "bounded semantic controls")]

use std::sync::Arc;

use dive_eval::{EvalError, Evaluator, Model, Options, St};
use dive_value::{Ctx, Value};

fn evaluator() -> Evaluator {
    let analysis = dive_sema::analyze_source(
        "CanonicalMember.tla",
        r"---- MODULE CanonicalMember ----
EXTENDS FiniteSets
P(S) == {0, FALSE} \in S
N(S) == {0, FALSE} \notin S
UC(S) == Cardinality(UNION {S})
Two(S) == UC(S) = 2
One(S) == UC(S) = 1
====
",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    Evaluator::new(Model::new(ir), Options::dive()).expect("evaluator")
}

fn evaluate(evaluator: &Evaluator, name: &str, input: Value) -> Result<Value, EvalError> {
    let def = evaluator.ir().root_def(name).expect("operator");
    let scope = evaluator.scope(def).expect("scope");
    let call = evaluator
        .scope_call_values(&scope, def, vec![input])
        .expect("parameter scope");
    evaluator.eval_scoped(&call, evaluator.ir().def(def).body, &St::constant())
}

#[test]
fn raw_nested_sets_have_the_same_membership_as_canonical_sets() {
    let evaluator = evaluator();
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        for values in [
            vec![Value::Int(0), Value::FALSE],
            vec![Value::FALSE, Value::Int(0), Value::FALSE],
        ] {
            let raw = Value::set_raw(vec![Value::set_raw(values, ctx)], ctx);
            let canonical = Value::set_canonical(
                vec![
                    Value::set_canonical(vec![Value::Int(0), Value::FALSE], Ctx::dive())
                        .expect("inner canonical set"),
                ],
                Ctx::dive(),
            )
            .expect("canonical set");
            for input in [raw, canonical] {
                for (name, expected) in [("P", Value::TRUE), ("N", Value::FALSE)] {
                    assert_eq!(
                        evaluate(&evaluator, name, input.clone()).expect("mathematical membership"),
                        expected,
                        "{name}/{ctx:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn singleton_union_canonicalizes_raw_finite_operands() {
    let evaluator = evaluator();
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        for values in [
            vec![Value::Int(0), Value::FALSE, Value::FALSE],
            vec![Value::FALSE, Value::Int(0)],
        ] {
            let input = Value::set_raw(values, ctx);
            for (name, expected) in [
                ("UC", Value::Int(2)),
                ("Two", Value::TRUE),
                ("One", Value::FALSE),
            ] {
                assert_eq!(
                    evaluate(&evaluator, name, input.clone()).expect("finite union"),
                    expected,
                    "{name}/{ctx:?}"
                );
            }
        }
    }
}

#[test]
fn singleton_union_does_not_bypass_unknown_child_identity() {
    let evaluator = evaluator();
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let empty = Value::set_raw(vec![], ctx);
        let input = Value::set_raw(
            vec![
                Value::seq_set(empty, ctx),
                Value::set_raw(vec![Value::tuple(vec![])], ctx),
            ],
            ctx,
        );
        let error = evaluate(&evaluator, "UC", input).expect_err("unproved symbolic identity");
        assert_eq!(error.class(), dive_eval::ErrorClass::Unsupported);
    }
}
