//! Unknown deferred providers cannot authorize normalization through callbacks.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_value::{Ctx, DeferredKind, DeferredSet, Value};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Unknown(Arc<AtomicUsize>);

impl DeferredSet for Unknown {
    fn kind(&self) -> DeferredKind {
        self.0.fetch_add(1, Ordering::SeqCst);
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    fn member(&self, _elem: &Value, _ctx: Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        vec![Value::TRUE]
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(self)
    }
}

#[test]
fn unknown_provider_refuses_without_any_provider_method() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(Unknown(calls.clone()));
    assert!(dive_eval::deferred_filter_normalization_operand(provider.as_ref()).is_none());
    let ctx = Ctx::tlc();
    let value = Value::deferred(provider, ctx);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let result = dive_model::symmetry::Reduction::normalize_concrete(&[value], ctx);
    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn filter_operand_normalization_changes_only_the_stored_bound_set() {
    for range in ["{2,1}", "{<<1>>,[a |-> 1,b |-> 1]}"] {
        let source = format!(
            "---- MODULE FilterOperand ----\nEXTENDS Naturals,TLC\nF == {{f \\in [Nat -> {range}] : Print(0, TRUE)}}\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("FilterOperand.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("valid lazy filter"));
        let def = ir.root_def("F").expect("filter definition");
        let ev = dive_eval::Evaluator::new(dive_eval::Model::new(ir), dive_eval::Options::tlc())
            .expect("valid evaluator");
        let value = ev
            .eval_def(def, &dive_eval::St::constant())
            .expect("lazy filter");
        let Value::Set(filter) = &value else {
            panic!("filter value")
        };
        let operand = dive_eval::deferred_filter_normalization_operand(
            filter.as_deferred().expect("deferred filter").as_ref(),
        )
        .expect("evaluator-owned filter");
        let Value::Set(domain) = operand else {
            panic!("bound set")
        };
        let components = domain.components();
        let Value::Set(stored_range) = &components[1].1 else {
            panic!("range")
        };
        assert!(stored_range.raw_elements().is_some());
        assert_eq!(ev.take_output(), Vec::<String>::new());
        let result = dive_model::symmetry::Reduction::normalize_concrete(&[value], Ctx::tlc());
        if range == "{2,1}" {
            assert!(result.is_ok());
            assert!(stored_range.raw_elements().is_none());
        } else {
            assert!(result.is_err());
            assert!(stored_range.raw_elements().is_some());
        }
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}
