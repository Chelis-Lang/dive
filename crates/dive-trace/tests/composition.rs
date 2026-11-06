//! Standalone replay checks bounded composition and remembers refused optional labels.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;

use dive_eval::ActionComposition;
use dive_trace::{COMPOSITION_UNSUPPORTED, Label, TraceDoc, Validation};
use serde_json::json;

#[test]
fn optional_label_checks_pure_composition_and_remembers_other_modes() {
    let source = "---- MODULE Composition ----\nVARIABLE x\nInit == x=0\nNext == x'=2\nAlias == (x'=1) \\cdot (x'=2)\nOpposite == (x'=1) \\cdot (x'=3)\n====\n";
    let analysis =
        dive_sema::analyze_source("Composition.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let mut opts = dive_model::BindOptions::tlc();
        opts.eval.action_composition = mode;
        let m = dive_model::bind(ir.clone(), &cfg, &opts).expect("model");
        let mut doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})]).expect("document");
        assert_eq!(
            dive_trace::validate_doc(&m, &doc, &[]).0.validation,
            Validation::Valid
        );
        doc.labels[1] = Some(Label {
            name: "Alias".to_owned(),
            location: None,
        });
        assert_eq!(
            dive_trace::validate_doc(&m, &doc, &[]).0.validation,
            if mode == ActionComposition::Tla {
                Validation::Valid
            } else {
                Validation::Unsupported(COMPOSITION_UNSUPPORTED.to_owned())
            }
        );
        doc.labels[1].as_mut().expect("label").name = "Opposite".into();
        let opposite = dive_trace::validate_doc(&m, &doc, &[]).0.validation;
        if mode == ActionComposition::Tla {
            assert!(
                matches!(opposite, Validation::Invalid(ref failure) if failure.check == dive_trace::Check::Label)
            );
        } else {
            assert_eq!(
                opposite,
                Validation::Unsupported(COMPOSITION_UNSUPPORTED.into())
            );
        }
    }
}

fn bound(source: &str, config: &str, mode: ActionComposition) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("module name");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(config).into_config().expect("config");
    let mut options = dive_model::BindOptions::dive();
    options.eval.action_composition = mode;
    dive_model::bind(ir, &config, &options).expect("model")
}

fn predicate(model: &dive_model::Model, name: &str) -> dive_model::Pred {
    let definition = model.ir.root_def(name).expect("definition");
    dive_model::Pred::new(
        model.ir.def(definition).body,
        model.evaluator.scope(definition).expect("scope"),
    )
}

fn state(x: i64) -> dive_trace::State {
    Arc::from([Some(x.into())])
}

#[test]
fn finite_relation_checks_actual_opposite_and_forged_words() {
    let source = include_str!("fixtures/composition/Reduction.tla");
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/composition/oracle.json")).expect("oracle");
    assert_eq!(oracle["oracle"], "tlc-2026.10.06.014338");
    assert_eq!(oracle["generated"], 13);
    assert_eq!(oracle["distinct"], 3);
    assert_eq!(oracle["exit"], 0);
    let model = bound(
        source,
        include_str!("fixtures/composition/Reduction.cfg"),
        ActionComposition::Tla,
    );
    // Brute-force First(<<0,0>>,t) and Reduction(t,u) independently.
    for x in 0..=3 {
        for y in 0..=5 {
            let relation = (0..=3).any(|tx| {
                (0..=3).any(|ty| {
                    let m = tx;
                    [1, 2].contains(&tx) && ty == tx && x == tx - m && y == ty + m
                })
            });
            let doc =
                TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":x,"y":y})]).expect("word");
            let validation = dive_trace::validate_doc(&model, &doc, &[]).0.validation;
            if relation {
                assert_eq!(validation, Validation::Valid, "{x},{y}");
            } else {
                assert!(
                    matches!(validation, Validation::Invalid(_)),
                    "{x},{y}: {validation:?}"
                );
            }
        }
    }
    let model = bound(
        source,
        "INIT Init\nNEXT Next\nINVARIANTS Inv Bad\n",
        ActionComposition::Tla,
    );
    for (y, valid) in [(4, true), (2, false), (99, false)] {
        let word = TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":0,"y":y})])
            .expect("word")
            .with_ending(dive_trace::Ending::Invariant("Bad".into()));
        let validation = dive_trace::validate_doc(&model, &word, &[]).0.validation;
        if valid {
            assert_eq!(validation, Validation::Valid);
        } else {
            assert!(
                matches!(validation, Validation::Invalid(_)),
                "{validation:?}"
            );
        }
    }
    let wrong_init =
        TraceDoc::behavior(vec![json!({"x":0,"y":2}), json!({"x":0,"y":4})]).expect("word");
    assert!(
        matches!(dive_trace::validate_doc(&model,&wrong_init,&[]).0.validation,Validation::Invalid(ref failure) if failure.check==dive_trace::Check::Init)
    );
}

#[test]
fn enabled_and_fairness_use_the_original_to_final_edge() {
    let source = include_str!("fixtures/composition/ReplayComposition.tla");
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/composition/oracle.json")).expect("oracle");
    assert_eq!(oracle["fairness"]["facts"]["nonstuttering_enabled"], true);
    assert_eq!(
        oracle["fairness"]["facts"]["expanded_nonstuttering_enabled"],
        false
    );
    assert_eq!(oracle["fairness"]["weak"]["violated"], "Weak");
    assert_eq!(oracle["fairness"]["strong"]["violated"], "Strong");
    let facts = bound(
        source,
        include_str!("fixtures/composition/facts.cfg"),
        ActionComposition::Tla,
    );
    let initial = TraceDoc::behavior(vec![json!({"x":0})])
        .expect("word")
        .with_ending(dive_trace::Ending::Invariant("OracleFacts".into()));
    assert_eq!(
        dive_trace::validate_doc(&facts, &initial, &[]).0.validation,
        Validation::Valid
    );
    let model = bound(source, "INIT Init\nNEXT Next\n", ActionComposition::Tla);
    let a = predicate(&model, "A");
    let next = model.ir.root_def("Next").expect("Next");
    let dive_ir::Expr::Builtin { args, .. } = model.ir.expr(model.ir.def(next).body) else {
        unreachable!("equality");
    };
    let sub = args[1];
    for current in 0..=2 {
        let successors: Vec<i64> = (0..=2)
            .filter(|final_state| {
                (0..=2).any(|intermediate| intermediate == 1 && *final_state == 0)
            })
            .collect();
        let relation = !successors.is_empty();
        let nonstuttering = successors.iter().any(|final_state| *final_state != current);
        assert_eq!(
            dive_trace::enabled::enabled(&model, &a, None, &state(current)),
            Ok(relation)
        );
        assert_eq!(
            dive_trace::enabled::enabled(&model, &a, Some(sub), &state(current)),
            Ok(nonstuttering)
        );
        for fairness in ["Weak", "Strong"] {
            assert_eq!(
                dive_trace::temporal::evaluate_lasso(
                    &model,
                    &predicate(&model, fairness),
                    &[state(current)],
                    0
                ),
                Ok(vec![!nonstuttering]),
                "{fairness} current={current}"
            );
        }
    }
}

#[test]
fn claimed_fairness_violations_require_the_actual_mathematical_truth() {
    for (property, config) in [
        ("Weak", include_str!("fixtures/composition/weak.cfg")),
        ("Strong", include_str!("fixtures/composition/strong.cfg")),
    ] {
        let source = include_str!("fixtures/composition/ReplayComposition.tla");
        let model = bound(source, config, ActionComposition::Tla);
        let mut word = TraceDoc::behavior(vec![json!({"x":0})])
            .expect("word")
            .with_ending(dive_trace::Ending::Stuttering);
        word.properties = vec![property.into()];
        assert!(matches!(
            dive_trace::validate_doc(&model, &word, &[]).0.validation,
            Validation::Invalid(_)
        ));
        let genuinely_false = source.replace("(x' = 0)", "(x' = 2)");
        let model = bound(&genuinely_false, config, ActionComposition::Tla);
        assert_eq!(
            dive_trace::validate_doc(&model, &word, &[]).0.validation,
            Validation::Valid
        );
    }
}

#[test]
fn deadlock_and_closing_edges_are_independently_checked() {
    let source = "---- MODULE Dead ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == (x'=1) \\cdot (x<1 /\\ x'=2)\n====\n";
    let model = bound(source, "INIT Init\nNEXT Next\n", ActionComposition::Tla);
    assert_eq!(
        dive_trace::enabled::enabled(&model, model.next.as_ref().expect("next"), None, &state(0)),
        Ok(false)
    );
    let word = TraceDoc::behavior(vec![json!({"x":0})])
        .expect("word")
        .with_ending(dive_trace::Ending::Deadlock);
    assert_eq!(
        dive_trace::validate_doc(&model, &word, &[]).0.validation,
        Validation::Valid
    );
    let source = "---- MODULE Loop ----\nVARIABLE x\nInit == x=0\nNext == IF x=0 THEN x'=1 ELSE ((x'=2) \\cdot (x'=0))\nP == <>(x=2)\n====\n";
    let model = bound(
        source,
        "INIT Init\nNEXT Next\nPROPERTY P\n",
        ActionComposition::Tla,
    );
    let mut word = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])
        .expect("word")
        .with_ending(dive_trace::Ending::Lasso { back_to: 0 });
    word.properties = vec!["P".into()];
    assert_eq!(
        dive_trace::validate_doc(&model, &word, &[]).0.validation,
        Validation::Valid
    );
    let forged = bound(
        &source.replace("(x'=0)", "(x'=2)"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
        ActionComposition::Tla,
    );
    assert!(matches!(
        dive_trace::validate_doc(&forged, &word, &[]).0.validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn unproved_rows_modes_effects_and_labels_keep_named_refusals() {
    for tail in [
        "x' \\in {2,3}",
        "TRUE",
        "(x'=1) \\cdot (x'=2)",
        "PrintT(TRUE) /\\ x'=2",
        "x'=RandomElement({2})",
        "x'=TLCGet(1)",
    ] {
        let source = format!(
            "---- MODULE Refused ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == (x'=1) \\cdot ({tail})\n====\n"
        );
        let model = bound(&source, "INIT Init\nNEXT Next\n", ActionComposition::Tla);
        let word = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})]).expect("word");
        assert_eq!(
            dive_trace::validate_doc(&model, &word, &[]).0.validation,
            Validation::Unsupported(COMPOSITION_UNSUPPORTED.into()),
            "{tail}"
        );
        assert!(!model.evaluator.has_print_event());
    }
    let source = "---- MODULE Partial ----\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == (x'=1) \\cdot (x'=2 /\\ y'=0)\n====\n";
    let model = bound(source, "INIT Init\nNEXT Next\n", ActionComposition::Tla);
    let word = TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":2,"y":0})]).expect("word");
    assert_eq!(
        dive_trace::validate_doc(&model, &word, &[]).0.validation,
        Validation::Unsupported(COMPOSITION_UNSUPPORTED.into())
    );
    for mode in [ActionComposition::Off, ActionComposition::Tlc] {
        let model = bound(
            include_str!("fixtures/composition/Reduction.tla"),
            include_str!("fixtures/composition/Reduction.cfg"),
            mode,
        );
        let word =
            TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":0,"y":2})]).expect("word");
        assert_eq!(
            dive_trace::validate_doc(&model, &word, &[]).0.validation,
            Validation::Unsupported(COMPOSITION_UNSUPPORTED.into())
        );
    }
    let source = "---- MODULE Label ----\nVARIABLE x\nInit == x=0\nNext == x'=2\nAlias == (x'=1) \\cdot (x' \\in {2,3})\n====\n";
    let model = bound(source, "INIT Init\nNEXT Next\n", ActionComposition::Tla);
    let mut word = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})]).expect("word");
    word.labels[1] = Some(Label {
        name: "Alias".into(),
        location: None,
    });
    assert_eq!(
        dive_trace::validate_doc(&model, &word, &[]).0.validation,
        Validation::Unsupported(COMPOSITION_UNSUPPORTED.into())
    );
}

#[test]
fn outer_let_values_follow_each_intermediate_and_cache_domains_stay_private() {
    let source = "---- MODULE Scoped ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x=0 /\\ y=0\nState == x+1\nNext == \\E n \\in {1,2}: LET q==State IN (x'=n /\\ y'=q) \\cdot (x'=State /\\ y'=q)\n====\n";
    for warm in [false, true] {
        let model = bound(source, "INIT Init\nNEXT Next\n", ActionComposition::Tla);
        let next = model.ir.root_def("Next").expect("Next");
        if warm {
            assert_eq!(
                model
                    .evaluator
                    .eval_def(
                        next,
                        &dive_eval::St::action(
                            Arc::from([Some(0.into()), Some(0.into())]),
                            Arc::from([Some(2.into()), Some(2.into())]),
                        ),
                    )
                    .expect("ordinary warm evaluation"),
                dive_value::Value::TRUE
            );
        }
        for (x, y, valid) in [(2, 2, true), (3, 3, true), (2, 1, false), (3, 1, false)] {
            let word =
                TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":x,"y":y})]).expect("word");
            let first = dive_trace::validate_doc(&model, &word, &[]).0.validation;
            let second = dive_trace::validate_doc(&model, &word, &[]).0.validation;
            assert_eq!(first, second, "warm={warm}");
            if valid {
                assert_eq!(first, Validation::Valid, "{x},{y} warm={warm}");
            } else {
                assert!(
                    matches!(first, Validation::Invalid(_)),
                    "{x},{y} warm={warm}: {first:?}"
                );
            }
        }
    }
}

#[test]
fn existing_outer_host_composition_denials_are_preserved() {
    #[derive(Debug)]
    struct Denied;
    impl dive_eval::Host for Denied {
        fn check_action_composition(&self) -> Result<(), dive_eval::HostError> {
            Err(dive_eval::HostError {
                code: 0,
                message: "outer host denied composition".into(),
            })
        }
    }
    let model = bound(
        "---- MODULE Denied ----\nVARIABLE x\nInit==x=0\nNext==(x'=1) \\cdot (x'=2)\n====\n",
        "INIT Init\nNEXT Next\n",
        ActionComposition::Tla,
    );
    let host: Arc<dyn dive_eval::Host> = Arc::new(Denied);
    model.evaluator.set_host(host.clone());
    let word = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})]).expect("word");
    assert_eq!(
        dive_trace::validate_doc(&model, &word, &[]).0.validation,
        Validation::Unsupported(COMPOSITION_UNSUPPORTED.into())
    );
    assert!(Arc::ptr_eq(
        &model.evaluator.host().expect("restored host"),
        &host
    ));
    assert_eq!(
        dive_trace::enabled::enabled(&model, model.next.as_ref().expect("Next"), None, &state(0)),
        Err(dive_trace::enabled::EnabledError::Unsupported(
            COMPOSITION_UNSUPPORTED.into()
        ))
    );
}
