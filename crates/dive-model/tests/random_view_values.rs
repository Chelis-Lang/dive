//! Representation admission never invokes deferred providers or normalization.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_value::{Ctx, DeferredKind, DeferredSet, Value};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Unknown(Arc<AtomicUsize>);
impl DeferredSet for Unknown {
    fn member(&self, _elem: &Value, _ctx: Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(true))
    }
    fn kind(&self) -> DeferredKind {
        self.0.fetch_add(1, Ordering::SeqCst);
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        vec![]
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(self)
    }
}

#[test]
fn stored_replay_shapes_are_admitted_without_normalization() {
    let ctx = Ctx::tlc();
    let set = Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx);
    let values = [
        Value::TRUE,
        Value::Int(0),
        set.clone(),
        Value::tuple(vec![set.clone()]),
        Value::record(
            vec![(dive_value::Interner::new().intern("a"), set.clone())],
            ctx,
        )
        .expect("record"),
        Value::fcn(
            vec![(Value::Int(0), set.clone()), (Value::Int(2), Value::Int(3))],
            ctx,
        )
        .expect("general function"),
    ];
    assert!(dive_model::random_view::admit_values(&values).is_ok());
    let Value::Set(set) = &set else {
        return;
    };
    assert!(set.raw_elements().is_some(), "admission must not normalize");
}

#[test]
fn unknown_and_unproved_shapes_refuse_without_provider_calls() {
    let ctx = Ctx::tlc();
    let calls = Arc::new(AtomicUsize::new(0));
    let deferred = Value::deferred(Arc::new(Unknown(calls.clone())), ctx);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(dive_model::random_view::admit_values(std::slice::from_ref(&deferred)).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let compound = Value::set_raw(vec![Value::tuple(vec![Value::Int(1)])], ctx);
    let mixed = Value::set_raw(vec![Value::TRUE, Value::Int(1)], ctx);
    let nonliteral = Value::tuple_fcn(vec![Value::Int(1)]);
    let symbolic = Value::interval(1.into(), 2.into(), ctx);
    for value in [compound, mixed, nonliteral, symbolic] {
        assert!(dive_model::random_view::admit_values(&[value]).is_err());
    }
    // Tuple construction currently hashes its children. Admission guarantees
    // exactly zero additional callbacks above that constructor baseline.
    let wrapped = Value::tuple(vec![deferred]);
    let before = calls.load(Ordering::SeqCst);
    assert!(dive_model::random_view::admit_values(&[wrapped]).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), before);
}

#[test]
fn width_and_depth_limits_refuse_without_expanding_a_work_vector() {
    let values = vec![Value::TRUE; dive_model::random_view::MAX_VALUES + 1];
    assert_eq!(
        dive_model::random_view::admit_values(&values),
        Err("random VIEW value work limit")
    );
    let mut value = Value::TRUE;
    for _ in 0..=dive_model::random_view::MAX_DEPTH {
        value = Value::tuple(vec![value]);
    }
    assert_eq!(
        dive_model::random_view::admit_values(&[value]),
        Err("random VIEW value nesting limit")
    );
}
