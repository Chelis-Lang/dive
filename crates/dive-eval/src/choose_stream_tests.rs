//! Function-space CHOOSE keeps the bulk path's order and admission.
#![allow(clippy::expect_used, clippy::panic, reason = "finite test assertions")]

use super::*;
use crate::{Evaluator, HostControl, Interrupted, Model, Options};
use dive_value::{Ctx, Interner};
use std::sync::atomic::{AtomicUsize, Ordering};

fn set(value: &Value) -> &Set {
    let Value::Set(set) = value else {
        panic!("set");
    };
    set
}

fn evaluator(body: &str, options: Options, cached: bool) -> Evaluator {
    let source =
        format!("---- MODULE Choice ----\nEXTENDS Naturals, TLC\nCONSTANT S\nQ == {body}\n====\n");
    let analysis = dive_sema::analyze_source("Choice.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let mut model = Model::new(ir);
    let names = &model.names;
    // Encounter order deliberately differs from canonical text order.
    let domain = Value::set(
        vec![Value::Str(names.intern("b")), Value::Str(names.intern("a"))],
        options.ctx,
    )
    .expect("domain");
    let range = Value::set(vec![1.into(), 2.into()], options.ctx).expect("range");
    let space = Value::fcn_set(domain, range, options.ctx);
    if cached {
        set(&space).elements(options.ctx).expect("bulk");
    }
    model.constants[0] = Some(ConstBinding::Value(space));
    Evaluator::new(model, options).expect("evaluator")
}

fn eval(ev: &Evaluator) -> Result<Value> {
    let def = ev.ir().root_def("Q").expect("Q");
    ev.eval_in(def, ev.ir().def(def).body, &St::constant())
}

#[test]
fn function_choices_preserve_order_errors_and_effects_in_both_profiles() {
    for options in [
        Options::dive(),
        Options::tlc(),
        Options {
            exploration: crate::ExplorationOrder::Tlc,
            ..Options::dive()
        },
    ] {
        for body in [
            r"CHOOSE f \in S : PrintT(f) /\ TRUE",
            r#"CHOOSE f \in S : PrintT(f) /\ f["a"] = 2"#,
            r#"CHOOSE f \in S : IF f["a"] = 1 THEN TRUE ELSE 1 \div 0 = 0"#,
            r"CHOOSE f \in S : PrintT(f) /\ FALSE",
            r"CHOOSE f \in S : 1",
            r"CHOOSE f \in S : 1 \div 0 = 0",
            r"CHOOSE <<x, y, z>> \in S : TRUE",
        ] {
            let stream = evaluator(body, options, false);
            let bulk = evaluator(body, options, true);
            let actual = eval(&stream);
            let expected = eval(&bulk);
            match (actual, expected) {
                (Ok(a), Ok(b)) => assert_eq!(a, b, "{body}"),
                (Err(a), Err(b)) => {
                    assert_eq!(a.class(), b.class(), "{body}");
                    assert_eq!(a.message(), b.message(), "{body}");
                    assert_eq!(a.ec(), b.ec(), "{body}");
                }
                (a, b) => panic!("{body}: {a:?} vs {b:?}"),
            }
            assert_eq!(stream.take_output(), bulk.take_output(), "{body}");
        }
    }
}

#[test]
fn concrete_function_cursor_matches_every_bulk_candidate_without_caching_output() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("0", options, false);
        let names = Interner::new();
        let text = vec![Value::Str(names.intern("b")), Value::Str(names.intern("a"))];
        let models = vec![Value::model(&names, "m_b"), Value::model(&names, "m_a")];
        for domain in [vec![1.into(), 2.into()], text.clone(), models.clone()] {
            for range in [vec![1.into(), 2.into()], text.clone(), models.clone()] {
                let space = Value::fcn_set(
                    Value::set(domain.clone(), options.ctx).expect("domain"),
                    Value::set(range, options.ctx).expect("range"),
                    options.ctx,
                );
                let actual = ev
                    .core
                    .choose_stream(set(&space))
                    .expect("cursor")
                    .expect("eligible")
                    .collect::<Vec<_>>();
                assert!(!set(&space).has_cached_elements());
                let expected = set(&space).elements(options.ctx).expect("bulk");
                assert_eq!(actual.as_slice(), expected.as_ref());
            }
        }
    }
}

#[test]
fn uncertain_cached_empty_and_cross_order_components_keep_bulk_path() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("0", options, false);
        let ctx = options.ctx;
        let base = Value::set(vec![1.into(), 2.into()], ctx).expect("base");
        let empty = Value::set(vec![], ctx).expect("empty");
        let other = Ctx {
            order: if ctx.order == dive_value::ValueOrder::Tlc {
                dive_value::ValueOrder::Canonical
            } else {
                dive_value::ValueOrder::Tlc
            },
            ..ctx
        };
        let cross = Value::set(vec![1.into()], other).expect("cross");
        let nested = Value::set(vec![Value::tuple(vec![1.into()])], ctx).expect("nested");
        for (domain, range) in [
            (empty.clone(), base.clone()),
            (base.clone(), empty),
            (cross, base.clone()),
            (nested, base.clone()),
            (Value::set_raw(vec![1.into()], ctx), base.clone()),
            (Value::interval(1.into(), 2.into(), ctx), base.clone()),
        ] {
            let space = Value::fcn_set(domain, range, ctx);
            assert!(
                ev.core
                    .choose_stream(set(&space))
                    .expect("decline")
                    .is_none()
            );
            assert!(!set(&space).has_cached_elements());
        }
        let space = Value::fcn_set(base.clone(), base, ctx);
        set(&space).elements(ctx).expect("preexisting output");
        assert!(
            ev.core
                .choose_stream(set(&space))
                .expect("cached")
                .is_none()
        );
    }
}

#[test]
fn function_cursor_preserves_size_overflow_and_bulk_cap_errors() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("0", options, false);
        for n in [31, 32, 65] {
            let space = Value::fcn_set(
                Value::set((0..n).map(Value::Int).collect(), options.ctx).expect("domain"),
                Value::set(vec![0.into(), 1.into()], options.ctx).expect("range"),
                options.ctx,
            );
            let actual = ev
                .core
                .choose_stream(set(&space))
                .err()
                .expect("size error");
            let expected =
                EvalError::from(set(&space).elements(options.ctx).expect_err("bulk error"));
            assert_eq!(actual.class(), expected.class());
            assert_eq!(actual.message(), expected.message());
            assert!(!set(&space).has_cached_elements());
        }
    }
}

#[derive(Debug)]
struct StopAfter(AtomicUsize);
impl HostControl for StopAfter {
    fn check(&self) -> std::result::Result<(), Interrupted> {
        if self.0.fetch_add(1, Ordering::Relaxed) >= 1 {
            Err(Interrupted::new(0))
        } else {
            Ok(())
        }
    }
}

#[test]
fn interrupted_function_choice_does_not_publish_a_startup_value() {
    for options in [Options::dive(), Options::tlc()] {
        let template = evaluator(r"CHOOSE f \in S : f[0] = 1", options, false);
        let mut model = Model::new(Arc::clone(template.ir()));
        let space = Value::fcn_set(
            Value::set((0..13).map(Value::Int).collect(), options.ctx).expect("domain"),
            Value::set(vec![0.into(), 1.into()], options.ctx).expect("range"),
            options.ctx,
        );
        model.constants[0] = Some(ConstBinding::Value(space));
        let ev = Evaluator::new(model, options).expect("large choice");
        let def = ev.ir().root_def("Q").expect("Q");
        let owner = ev
            .host_control(Some(Arc::new(StopAfter(AtomicUsize::new(0)))))
            .expect("control");
        assert_eq!(
            ev.precompute(def).expect_err("interrupted").class(),
            crate::ErrorClass::Interrupted
        );
        assert!(ev.precomputed(def).is_none());
        drop(owner);
        let actual = eval(&ev).expect("retry");
        let expected = Value::fcn(
            (0..13)
                .map(|i| (Value::Int(i), Value::Int(i64::from(i == 0))))
                .collect(),
            options.ctx,
        )
        .expect("first matching function");
        assert_eq!(actual, expected);
    }
}
