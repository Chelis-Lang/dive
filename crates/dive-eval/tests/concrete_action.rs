//! Compare concrete-action shortcuts with ordinary evaluation of the same IR.
#![allow(clippy::expect_used, reason = "test inputs and exact comparisons")]

use std::sync::Arc;

use dive_eval::{ConstBinding, Evaluator, Model, Options, St};
use dive_value::Value;

const SOURCE: &str = r"---- MODULE ConcreteAction ----
EXTENDS Naturals
CONSTANT Owners
VARIABLE x
Update(i) == x' = [x EXCEPT ![i] = 1]
Different(i) == x' # [x EXCEPT ![i] = 1]
NoTwo == \A j \in Owners : 2 # x[j]
SomeOne == \E j \in Owners : x[j] = 1
Step == \E i \in Owners : NoTwo /\ Update(i)
Opposite == \A i \in Owners : Different(i) /\ ~SomeOne
Missing == x' = [x EXCEPT ![100] = 1]
Two == x' = [x EXCEPT ![1] = 1, ![2] = 1]
Dependent == \E i \in Owners : x' = [x EXCEPT ![i] = @ + 1]
====";

#[test]
fn complete_partial_and_mixed_tables_match_ordinary_predicates() {
    let analysis =
        dive_sema::analyze_source("ConcreteAction.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    for options in [Options::tlc(), Options::dive()] {
        let ctx = options.ctx;
        let mut model = Model::new(ir.clone());
        let keys = (0..40).map(Value::Int).collect();
        model.constants[0] = Some(ConstBinding::Value(Value::set(keys, ctx).expect("owners")));
        let evaluator = Evaluator::new(model, options).expect("evaluator");
        let table = |changes: &[(i64, Value)], len: i64| {
            let pairs = (0..len)
                .map(|index| {
                    let value = changes
                        .iter()
                        .find(|(key, _)| *key == index)
                        .map_or(Value::Int(0), |(_, value)| value.clone());
                    (Value::Int(index), value)
                })
                .collect();
            Arc::from([Some(Value::fcn(pairs, ctx).expect("table"))])
        };
        for (source, target) in [
            (table(&[], 40), table(&[(39, Value::Int(1))], 40)),
            (
                table(&[], 40),
                table(&[(1, Value::Int(1)), (39, Value::Int(1))], 40),
            ),
            (table(&[], 40), table(&[], 40)),
            (
                table(&[(39, Value::Int(1))], 40),
                table(&[(39, Value::Int(1))], 40),
            ),
            (table(&[(39, Value::Bool(false))], 40), table(&[], 40)),
            (table(&[], 40), table(&[(39, Value::Bool(true))], 40)),
            (table(&[(1, Value::Int(2))], 39), table(&[], 39)),
            (table(&[], 38), table(&[], 39)),
        ] {
            let action = St::action(source, target);
            for name in ["Step", "Opposite", "Missing", "Two", "Dependent"] {
                let def = ir.root_def(name).expect("definition");
                let expr = ir.def(def).body;
                let scope = evaluator.scope(def).expect("scope");
                let ordinary = evaluator.eval_in(def, expr, &action);
                let scoped = evaluator.eval_scoped(&scope, expr, &action);
                assert_eq!(
                    ordinary.map_err(|e| e.to_string()),
                    scoped.map_err(|e| e.to_string()),
                    "{name}"
                );
            }
        }
    }
}
