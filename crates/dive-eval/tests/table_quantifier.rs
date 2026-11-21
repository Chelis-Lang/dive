//! Pair quantification must preserve the ordinary evaluator's short circuit.
#![allow(clippy::expect_used, reason = "test fixtures and exact assertions")]

use std::sync::Arc;

use dive_eval::{ConstBinding, Evaluator, Model, Options, St};
use dive_value::{Interner, Value};

const SOURCE: &str = r#"---- MODULE TablePairs ----
CONSTANT Owners
VARIABLE state
Conflict == \E a,b \in Owners :
    /\ state[a] = "red"
    /\ "blue" = state[b]
Safe == \A a,b \in Owners : ~
    /\ state[a] = "red"
    /\ "blue" = state[b]
Same == \E a,b \in Owners : /\ state[a] = "red" /\ "blue" = state[a]
===="#;

#[test]
fn separate_witnesses_and_partial_table_fallbacks() {
    let analysis =
        dive_sema::analyze_source("TablePairs.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let atoms = Interner::new();
    let owners: Vec<_> = (0..40)
        .map(|i| Value::Model(atoms.intern(&format!("r{i}"))))
        .collect();
    for options in [Options::tlc(), Options::dive()] {
        let mut model = Model::new(ir.clone());
        model.constants[0] = Some(ConstBinding::Value(
            Value::set(owners.clone(), options.ctx).expect("owners"),
        ));
        let ctx = options.ctx;
        let ev = Evaluator::new(model, options).expect("evaluator");
        for (red, blue) in [(false, false), (true, false), (false, true), (true, true)] {
            let pairs = owners
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    let status = if red && i == 7 {
                        "red"
                    } else if blue && i == 34 {
                        "blue"
                    } else {
                        "green"
                    };
                    (k.clone(), Value::Str(atoms.intern(status)))
                })
                .collect();
            let st = St::state(Arc::from([Some(Value::fcn(pairs, ctx).expect("table"))]));
            for (name, expected) in [
                ("Conflict", red && blue),
                ("Safe", !(red && blue)),
                ("Same", false),
            ] {
                let def = ir.root_def(name).expect("definition");
                assert_eq!(
                    ev.eval_in(def, ir.def(def).body, &st)
                        .expect("total predicate"),
                    Value::Bool(expected)
                );
            }
        }
        let table = Value::fcn(
            owners
                .iter()
                .skip(1)
                .map(|k| (k.clone(), Value::Str(atoms.intern("green"))))
                .collect(),
            ctx,
        )
        .expect("partial table");
        let st = St::state(Arc::from([Some(table)]));
        let def = ir.root_def("Safe").expect("Safe");
        assert!(ev.eval_in(def, ir.def(def).body, &st).is_err());
    }
}
