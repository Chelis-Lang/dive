//! Predicate analysis depends on syntax and captures, never domain cardinality.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_eval::{ErrorClass, St};
use dive_liveness::formula::{FormulaError, Limits};
use dive_liveness::plan::{PlanningError, lower};
use dive_model::{Model, Pred};
use dive_trace::temporal::evaluate_lasso;
use dive_value::Value;

fn model(definitions: &str, tlc: bool) -> Model {
    configured_model(definitions, "", tlc)
}

fn configured_model(definitions: &str, extra_config: &str, tlc: bool) -> Model {
    let source = format!(
        "---- MODULE BinderAnalysis ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\n{definitions}\n====\n"
    );
    let analysis = dive_sema::analyze_source(
        "BinderAnalysis.tla",
        &source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse(&format!("INIT Init\nNEXT Next\n{extra_config}"))
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &if tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::dive()
        },
    )
    .expect("binding")
}

fn predicate(model: &Model, name: &str) -> Pred {
    let def = model.ir.root_def(name).expect("definition");
    Pred::new(
        model.ir.def(def).body,
        model.evaluator.scope(def).expect("scope"),
    )
}

#[test]
fn large_constant_binders_keep_concrete_semantics_and_independent_replay() {
    let definitions = r"
Eq(a,b) == a=b
Pass(F(_,_),a,b) == F(a,b)
Q == \A i \in 1..5000 : Eq(i,i) /\ x
Dependent == \A i \in 1..5000 : \A j \in {i} : Pass(Eq,i,j) /\ x
Mixed == \A i \in 1..5000 : \A j \in (IF x THEN {i} ELSE {}) : j=i
Map == x /\ {i : i \in 1..5000} = 1..5000
Filter == x /\ {i \in 1..5000 : Eq(i,i)} = 1..5000
Function == x /\ [i \in 1..5000 |-> i][5000]=5000
P == []<>(Q /\ Dependent /\ Mixed /\ Map /\ Filter /\ Function)
";
    for tlc in [true, false] {
        let model = model(definitions, tlc);
        let pred = predicate(&model, "P");
        let plan = lower(&model, &pred, Limits::default()).expect("one analysis per binder");
        assert_eq!(plan.atoms.len(), 1);
        for truth in [true, false] {
            let state: dive_trace::State = Arc::from([Some(Value::Bool(truth))]);
            let row = plan
                .atoms
                .iter()
                .map(|atom| {
                    atom.evaluate(&model, &St::action(state.clone(), state.clone()))
                        .expect("concrete atom")
                })
                .collect::<Vec<_>>();
            assert_eq!(row, vec![truth]);
            assert_eq!(
                plan.formula
                    .evaluate_word(&[row], &[vec![]], 0, 100)
                    .expect("formula"),
                vec![truth],
            );
            assert_eq!(
                evaluate_lasso(&model, &pred, &[state], 0).expect("independent replay"),
                vec![truth]
            );
        }
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}

#[test]
fn static_admission_does_not_allocate_constant_domains() {
    for tlc in [true, false] {
        let model = model(
            r"
Huge == []<>(\A i \in 1..1000000000 : i>=1 /\ x)
HugeFunctions == []<>(\A f \in [1..32 -> BOOLEAN] : f[1] \/ x)
",
            tlc,
        );
        for name in ["Huge", "HugeFunctions"] {
            let plan = lower(&model, &predicate(&model, name), Limits::default()).expect(name);
            assert_eq!(plan.atoms.len(), 1);
        }
    }
}

#[test]
fn temporal_expansion_is_bounded_before_retaining_concrete_scopes() {
    for tlc in [true, false] {
        let model = model(r"P == \A i \in 1..1000000000 : []<>(x /\ i>=1)", tlc);
        assert!(matches!(
            lower(&model, &predicate(&model, "P"), Limits::default()),
            Err(PlanningError::Formula {
                error: FormulaError::Limit(_),
                ..
            }),
        ));
    }
}

#[test]
fn unknown_values_never_mask_effects_or_runtime_errors() {
    for tlc in [true, false] {
        let model = model(
            r#"
Effect(v) == Print(v,v)
Bad == []<>(\A i \in 1..5000 : Effect(i)=i /\ x)
EmptyEffect == []<>(\A i \in {} : Effect(i)=i /\ x)
Hidden(v) == TLCGet("level")=v
Introspection == []<>(\A i \in 1..5000 : Hidden(i) /\ x)
Error == []<>(\A i \in 1..5000 : (i \div 0)=1 /\ x)
"#,
            tlc,
        );
        for name in ["Bad", "EmptyEffect", "Introspection"] {
            assert!(
                lower(&model, &predicate(&model, name), Limits::default()).is_err(),
                "{name}"
            );
        }
        assert_eq!(model.take_output(), Vec::<String>::new());
        let pred = predicate(&model, "Error");
        let plan = lower(&model, &pred, Limits::default())
            .expect("runtime error is not evaluated during analysis");
        let state: dive_trace::State = Arc::from([Some(Value::Bool(true))]);
        let error = plan.atoms[0]
            .evaluate(&model, &St::action(state.clone(), state))
            .expect_err("concrete division by zero");
        assert_eq!(error.class(), ErrorClass::Eval);
        assert!(!error.is_analysis_only());
    }
}

#[test]
fn temporal_helpers_under_binders_keep_their_actual_formula_level() {
    for tlc in [true, false] {
        let model = model(
            r"
Temporal(v) == []<>(x /\ v=2)
P == \A i \in {1,2} : Temporal(i)
",
            tlc,
        );
        let pred = predicate(&model, "P");
        let plan = lower(&model, &pred, Limits::default()).expect("actual temporal expansion");
        assert_eq!(plan.atoms.len(), 2);
        let state: dive_trace::State = Arc::from([Some(Value::Bool(true))]);
        let rows = plan
            .atoms
            .iter()
            .map(|atom| {
                atom.evaluate(&model, &St::action(state.clone(), state.clone()))
                    .expect("concrete temporal binding")
            })
            .collect::<Vec<_>>();
        assert_eq!(rows, vec![false, true]);
        assert_eq!(
            plan.formula
                .evaluate_word(&[rows], &[vec![]], 0, 100)
                .expect("formula"),
            vec![false]
        );
        assert_eq!(
            evaluate_lasso(&model, &pred, &[state], 0).expect("independent temporal expansion"),
            vec![false]
        );
    }
}

#[test]
fn configured_and_substituted_helpers_retain_concrete_binder_contexts() {
    let definitions = r"
CONSTANT Op(_)
Plain(v) == v>=1 /\ x
---- MODULE BinderHelper ----
CONSTANT C
Same(v) == v=C
====
Unit == INSTANCE BinderHelper WITH C <- 1
Configured == []<>(\A i \in 1..5000 : Op(i))
Substituted == []<>(\A i \in 1..5000 : Unit!Same(i) \/ x)
";
    for tlc in [true, false] {
        let model = configured_model(definitions, "CONSTANT Op <- Plain\n", tlc);
        for name in ["Configured", "Substituted"] {
            let pred = predicate(&model, name);
            let plan = lower(&model, &pred, Limits::default()).expect(name);
            for truth in [true, false] {
                let state: dive_trace::State = Arc::from([Some(Value::Bool(truth))]);
                assert_eq!(
                    plan.atoms[0]
                        .evaluate(&model, &St::action(state.clone(), state.clone()))
                        .expect("concrete helper"),
                    truth
                );
                assert_eq!(
                    evaluate_lasso(&model, &pred, &[state], 0).expect("independent helper replay"),
                    vec![truth]
                );
            }
        }
    }
}

#[test]
fn configured_effects_cannot_be_hidden_by_constant_binder_analysis() {
    for tlc in [true, false] {
        let model = configured_model(
            r"
CONSTANT Op(_)
Effect(v) == Print(v,v)
P == []<>(\A i \in 1..5000 : Op(i)=i /\ x)
",
            "CONSTANT Op <- Effect\n",
            tlc,
        );
        assert!(lower(&model, &predicate(&model, "P"), Limits::default()).is_err());
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}

#[test]
fn repeated_fairness_keeps_each_concrete_scope_under_a_small_work_limit() {
    // Repeated state-dependent LET arguments under quantified fairness, as in
    // Kafka's KIP-996 model. The retained actions must still read each actual.
    let definitions = r"
Copy(v) == v /\ v
Test(i) == LET a == Copy(x = i) b == Copy(a) c == Copy(b) IN Copy(c)
Act(i) == Test(i) /\ x' = ~x
Fair == \A i \in BOOLEAN : WF_x(Act(i))
";
    for tlc in [true, false] {
        let model = model(definitions, tlc);
        let plan = lower(
            &model,
            &predicate(&model, "Fair"),
            Limits {
                expansion_steps: 500,
                ..Limits::default()
            },
        )
        .expect("repeated validated arguments");
        let edge = St::action(
            Arc::from([Some(Value::Bool(true))]),
            Arc::from([Some(Value::Bool(false))]),
        );
        let actions: Vec<_> = plan
            .atoms
            .iter()
            .filter(|atom| {
                matches!(
                    atom.test,
                    dive_liveness::plan::AtomTest::Nonstuttering { .. }
                )
            })
            .map(|atom| atom.evaluate(&model, &edge).expect("concrete edge"))
            .collect();
        assert_eq!(actions.len(), 2);
        assert_ne!(actions[0], actions[1]);
    }
}

#[test]
fn repeated_fairness_rechecks_an_unused_eager_actual_in_each_concrete_scope() {
    let definitions = r"
Keep(ignored) == TRUE
Guard(i) == Keep(1 \div (2-i))
Act(i) == Guard(i) /\ Next
First == WF_x(Act(1))
Both == First /\ WF_x(Act(2))
";
    for tlc in [true, false] {
        let model = model(definitions, tlc);
        lower(&model, &predicate(&model, "First"), Limits::default())
            .expect("defined first binding");
        let error = lower(&model, &predicate(&model, "Both"), Limits::default())
            .expect_err("zero denominator on second binding");
        assert!(
            error.to_string().contains("constant temporal argument"),
            "{error}"
        );
    }
}
