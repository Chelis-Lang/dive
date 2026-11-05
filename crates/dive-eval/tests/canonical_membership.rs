//! Mathematical membership must canonicalize explicit raw children before equality.
#![allow(clippy::expect_used, reason = "bounded semantic controls")]

use std::sync::Arc;

use dive_eval::{Evaluator, Model, Options, St};
use dive_value::{Ctx, Value};

#[test]
fn raw_nested_sets_have_the_same_membership_as_canonical_sets() {
    let analysis = dive_sema::analyze_source(
        "CanonicalMember.tla",
        r"---- MODULE CanonicalMember ----
P(S) == {0, FALSE} \in S
N(S) == {0, FALSE} \notin S
====
",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let evaluator = Evaluator::new(Model::new(ir), Options::dive()).expect("evaluator");
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
                    let def = evaluator.ir().root_def(name).expect("operator");
                    let scope = evaluator.scope(def).expect("scope");
                    let call = evaluator
                        .scope_call_values(&scope, def, vec![input.clone()])
                        .expect("parameter scope");
                    assert_eq!(
                        evaluator
                            .eval_scoped(&call, evaluator.ir().def(def).body, &St::constant())
                            .expect("mathematical membership"),
                        expected,
                        "{name}/{ctx:?}"
                    );
                }
            }
        }
    }
}
