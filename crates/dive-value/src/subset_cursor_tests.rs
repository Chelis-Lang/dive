//! Exhaustive original-order references and transactional nonconsumption.
#![allow(clippy::expect_used, reason = "bounded value/reference controls")]

use super::*;
use crate::{Ctx, Set};
fn set(value: &Value) -> &Set {
    match value {
        Value::Set(set) => Some(set),
        _ => None,
    }
    .expect("set fixture")
}
fn source(ctx: Ctx, size: usize) -> Value {
    Value::subset(
        Value::interval(
            1.into(),
            i64::try_from(size).expect("small fixture").into(),
            ctx,
        ),
        ctx,
    )
}
fn allowed_values(value: &Value, allowed: &[usize], ctx: Ctx) -> bool {
    set(value)
        .elements(ctx)
        .expect("bounded reference")
        .iter()
        .all(|value| {
            value
                .as_int()
                .and_then(|i| i.to_i64())
                .and_then(|i| usize::try_from(i - 1).ok())
                .is_some_and(|index| allowed.contains(&index))
        })
}
#[test]
fn every_mask_and_start_position_retains_the_original_ordered_subsequence() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let original = source(ctx, 4);
        // Independent original bulk result is generated before typed cursors.
        let reference = set(&original).elements(ctx).expect("complete reference");
        for bits in 0_u32..16 {
            let allowed: Vec<_> = (0..4).filter(|index| bits & (1 << index) != 0).collect();
            for consumed in 1..=reference.len() {
                let value = source(ctx, 4);
                let mut cursor = set(&value)
                    .concrete_subset_stream(ctx)
                    .expect("prepare")
                    .expect("positive");
                for expected in reference.iter().take(consumed) {
                    assert_eq!(cursor.next().as_ref(), Some(expected));
                }
                let expected: Vec<_> = reference
                    .iter()
                    .skip(consumed)
                    .filter(|value| allowed_values(value, &allowed, ctx))
                    .cloned()
                    .collect();
                let mut actual = Vec::new();
                let mut scratch = SubsetScratch::default();
                while cursor
                    .prepare_masked_next(&allowed, &mut scratch, || Ok(()))
                    .expect("pure seek")
                {
                    scratch
                        .prepare_commit(&mut cursor)
                        .expect("unchanged position")
                        .commit();
                    assert_eq!(cursor.current_indices(), None, "commit builds no Value");
                    let Some(value) = cursor.next() else {
                        break;
                    };
                    actual.push(value);
                }
                assert_eq!(actual, expected, "mask {bits}, prefix {consumed}");
            }
        }
    }
}

#[test]
fn abandoned_or_stale_commit_preserves_the_exact_unstarted_original_candidate() {
    let ctx = Ctx::tlc();
    let value = source(ctx, 4);
    let mut cursor = set(&value)
        .concrete_subset_stream(ctx)
        .expect("prepare")
        .expect("positive");
    let mut scratch = SubsetScratch::default();
    assert!(
        !cursor
            .prepare_masked_next(&[2, 3], &mut scratch, || Ok(()))
            .expect("before empty")
    );
    assert_eq!(cursor.next(), Some(Value::empty_set(ctx)));
    assert!(
        cursor
            .prepare_masked_next(&[2, 3], &mut scratch, || Ok(()))
            .expect("prepare target")
    );
    // A failed permission decision drops only exclusive references.
    {
        let _transaction = scratch.prepare_commit(&mut cursor).expect("ready");
    }
    assert_eq!(
        cursor.next(),
        Some(Value::set(vec![1.into()], ctx).expect("singleton"))
    );
    assert!(
        scratch.prepare_commit(&mut cursor).is_none(),
        "stale target must not commit"
    );
    assert_eq!(
        cursor.next(),
        Some(Value::set(vec![2.into()], ctx).expect("singleton"))
    );
    // Resume from the actual consumed position, without revisiting earlier work.
    assert!(
        cursor
            .prepare_masked_next(&[2, 3], &mut scratch, || Ok(()))
            .expect("resume")
    );
    scratch
        .prepare_commit(&mut cursor)
        .expect("new current")
        .commit();
    assert_eq!(
        cursor.next(),
        Some(Value::set(vec![3.into()], ctx).expect("singleton"))
    );
}

#[test]
fn cancellation_and_invalid_index_data_never_consume_the_source() {
    let ctx = Ctx::tlc();
    for stop in 0..12 {
        let value = source(ctx, 5);
        let mut cursor = set(&value)
            .concrete_subset_stream(ctx)
            .expect("prepare")
            .expect("positive");
        cursor.next().expect("empty");
        let mut scratch = SubsetScratch::default();
        let mut calls = 0;
        let result = cursor.prepare_masked_next(&[1, 3, 4], &mut scratch, || {
            calls += 1;
            if calls > stop {
                Err(ValueError::new(
                    ErrorKind::DiveLimit,
                    "original cancellation",
                ))
            } else {
                Ok(())
            }
        });
        if let Err(error) = result {
            assert_eq!(error.message(), "original cancellation");
            assert!(scratch.prepare_commit(&mut cursor).is_none());
        }
        assert!(calls > 0);
        assert_eq!(
            cursor.next(),
            Some(Value::set(vec![1.into()], ctx).expect("first singleton"))
        );
    }
    for allowed in [vec![2, 1], vec![1, 1], vec![5]] {
        let value = source(ctx, 5);
        let mut cursor = set(&value)
            .concrete_subset_stream(ctx)
            .expect("prepare")
            .expect("positive");
        cursor.next().expect("empty");
        assert_eq!(
            cursor
                .prepare_masked_next(&allowed, &mut SubsetScratch::default(), || Ok(()))
                .expect_err("invalid request")
                .kind(),
            ErrorKind::Argument
        );
        assert_eq!(
            cursor.next(),
            Some(Value::set(vec![1.into()], ctx).expect("first singleton"))
        );
    }
}

#[test]
fn typed_preparation_preserves_required_component_failure_and_explicit_size() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let value = Value::subset(source(ctx, 31), ctx);
        let original = set(&value)
            .concrete_element_stream(ctx)
            .err()
            .expect("component boundary");
        let actual = set(&value)
            .concrete_subset_stream(ctx)
            .err()
            .expect("same boundary");
        assert_eq!(
            (actual.kind(), actual.message()),
            (original.kind(), original.message())
        );
        let value = Value::subset(source(ctx, 5), ctx);
        let before = set(&value).size(ctx);
        let mut cursor = set(&value)
            .concrete_subset_stream(ctx)
            .expect("base32 prepared")
            .expect("positive");
        assert_eq!(cursor.base_elements().len(), 32);
        assert_eq!(cursor.next(), Some(Value::empty_set(ctx)));
        assert_eq!(set(&value).size(ctx), before);
    }
}
