//! Native actual admission is separate from independent word and witness truth.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded adversarial fixture assertions"
)]
use dive_model::{BindOptions, Pred};
use dive_value::Value;
use std::sync::Arc;
fn input(source: &str) -> Arc<dive_ir::Ir> {
    let a = dive_sema::analyze_source("Native.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    Arc::new(dive_ir::lower(&a).expect("IR"))
}
fn config() -> dive_config::ModelConfig {
    dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config")
}
#[test]
fn native_words_are_independently_evaluated_at_every_closing_position() {
    let mut cases = 0;
    for (extends, op, atom) in [
        ("Sequences", "Len", "F(x)=1"),
        ("Sequences", "Head", "F(x)"),
        ("FiniteSets", "Cardinality", "F(x)=1"),
    ] {
        let source = format!(
            "---- MODULE Native ----\nEXTENDS {extends}\nVARIABLE x\nInit==x=TRUE\nNext==UNCHANGED x\nUse(F(_))==[]<>({atom})\nRelay(G(_))==Use(G)\nLive==Relay({op})\n====\n"
        );
        let ir = input(&source);
        for (options, ctx) in [
            (BindOptions::tlc(), dive_value::Ctx::tlc()),
            (BindOptions::dive(), dive_value::Ctx::dive()),
        ] {
            let model = dive_model::bind(ir.clone(), &config(), &options).expect("bind");
            let d = ir.root_def("Live").expect("property");
            let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
            let plan = dive_liveness::plan::lower(
                &model,
                &pred,
                dive_liveness::formula::Limits::default(),
            )
            .expect("plan");
            for n in 1..=4 {
                for mask in 0_usize..(1 << n) {
                    let values: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
                    let states: Vec<dive_trace::State> = values
                        .iter()
                        .map(|b| {
                            let x = match op {
                                "Len" => Value::tuple(if *b { vec![1.into()] } else { vec![] }),
                                "Head" => Value::tuple(vec![(*b).into()]),
                                _ => Value::set(if *b { vec![1.into()] } else { vec![] }, ctx)
                                    .expect("stored set"),
                            };
                            Arc::from([Some(x)])
                        })
                        .collect();
                    for back in 0..n {
                        cases += 1;
                        let actual =
                            dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                                .expect("independent word");
                        assert_eq!(actual, vec![values[back..].iter().any(|b| *b); n]);
                        let bins: Vec<Vec<bool>> = (0..n)
                            .map(|i| {
                                let edge = dive_eval::St::action(
                                    states[i].clone(),
                                    states[if i + 1 == n { back } else { i + 1 }].clone(),
                                );
                                plan.atoms
                                    .iter()
                                    .map(|a| a.evaluate(&model, &edge).expect("atom"))
                                    .collect()
                            })
                            .collect();
                        assert_eq!(
                            plan.formula
                                .evaluate_word(&bins, &bins, back, 1_000_000)
                                .expect("kernel"),
                            actual
                        );
                    }
                }
            }
        }
    }
    assert_eq!(cases, 588);
}
#[test]
fn native_metadata_admission_refuses_unknown_configured_foreign_and_budget_inputs() {
    use dive_ir::{OpRef, StdOp};
    let source = "---- MODULE Native ----\nEXTENDS Sequences, TLC\nCONSTANT K(_)\nVARIABLE x\nInit==x=TRUE\nNext==UNCHANGED x\nGood(v)==v\nUse(F(_))==TRUE\n====\n";
    let ir = input(source);
    let base_cfg = dive_config::parse("INIT Init\nNEXT Next\nCONSTANT K <- Good\n")
        .into_config()
        .expect("base cfg");
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nCONSTANT K <- Len\n")
        .into_config()
        .expect("cfg");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &base_cfg, &options).expect("model");
        let def = ir.root_def("Use").expect("def");
        let scope = model.evaluator.scope(def).expect("scope");
        for op in [
            StdOp::SequencesLen,
            StdOp::SequencesHead,
            StdOp::SequencesAppend,
            StdOp::FiniteSetsCardinality,
        ] {
            assert_eq!(
                model
                    .evaluator
                    .scope_native_operator_analysis(&scope, &OpRef::Std(op), &mut 50_000)
                    .expect("native metadata"),
                Some(op)
            );
            assert!(
                model
                    .evaluator
                    .scope_native_operator_analysis(&scope, &OpRef::Std(op), &mut 0)
                    .is_err()
            );
        }
        for op in [
            OpRef::Std(StdOp::TlcPrintT),
            OpRef::Std(StdOp::SequencesTail),
            OpRef::Builtin(dive_ir::BuiltinOp::Not),
            OpRef::Def(def),
        ] {
            assert!(
                model
                    .evaluator
                    .scope_native_operator_analysis(&scope, &op, &mut 50_000)
                    .expect("typed refusal")
                    .is_none()
            );
        }
        assert!(
            model
                .evaluator
                .scope_native_operator_analysis(
                    &scope,
                    &OpRef::Def(dive_ir::DefId(u32::MAX)),
                    &mut 50_000
                )
                .is_err()
        );
        let foreign =
            dive_model::bind(Arc::new(ir.as_ref().clone()), &base_cfg, &options).expect("foreign");
        assert!(
            model
                .evaluator
                .scope_native_operator_analysis(
                    &foreign.evaluator.scope(def).expect("foreign scope"),
                    &OpRef::Std(StdOp::SequencesLen),
                    &mut 50_000
                )
                .is_err()
        );
        let configured = dive_model::bind(ir.clone(), &cfg, &options).expect("configured");
        let scope = configured.evaluator.scope(def).expect("configured scope");
        assert!(
            configured
                .evaluator
                .scope_native_operator_analysis(
                    &scope,
                    &OpRef::Std(StdOp::SequencesLen),
                    &mut 50_000
                )
                .expect("configured exclusion")
                .is_none()
        );
        assert_eq!(configured.evaluator.take_output(), Vec::<String>::new());
    }
}
#[derive(Debug)]
struct Opaque(Arc<std::sync::atomic::AtomicUsize>);
impl dive_value::DeferredSet for Opaque {
    fn kind(&self) -> dive_value::DeferredKind {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![1.into()])
    }
    fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        vec![]
    }
}
#[test]
fn independent_native_word_refuses_providers_foreign_names_order_and_budget_before_callbacks() {
    use dive_value::{Ctx, Interner};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let ir = input(
        "---- MODULE Native ----\nEXTENDS FiniteSets\nVARIABLE x\nInit==x=TRUE\nNext==UNCHANGED x\nUse(F(_))==[]<>(F(x)=1)\nLive==Use(Cardinality)\n====\n",
    );
    for (options, ctx, wrong) in [
        (BindOptions::tlc(), Ctx::tlc(), Ctx::dive()),
        (BindOptions::dive(), Ctx::dive(), Ctx::tlc()),
    ] {
        let model = dive_model::bind(ir.clone(), &config(), &options).expect("model");
        let d = ir.root_def("Live").expect("role");
        let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Value::deferred(Arc::new(Opaque(calls.clone())), ctx);
        // A value sorted in the other profile's order is refused before
        // evaluation (DIVE-V0043), even a singleton.
        let mut values = vec![
            provider.clone(),
            Value::set(vec![1.into()], wrong).expect("opposite order"),
        ];
        let foreign = Interner::new();
        values.push(Value::set_raw(
            vec![Value::Model(foreign.intern("foreign"))],
            ctx,
        ));
        for value in values {
            let baseline = calls.load(Ordering::Relaxed);
            let states: Vec<dive_trace::State> = vec![Arc::from([Some(value)])];
            assert!(dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err());
            assert_eq!(
                calls.load(Ordering::Relaxed),
                baseline,
                "provider never consulted"
            );
        }
        let nested = Value::tuple(vec![provider]);
        let baseline = calls.load(Ordering::Relaxed);
        let states: Vec<dive_trace::State> = vec![Arc::from([Some(nested)])];
        assert!(dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err());
        assert_eq!(
            calls.load(Ordering::Relaxed),
            baseline,
            "constructor baseline separate from admission"
        );
    }
}

/// A typed state sorted in the other profile's order would make the core
/// evaluator read it in the wrong order (`CHOOSE` picks `"alpha"` instead of
/// TLC's `"zeta"`), so it is refused before evaluation, never decided.
#[test]
fn typed_state_in_the_other_profiles_order_is_refused_not_misread() {
    use dive_value::Ctx;
    let ir = input(
        "---- MODULE Native ----\nVARIABLE x\nZ == \"zeta\"\nA == \"alpha\"\nInit==x={Z, A}\nNext==UNCHANGED x\nLive==[]<>((CHOOSE e \\in x : TRUE) = Z)\n====\n",
    );
    for (options, ctx, wrong, own_truth) in [
        (BindOptions::tlc(), Ctx::tlc(), Ctx::dive(), true),
        (BindOptions::dive(), Ctx::dive(), Ctx::tlc(), false),
    ] {
        let model = dive_model::bind(ir.clone(), &config(), &options).expect("model");
        let d = ir.root_def("Live").expect("role");
        let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
        let elems = vec![
            Value::string(&model.names, "zeta"),
            Value::string(&model.names, "alpha"),
        ];
        let own: Vec<dive_trace::State> = vec![Arc::from([Some(
            Value::set(elems.clone(), ctx).expect("set"),
        )])];
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(&model, &pred, &own, 0),
            Ok(vec![own_truth])
        );
        let other: Vec<dive_trace::State> =
            vec![Arc::from([Some(Value::set(elems, wrong).expect("set"))])];
        assert!(matches!(
            dive_trace::temporal::evaluate_lasso(&model, &pred, &other, 0),
            Err(dive_trace::temporal::TemporalError::Unsupported(ref why)) if why.contains("DIVE-V0043")
        ));
    }
}
