//! Configured call origin survives local recursive source and actual function slots.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "scoped boundary assertions"
)]

use dive_eval::{Host, HostError, St};
use dive_ir::{Expr, StdOp};
use dive_liveness::{formula::Limits as FormulaLimits, plan};
use dive_model::{Model, Pred};
use dive_runtime::CheckOptions;
use dive_trace::{State, enabled::enabled, temporal::evaluate_lasso};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
struct Hooks(AtomicUsize);
impl Host for Hooks {
    fn check_native_access(&self, _: StdOp) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn print_output(&self, _: &str) -> Result<bool, HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(false)
    }
}
fn bound(definitions: &str, call: &str, configured: bool, options: &CheckOptions) -> Model {
    let declaration = if configured { "CONSTANT Op(_)\n" } else { "" };
    let source = format!(
        "---- MODULE RecursiveConfigured ----\nEXTENDS Integers,TLC\n{declaration}VARIABLE x\nInit==x=0\nNext==x'=x\n{definitions}\nA==({call}) /\\ x'=x\nP==<>[][A]_x\n====\n"
    );
    let a = dive_sema::analyze_source(
        "RecursiveConfigured.tla",
        &source,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{:?}", a.diagnostics());
    let config = format!(
        "INIT Init\nNEXT Next\nPROPERTY P\n{}",
        if configured { "CONSTANT Op <- F\n" } else { "" }
    );
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &dive_config::parse(&config).into_config().expect("config"),
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}
fn pred(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect(name);
    Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("scope"),
    )
}
fn state() -> State {
    Arc::from([Some(Value::Int(0))])
}

#[test]
fn configured_local_recursion_refuses_in_every_route_before_callbacks() {
    let rec = "LET f[t \\in {0}]==IF t=0 THEN v ELSE f[t] IN f[0]";
    let cases = [
        (format!("F(v)=={rec}"), "Op(x=0)"),
        (format!("Helper(v)=={rec}\nF(v)==Helper(v)"), "Op(x=0)"),
        (
            format!("Helper(v)=={rec}\nF(v)==LET held==Helper(v) IN held"),
            "Op(x=0)",
        ),
        (format!("Helper(v)=={rec}\nF(v)==v"), "Op(Helper(x=0))"),
        (format!("Helper(v)=={rec}\nF(v)==TRUE"), "Op(Helper(x=0))"),
        (
            "F(v)==v".into(),
            "Op(LET f[t \\in {0}]==IF t=0 THEN TRUE ELSE f[t] IN f[0])",
        ),
        (
            "F(v)==TRUE".into(),
            "Op(LET f[t \\in {0}]==IF t=0 THEN TRUE ELSE f[t] IN f[0])",
        ),
        (
            "F(v)==LET f[t \\in {}]==IF t=0 THEN v ELSE f[t] IN DOMAIN f={}".into(),
            "Op(x'=0)",
        ),
        (
            "F(v)==LET held==v IN LET f[t \\in {}]==IF t=0 THEN held ELSE f[t] IN DOMAIN f={}"
                .into(),
            "Op(x'=0)",
        ),
        (
            "F(v)==LET f[t \\in {}]==IF t=0 THEN PrintT(v) ELSE f[t] IN DOMAIN f={}".into(),
            "Op(x=0)",
        ),
    ];
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for (definitions, call) in &cases {
            let model = bound(definitions, call, true, &options);
            assert!(!model.ir.def(model.ir.root_def("F").expect("F")).recursive);
            assert!(
                model
                    .ir
                    .nodes()
                    .iter()
                    .any(|n| matches!(n.expr, Expr::RecFcn { .. }))
            );
            assert!(
                !model.implied_temporals.is_empty(),
                "genuine temporal obligation"
            );
            let hooks = Arc::new(Hooks::default());
            model.evaluator.set_host(hooks.clone());
            let action = pred(&model, "A");
            let property = pred(&model, "P");
            let before = state();
            assert!(
                plan::lower(&model, &property, FormulaLimits::default()).is_err(),
                "planner {call}"
            );
            assert!(
                evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0).is_err(),
                "word {call}"
            );
            assert_ne!(
                enabled(&model, &action, None, &before),
                Ok(false),
                "ENABLED must not certify Disabled: {call}"
            );
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0, "no callback {call}");
            assert_eq!(model.output, [] as [dive_model::Message; 0]);
        }
    }
}

#[test]
fn plain_configured_and_unconfigured_recursive_calls_keep_real_enabled_witnesses() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for (definitions, call, configured) in [
            ("F(v)==v", "Op(x=0)", true),
            (
                "F(v)==LET f[t \\in {0}]==IF t=0 THEN v ELSE f[t] IN f[0]",
                "F(x=0)",
                false,
            ),
        ] {
            let model = bound(definitions, call, configured, &options);
            let action = pred(&model, "A");
            let before = state();
            plan::lower(&model, &pred(&model, "P"), FormulaLimits::default())
                .expect("planner positive");
            assert_eq!(
                evaluate_lasso(&model, &pred(&model, "P"), std::slice::from_ref(&before), 0)
                    .expect("word positive"),
                vec![true]
            );
            assert_eq!(enabled(&model, &action, None, &before), Ok(true));
        }
    }
}

#[test]
fn configured_recursive_domain_is_refused_before_its_original_division_error() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(
            "F(v)==LET f[t \\in {1 \\div 0}]==IF t=0 THEN v ELSE f[t] IN DOMAIN f={}",
            "Op(x=0)",
            true,
            &options,
        );
        let property = pred(&model, "P");
        let before = state();
        let errors = [
            plan::lower(&model, &property, FormulaLimits::default())
                .expect_err("planner source refusal")
                .to_string(),
            evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0)
                .expect_err("word source refusal")
                .to_string(),
        ];
        for error in errors {
            assert!(
                error.contains("recursive function in configured"),
                "{error}"
            );
        }
    }
}

#[test]
fn configured_unused_nested_helper_recursive_actuals_refuse_in_every_route() {
    let actuals = [
        "LET f[t \\in {1 \\div 0}]==IF t=0 THEN v ELSE f[t] IN DOMAIN f",
        "LET f[t \\in {}]==IF t=0 THEN v ELSE f[t] IN DOMAIN f",
    ];
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for actual in actuals {
            for body in [
                format!("Ignore({actual})"),
                format!("LET held==({actual}) IN Ignore(held)"),
                format!("Forward({actual})"),
                "Ignore(Build(v))".into(),
            ] {
                let definitions = format!(
                    "Ignore(a)==TRUE\nForward(a)==Ignore(a)\nBuild(v)=={actual}\nF(v)=={body}"
                );
                let model = bound(&definitions, "Op(x=0)", true, &options);
                assert!(!model.ir.def(model.ir.root_def("F").expect("F")).recursive);
                assert!(
                    model
                        .ir
                        .nodes()
                        .iter()
                        .any(|n| matches!(n.expr, Expr::RecFcn { .. }))
                );
                assert!(
                    !model.implied_temporals.is_empty(),
                    "genuine temporal obligation"
                );
                let action = pred(&model, "A");
                let property = pred(&model, "P");
                let before = state();
                let st = St::action(before.clone(), before.clone());
                assert_eq!(
                    model
                        .eval(&action, &st)
                        .expect("ordinary unused lazy argument"),
                    Value::Bool(true)
                );
                let hooks = Arc::new(Hooks::default());
                model.evaluator.set_host(hooks.clone());
                let errors = [
                    plan::lower(&model, &property, FormulaLimits::default())
                        .expect_err("planner unused recursive helper actual")
                        .to_string(),
                    evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0)
                        .expect_err("word unused recursive helper actual")
                        .to_string(),
                ];
                for error in errors {
                    assert!(
                        error.contains("recursive function in configured"),
                        "{body}: {error}"
                    );
                }
                assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
                assert_eq!(model.output, [] as [dive_model::Message; 0]);
                assert_eq!(
                    model
                        .eval(&action, &st)
                        .expect("refusal leaves original lazy evaluation intact"),
                    Value::Bool(true)
                );
            }
        }
    }
}

#[test]
fn nested_helper_plain_configured_and_ordinary_recursive_positives_keep_witnesses() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for configured in [false, true] {
            let body = if configured {
                "Ignore(v)"
            } else {
                "Ignore(LET f[t \\in {0}]==IF t=0 THEN v ELSE f[t] IN f[0])"
            };
            let model = bound(
                &format!("Ignore(a)==TRUE\nF(v)=={body}"),
                if configured { "Op(x=0)" } else { "F(x=0)" },
                configured,
                &options,
            );
            let action = pred(&model, "A");
            let property = pred(&model, "P");
            let before = state();
            plan::lower(&model, &property, FormulaLimits::default()).expect("planner positive");
            assert_eq!(
                evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0)
                    .expect("word positive"),
                vec![true]
            );
            assert_eq!(enabled(&model, &action, None, &before), Ok(true));
        }
        // The configured-depth-zero finite route must not start inspecting or
        // evaluating an ordinary helper's unused expression actuals.
        let ordinary = bound(
            "Ignore(a)==TRUE\nF(v)==Ignore(LET f[t \\in {1 \\div 0}]==IF t=0 THEN v ELSE f[t] IN DOMAIN f)",
            "F(x=0)",
            false,
            &options,
        );
        let action = pred(&ordinary, "A");
        let before = state();
        let st = St::action(before.clone(), before);
        assert_eq!(
            ordinary.eval(&action, &st).expect("unused actual"),
            Value::Bool(true)
        );
    }
}

#[test]
fn configured_unused_higher_order_helper_actual_remains_a_refusal() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(
            "Identity(v)==v\nIgnore(H(_),v)==TRUE\nF(v)==Ignore(Identity,v)",
            "Op(x=0)",
            true,
            &options,
        );
        let action = pred(&model, "A");
        let property = pred(&model, "P");
        let before = state();
        let st = St::action(before.clone(), before.clone());
        assert_eq!(
            model
                .eval(&action, &st)
                .expect("original unused operator actual"),
            Value::Bool(true)
        );
        let hooks = Arc::new(Hooks::default());
        model.evaluator.set_host(hooks.clone());
        assert!(plan::lower(&model, &property, FormulaLimits::default()).is_err());
        assert!(evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0).is_err());
        assert_ne!(enabled(&model, &action, None, &before), Ok(false));
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        assert_eq!(model.output, [] as [dive_model::Message; 0]);
    }
}
