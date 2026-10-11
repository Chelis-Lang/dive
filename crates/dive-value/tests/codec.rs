//! Property tests for the value codec (`dive_value::codec`) and the cached
//! fingerprint (`TlcFingerprint::fingerprint_cached`).
//!
//! - The lossless encoding round-trips every representation: the decoded value is
//!   equal, prints identically (printing depends on representation), and re-encodes
//!   to the same bytes.
//! - The canonical key is a perfect equality test: keys are equal exactly when the
//!   values are equal, whatever their representation.
//! - Both encodings are prefix-free and decoding never panics on arbitrary bytes.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use std::sync::{Arc, LazyLock};

use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::print::to_tlc_string_lenient;
use dive_value::{
    Ctx, DeferredKind, DeferredSet, Int, IntMode, Interner, Value, ValueOrder, codec, tlc,
};
use num_bigint::BigInt;
use proptest::prelude::*;

static NAMES: LazyLock<Interner> = LazyLock::new(|| {
    let names = Interner::new();
    for n in ["zz", "b", "a", "m2", "m1", "fy", "fx", "c", "T_q1"] {
        names.intern(n);
    }
    names
});

fn ctx_for(order: ValueOrder) -> Ctx {
    Ctx {
        int_mode: IntMode::Big,
        order,
    }
}

fn leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        (-4i64..5).prop_map(Value::Int),
        any::<i64>().prop_map(Value::Int),
        Just(Value::int(Int::from_big(BigInt::from(i64::MAX) * 4))),
        Just(Value::int(Int::from_big(BigInt::from(i64::MIN) * 3))),
        prop::sample::select(vec!["a", "b", "c", "zz", "ü", ""])
            .prop_map(|s| Value::string(&NAMES, s)),
        prop::sample::select(vec!["m1", "m2", "T_q1"]).prop_map(|s| Value::model(&NAMES, s)),
    ]
}

/// Values of every representation (except evaluator-driven sets, tested separately).
/// Constructions TLC refuses fall back to a tuple.
fn value(order: ValueOrder) -> impl Strategy<Value = Value> {
    let ctx = ctx_for(order);
    leaf().prop_recursive(3, 24, 4, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(move |v| Value::set(
                v.clone(),
                ctx
            )
            .unwrap_or_else(|_| Value::tuple(v))),
            prop::collection::vec(inner.clone(), 0..4).prop_map(move |v| Value::set_raw(v, ctx)),
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::tuple),
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::tuple_fcn),
            (inner.clone(), inner.clone()).prop_map(move |(x, y)| {
                Value::record(vec![(NAMES.intern("fx"), x), (NAMES.intern("fy"), y)], ctx)
                    .unwrap_or(Value::FALSE)
            }),
            // A string-domain function that is not a record literal.
            (inner.clone(), inner.clone()).prop_map(move |(x, y)| {
                Value::fcn(
                    vec![
                        (Value::string(&NAMES, "fx"), x),
                        (Value::string(&NAMES, "fy"), y),
                    ],
                    ctx,
                )
                .unwrap_or(Value::FALSE)
            }),
            (inner.clone(), inner.clone()).prop_map(move |(x, y)| Value::fcn(
                vec![(Value::Int(3), x), (Value::Int(7), y)],
                ctx
            )
            .unwrap_or(Value::TRUE)),
            (-3i64..3, -3i64..4).prop_map(move |(a, b)| Value::interval(a.into(), b.into(), ctx)),
            prop::collection::vec(-2i64..3, 0..3).prop_map(move |v| {
                let base = Value::set(v.into_iter().map(Value::Int).collect(), ctx)
                    .unwrap_or(Value::FALSE);
                Value::subset(base, ctx)
            }),
            (0i64..3, 0i64..3).prop_map(move |(a, b)| Value::fcn_set(
                Value::interval(Int::ONE, a.into(), ctx),
                Value::interval(Int::ONE, b.into(), ctx),
                ctx
            )),
            (0i64..3, 0i64..3).prop_map(move |(a, b)| Value::product(
                vec![
                    Value::interval(Int::ONE, a.into(), ctx),
                    Value::boolean_set(ctx),
                    Value::interval(Int::ZERO, b.into(), ctx),
                ],
                ctx
            )),
            (0i64..3).prop_map(move |a| Value::record_set(
                vec![
                    (NAMES.intern("fy"), Value::interval(Int::ONE, a.into(), ctx)),
                    (NAMES.intern("fx"), Value::boolean_set(ctx)),
                ],
                ctx
            )
            .unwrap_or(Value::FALSE)),
        ]
    })
}

/// Values that may include infinite sets (lossless encoding only).
fn any_value(order: ValueOrder) -> impl Strategy<Value = Value> {
    let ctx = ctx_for(order);
    prop_oneof![
        3 => value(order),
        1 => prop_oneof![
            Just(Value::nat(ctx)),
            Just(Value::int_set(ctx)),
            Just(Value::string_set(ctx)),
            Just(Value::seq_set(Value::boolean_set(ctx), ctx)),
            Just(Value::seq_set(Value::empty_set(ctx), ctx)),
        ],
        1 => (value(order), value(order)).prop_map(|(a, b)| Value::tuple(vec![a, b])),
    ]
}

fn encode(v: &Value, ctx: Ctx) -> Vec<u8> {
    let mut out = Vec::new();
    codec::encode(v, ctx, &mut out).expect("lossless encoding of a non-deferred value");
    out
}

fn key(v: &Value, ctx: Ctx) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    codec::encode_canonical(v, ctx, &mut out).ok()?;
    Some(out)
}

fn decode(bytes: &[u8]) -> Value {
    let mut input = bytes;
    let v = codec::decode(&mut input, &mut |t| NAMES.resolve(t)).expect("decodes");
    assert!(input.is_empty(), "decoding left {} bytes", input.len());
    v
}

const ORDERS: [ValueOrder; 2] = [ValueOrder::Tlc, ValueOrder::Canonical];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn frontier_bound_admits_lossless_values_without_underestimating(
        v in any_value(ValueOrder::Tlc),
        w in any_value(ValueOrder::Canonical),
    ) {
        for (v,order) in [(v,ValueOrder::Tlc),(w,ValueOrder::Canonical)] {
            let bound=codec::frontier_size_bound(&v,64*1024*1024).expect("callback-free finite representation");
            prop_assert!(encode(&v,ctx_for(order)).len() <= bound);
        }
    }

    #[test]
    fn lossless_round_trip_preserves_representation(
        v in any_value(ValueOrder::Tlc),
        w in any_value(ValueOrder::Canonical),
    ) {
        for (v, order) in [(v, ValueOrder::Tlc), (w, ValueOrder::Canonical)] {
            let ctx = ctx_for(order);
            let bytes = encode(&v, ctx);
            let back = decode(&bytes);
            prop_assert_eq!(&encode(&back, ctx), &bytes);
            prop_assert_eq!(&back, &v);
            // `==` changes no TLC-observable state, not even the `isNorm` flag of a
            // nested unnormalized set (issue #32): the encodings are unchanged.
            prop_assert_eq!(encode(&back, ctx), encode(&v, ctx));
            prop_assert_eq!(encode(&v, ctx), bytes);
            prop_assert_eq!(to_tlc_string_lenient(&back, ctx), to_tlc_string_lenient(&v, ctx));
        }
    }

    #[test]
    fn canonical_keys_decide_equality(a in value(ValueOrder::Tlc), b in value(ValueOrder::Tlc)) {
        for order in ORDERS {
            let ctx = ctx_for(order);
            if let (Some(ka), Some(kb)) = (key(&a, ctx), key(&b, ctx)) {
                prop_assert_eq!(ka == kb, a == b, "a = {:?}, b = {:?}", a, b);
                prop_assert_eq!(decode(&ka), a.clone());
            }
            prop_assert_eq!(key(&a, ctx), key(&a.clone(), ctx));
        }
    }

    #[test]
    fn canonical_keys_ignore_representation(v in value(ValueOrder::Tlc)) {
        // Decoding a key gives an equal value in canonical representation (intervals
        // for integer ranges, normalized sets for lazy ones): same key.
        for order in ORDERS {
            let ctx = ctx_for(order);
            if let Some(k) = key(&v, ctx) {
                let w = decode(&k);
                prop_assert_eq!(&w, &v);
                prop_assert_eq!(key(&w, ctx), Some(k));
            }
        }
    }

    #[test]
    fn encodings_are_prefix_free(vs in prop::collection::vec(value(ValueOrder::Tlc), 0..6)) {
        let ctx = ctx_for(ValueOrder::Tlc);
        let mut lossless = Vec::new();
        let mut canonical = Vec::new();
        let mut keyed = Vec::new();
        for v in &vs {
            codec::encode(v, ctx, &mut lossless).expect("encodes");
            if codec::encode_canonical(v, ctx, &mut canonical).is_ok() {
                keyed.push(v.clone());
            } else {
                // A failed canonical encoding may leave a partial suffix: restart.
                canonical.clear();
                keyed.clear();
            }
        }
        for (bytes, expect) in [(&lossless, &vs), (&canonical, &keyed)] {
            let mut input = bytes.as_slice();
            for v in expect {
                let got = codec::decode(&mut input, &mut |t| NAMES.resolve(t)).expect("decodes");
                prop_assert_eq!(&got, v);
            }
            prop_assert!(input.is_empty());
        }
    }

    #[test]
    fn decoding_garbage_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..64)) {
        let mut input = bytes.as_slice();
        let _ = codec::decode(&mut input, &mut |t| NAMES.resolve(t));
    }

    #[test]
    fn cached_fingerprints_equal_uncached(v in value(ValueOrder::Tlc)) {
        let f = TlcFingerprint::new(0).expect("polynomial 0");
        let g = TlcFingerprint::new(5).expect("polynomial 5");
        for order in ORDERS {
            let ctx = ctx_for(order);
            for f in [&f, &g, &f] {
                prop_assert_eq!(f.fingerprint_cached(&v, ctx).ok(), f.fingerprint(&v, ctx).ok());
            }
        }
    }
}

#[test]
fn representations_with_equal_values_share_a_key() {
    let ctx = Ctx::tlc();
    let pairs = [
        (
            Value::interval(Int::from(1), Int::from(3), ctx),
            Value::set(vec![Value::Int(2), Value::Int(1), Value::Int(3)], ctx),
        ),
        (
            Value::interval(Int::from(4), Int::from(2), ctx),
            Ok(Value::empty_set(ctx)),
        ),
        (
            Value::set_raw(vec![Value::Int(2), Value::Int(1), Value::Int(2)], ctx),
            Value::set(vec![Value::Int(1), Value::Int(2)], ctx),
        ),
        (
            Value::subset(Value::interval(Int::from(1), Int::from(2), ctx), ctx),
            Value::set(
                vec![
                    Value::empty_set(ctx),
                    Value::set(vec![Value::Int(1)], ctx).expect("set"),
                    Value::set(vec![Value::Int(2)], ctx).expect("set"),
                    Value::interval(Int::from(1), Int::from(2), ctx),
                ],
                ctx,
            ),
        ),
        (
            Value::record(vec![(NAMES.intern("fx"), Value::Int(1))], ctx).expect("record"),
            Value::fcn(vec![(Value::string(&NAMES, "fx"), Value::Int(1))], ctx),
        ),
    ];
    for (a, b) in pairs {
        let b = b.expect("value");
        assert_eq!(a, b);
        assert_eq!(key(&a, ctx), key(&b, ctx), "{a:?} vs {b:?}");
        // The lossless encodings differ: they keep the representation.
        assert_ne!(encode(&a, ctx), encode(&b, ctx), "{a:?} vs {b:?}");
    }
}

#[test]
fn huge_intervals_have_constant_size_keys() {
    let ctx = Ctx::tlc();
    let v = Value::interval(Int::from(0), Int::from(2_147_483_646), ctx);
    let k = key(&v, ctx).expect("an interval has a key");
    assert!(k.len() < 16);
    assert_eq!(decode(&k), v);
}

#[test]
fn structural_tuple_function_tag_preserves_kind_and_unchanged_quotient_keys() {
    for order in ORDERS {
        let ctx = ctx_for(order);
        for values in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Int(1), Value::Int(2)],
        ] {
            let literal = Value::tuple(values.clone());
            let ordinary = Value::fcn(
                values
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, v)| (Value::Int(i64::try_from(i + 1).expect("small domain")), v))
                    .collect(),
                ctx,
            )
            .expect("contiguous ordinary function");
            let function = Value::tuple_fcn(values);
            let literal_bytes = encode(&literal, ctx);
            let function_bytes = encode(&function, ctx);
            assert_eq!(literal_bytes[0], 0x08);
            assert_eq!(function_bytes[0], 0x0A);
            assert_eq!(&literal_bytes[1..], &function_bytes[1..]);
            assert_eq!(encode(&ordinary, ctx), function_bytes);
            let nested_literal = Value::tuple(vec![
                Value::record(vec![(NAMES.intern("fx"), literal.clone())], ctx).expect("record"),
                literal.clone(),
            ]);
            let nested_function = Value::tuple(vec![
                Value::record(vec![(NAMES.intern("fx"), ordinary.clone())], ctx).expect("record"),
                function.clone(),
            ]);
            let nested_bytes = encode(&nested_function, ctx);
            assert_eq!(encode(&decode(&nested_bytes), ctx), nested_bytes);
            assert_ne!(encode(&nested_literal, ctx), nested_bytes);
            assert_eq!(key(&nested_literal, ctx), key(&nested_function, ctx));
            assert!(matches!(decode(&literal_bytes), Value::Fcn(f) if f.is_tuple_literal()));
            assert!(
                matches!(decode(&function_bytes), Value::Fcn(f) if !f.is_tuple_literal() && f.as_tuple().is_some())
            );
            assert_eq!(key(&literal, ctx), key(&function, ctx));
            assert_eq!(key(&function, ctx).expect("unchanged key"), literal_bytes);
            for polynomial in [0, 5] {
                let fp = TlcFingerprint::new(polynomial).expect("polynomial");
                assert_eq!(
                    fp.fingerprint(&literal, ctx),
                    fp.fingerprint(&function, ctx)
                );
                assert_eq!(
                    fp.fingerprint(&literal, ctx),
                    fp.fingerprint(&ordinary, ctx)
                );
                assert_eq!(
                    fp.fingerprint(&nested_literal, ctx),
                    fp.fingerprint(&nested_function, ctx)
                );
            }
            for cut in 0..function_bytes.len() {
                assert!(codec::decode(&mut &function_bytes[..cut], &mut |_| None).is_err());
            }
        }
    }
    // Old ambiguous bytes can only mean a literal tuple. A new decoder cannot
    // infer that an old writer discarded the function's literal bit.
    assert!(matches!(decode(&[0x08, 1, 2, 2]), Value::Fcn(f) if f.is_tuple_literal()));
    assert!(matches!(decode(&[0x0A, 1, 2, 2]), Value::Fcn(f) if !f.is_tuple_literal()));
}

#[test]
fn canonical_keys_fail_where_fingerprints_fail() {
    let ctx = Ctx::tlc();
    let f = TlcFingerprint::new(0).expect("polynomial 0");
    for v in [
        Value::nat(ctx),
        Value::tuple(vec![Value::int_set(ctx)]),
        Value::seq_set(Value::boolean_set(ctx), ctx),
        Value::set_raw(vec![Value::Int(1), Value::string(&NAMES, "a")], ctx),
    ] {
        let fp = f.fingerprint(&v, ctx).map_err(|e| e.message().to_owned());
        let mut out = Vec::new();
        let k = codec::encode_canonical(&v, ctx, &mut out).map_err(|e| e.message().to_owned());
        assert!(fp.is_err());
        assert_eq!(k.err(), fp.err());
    }
}

#[test]
fn unnormalized_sets_keep_tlc_semantics() {
    let ctx = Ctx::tlc();
    // `1 \in {1, "a"}` is TRUE while the set is unnormalized: TLC scans it linearly.
    let raw = Value::set_raw(vec![Value::Int(1), Value::string(&NAMES, "a")], ctx);
    let back = decode(&encode(&raw, ctx));
    assert_eq!(tlc::member(&Value::Int(1), &back, ctx), Ok(true));
    // Once TLC has normalized a set, membership is a binary search.
    let norm = Value::set_raw(
        vec![
            Value::record(vec![(NAMES.intern("fx"), Value::Int(1))], ctx).expect("record"),
            Value::record(vec![(NAMES.intern("fx"), Value::Int(2))], ctx).expect("record"),
        ],
        ctx,
    );
    let f = TlcFingerprint::new(0).expect("polynomial 0");
    f.fingerprint(&norm, ctx).expect("fingerprints");
    let tuple = Value::tuple(vec![Value::Int(1)]);
    let before = tlc::member(&tuple, &norm, ctx);
    let back = decode(&encode(&norm, ctx));
    assert_eq!(tlc::member(&tuple, &back, ctx), before);
}

#[derive(Debug)]
struct Evens(i64);

impl DeferredSet for Evens {
    fn kind(&self) -> DeferredKind {
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        Ok((0..self.0).map(|i| Value::Int(2 * i)).collect())
    }
    fn member(&self, _elem: &Value, _ctx: Ctx) -> Option<dive_value::Result<bool>> {
        None
    }
}

#[test]
fn deferred_sets_are_written_as_their_elements() {
    let ctx = Ctx::tlc();
    let v = Value::deferred(Arc::new(Evens(3)), ctx);
    let back = decode(&encode(&v, ctx));
    assert_eq!(back, v);
    let e = Value::set(vec![Value::Int(0), Value::Int(2), Value::Int(4)], ctx).expect("set");
    assert_eq!(encode(&back, ctx), encode(&e, ctx));
    assert_eq!(key(&v, ctx), key(&e, ctx));
}

#[test]
fn unknown_tokens_are_errors() {
    let ctx = Ctx::tlc();
    let bytes = encode(&Value::string(&NAMES, "zz"), ctx);
    let r = codec::decode(&mut bytes.as_slice(), &mut |_| None);
    assert_eq!(r.map_err(|e| e.kind()), Err(dive_value::ErrorKind::Codec));
}

#[derive(Debug)]
struct NoCallback(Arc<std::sync::atomic::AtomicUsize>);
impl DeferredSet for NoCallback {
    fn kind(&self) -> DeferredKind {
        DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![Value::TRUE])
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(Ok(true))
    }
}
#[test]
fn frontier_admission_never_invokes_nested_deferred_callbacks() {
    use std::sync::atomic::Ordering;
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let lazy = Value::deferred(Arc::new(NoCallback(calls.clone())), ctx);
        // Direct and Raw constructors do not hash/enumerate the child.
        for fresh in [lazy.clone(), Value::set_raw(vec![lazy.clone()], ctx)] {
            assert_eq!(calls.load(Ordering::Relaxed), 0);
            assert!(codec::frontier_size_bound(&fresh, 64 * 1024 * 1024).is_err());
            assert_eq!(calls.load(Ordering::Relaxed), 0);
        }
        // Existing tuple/record constructors hash children, so measure their
        // construction separately. Admission must add exactly zero callbacks,
        // even when construction has already cached deferred elements.
        let tuple = Value::tuple(vec![lazy.clone()]);
        let record = Value::record(vec![(NAMES.intern("frontier"), lazy)], ctx).expect("record");
        let nested = Value::tuple(vec![Value::set_raw(vec![tuple.clone()], ctx)]);
        for constructed in [tuple, record, nested] {
            let before = calls.load(Ordering::Relaxed);
            eprintln!("constructor baseline {ctx:?}: {before} callbacks");
            assert!(codec::frontier_size_bound(&constructed, 64 * 1024 * 1024).is_err());
            assert_eq!(calls.load(Ordering::Relaxed), before);
        }
        assert!(
            codec::frontier_size_bound(&Value::tuple(vec![Value::TRUE; 65537]), usize::MAX)
                .is_err()
        );
        assert!(codec::frontier_size_bound(&Value::Int(1), 0).is_err());
    }
}
