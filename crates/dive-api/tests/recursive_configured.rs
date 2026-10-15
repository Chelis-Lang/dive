//! Configured call origin survives local recursive source and actual function slots.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "scoped boundary assertions"
)]

use dive_check::CheckOptions;
use dive_eval::{Host, HostError, SlotView, St};
use dive_ir::{Expr, LocalRef, StdOp};
use dive_liveness::{formula::Limits as FormulaLimits, plan};
use dive_model::{Model, Pred};
use dive_trace::{
    State,
    exists::{self, Decision},
    temporal::evaluate_lasso,
};
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
fn configured_local_recursion_refuses_in_four_independent_routes_before_callbacks() {
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
                model
                    .evaluator
                    .check_finite_bindings(
                        &action.scope,
                        action.expr,
                        &St::action(before.clone(), before.clone()),
                        4096
                    )
                    .is_err(),
                "finite {call}"
            );
            assert!(
                plan::lower(&model, &property, FormulaLimits::default()).is_err(),
                "planner {call}"
            );
            assert!(
                evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0).is_err(),
                "word {call}"
            );
            assert!(
                exists::exists_successor_pure(
                    &model,
                    &action,
                    &before,
                    None,
                    exists::Limits::default(),
                    4096
                )
                .is_err(),
                "EXISTS must not certify Disabled: {call}"
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
            model
                .evaluator
                .check_finite_bindings(
                    &action.scope,
                    action.expr,
                    &St::action(before.clone(), before.clone()),
                    4096,
                )
                .expect("finite positive");
            plan::lower(&model, &pred(&model, "P"), FormulaLimits::default())
                .expect("planner positive");
            assert_eq!(
                evaluate_lasso(&model, &pred(&model, "P"), std::slice::from_ref(&before), 0)
                    .expect("word positive"),
                vec![true]
            );
            let Decision::Enabled { witness } = exists::exists_successor_pure(
                &model,
                &action,
                &before,
                None,
                exists::Limits::default(),
                4096,
            )
            .expect("independent decision") else {
                panic!("actual enabled relation")
            };
            assert_eq!(
                model
                    .eval(&action, &St::action(before, witness))
                    .expect("original witness"),
                Value::Bool(true)
            );
        }
    }
}

#[test]
fn actual_recursive_self_slot_is_refused_even_after_ordinary_body_cache_is_warm() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound(
            "F(v)==v[0]\nR==LET f[t \\in {0}]==IF t=0 THEN TRUE ELSE Op(f) IN f[0]",
            "TRUE",
            true,
            &options,
        );
        let ev = &model.evaluator;
        let r = model.ir.root_def("R").expect("R");
        let root = ev.scope(r).expect("root");
        let Expr::Let { defs, .. } = model.ir.expr(model.ir.def(r).body) else {
            panic!("LET")
        };
        let rec = model.ir.def(defs[0]).body;
        let Expr::RecFcn { name, .. } = model.ir.expr(rec) else {
            panic!("RecFcn")
        };
        let entered = ev
            .scope_let(&root, model.ir.def(r).body)
            .expect("LET context");
        let before = state();
        let st = St::state(before.clone());
        let function = ev
            .scope_recursive_function_bounded(&entered, rec, &st, 4096)
            .expect("actual runtime function");
        let contexts = ev
            .recursive_function_body_scopes_bounded(&entered, &function, &st, 4096)
            .expect("actual contexts");
        let scope = &contexts.bindings[0];
        assert!(
            matches!(ev.scope_slot(scope,LocalRef{up:0,slot:*name}),SlotView::Function(actual)if Arc::ptr_eq(&actual,&function))
        );
        let Expr::If { other, .. } = model.ir.expr(contexts.body) else {
            panic!("body")
        };
        let configured = Pred::new(*other, scope.clone());
        for _ in 0..2 {
            assert_eq!(
                ev.eval_scoped(scope, contexts.body, &contexts.state)
                    .expect("ordinary body"),
                Value::Bool(true)
            );
            let error = ev
                .check_finite_bindings(scope, configured.expr, &st, 4096)
                .expect_err("actual Function finite refusal");
            assert!(error.to_string().contains("configured"), "{error}");
            let error = plan::lower(&model, &configured, FormulaLimits::default())
                .expect_err("actual Function planner refusal");
            assert!(error.to_string().contains("configured"), "{error}");
            let error = evaluate_lasso(&model, &configured, std::slice::from_ref(&before), 0)
                .expect_err("actual Function independent word refusal");
            assert!(error.to_string().contains("configured"), "{error}");
            let error = exists::exists_successor_pure(
                &model,
                &configured,
                &before,
                None,
                exists::Limits::default(),
                4096,
            )
            .expect_err("actual Function EXISTS refusal");
            assert!(error.to_string().contains("configured"), "{error}");
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
        let action = pred(&model, "A");
        let property = pred(&model, "P");
        let before = state();
        let errors = [
            model
                .evaluator
                .check_finite_bindings(
                    &action.scope,
                    action.expr,
                    &St::action(before.clone(), before.clone()),
                    4096,
                )
                .expect_err("finite source refusal")
                .to_string(),
            plan::lower(&model, &property, FormulaLimits::default())
                .expect_err("planner source refusal")
                .to_string(),
            evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0)
                .expect_err("word source refusal")
                .to_string(),
            exists::exists_successor_pure(
                &model,
                &action,
                &before,
                None,
                exists::Limits::default(),
                4096,
            )
            .expect_err("EXISTS source refusal")
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
fn configured_unused_nested_helper_recursive_actuals_refuse_in_all_four_routes() {
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
                    model
                        .evaluator
                        .check_finite_bindings(&action.scope, action.expr, &st, 4096)
                        .expect_err("finite unused recursive helper actual")
                        .to_string(),
                    plan::lower(&model, &property, FormulaLimits::default())
                        .expect_err("planner unused recursive helper actual")
                        .to_string(),
                    evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0)
                        .expect_err("word unused recursive helper actual")
                        .to_string(),
                    exists::exists_successor_pure(
                        &model,
                        &action,
                        &before,
                        None,
                        exists::Limits::default(),
                        4096,
                    )
                    .expect_err("EXISTS cannot certify Disabled")
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
            model
                .evaluator
                .check_finite_bindings(
                    &action.scope,
                    action.expr,
                    &St::action(before.clone(), before.clone()),
                    4096,
                )
                .expect("ordinary helper path remains supported");
            plan::lower(&model, &property, FormulaLimits::default()).expect("planner positive");
            assert_eq!(
                evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0)
                    .expect("word positive"),
                vec![true]
            );
            let Decision::Enabled { witness } = exists::exists_successor_pure(
                &model,
                &action,
                &before,
                None,
                exists::Limits::default(),
                4096,
            )
            .expect("real independent witness") else {
                panic!("actual enabled relation")
            };
            assert_eq!(
                model
                    .eval(&action, &St::action(before, witness))
                    .expect("original witness"),
                Value::Bool(true)
            );
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
        ordinary
            .evaluator
            .check_finite_bindings(&action.scope, action.expr, &st, 4096)
            .expect("ordinary unused domain keeps old no-evaluation boundary");
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
        let errors = [model
            .evaluator
            .check_finite_bindings(&action.scope, action.expr, &st, 4096)
            .expect_err("finite higher order")];
        for error in errors {
            assert!(
                error.to_string().contains("higher-order configured"),
                "{error}"
            );
        }
        assert!(plan::lower(&model, &property, FormulaLimits::default()).is_err());
        assert!(evaluate_lasso(&model, &property, std::slice::from_ref(&before), 0).is_err());
        assert!(
            exists::exists_successor_pure(
                &model,
                &action,
                &before,
                None,
                exists::Limits::default(),
                4096
            )
            .is_err()
        );
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        assert_eq!(model.output, [] as [dive_model::Message; 0]);
    }
}
