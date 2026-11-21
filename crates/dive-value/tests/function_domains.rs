//! Exact domain descriptors preserve values, checked errors and encodings.
#![allow(clippy::expect_used, reason = "test construction and exact assertions")]
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, DomainLookup, Int, Interner, Value, codec, tlc};

fn tuple(x: i64, y: i64) -> Value {
    Value::tuple(vec![Value::Int(x), Value::Int(y)])
}
fn fcn(pairs: Vec<(Value, Value)>, ctx: Ctx) -> dive_value::Fcn {
    let Value::Fcn(f) = Value::fcn(pairs, ctx).expect("function") else {
        unreachable!()
    };
    f
}

#[test]
fn dense_integer_and_cartesian_indices_match_checked_scans_with_fallbacks() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let keys = [tuple(-1, 3), tuple(-1, 4), tuple(0, 3), tuple(0, 4)];
        let function = fcn(
            keys.iter()
                .enumerate()
                .map(|(i, k)| {
                    (
                        k.clone(),
                        Value::from(i64::try_from(i).expect("small test index")),
                    )
                })
                .collect(),
            ctx,
        );
        for x in -2..2 {
            for y in 2..6 {
                let arg = tuple(x, y);
                let expected = function
                    .iter()
                    .position(|(k, _)| tlc::equals(&k, &arg, ctx).expect("integers compare"));
                assert_eq!(
                    function.checked_domain_index(&arg),
                    expected.map_or(DomainLookup::Missing, DomainLookup::Found)
                );
            }
        }
        assert_eq!(
            function.checked_domain_index(&Value::tuple(vec![Value::Int(99), Value::TRUE])),
            DomainLookup::Unknown
        );
        assert_eq!(
            function.checked_domain_index(&Value::tuple(vec![Value::Int(0)])),
            DomainLookup::Unknown
        );
        let integers = fcn(
            (-2..3)
                .map(|i| (Value::Int(i), Value::Int(i * 2)))
                .collect(),
            ctx,
        );
        for i in -4..5 {
            let want = (-2..3)
                .contains(&i)
                .then(|| usize::try_from(i + 2).expect("nonnegative"));
            assert_eq!(
                integers.checked_domain_index(&Value::Int(i)),
                want.map_or(DomainLookup::Missing, DomainLookup::Found)
            );
        }
        assert_eq!(
            integers.checked_domain_index(&Value::TRUE),
            DomainLookup::Unknown
        );
        let sparse = fcn(
            vec![
                (tuple(0, 0), Value::TRUE),
                (tuple(0, 1), Value::FALSE),
                (tuple(1, 0), Value::TRUE),
            ],
            ctx,
        );
        assert_eq!(
            sparse.checked_domain_index(&tuple(1, 1)),
            DomainLookup::Unknown
        );
        let huge = fcn(
            vec![
                (Value::Int(i64::MIN), Value::TRUE),
                (Value::Int(i64::MAX), Value::FALSE),
            ],
            ctx,
        );
        assert_eq!(
            huge.checked_domain_index(&Value::Int(0)),
            DomainLookup::Unknown
        );
    }
}

#[test]
fn exact_domain_checks_keep_mismatched_products_and_ranges_distinct() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let f = fcn(
            vec![
                (tuple(0, 3), Value::TRUE),
                (tuple(0, 4), Value::FALSE),
                (tuple(1, 3), Value::TRUE),
                (tuple(1, 4), Value::FALSE),
            ],
            ctx,
        );
        let product = |x, y| {
            Value::product(
                vec![
                    Value::interval(0.into(), x, ctx),
                    Value::interval(3.into(), y, ctx),
                ],
                ctx,
            )
        };
        let same = product(1.into(), 4.into());
        let shifted = Value::product(
            vec![
                Value::interval(1.into(), 2.into(), ctx),
                Value::interval(3.into(), 4.into(), ctx),
            ],
            ctx,
        );
        for (domain, expected) in [
            (same, true),
            (shifted, false),
            (product(1.into(), 3.into()), false),
        ] {
            assert_eq!(
                f.domain_equals(domain.as_set().expect("set")),
                Some(expected)
            );
            assert_eq!(
                tlc::member(
                    &Value::Fcn(f.clone()),
                    &Value::fcn_set(domain, Value::boolean_set(ctx), ctx),
                    ctx
                )
                .expect("membership"),
                expected
            );
        }
        let integers = fcn((0..3).map(|i| (Value::Int(i), Value::TRUE)).collect(), ctx);
        assert_eq!(
            integers.domain_equals(
                Value::interval(Int::ZERO, 2.into(), ctx)
                    .as_set()
                    .expect("interval")
            ),
            Some(true)
        );
        let bad_range = Value::fcn_set(
            Value::interval(Int::ZERO, 2.into(), ctx),
            Value::interval(Int::ZERO, Int::ONE, ctx),
            ctx,
        );
        assert!(
            tlc::member(&Value::Fcn(integers), &bad_range, ctx).is_err(),
            "domain proof must not skip range errors"
        );
    }
}

#[test]
fn same_domain_updates_match_reconstruction_hash_fingerprint_codec_and_context() {
    let names = Interner::new();
    let fp = TlcFingerprint::new(0).expect("polynomial");
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let cases = vec![
            Value::tuple(vec![Value::TRUE, Value::FALSE]),
            Value::tuple_fcn(vec![Value::TRUE, Value::FALSE]),
            Value::record(
                vec![
                    (names.intern("z"), Value::TRUE),
                    (names.intern("a"), Value::FALSE),
                ],
                ctx,
            )
            .expect("record"),
            Value::fcn(
                vec![
                    (Value::string(&names, "z"), Value::TRUE),
                    (Value::string(&names, "a"), Value::FALSE),
                ],
                ctx,
            )
            .expect("string function"),
            Value::fcn(
                vec![(Value::Int(0), Value::TRUE), (Value::Int(1), Value::FALSE)],
                ctx,
            )
            .expect("integers"),
            Value::fcn(
                vec![(tuple(0, 1), Value::TRUE), (tuple(1, 1), Value::FALSE)],
                ctx,
            )
            .expect("product"),
        ];
        for original in cases {
            let f = original.as_fcn().expect("function");
            let _ = fp
                .fingerprint_cached(&original, ctx)
                .expect("warm original FP");
            let updated = f
                .with_value_at(0, Value::Int(77), ctx)
                .expect("same domain");
            let mut pairs: Vec<_> = f.iter().map(|(k, v)| (k, v.clone())).collect();
            pairs[0].1 = Value::Int(77);
            let reference = if f.is_tuple_literal() {
                Value::tuple(pairs.into_iter().map(|(_, v)| v).collect())
            } else if f.is_record_literal() {
                Value::record(
                    f.as_record()
                        .expect("record")
                        .iter()
                        .enumerate()
                        .map(|(i, (n, v))| {
                            (n.clone(), if i == 0 { Value::Int(77) } else { v.clone() })
                        })
                        .collect(),
                    ctx,
                )
                .expect("record")
            } else {
                Value::fcn(pairs, ctx).expect("reconstruction")
            };
            let got = Value::Fcn(updated);
            assert_eq!(got, reference);
            assert_eq!(got.hash64(), reference.hash64());
            assert_eq!(
                fp.fingerprint(&got, ctx).expect("FP"),
                fp.fingerprint_cached(&reference, ctx).expect("FP")
            );
            let mut a = Vec::new();
            let mut b = Vec::new();
            codec::encode(&got, ctx, &mut a).expect("encode");
            codec::encode(&reference, ctx, &mut b).expect("encode");
            assert_eq!(a, b, "literal/order/representation must match");
            let decoded =
                codec::decode(&mut a.as_slice(), &mut |id| names.resolve(id)).expect("decode");
            assert_eq!(decoded, got);
            let mut other = ctx;
            other.order = if ctx.order == dive_value::ValueOrder::Tlc {
                dive_value::ValueOrder::Canonical
            } else {
                dive_value::ValueOrder::Tlc
            };
            if f.as_tuple().is_none() {
                assert!(f.with_value_at(0, Value::FALSE, other).is_err());
            }
        }
    }
}

#[test]
fn unknown_raw_and_provider_keys_do_not_gain_normalization_or_force_work() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct Source(Arc<AtomicUsize>);
    impl dive_value::DeferredSet for Source {
        fn kind(&self) -> dive_value::DeferredKind {
            dive_value::DeferredKind::Filter
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(vec![Value::Int(1)])
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            None
        }
    }
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let calls = Arc::new(AtomicUsize::new(0));
        let source = Value::deferred(Arc::new(Source(calls.clone())), ctx);
        let raw = Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx);
        for key in [source, raw.clone()] {
            let function = fcn(vec![(key, Value::TRUE)], ctx);
            calls.store(0, Ordering::Relaxed);
            for _ in 0..2 {
                assert_eq!(
                    function.checked_domain_index(&Value::Int(1)),
                    DomainLookup::Unknown
                );
                assert!(function.tlc_domain_indices().is_none());
                assert!(function.with_value_at(0, Value::FALSE, ctx).is_err());
            }
            assert_eq!(calls.load(Ordering::Relaxed), 0);
            assert!(!raw.as_set().expect("raw set").is_tlc_normalized());
        }
    }
}

#[test]
fn same_order_borrowed_views_preserve_provider_error_and_range_forcing_order() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct Failing(Arc<AtomicUsize>);
    impl dive_value::DeferredSet for Failing {
        fn kind(&self) -> dive_value::DeferredKind {
            dive_value::DeferredKind::Filter
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(dive_value::ValueError::new(
                dive_value::ErrorKind::NotEnumerable,
                "domain-view provider error",
            ))
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            None
        }
    }
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let calls = Arc::new(AtomicUsize::new(0));
        let source = Value::deferred(Arc::new(Failing(calls.clone())), ctx);
        let left = Value::fcn(vec![(Value::Int(0), source.clone())], ctx).expect("left");
        let different = Value::fcn(vec![(Value::Int(1), source.clone())], ctx).expect("different");
        calls.store(0, Ordering::Relaxed);
        assert!(!tlc::equals(&left, &different, ctx).expect("domain mismatch before range"));
        assert!(
            tlc::compare(&left, &different, ctx)
                .expect("domain comparison before range")
                .is_lt()
        );
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "borrowing must not force uninspected range"
        );
        let equal_domain =
            Value::fcn(vec![(Value::Int(0), Value::empty_set(ctx))], ctx).expect("equal domain");
        for _ in 0..2 {
            calls.store(0, Ordering::Relaxed);
            let error = tlc::equals(&left, &equal_domain, ctx)
                .expect_err("range provider remains observable");
            assert_eq!(error.kind(), dive_value::ErrorKind::NotEnumerable);
            assert_eq!(error.message(), "domain-view provider error");
            assert_eq!(calls.load(Ordering::Relaxed), 1);
        }
        let key_function = Value::fcn(vec![(source, Value::TRUE)], ctx).expect("unknown key");
        for _ in 0..2 {
            calls.store(0, Ordering::Relaxed);
            let error = tlc::equals(&key_function, &key_function, ctx)
                .expect_err("unknown-key comparison retains provider error");
            assert_eq!(error.message(), "domain-view provider error");
            assert_eq!(
                calls.load(Ordering::Relaxed),
                1,
                "same-order view neither forces early nor suppresses error"
            );
        }
    }
}

#[test]
fn model_domain_indices_match_checked_equality_across_interners_and_orders() {
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        for prefix in ["r", "A_r"] {
            let source = Interner::new();
            let query = Interner::new();
            let fnc = fcn(
                (0..64)
                    .rev()
                    .map(|i| {
                        (
                            Value::Model(source.intern(&format!("{prefix}{i}"))),
                            Value::Int(i),
                        )
                    })
                    .collect(),
                ctx,
            );
            for i in 0..65 {
                let key = Value::Model(query.intern(&format!("{prefix}{i}")));
                let actual = (0..fnc.len()).position(|j| {
                    let (stored, _) = fnc.entry(j).expect("table row");
                    tlc::equals(&stored, &key, ctx).expect("same model type")
                });
                let index = fnc.checked_domain_index(&key);
                if index != DomainLookup::Unknown {
                    assert_eq!(
                        index,
                        actual.map_or(DomainLookup::Missing, DomainLookup::Found)
                    );
                }
                if i < 64 {
                    assert_eq!(fnc.apply(&key), Some(&Value::Int(i)));
                }
            }
            assert_eq!(
                fnc.checked_domain_index(&Value::Int(0)),
                DomainLookup::Unknown
            );
            let other = Value::Model(query.intern("B_miss"));
            let index = fnc.checked_domain_index(&other);
            assert!(
                index == DomainLookup::Unknown || (prefix == "r" && index == DomainLookup::Missing)
            );
        }
    }
}

#[test]
fn a_token_sorted_domain_can_contain_duplicate_text_from_another_interner() {
    let original = Interner::new();
    let foreign = Interner::new();
    let mut pairs: Vec<_> = (0..39)
        .map(|i| {
            let _ = foreign.intern(&format!("filler{i}"));
            (
                Value::Model(original.intern(&format!("r{i:02}"))),
                Value::Int(i),
            )
        })
        .collect();
    let duplicate = Value::Model(foreign.intern("r00"));
    pairs.push((duplicate.clone(), Value::Int(39)));
    let table = fcn(pairs, Ctx::tlc());
    assert_eq!(table.len(), 40);
    assert_eq!(
        table.checked_domain_index(&duplicate),
        DomainLookup::Unknown
    );
    assert_eq!(table.apply(&duplicate), Some(&Value::Int(0)));
}
