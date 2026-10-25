//! Concrete cursors retain component checks without an outer aggregate size.
use super::*;
use crate::{DeferredKind, DeferredSet};
use std::sync::atomic::{AtomicUsize, Ordering};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
fn set(value: &Value) -> std::result::Result<&Set, &'static str> {
    match value {
        Value::Set(set) => Ok(set),
        _ => Err("set expected"),
    }
}

#[test]
fn recursive_concrete_outputs_match_complete_normalized_bulk_values() -> TestResult {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let base = Value::set(vec![Value::Int(2), Value::Int(1)], ctx)?;
        let inner = Value::subset(base, ctx);
        for value in [
            Value::subset(inner.clone(), ctx),
            Value::product(vec![inner.clone(), inner.clone()], ctx),
            Value::fcn_set(Value::set(vec![1.into(), 2.into()], ctx)?, inner, ctx),
        ] {
            let expected = set(&value)?.elements(ctx)?;
            let actual: Vec<_> = set(&value)?
                .concrete_element_stream(ctx)?
                .ok_or("concrete cursor")?
                .collect();
            assert_eq!(actual.as_slice(), expected.as_ref());
            assert_eq!(actual.len(), 16);
        }
    }
    Ok(())
}

#[test]
fn outer_cardinality_is_unchanged_but_first_empty_does_not_size_the_outer_set() -> TestResult {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let inner = Value::subset(Value::interval(1.into(), 5.into(), ctx), ctx);
        let outer = Value::subset(inner, ctx);
        if ctx.int_mode == IntMode::Tlc {
            assert_eq!(
                set(&outer)?.size(ctx).err().ok_or("TLC overflow")?.kind(),
                ErrorKind::Overflow
            );
        } else {
            assert_eq!(set(&outer)?.size(ctx)?, 1_u64 << 32);
        }
        let mut cursor = set(&outer)?
            .concrete_element_stream(ctx)?
            .ok_or("nested cursor")?;
        assert_eq!(cursor.next(), Some(Value::empty_set(ctx)));
    }
    Ok(())
}

#[test]
fn required_component_size_and_raw_normalization_precede_first_output() -> TestResult {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let base = Value::subset(Value::interval(1.into(), 31.into(), ctx), ctx);
        let expected = set(&base)?
            .elements(ctx)
            .err()
            .ok_or("required component cannot materialize")?;
        let outer = Value::subset(base, ctx);
        let actual = set(&outer)?
            .concrete_element_stream(ctx)
            .err()
            .ok_or("component error before output")?;
        assert_eq!(actual.kind(), expected.kind());
        assert_eq!(actual.message(), expected.message());
        let raw = Value::set_raw(vec![2.into(), 1.into()], ctx);
        assert!(!set(&raw)?.is_tlc_normalized());
        let outer = Value::subset(raw.clone(), ctx);
        let mut cursor = set(&outer)?
            .concrete_element_stream(ctx)?
            .ok_or("raw component")?;
        assert!(set(&raw)?.is_tlc_normalized());
        assert_eq!(cursor.next(), Some(Value::empty_set(ctx)));
        let invalid = Value::subset(
            Value::set_raw(vec![Value::Int(1), Value::Bool(true)], ctx),
            ctx,
        );
        assert!(set(&invalid)?.concrete_element_stream(ctx).is_err());
    }
    Ok(())
}

#[derive(Debug)]
struct Probe(Arc<AtomicUsize>);
impl DeferredSet for Probe {
    fn kind(&self) -> DeferredKind {
        self.0.fetch_add(1, Ordering::Relaxed);
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(vec![1.into()])
    }
    fn member(&self, _value: &Value, _ctx: Ctx) -> Option<Result<bool>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Vec::new()
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Some(self)
    }
}

#[test]
fn hidden_provider_and_cross_order_storage_decline_without_callbacks_or_normalization() -> TestResult
{
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(Probe(calls.clone()));
        let hidden = Value::tuple(vec![Value::deferred(provider.clone(), ctx)]);
        let raw = Value::set_raw(vec![hidden], ctx);
        let outer = Value::subset(raw.clone(), ctx);
        let before = calls.load(Ordering::Relaxed);
        assert!(set(&outer)?.concrete_element_stream(ctx)?.is_none());
        assert_eq!(calls.load(Ordering::Relaxed), before);
        assert!(!set(&raw)?.is_tlc_normalized());
        assert_eq!(provider.elements(ctx)?, vec![1.into()]);
        assert_eq!(calls.load(Ordering::Relaxed), before + 1);
        let other = if ctx.order == crate::ValueOrder::Tlc {
            Ctx::dive()
        } else {
            Ctx::tlc()
        };
        let raw = Value::set_raw(vec![2.into(), 1.into()], other);
        let outer = Value::subset(raw.clone(), ctx);
        assert!(set(&outer)?.concrete_element_stream(ctx)?.is_none());
        assert!(!set(&raw)?.is_tlc_normalized());
    }
    Ok(())
}
