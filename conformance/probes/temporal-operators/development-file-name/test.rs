//! Pure user operator formals retain captured scopes without forcing callbacks.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded fixture assertions"
)]
use dive_liveness::analyze::Limits;
use dive_model::{BindOptions, Pred};
use std::sync::Arc;

const SOURCE: &str = r"---- MODULE Operators ----
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
P(v) == v
Temporal(v) == []<>v
Use(F(_)) == []<>F(x)
Direct(F(_)) == F(x)
Relay(G(_)) == Use(G)
Capture(saved) == LET Local(v) == v = saved IN Use(Local)
Live == Use(P)
T == Direct(Temporal)
R == Relay(P)
L == LET Local(v) == v IN Use(Local)
C == Capture(TRUE)
====
";
fn input(source: &str) -> Arc<dive_ir::Ir> {
    let a = dive_sema::analyze_source("Operators.tla", source, &Default::default());
    assert!(a.accepted(), "{a:?}");
    Arc::new(dive_ir::lower(&a).expect("lower"))
}

#[test]
fn retained_operator_formals_match_independent_words_and_closing_edges() {
    let ir = input(SOURCE);
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &config, &options).expect("model");
        for n in 1..=4 {
            for mask in 0_usize..(1 << n) {
                let values: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
                let states: Vec<dive_trace::State> = values
                    .iter()
                    .map(|b| Arc::from([Some((*b).into())]))
                    .collect();
                for back in 0..n {
                    let expected = vec![values[back..].iter().any(|b| *b); n];
                    for name in ["Live", "T", "R", "L", "C"] {
                        let d = ir.root_def(name).expect("def");
                        let pred =
                            Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
                        let actual =
                            dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                                .expect(name);
                        assert_eq!(actual, expected, "{name} {values:?} {back}");
                        let plan =
                            dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                                .expect(name);
                        let bins: Vec<Vec<bool>> = (0..n)
                            .map(|i| {
                                let st = dive_eval::St::action(
                                    states[i].clone(),
                                    states[if i + 1 == n { back } else { i + 1 }].clone(),
                                );
                                plan.atoms
                                    .iter()
                                    .map(|a| a.evaluate(&model, &st).expect("atom"))
                                    .collect()
                            })
                            .collect();
                        assert_eq!(
                            plan.formula
                                .evaluate_word(&bins, &bins, back, 1_000_000)
                                .expect("formula"),
                            actual
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn clean_paired_primary_models_have_final_outcomes_with_fresh_validation() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/temporal-operators");
    let selection: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("selection.json")).expect("selection"))
            .expect("json");
    for case in selection.as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let src =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.tla")))
                .expect("source");
        let ir = input(&src);
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
            .into_config()
            .expect("cfg");
        let result = dive_api::tlc_temporal::check_final(
            &ir,
            &cfg,
            &dive_check::CheckOptions::tlc(0, 0),
            Default::default(),
        );
        if name.contains("Effect") {
            let refusal = result.expect_err("effect refuses before host");
            assert_eq!(refusal.workers.as_ref().expect("workers").used, 0);
        } else {
            assert!(result.is_ok(), "{name}: {result:?}");
        }
    }
}
