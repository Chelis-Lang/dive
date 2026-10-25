//! Capability rejection and real captured owner lifetime for the pull cursor.
#![allow(
    clippy::expect_used,
    reason = "bounded lowered fixtures and ownership assertions"
)]
use super::*;
use crate::{ConstBinding, Evaluator, Model, Options, St};
use std::sync::atomic::{AtomicUsize, Ordering};

fn evaluator(text: &str, options: Options) -> Evaluator {
    let source = format!("---- MODULE Owner ----\nEXTENDS Integers, TLC\n{text}\n====\n");
    let analysis = dive_sema::analyze_source("Owner.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    Evaluator::new(Model::new(ir), options).expect("evaluator")
}
fn set(value: &Value) -> &dive_value::Set {
    match value {
        Value::Set(set) => Some(set),
        _ => None,
    }
    .expect("set fixture")
}

#[derive(Debug)]
struct Probe {
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Probe {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
impl DeferredSet for Probe {
    fn kind(&self) -> DeferredKind {
        self.calls.fetch_add(1, Ordering::Relaxed);
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![1.into()])
    }
    fn member(&self, _value: &Value, _ctx: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Vec::new()
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(self)
    }
}

#[test]
fn unknown_deferred_type_declines_without_overridable_metadata_or_provider_calls() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("F == 1", options);
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let value = Value::deferred(
            Arc::new(Probe {
                calls: calls.clone(),
                drops: drops.clone(),
            }),
            options.ctx,
        );
        let before = calls.load(Ordering::Relaxed);
        assert!(
            ev.core
                .in_element_stream(set(&value))
                .expect("metadata only")
                .is_none()
        );
        assert_eq!(calls.load(Ordering::Relaxed), before);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        assert_eq!(
            set(&value)
                .elements(options.ctx)
                .expect("ordinary provider")
                .as_ref(),
            [Value::Int(1)]
        );
        assert_eq!(calls.load(Ordering::Relaxed), before + 1);
        drop(value);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn foreign_core_and_cached_history_decline_before_any_extra_predicate_effect() {
    for options in [Options::dive(), Options::tlc()] {
        let source = "F == {A \\in SUBSET (1..2) : Print(5, TRUE)}";
        let first = evaluator(source, options);
        let foreign = evaluator(source, options);
        let def = first.ir().root_def("F").expect("F");
        let value = first
            .eval_def(def, &St::constant())
            .expect("captured filter");
        assert!(!set(&value).has_cached_elements());
        assert!(
            foreign
                .core
                .in_element_stream(set(&value))
                .expect("foreign decline")
                .is_none()
        );
        assert_eq!(first.take_output(), Vec::<String>::new());
        assert_eq!(
            set(&value)
                .elements(options.ctx)
                .expect("original full provider")
                .len(),
            4
        );
        assert_eq!(
            first.take_output(),
            ["5  TRUE", "5  TRUE", "5  TRUE", "5  TRUE"]
        );
        assert!(set(&value).has_cached_elements());
        assert!(
            first
                .core
                .in_element_stream(set(&value))
                .expect("history decline")
                .is_none()
        );
        assert_eq!(first.take_output(), Vec::<String>::new());
    }
}

#[test]
fn cursor_really_retains_the_original_core_until_its_active_environment_is_dropped() {
    for options in [Options::dive(), Options::tlc()] {
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let analysis = dive_sema::analyze_source(
            "Owner.tla",
            "---- MODULE Owner ----\nCONSTANT C\nF == {A \\in SUBSET {1,2} : TRUE}\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let def = ir.root_def("F").expect("F");
        let mut model = Model::new(ir);
        model.constants[0] = Some(ConstBinding::Value(Value::deferred(
            Arc::new(Probe {
                calls: calls.clone(),
                drops: drops.clone(),
            }),
            options.ctx,
        )));
        let ev = Evaluator::new(model, options).expect("evaluator");
        let value = ev.eval_def(def, &St::constant()).expect("private Filter");
        let before = calls.load(Ordering::Relaxed);
        let mut cursor = ev
            .core
            .in_element_stream(set(&value))
            .expect("cursor construction")
            .expect("positive cursor route")
            .with_owner(value);
        drop(ev);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        assert_eq!(
            cursor.next().expect("empty").expect("original predicate"),
            Value::empty_set(options.ctx)
        );
        assert_eq!(calls.load(Ordering::Relaxed), before);
        drop(cursor);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn accumulated_real_filter_frames_prepare_pull_and_drop_on_a_small_stack() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("F(S) == {a \\in S : TRUE}", options);
        let def = ev.ir().root_def("F").expect("F");
        let body = ev.ir().def(def).body;
        let mut value = Value::subset(
            Value::interval(1.into(), 0.into(), options.ctx),
            options.ctx,
        );
        // Repeated real body evaluation accumulates captures without increasing
        // the IR depth. Each frame uses the original lowered parameter slot.
        for _ in 0..4096 {
            let mut scope = ev.scope(def).expect("real parameter frame");
            scope.env.bind(0, crate::env::Sv::Val(value));
            value = ev
                .core
                .eval_v(&mut scope.env, &St::constant(), body)
                .expect("original private Filter constructor");
        }
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let mut cursor = ev
                    .core
                    .in_element_stream(set(&value))
                    .expect("iterative preparation")
                    .expect("private chain")
                    .with_owner(value);
                assert_eq!(cursor.filters.len(), 4096);
                assert_eq!(
                    cursor.next().expect("one value").expect("TRUE filters"),
                    Value::empty_set(options.ctx)
                );
                // Deliberately stop before exhaustion, as an Init sink does.
                drop(cursor);
                assert_eq!(ev.take_output(), Vec::<String>::new());
            })
            .expect("small-stack cursor thread")
            .join()
            .expect("cursor teardown");
    }
}
