//! Replay admits the bounded pure relation and retains composition refusals.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_eval::{ActionComposition, St};
use dive_model::{Model, Pred};
use dive_trace::enabled::{EnabledError, enabled};
use dive_trace::temporal::{TemporalError, evaluate_lasso};
use serde_json::json;

fn source(next: &str, extra: &str) -> String {
    format!(
        "---- MODULE Composition ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == {next}\n{extra}\n====\n"
    )
}

fn model(next: &str, extra: &str, mode: ActionComposition) -> Model {
    let src = source(next, extra);
    let analysis =
        dive_sema::analyze_source("Composition.tla", &src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let mut opts = dive_model::BindOptions::tlc();
    opts.eval.action_composition = mode;
    dive_model::bind(ir, &cfg, &opts).expect("model")
}

fn doc(x: i64) -> TraceDoc {
    TraceDoc::behavior(vec![json!({"x":0}), json!({"x":x})]).expect("document")
}

fn pred(m: &Model, name: &str) -> Pred {
    let d = m.ir.root_def(name).expect("definition");
    Pred::new(m.ir.def(d).body, m.evaluator.scope(d).expect("scope"))
}

#[test]
fn public_api_and_raw_replay_check_pure_composition_and_refuse_tlc_mode() {
    let src = source("(x'=1) \\cdot (x'=2)", "");
    let api = SpecModel::from_source(
        "Composition.tla",
        &src,
        "INIT Init\nNEXT Next\n",
        &TraceOptions::default(),
    )
    .expect("API");
    for x in [2, 9] {
        let validation = api.validate(&doc(x)).validation;
        if x == 2 {
            assert_eq!(validation, Validation::Valid);
        } else {
            assert!(matches!(validation, Validation::Invalid(_)));
        }
    }
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("(x'=1) \\cdot (x'=2)", "", mode);
        for x in [2, 9] {
            let trace = dive_trace::Trace::decode(&m, &doc(x)).expect("decode");
            let validation = dive_trace::validate(&m, &trace).validation;
            if mode == ActionComposition::Tlc {
                assert_eq!(
                    validation,
                    Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
                );
            } else if x == 2 {
                assert_eq!(validation, Validation::Valid);
            } else {
                assert!(matches!(validation, Validation::Invalid(_)));
            }
        }
        // The ordinary evaluator is still permitted after the replay host drops.
        let d = m.ir.root_def("Next").expect("Next");
        for (x, expected) in [(2, true), (9, false)] {
            assert_eq!(
                m.evaluator
                    .eval_def(
                        d,
                        &St::action(Arc::from([Some(0.into())]), Arc::from([Some(x.into())]))
                    )
                    .expect("ordinary evaluation"),
                dive_value::Value::Bool(expected)
            );
        }
    }
}

#[test]
fn refused_composition_runs_no_operand_print_or_random_callback() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model(
            "(PrintT(TRUE) /\\ x'=RandomElement({1})) \\cdot (x'=2)",
            "",
            mode,
        );
        assert_eq!(
            dive_trace::validate_doc(&m, &doc(2), &[]).0.validation,
            Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
        );
        assert!(!m.evaluator.has_print_event());
        assert_eq!(m.evaluator.take_output(), [] as [String; 0]);
    }
}

#[test]
fn optional_labels_check_pure_composition_and_remember_tlc_refusal() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("x'=2", "Alias == (x'=1) \\cdot (x'=2)", mode);
        let mut document = doc(2);
        document.labels[1] = Some(dive_trace::Label {
            name: "Alias".to_owned(),
            location: None,
        });
        assert_eq!(
            dive_trace::validate_doc(&m, &document, &[]).0.validation,
            if mode == ActionComposition::Tla {
                Validation::Valid
            } else {
                Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
            }
        );
    }
}

#[test]
fn untaken_composition_and_earlier_invalid_init_keep_real_results() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("IF TRUE THEN x'=2 ELSE ((x'=1) \\cdot (x'=2))", "", mode);
        assert_eq!(
            dive_trace::validate_doc(&m, &doc(2), &[]).0.validation,
            Validation::Valid
        );
        assert!(matches!(
            dive_trace::validate_doc(&m, &doc(9), &[]).0.validation,
            Validation::Invalid(_)
        ));
        let m = model("(x'=1) \\cdot (x'=2)", "", mode);
        let wrong_init =
            TraceDoc::behavior(vec![json!({"x":7}), json!({"x":2})]).expect("document");
        assert!(matches!(
            dive_trace::validate_doc(&m, &wrong_init, &[]).0.validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn public_enabled_and_lasso_predicates_share_the_bounded_relation() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model(
            "x'=x",
            "A == (x'=1) \\cdot (x'=2)\nP == [][A]_x\nE == []<>(ENABLED A)\nF == WF_x(A)",
            mode,
        );
        let state: dive_trace::State = Arc::from([Some(0.into())]);
        // Public queries and lasso predicates use the same relation and
        // preserve the alternate-mode refusal.
        assert_eq!(
            enabled(&m, &pred(&m, "A"), None, &state),
            if mode == ActionComposition::Tla {
                Ok(true)
            } else {
                Err(EnabledError::Unsupported(
                    dive_trace::COMPOSITION_UNSUPPORTED.to_owned(),
                ))
            },
            "{mode:?}"
        );
        for property in ["P", "E", "F"] {
            let actual = evaluate_lasso(&m, &pred(&m, property), std::slice::from_ref(&state), 0);
            if mode == ActionComposition::Tla {
                assert_eq!(actual, Ok(vec![property != "F"]), "{property}");
            } else {
                assert!(
                    matches!(actual, Err(TemporalError::Unsupported(ref feature)) if feature == dive_trace::COMPOSITION_UNSUPPORTED),
                    "{mode:?} {property}"
                );
            }
        }
        assert!(!m.evaluator.has_print_event());
        // The refusal does not leak: the ordinary evaluator still composes.
        let next = m.ir.root_def("A").expect("A");
        assert_eq!(
            m.evaluator
                .eval_def(
                    next,
                    &St::action(state.clone(), Arc::from([Some(2.into())]))
                )
                .expect("ordinary evaluation"),
            dive_value::Value::TRUE
        );
    }
}

/// A retained pure composition argument is decided cold and warm without
/// reading a value cached outside the validator (ADR 0103).
#[test]
fn public_enabled_does_not_depend_on_cache_warmth() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        for warm in [false, true] {
            let m = model("x'=2", "W(p) == p\nCaller == W((x'=1) \\cdot (x'=2))", mode);
            let caller = m.ir.root_def("Caller").expect("caller");
            let wrapped = m.ir.root_def("W").expect("wrapper");
            let dive_ir::Expr::Apply { args, .. } = &m.ir.node(m.ir.def(caller).body).expr else {
                unreachable!("fixture is a call");
            };
            let scope = m
                .evaluator
                .scope_apply(
                    &m.evaluator.scope(caller).expect("caller scope"),
                    wrapped,
                    args,
                )
                .expect("retained scope");
            let p = Pred::new(m.ir.def(wrapped).body, scope);
            let state: dive_trace::State = Arc::from([Some(0.into())]);
            if warm {
                let st = St::action(state.clone(), Arc::from([Some(2.into())]));
                assert_eq!(
                    m.evaluator
                        .eval_scoped(&p.scope, p.expr, &st)
                        .expect("warm"),
                    dive_value::Value::Bool(true)
                );
            }
            assert_eq!(
                enabled(&m, &p, None, &state),
                if mode == ActionComposition::Tla {
                    Ok(true)
                } else {
                    Err(EnabledError::Unsupported(
                        dive_trace::COMPOSITION_UNSUPPORTED.to_owned(),
                    ))
                },
                "{mode:?} warm={warm}"
            );
        }
    }
}

#[test]
fn warmed_same_owned_scopes_do_not_supply_replay_truth() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        for warm in [false, true] {
            for x in [2, 9] {
                for (wrapper, argument, admitted) in [
                    ("p", "(x'=1) \\cdot (x'=2)", true),
                    ("p[0]", "[n \\in {0} |-> ((x'=1) \\cdot (x'=2))]", true),
                    (
                        "p = [n \\in {0} |-> TRUE]",
                        "[n \\in {0} |-> ((x'=1) \\cdot (x'=2))]",
                        true,
                    ),
                    ("p", "(x'=1) \\cdot (x' \\in {2,3})", false),
                    (
                        "p[0]",
                        "[n \\in {0} |-> ((x'=1) \\cdot (x' \\in {2,3}))]",
                        false,
                    ),
                    (
                        "p = [n \\in {0} |-> TRUE]",
                        "[n \\in {0} |-> ((x'=1) \\cdot (x' \\in {2,3}))]",
                        false,
                    ),
                ] {
                    let extra = format!("W(p) == {wrapper}\nCaller == W({argument})");
                    let mut m = model("x'=2", &extra, mode);
                    let caller = m.ir.root_def("Caller").expect("caller");
                    let wrapped = m.ir.root_def("W").expect("wrapper");
                    let dive_ir::Expr::Apply { args, .. } = &m.ir.node(m.ir.def(caller).body).expr
                    else {
                        unreachable!("fixture is a call");
                    };
                    let scope = m
                        .evaluator
                        .scope_apply(
                            &m.evaluator.scope(caller).expect("caller scope"),
                            wrapped,
                            args,
                        )
                        .expect("retained scope");
                    let p = Pred::new(m.ir.def(wrapped).body, scope);
                    let st = St::action(Arc::from([Some(0.into())]), Arc::from([Some(x.into())]));
                    if warm {
                        assert_eq!(
                            m.evaluator
                                .eval_scoped(&p.scope, p.expr, &st)
                                .expect("warm actual expression"),
                            dive_value::Value::Bool(x == 2)
                        );
                    }
                    m.next = Some(p.clone());
                    m.validate_context().expect("same-owned valid model");
                    let validation = dive_trace::validate_doc(&m, &doc(x), &[]).0.validation;
                    if mode == ActionComposition::Tlc || !admitted {
                        assert_eq!(
                            validation,
                            Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
                        );
                    } else if x == 2 {
                        assert_eq!(validation, Validation::Valid);
                    } else {
                        assert!(matches!(validation, Validation::Invalid(_)));
                    }
                    if warm {
                        assert_eq!(
                            m.evaluator
                                .eval_scoped(&p.scope, p.expr, &st)
                                .expect("ordinary scope remains usable"),
                            dive_value::Value::Bool(x == 2)
                        );
                    }
                }
            }
        }
    }
}

/// Review probe for ADR 0103: replay reuses only lazy values it computed
/// itself, so a verdict never depends on how warm the evaluator's caches were.
/// `W(p) == p` retains `Caller`'s composition argument; one ordinary
/// `eval_scoped` forces it to `TRUE` for `<<x=0, x=2>>`.
#[test]
fn replay_verdict_does_not_depend_on_cache_warmth() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let verdicts = [false, true].map(|warm| {
            let mut m = model("x'=2", "W(p) == p\nCaller == W((x'=1) \\cdot (x'=2))", mode);
            let caller = m.ir.root_def("Caller").expect("caller");
            let wrapped = m.ir.root_def("W").expect("wrapper");
            let dive_ir::Expr::Apply { args, .. } = &m.ir.node(m.ir.def(caller).body).expr else {
                unreachable!("fixture is a call");
            };
            let scope = m
                .evaluator
                .scope_apply(
                    &m.evaluator.scope(caller).expect("caller scope"),
                    wrapped,
                    args,
                )
                .expect("retained scope");
            let p = Pred::new(m.ir.def(wrapped).body, scope);
            if warm {
                let st = St::action(Arc::from([Some(0.into())]), Arc::from([Some(2.into())]));
                assert_eq!(
                    m.evaluator
                        .eval_scoped(&p.scope, p.expr, &st)
                        .expect("warm"),
                    dive_value::Value::Bool(true)
                );
            }
            m.next = Some(p);
            m.validate_context().expect("same-owned valid model");
            // A second run must not reuse the first run's values either.
            let first = dive_trace::validate_doc(&m, &doc(2), &[]).0.validation;
            let second = dive_trace::validate_doc(&m, &doc(2), &[]).0.validation;
            assert_eq!(first, second, "warm={warm}");
            first
        });
        assert_eq!(
            verdicts[0], verdicts[1],
            "{mode:?}: cold and warm replay disagree"
        );
        assert_eq!(
            verdicts[0],
            if mode == ActionComposition::Tla {
                Validation::Valid
            } else {
                Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
            }
        );
    }
}

#[test]
fn pure_warmed_arguments_remain_decidable() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        for x in [2, 9] {
            let mut m = model(
                "x'=2",
                "W(p) == p\nCaller == W(IF TRUE THEN x'=2 ELSE ((x'=1) \\cdot (x'=2)))",
                mode,
            );
            let caller = m.ir.root_def("Caller").expect("caller");
            let wrapped = m.ir.root_def("W").expect("wrapper");
            let dive_ir::Expr::Apply { args, .. } = &m.ir.node(m.ir.def(caller).body).expr else {
                unreachable!("fixture is a call");
            };
            let scope = m
                .evaluator
                .scope_apply(&m.evaluator.scope(caller).expect("scope"), wrapped, args)
                .expect("retained scope");
            let p = Pred::new(m.ir.def(wrapped).body, scope);
            let st = St::action(Arc::from([Some(0.into())]), Arc::from([Some(x.into())]));
            assert_eq!(
                m.evaluator
                    .eval_scoped(&p.scope, p.expr, &st)
                    .expect("warm pure argument"),
                dive_value::Value::Bool(x == 2)
            );
            m.next = Some(p);
            let result = dive_trace::validate_doc(&m, &doc(x), &[]).0.validation;
            if x == 2 {
                assert_eq!(result, Validation::Valid);
            } else {
                assert!(matches!(result, Validation::Invalid(_)));
            }
        }
    }
}

#[test]
fn composed_closing_edges_are_checked_independently_of_the_forward_prefix() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let with_property = |next: &str| {
            let src = source(next, "P == <>(x=2)");
            let analysis =
                dive_sema::analyze_source("Composition.tla", &src, &dive_sema::Options::default());
            assert!(analysis.accepted());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
                .into_config()
                .expect("configuration");
            let mut options = dive_model::BindOptions::tlc();
            options.eval.action_composition = mode;
            dive_model::bind(ir, &cfg, &options).expect("model")
        };
        for warm in [false, true] {
            let m = with_property("IF x=0 THEN x'=1 ELSE ((x'=2) \\cdot (x'=0))");
            if warm {
                let next = m.ir.root_def("Next").expect("Next");
                assert_eq!(
                    m.evaluator
                        .eval_def(
                            next,
                            &St::action(Arc::from([Some(1.into())]), Arc::from([Some(0.into())]))
                        )
                        .expect("ordinary composed closing edge"),
                    dive_value::Value::TRUE
                );
            }
            let prefix = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])
                .expect("actual pure forward prefix");
            assert_eq!(
                dive_trace::validate_doc(&m, &prefix, &[]).0.validation,
                Validation::Valid
            );
            let mut document = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])
                .expect("word")
                .with_ending(dive_trace::Ending::Lasso { back_to: 0 });
            document.properties = vec!["P".into()];
            assert_eq!(
                dive_trace::validate_doc(&m, &document, &[]).0.validation,
                if mode == ActionComposition::Tla {
                    Validation::Valid
                } else {
                    Validation::Unsupported(format!(
                        "unsupported temporal validation: {}",
                        dive_trace::COMPOSITION_UNSUPPORTED
                    ))
                }
            );
            assert_eq!(m.evaluator.take_output(), [] as [String; 0]);
            let pure = with_property("IF x=0 THEN x'=1 ELSE x'=0");
            assert_eq!(
                dive_trace::validate_doc(&pure, &document, &[]).0.validation,
                Validation::Valid
            );
            let mut forged = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":9})])
                .expect("forged word")
                .with_ending(dive_trace::Ending::Lasso { back_to: 0 });
            forged.properties = vec!["P".into()];
            assert!(matches!(
                dive_trace::validate_doc(&pure, &forged, &[]).0.validation,
                Validation::Invalid(_)
            ));
        }
    }
}
