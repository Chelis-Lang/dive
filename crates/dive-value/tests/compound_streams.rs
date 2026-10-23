//! Component normalization and bounded compound output are separate operations.
#![allow(clippy::expect_used, reason = "explicit finite value fixtures")]

use dive_value::{Ctx, Int, Value};

#[test]
fn a_partial_compound_stream_normalizes_components_without_caching_all_outputs() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let base = Value::set_raw(vec![Value::Int(2), Value::Int(1), Value::Int(2)], ctx);
        let Value::Set(base_set) = &base else {
            unreachable!()
        };
        assert!(!base_set.is_tlc_normalized());
        let Value::Set(subsets) = Value::subset(base.clone(), ctx) else {
            unreachable!()
        };
        let mut stream = subsets
            .concrete_element_stream(ctx)
            .expect("concrete capability check")
            .expect("normalized base");
        assert!(base_set.is_tlc_normalized());
        assert_eq!(stream.next(), Some(Value::set(vec![], ctx).expect("empty")));
        assert_eq!(
            stream.next(),
            Some(Value::set(vec![Value::Int(1)], ctx).expect("singleton"))
        );
        drop(stream);
        assert!(subsets.stored_elements().is_none());
        assert_eq!(
            subsets
                .elements(ctx)
                .expect("later complete enumeration")
                .len(),
            4
        );
    }
}

#[test]
fn streaming_does_not_request_the_complete_compound_cardinality() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let base = Value::interval(Int::ONE, Int::from(31), ctx);
        let Value::Set(subsets) = Value::subset(base.clone(), ctx) else {
            unreachable!()
        };
        assert_eq!(
            subsets
                .concrete_element_stream(ctx)
                .expect("concrete capability check")
                .expect("base admits enumeration")
                .next(),
            Some(Value::set(vec![], ctx).expect("empty"))
        );
        // Normalized bulk callers retain their original size/overflow contract.
        assert!(subsets.elements(ctx).is_err());
    }
}

fn set(value: &Value) -> &dive_value::Set {
    let Value::Set(set) = value else {
        unreachable!()
    };
    set
}

#[test]
fn empty_product_and_record_skip_mixed_scalar_normalization() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        for record in [false, true] {
            let mixed = Value::set_raw(vec![Value::Int(1), Value::TRUE], ctx);
            let empty = Value::set_raw(vec![], ctx);
            let domain = if record {
                let names = dive_value::Interner::new();
                Value::record_set(
                    vec![
                        (names.intern("a"), mixed.clone()),
                        (names.intern("b"), empty),
                    ],
                    ctx,
                )
                .expect("record domain")
            } else {
                Value::product(vec![mixed.clone(), empty], ctx)
            };
            assert!(
                set(&domain)
                    .concrete_element_stream(ctx)
                    .expect("constructor")
                    .expect("scalar components")
                    .next()
                    .is_none()
            );
            assert!(!set(&mixed).is_tlc_normalized());
        }
    }
}

#[test]
fn raw_constructor_short_circuit_preserves_interval_size_error_priority() {
    let ctx = Ctx::tlc();
    let mixed = Value::set_raw(vec![Value::Int(1), Value::TRUE], ctx);
    let huge = Value::interval(Int::ZERO, Int::from(i64::from(i32::MAX)), ctx);
    let product = Value::product(vec![mixed.clone(), huge.clone()], ctx);
    let Err(error) = set(&product).concrete_element_stream(ctx) else {
        unreachable!()
    };
    assert_eq!(error.kind(), dive_value::ErrorKind::TooLarge);
    assert!(
        error
            .message()
            .starts_with("Size of interval value exceeds")
    );
    assert!(!set(&mixed).is_tlc_normalized());
    let empty_first = Value::product(vec![Value::set_raw(vec![], ctx), huge.clone()], ctx);
    assert!(
        set(&empty_first)
            .concrete_element_stream(ctx)
            .expect("first empty masks later size")
            .expect("scalar components")
            .next()
            .is_none()
    );
    let functions = Value::fcn_set(huge, Value::set_raw(vec![], ctx), ctx);
    assert!(set(&functions).concrete_element_stream(ctx).is_err());
}

#[test]
fn empty_function_domain_normalizes_only_the_domain_and_yields_a_function() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let empty = Value::set_raw(vec![], ctx);
        let mixed = Value::set_raw(vec![Value::Int(1), Value::TRUE], ctx);
        let functions = Value::fcn_set(empty.clone(), mixed.clone(), ctx);
        let mut stream = set(&functions)
            .concrete_element_stream(ctx)
            .expect("constructor")
            .expect("scalar components");
        let Value::Fcn(function) = stream.next().expect("empty function") else {
            unreachable!()
        };
        assert!(!function.is_tuple_literal());
        assert!(!function.is_record_literal());
        assert!(function.is_empty());
        assert!(stream.next().is_none());
        assert!(set(&empty).is_tlc_normalized());
        assert!(!set(&mixed).is_tlc_normalized());
        let no_functions = Value::fcn_set(mixed.clone(), Value::set_raw(vec![], ctx), ctx);
        assert!(
            set(&no_functions)
                .concrete_element_stream(ctx)
                .expect("empty range before domain normalization")
                .expect("scalar components")
                .next()
                .is_none()
        );
        assert!(!set(&mixed).is_tlc_normalized());
    }
}

#[derive(Debug)]
struct Provider(std::sync::atomic::AtomicUsize);
impl dive_value::DeferredSet for Provider {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![])
    }
    fn member(&self, _elem: &Value, _ctx: Ctx) -> Option<dive_value::Result<bool>> {
        None
    }
}

#[test]
fn an_ineligible_nested_provider_does_not_normalize_any_prior_component() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let provider = std::sync::Arc::new(Provider(std::sync::atomic::AtomicUsize::new(0)));
        let nested = Value::set_raw(vec![Value::deferred(provider.clone(), ctx)], ctx);
        let first = Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx);
        let product = Value::product(vec![first.clone(), nested.clone()], ctx);
        assert!(
            set(&product)
                .concrete_element_stream(ctx)
                .expect("read-only capability")
                .is_none()
        );
        assert!(!set(&first).is_tlc_normalized());
        assert!(!set(&nested).is_tlc_normalized());
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
}
