//! Direct configured calls retain lazy scope and independent word validation.
#![allow(clippy::expect_used, clippy::panic, reason = "conformance assertions")]

use std::{path::Path, sync::Arc};

use dive_api::temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_eval::{St, StateRef};
use dive_liveness::{analyze::Limits, formula::Limits as FormulaLimits, plan};
use dive_model::{BindOptions, Model, Pred};
use dive_trace::{Ending, Validation, temporal::evaluate_lasso};
use dive_value::Value;

fn bound(source: &str, config: &str, options: &CheckOptions) -> Model {
    let name = source.split_whitespace().nth(2).expect("module name");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{:?}", a.diagnostics());
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &dive_config::parse(config).into_config().expect("config"),
        &BindOptions {
            eval: options.eval,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}

fn pred(model: &Model, name: &str) -> Pred {
    let def = model.ir.root_def(name).expect("definition");
    Pred::new(
        model.ir.def(def).body,
        model.evaluator.scope(def).expect("scope"),
    )
}

fn state(x: i64) -> dive_trace::State {
    Arc::from([Some(Value::from(x))])
}

#[test]
fn configured_reply_matches_pinned_counts_and_rejects_forged_words() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for suffix in ["Holds", "Fails"] {
        let name = format!("ConfiguredReply{suffix}");
        let relative = format!("specifications/{name}/{name}");
        let base = root.join("conformance/corpus/configured-temporal-operators");
        let source = std::fs::read_to_string(base.join(format!("{relative}.tla"))).expect("source");
        let config = std::fs::read_to_string(base.join(format!("{relative}.cfg"))).expect("config");
        let golden: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "conformance/golden/tlc-2026.10.06.014338/configured-temporal-operators/{relative}.cfg.json"
            )))
            .expect("generated golden"),
        )
        .expect("golden JSON");
        assert_eq!(golden["oracle"], "tlc-2026.10.06.014338");
        assert_eq!(
            golden["result"]["exit_code"],
            if suffix == "Holds" { 0 } else { 13 }
        );
        for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let model = bound(&source, &config, &options);
            let cfg = dive_config::parse(&config).into_config().expect("config");
            let report =
                temporal::check_final(&model.ir, &cfg, &options, Limits::default()).expect(&name);
            let capture = match report.outcome() {
                Outcome::Holds(c) => {
                    assert_eq!(suffix, "Holds");
                    assert_eq!(golden["result"]["error_class"], "TLC_SUCCESS");
                    c.as_ref()
                }
                Outcome::Violation(v) => {
                    assert_eq!(suffix, "Fails");
                    assert_eq!(
                        golden["result"]["error_class"],
                        "TLC_TEMPORAL_PROPERTY_VIOLATED"
                    );
                    assert_eq!(golden["result"]["violated_property"], "Live");
                    assert_eq!(v.validation().validation, Validation::Valid);
                    assert_eq!(v.document().properties, ["Live"]);
                    let fresh = bound(&source, &config, &options);
                    let mut oracle_word =
                        dive_trace::TraceDoc::parse(&golden["result"]["trace"]["json"].to_string())
                            .expect("pinned TLC word")
                            .with_ending(Ending::Stuttering);
                    oracle_word.properties = vec!["Live".into()];
                    assert_eq!(
                        dive_trace::validate_doc(&fresh, &oracle_word, &cfg.model_values)
                            .0
                            .validation,
                        Validation::Valid
                    );
                    assert_eq!(
                        dive_trace::validate_doc(&fresh, v.document(), &cfg.model_values)
                            .0
                            .validation,
                        Validation::Valid
                    );
                    let mut forged = v.document().clone();
                    forged.properties = vec!["Unconfigured".into()];
                    assert!(matches!(
                        dive_trace::validate_doc(&fresh, &forged, &cfg.model_values)
                            .0
                            .validation,
                        Validation::Invalid(_)
                    ));
                    let mut forged = v.document().clone();
                    forged.ending = Some(Ending::Lasso {
                        back_to: forged.states.len(),
                    });
                    assert!(matches!(
                        dive_trace::validate_doc(&fresh, &forged, &cfg.model_values)
                            .0
                            .validation,
                        Validation::Invalid(_)
                    ));
                    &v.candidate().exploration
                }
                Outcome::Safety(s) => panic!("unexpected safety result: {:?}", s.verdict),
            };
            let counts = &golden["result"]["counts"];
            assert_eq!(
                capture.stats.generated,
                counts["states_generated"].as_u64().expect("generated")
            );
            assert_eq!(
                capture.stats.distinct,
                counts["distinct_states"].as_u64().expect("distinct")
            );
            assert_eq!(
                capture.stats.depth,
                counts["depth"].as_u64().expect("depth")
            );
            let fair = &model.temporals[0].pred;
            assert!(evaluate_lasso(&model, fair, &[state(0), state(1)], 1).expect("fair word")[0]);
            assert!(!evaluate_lasso(&model, fair, &[state(0)], 0).expect("unfair stutter")[0]);
        }
    }
}

#[test]
fn same_expression_in_distinct_lazy_scopes_and_let_actuals_remains_distinct() {
    let source = "---- MODULE ConfiguredScopes ----\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nIdentity(v)==v\nWrap(v)==Op(v)\nP==[]<>Wrap(x=0) /\\ []<>Wrap(x=1)\nLetP==[]<>(LET v==x IN Op(v=0))\n====\n";
    let config = "INIT Init\nNEXT Next\nCONSTANT Op <- Identity\n";
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(source, config, &options);
        let p = plan::lower(&model, &pred(&model, "P"), FormulaLimits::default()).expect("plan");
        assert_eq!(p.atoms.len(), 2);
        assert_eq!(p.atoms[0].pred.expr, p.atoms[1].pred.expr);
        let st = St::state(state(0));
        assert!(p.atoms[0].evaluate(&model, &st).expect("first scope"));
        assert!(!p.atoms[1].evaluate(&model, &st).expect("different scope"));
        let p =
            plan::lower(&model, &pred(&model, "LetP"), FormulaLimits::default()).expect("LET plan");
        assert!(p.atoms[0].evaluate(&model, &st).expect("captured zero"));
        assert!(
            !p.atoms[0]
                .evaluate(&model, &St::state(state(1)))
                .expect("captured one")
        );
    }
}

#[test]
fn unused_actual_binder_domains_are_inspected_without_evaluation() {
    let source = "---- MODULE ConfiguredUnused ----\nEXTENDS Naturals\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nIgnore(v)==TRUE\nCall==Op(\\A i \\in (1 \\div 0): i=0)\nP==<>Call\n====\n";
    let config = "INIT Init\nNEXT Next\nCONSTANT Op <- Ignore\nPROPERTY P\n";
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(source, config, &options);
        let p = pred(&model, "P");
        plan::lower(&model, &p, FormulaLimits::default()).expect("unused domain is not forced");
        let call = pred(&model, "Call");
        model
            .evaluator
            .check_finite_bindings(&call.scope, call.expr, &St::state(state(0)), 4096)
            .expect("unused finite guard domain is not forced");
        assert_eq!(
            model
                .evaluator
                .eval_scoped_pure(&call.scope, call.expr, &St::state(state(0)), 4096)
                .expect("original unused call"),
            Value::Bool(true)
        );
        assert!(evaluate_lasso(&model, &p, &[state(0)], 0).expect("independent unused domain")[0]);
        let cfg = dive_config::parse(config).into_config().expect("config");
        assert!(matches!(
            temporal::check_final(&model.ir, &cfg, &options, Limits::default())
                .expect("unused domain")
                .outcome(),
            Outcome::Holds(_)
        ));
    }
}

#[test]
fn direct_configured_limits_and_original_evaluation_errors_remain_errors() {
    let source = "---- MODULE ConfiguredBounds ----\nEXTENDS Naturals\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nIdentity(v)==v\nP==<>(Op(x=0))\nBad==<>(Op(1 \\div 0=0))\n====\n";
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(
            source,
            "INIT Init\nNEXT Next\nCONSTANT Op <- Identity\n",
            &options,
        );
        let limits = dive_liveness::formula::Limits {
            expansion_steps: 1,
            ..FormulaLimits::default()
        };
        assert!(matches!(
            plan::lower(&model, &pred(&model, "P"), limits),
            Err(plan::PlanningError::Formula {
                error: dive_liveness::formula::FormulaError::Limit(_),
                ..
            })
        ));
        let failure = plan::lower(&model, &pred(&model, "Bad"), FormulaLimits::default())
            .expect_err("actual arithmetic error");
        let plan::PlanningError::Evaluation { error, .. } = failure else {
            panic!("typed original error: {failure:?}")
        };
        assert_eq!(error.class(), dive_eval::ErrorClass::Eval);
        assert_eq!(error.ec().code(), 2179);
        // The original invocation retains missing-state errors as well.
        let p = plan::lower(&model, &pred(&model, "P"), FormulaLimits::default()).expect("plan");
        assert!(matches!(St::constant().s0, StateRef::Empty));
        assert!(p.atoms[0].evaluate(&model, &St::constant()).is_err());
    }
}

#[test]
fn configured_native_captured_recursive_temporal_and_effectful_targets_stay_refused() {
    let prefix = "---- MODULE ConfiguredRefusals ----\nEXTENDS Naturals, Sequences, TLC\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\n";
    let cases = [
        ("", "Op(<<0>>)=1", "Len"),
        (
            "RECURSIVE Recur(_)\nRecur(v)==IF v=0 THEN TRUE ELSE Recur(v-1)\n",
            "Op(0)",
            "Recur",
        ),
        ("Timed(v)==<>v\n", "Op(x=0)", "Timed"),
        ("Effect(v)==PrintT(v)\n", "Op(x)", "Effect"),
        ("Enable(v)==ENABLED(x'=v)\n", "Op(x)", "Enable"),
        ("Ignore(v)==TRUE\n", "Op(PrintT(0))", "Ignore"),
        (
            "Identity(v)==v\nForward(H(_),v)==H(v)\n",
            "Forward(Op,x=0)",
            "Identity",
        ),
    ];
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for (definitions, call, target) in cases {
            let source = format!("{prefix}{definitions}P==<>({call})\n====\n");
            let config = format!("INIT Init\nNEXT Next\nCONSTANT Op <- {target}\n");
            let model = bound(&source, &config, &options);
            assert!(
                matches!(
                    plan::lower(&model, &pred(&model, "P"), FormulaLimits::default()),
                    Err(plan::PlanningError::Unsupported(_))
                ),
                "{target} {call}"
            );
            assert!(
                evaluate_lasso(&model, &pred(&model, "P"), &[state(0)], 0).is_err(),
                "independent {target} {call}"
            );
        }
    }
}

#[test]
fn configured_cycles_and_concrete_binders_keep_existing_resource_limits() {
    let prefix = "---- MODULE ConfiguredLimits ----\nEXTENDS Naturals\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\n";
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let source = format!("{prefix}Loop(v)==Op(v)\nP==<>Op(x=0)\n====\n");
        let model = bound(
            &source,
            "INIT Init\nNEXT Next\nCONSTANT Op <- Loop\n",
            &options,
        );
        assert!(matches!(
            plan::lower(&model, &pred(&model, "P"), FormulaLimits::default()),
            Err(plan::PlanningError::Formula {
                error: dive_liveness::formula::FormulaError::Limit(_),
                ..
            })
        ));
        assert!(evaluate_lasso(&model, &pred(&model, "P"), &[state(0)], 0).is_err());
        let source =
            format!("{prefix}Guarded(v)==\\A i \\in {{0,1}}:i=i\nCall==Op(x)\nP==<>Call\n====\n");
        let model = bound(
            &source,
            "INIT Init\nNEXT Next\nCONSTANT Op <- Guarded\n",
            &options,
        );
        let call = pred(&model, "Call");
        let failure = model
            .evaluator
            .check_finite_bindings(&call.scope, call.expr, &St::state(state(0)), 1)
            .expect_err("binder bound");
        assert_eq!(failure.class(), dive_eval::ErrorClass::ResourceLimit);
        model
            .evaluator
            .check_finite_bindings(&call.scope, call.expr, &St::state(state(0)), 4096)
            .expect("ordinary binder bound");
    }
}

#[test]
fn malformed_configured_target_references_and_arities_refuse_before_use() {
    let source = "---- MODULE ConfiguredMetadata ----\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nIdentity(v)==v\nTwo(v,w)==v=w\nP==<>Op(x=0)\n====\n";
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(
            source,
            "INIT Init\nNEXT Next\nCONSTANT Op <- Identity\n",
            &options,
        );
        let two = model.ir.root_def("Two").expect("two");
        for target in [dive_ir::DefId(u32::MAX), two] {
            let mut metadata = dive_eval::Model::new(model.ir.clone());
            metadata.names = model.names.clone();
            metadata.constants[0] = Some(dive_eval::ConstBinding::Def(target));
            assert!(dive_eval::Evaluator::new(metadata, options.eval).is_err());
        }
        let identity = model.ir.root_def("Identity").expect("identity");
        let mut metadata = dive_eval::Model::new(model.ir.clone());
        metadata.names = model.names.clone();
        metadata.constants[0] = Some(dive_eval::ConstBinding::Def(identity));
        metadata.def_values.insert(identity, Value::Bool(true));
        let Err(error) = dive_eval::Evaluator::new(metadata, options.eval) else {
            panic!("parameterized value override must refuse at evaluator entry");
        };
        assert_eq!(error.kind(), dive_eval::ContextErrorKind::Arity);
    }
}
