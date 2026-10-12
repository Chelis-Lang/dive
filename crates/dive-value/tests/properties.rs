//! Property tests for the value layer: the order is total and agrees with `==`,
//! hashing agrees with `==`, normalization is idempotent, set operations obey the
//! algebraic laws, lazy membership equals enumerated membership, and both integer
//! modes are exact.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Int, IntMode, Interner, Value, ValueOrder, ops, tlc};
use num_bigint::BigInt;
use proptest::prelude::*;

static NAMES: LazyLock<Interner> = LazyLock::new(|| {
    let names = Interner::new();
    // Intern in an order that differs from the lexicographic one.
    for n in ["zz", "b", "a", "m2", "m1", "fy", "fx", "c"] {
        names.intern(n);
    }
    names
});

const ORDERS: [ValueOrder; 2] = [ValueOrder::Tlc, ValueOrder::Canonical];

fn ctx_for(order: ValueOrder) -> Ctx {
    Ctx {
        int_mode: IntMode::Big,
        order,
    }
}

fn hash_of(v: &Value) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        (-4i64..5).prop_map(Value::Int),
        prop_oneof![
            Just(i64::MIN),
            Just(i64::MAX),
            Just(i64::from(i32::MIN)),
            Just(i64::from(i32::MAX))
        ]
        .prop_map(Value::Int),
        Just(Value::int(Int::from_big(BigInt::from(i64::MAX) * 4))),
        prop::sample::select(vec!["a", "b", "c", "zz"]).prop_map(|s| Value::string(&NAMES, s)),
        prop::sample::select(vec!["m1", "m2"]).prop_map(|s| Value::model(&NAMES, s)),
    ]
}

/// Arbitrary values, possibly mixing kinds; set construction that TLC would refuse
/// is skipped (`Value::set` errors) by falling back to a tuple.
fn value(order: ValueOrder) -> impl Strategy<Value = Value> {
    let ctx = ctx_for(order);
    leaf().prop_recursive(3, 24, 4, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(move |v| Value::set(
                v.clone(),
                ctx
            )
            .unwrap_or_else(|_| Value::tuple(v))),
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::tuple),
            (inner.clone(), inner.clone()).prop_map(move |(x, y)| {
                Value::record(vec![(NAMES.intern("fx"), x), (NAMES.intern("fy"), y)], ctx)
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
        ]
    })
}

fn int_set(order: ValueOrder) -> impl Strategy<Value = Value> {
    prop::collection::vec(-6i64..6, 0..7).prop_map(move |v| {
        Value::set(v.into_iter().map(Value::Int).collect(), ctx_for(order)).unwrap_or(Value::FALSE)
    })
}

fn tlc_order(a: &Value, b: &Value) -> Ordering {
    a.cmp_with(b, ValueOrder::Tlc)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn order_is_total_and_agrees_with_eq(a in value(ValueOrder::Tlc), b in value(ValueOrder::Tlc), c in value(ValueOrder::Tlc)) {
        let (ab, ba) = (tlc_order(&a, &b), tlc_order(&b, &a));
        prop_assert_eq!(ab, ba.reverse());
        prop_assert_eq!(ab == Ordering::Equal, a == b);
        prop_assert_eq!(tlc_order(&a, &a), Ordering::Equal);
        if ab != Ordering::Greater && tlc_order(&b, &c) != Ordering::Greater {
            prop_assert_ne!(tlc_order(&a, &c), Ordering::Greater);
        }
    }

    #[test]
    fn canonical_order_is_total_and_agrees_with_eq(a in value(ValueOrder::Canonical), b in value(ValueOrder::Canonical)) {
        let o = ValueOrder::Canonical;
        prop_assert_eq!(a.cmp_with(&b, o), b.cmp_with(&a, o).reverse());
        prop_assert_eq!(a.cmp_with(&b, o) == Ordering::Equal, a == b);
    }

    #[test]
    fn hash_agrees_with_eq(a in value(ValueOrder::Tlc), b in value(ValueOrder::Tlc)) {
        if a == b {
            prop_assert_eq!(hash_of(&a), hash_of(&b));
        }
        let c = a.clone();
        prop_assert!(a == c);
        prop_assert_eq!(hash_of(&a), hash_of(&c));
    }

    #[test]
    fn tlc_checked_ops_agree_with_total_order(a in value(ValueOrder::Tlc), b in value(ValueOrder::Tlc)) {
        let ctx = ctx_for(ValueOrder::Tlc);
        if let Ok(o) = tlc::compare(&a, &b, ctx) {
            prop_assert_eq!(o, tlc_order(&a, &b));
        }
        if let Ok(e) = tlc::equals(&a, &b, ctx) {
            prop_assert_eq!(e, a == b);
        }
    }

    #[test]
    fn normalization_is_idempotent_and_order_independent(
        elems in prop::collection::vec(value(ValueOrder::Tlc), 0..6),
        seed in any::<u64>(),
    ) {
        let ctx = ctx_for(ValueOrder::Tlc);
        if let Ok(s) = Value::set(elems.clone(), ctx) {
            let e = s.as_set().expect("set").elements(ctx).expect("enumerable");
            // TLC's comparability is asymmetric (`RecordValue.compareTo(tuple)` fails
            // where `TupleValue.compareTo(record)` succeeds), so re-sorting sorted
            // elements can fail in TLC, and in dive's port: fixture AsymSetTupleFirst.
            // Whenever it succeeds it is the identity.
            if let Ok(again) = Value::set(e.to_vec(), ctx) {
                prop_assert!(s == again);
                prop_assert_eq!(&*e, &*again.as_set().expect("set").elements(ctx).expect("enumerable"));
            }
            // Re-sorting under the total order (the comparable case) is the identity.
            let mut sorted = e.to_vec();
            sorted.sort_by(tlc_order);
            prop_assert_eq!(&sorted[..], &*e);
            // Sorted, without duplicates.
            for w in e.windows(2) {
                prop_assert_eq!(tlc_order(&w[0], &w[1]), Ordering::Less);
            }
            // A permutation gives the same set (when all pairs are comparable it
            // cannot fail, otherwise skip).
            let mut perm = elems;
            let n = perm.len();
            if n > 1 {
                perm.rotate_left(usize::try_from(seed % n as u64).unwrap_or(0));
            }
            if let Ok(t) = Value::set(perm, ctx) {
                prop_assert!(s == t);
            }
        }
    }

    #[test]
    fn set_algebra_laws(a in int_set(ValueOrder::Tlc), b in int_set(ValueOrder::Tlc), c in int_set(ValueOrder::Tlc)) {
        let ctx = ctx_for(ValueOrder::Tlc);
        let u = |x: &Value, y: &Value| ops::union(x, y, ctx).expect("union");
        let i = |x: &Value, y: &Value| ops::intersect(x, y, ctx).expect("intersect");
        let d = |x: &Value, y: &Value| ops::difference(x, y, ctx).expect("difference");
        let size = |x: &Value| x.as_set().expect("set").size(ctx).expect("size");
        prop_assert!(u(&a, &b) == u(&b, &a));
        prop_assert!(i(&a, &b) == i(&b, &a));
        prop_assert!(u(&u(&a, &b), &c) == u(&a, &u(&b, &c)));
        prop_assert!(i(&i(&a, &b), &c) == i(&a, &i(&b, &c)));
        prop_assert!(u(&a, &a) == a);
        prop_assert!(i(&a, &u(&a, &b)) == a);
        prop_assert!(i(&a, &u(&b, &c)) == u(&i(&a, &b), &i(&a, &c)));
        prop_assert!(u(&d(&a, &b), &i(&a, &b)) == a);
        prop_assert_eq!(size(&u(&a, &b)) + size(&i(&a, &b)), size(&a) + size(&b));
        prop_assert!(d(&a, &a) == Value::empty_set(ctx));
    }

    #[test]
    fn lazy_membership_equals_enumerated_membership(
        lo in -3i64..3, hi in -3i64..4,
        base in prop::collection::vec(-2i64..3, 0..4),
        rng in prop::collection::vec(0i64..3, 1..3),
        cands in prop::collection::vec(value(ValueOrder::Tlc), 0..6),
    ) {
        let ctx = ctx_for(ValueOrder::Tlc);
        let ints = |v: &[i64]| Value::set(v.iter().copied().map(Value::Int).collect(), ctx).expect("ints");
        let (b, r) = (ints(&base), ints(&rng));
        let lazies = [
            Value::interval(lo.into(), hi.into(), ctx),
            Value::subset(b.clone(), ctx),
            Value::fcn_set(b.clone(), r.clone(), ctx),
            Value::product(vec![b.clone(), r.clone()], ctx),
            Value::record_set(vec![(NAMES.intern("fx"), b), (NAMES.intern("fy"), r)], ctx).expect("rcdset"),
        ];
        for lazy in &lazies {
            let elems = lazy.as_set().expect("set").elements(ctx).expect("small");
            let enumerated = Value::set(elems.to_vec(), ctx).expect("normalized");
            prop_assert!(*lazy == enumerated);
            prop_assert_eq!(hash_of(lazy), hash_of(&enumerated));
            let fp = TlcFingerprint::new(0).expect("poly");
            prop_assert_eq!(fp.fingerprint(lazy, ctx).expect("fp"), fp.fingerprint(&enumerated, ctx).expect("fp"));
            let mut all: Vec<Value> = elems.to_vec();
            all.extend(cands.iter().cloned());
            for x in &all {
                let m1 = tlc::member(x, lazy, ctx);
                let m2 = tlc::member(x, &enumerated, ctx);
                if let (Ok(m1), Ok(m2)) = (&m1, &m2) {
                    prop_assert_eq!(m1, m2, "{:?} in {:?}", x, lazy);
                }
                if elems.contains(x) {
                    prop_assert_eq!(m1.ok(), Some(true));
                }
            }
        }
    }

    #[test]
    fn tlc_overflow_is_exact(a in boundary_i32(), b in boundary_i32()) {
        let (x, y) = (Int::from(a), Int::from(b));
        let m = IntMode::Tlc;
        let (wa, wb) = (i128::from(a), i128::from(b));
        let fits = |r: i128| i32::try_from(r).is_ok();
        let check = |got: Result<Int, dive_value::ValueError>, want: Option<i128>| match (got, want) {
            (Ok(g), Some(w)) => fits(w) && g.to_i64().map(i128::from) == Some(w),
            (Err(e), Some(w)) => !fits(w) && e.kind() == dive_value::ErrorKind::Overflow,
            (Err(_), None) => true,
            (Ok(_), None) => false,
        };
        prop_assert!(check(x.add(&y, m), Some(wa + wb)));
        prop_assert!(check(x.sub(&y, m), Some(wa - wb)));
        prop_assert!(check(x.mul(&y, m), Some(wa * wb)));
        prop_assert!(check(x.neg(m), Some(-wa)));
        if b != 0 {
            prop_assert!(check(x.div(&y, m), Some(num_integer::Integer::div_floor(&wa, &wb))));
        }
        if b > 0 {
            prop_assert!(check(x.rem(&y, m), Some(num_integer::Integer::mod_floor(&wa, &wb))));
        }
        if (0..40).contains(&b) {
            let want = BigInt::from(a).pow(u32::try_from(b).unwrap_or(0));
            let want = i128::try_from(want).ok();
            if !(a == 0 && b == 0) {
                prop_assert!(check(x.pow(&y, m), Some(want.unwrap_or(i128::MAX))));
            }
        }
    }

    #[test]
    fn big_mode_is_mathematical(a in any::<i64>(), b in any::<i64>(), e in 0u32..70) {
        let (x, y) = (Int::from(a), Int::from(b));
        let m = IntMode::Big;
        let (ba, bb) = (BigInt::from(a), BigInt::from(b));
        prop_assert_eq!(x.add(&y, m).expect("add").to_big(), &ba + &bb);
        prop_assert_eq!(x.sub(&y, m).expect("sub").to_big(), &ba - &bb);
        prop_assert_eq!(x.mul(&y, m).expect("mul").to_big(), &ba * &bb);
        prop_assert_eq!(x.neg(m).expect("neg").to_big(), -&ba);
        if b != 0 {
            prop_assert_eq!(x.div(&y, m).expect("div").to_big(), num_integer::Integer::div_floor(&ba, &bb));
        }
        if b > 0 {
            prop_assert_eq!(x.rem(&y, m).expect("rem").to_big(), num_integer::Integer::mod_floor(&ba, &bb));
        }
        if a != 0 || e != 0 {
            let p = x.pow(&Int::from(i64::from(e)), m).expect("pow");
            prop_assert_eq!(p.to_big(), ba.pow(e));
        }
        // Canonical representation: equal integers are equal values.
        let v = Value::int(x.mul(&y, m).expect("mul").div(&y.add(&Int::ONE, m).expect("add"), m).unwrap_or(Int::ZERO));
        prop_assert_eq!(v.clone(), Value::int(Int::from_big(v.as_int().expect("int").to_big())));
    }
}

fn boundary_i32() -> impl Strategy<Value = i32> {
    prop_oneof![
        Just(i32::MIN),
        Just(i32::MIN + 1),
        Just(i32::MAX),
        Just(i32::MAX - 1),
        Just(-1),
        Just(0),
        Just(1),
        Just(2),
        Just(46_341),
        Just(-46_341),
        Just(65_536),
        any::<i32>(),
        -40i32..40,
    ]
}

#[test]
fn both_orders_build_the_same_sets() {
    for order in ORDERS {
        let ctx = ctx_for(order);
        let s = Value::set(
            vec![Value::string(&NAMES, "a"), Value::string(&NAMES, "zz")],
            ctx,
        )
        .expect("strings");
        let first = s.as_set().expect("set").elements(ctx).expect("enum")[0].clone();
        let want = if order == ValueOrder::Tlc { "zz" } else { "a" };
        assert!(first == Value::string(&NAMES, want), "{order:?}");
    }
}

/// Records over a few field-name sets, with scalar or record fields: the
/// shapes the per-field record comparability rule must get right.
fn record_value(order: ValueOrder) -> impl Strategy<Value = Value> {
    let ctx = ctx_for(order);
    let field = prop_oneof![
        leaf(),
        (leaf(), leaf()).prop_map(move |(x, y)| {
            Value::record(vec![(NAMES.intern("fx"), x), (NAMES.intern("fy"), y)], ctx)
                .unwrap_or(Value::FALSE)
        }),
        Just(Value::tuple(Vec::new())),
    ];
    (
        prop::sample::subsequence(vec!["fx", "fy", "c"], 1..=3),
        prop::collection::vec(field, 3),
    )
        .prop_map(move |(names, fields)| {
            Value::record(
                names
                    .iter()
                    .zip(fields)
                    .map(|(n, v)| (NAMES.intern(n), v))
                    .collect(),
                ctx,
            )
            .unwrap_or(Value::FALSE)
        })
}

fn record_or_value(order: ValueOrder) -> impl Strategy<Value = Value> {
    prop_oneof![3 => record_value(order), 1 => value(order)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// Elements the shape test calls comparable compare in TLC without error,
    /// in both directions, and as the total order does.
    #[test]
    fn comparable_elements_compare_without_error(
        elems in prop::collection::vec(record_or_value(ValueOrder::Tlc), 0..6),
    ) {
        let ctx = ctx_for(ValueOrder::Tlc);
        if Value::elements_comparable(&elems) {
            for a in &elems {
                for b in &elems {
                    let c = tlc::compare(a, b, ctx);
                    prop_assert!(c.is_ok(), "{a:?} vs {b:?}: {c:?}");
                    prop_assert_eq!(c.ok(), Some(a.cmp_with(b, ValueOrder::Tlc)));
                }
            }
        }
    }

    /// The selection-based comparisons of a set given by its distinct
    /// elements agree with building the set and comparing it in full.
    #[test]
    fn distinct_set_comparison_matches_the_built_set_tlc(
        elems in prop::collection::vec(record_value(ValueOrder::Tlc), 0..7),
        others in prop::collection::vec(record_value(ValueOrder::Tlc), 0..7),
        reuse in any::<bool>(),
    ) {
        distinct_set_case(ValueOrder::Tlc, &elems, &others, reuse)?;
    }

    #[test]
    fn distinct_set_comparison_matches_the_built_set_canonical(
        elems in prop::collection::vec(record_value(ValueOrder::Canonical), 0..7),
        others in prop::collection::vec(record_value(ValueOrder::Canonical), 0..7),
        reuse in any::<bool>(),
    ) {
        distinct_set_case(ValueOrder::Canonical, &elems, &others, reuse)?;
    }
}

fn distinct_set_case(
    order: ValueOrder,
    elems: &[Value],
    others: &[Value],
    reuse: bool,
) -> Result<(), TestCaseError> {
    let ctx = ctx_for(order);
    let mut distinct: Vec<Value> = Vec::new();
    for e in elems {
        if !distinct.contains(e) {
            distinct.push(e.clone());
        }
    }
    if !Value::elements_comparable(&distinct) {
        return Ok(());
    }
    let other = if reuse {
        let mut v = distinct.clone();
        v.reverse();
        Value::set_raw(v, ctx)
    } else {
        Value::set_raw(others.to_vec(), ctx)
    };
    let refs: Vec<&Value> = distinct.iter().collect();
    let built = Value::set_raw(distinct.clone(), ctx);
    if order == ValueOrder::Tlc {
        let fast = tlc::compare_distinct_set(&refs, &other, ctx);
        let full = tlc::compare(&built, &other, ctx);
        match (fast, full) {
            (Ok(Some(f)), Ok(g)) => prop_assert_eq!(f, g),
            (Err(f), Err(g)) => prop_assert_eq!(f.message(), g.message()),
            (f, g) => prop_assert!(false, "fast {f:?}, full {g:?}"),
        }
    } else if let Some(f) = Value::cmp_distinct_set(&refs, &other, order) {
        prop_assert_eq!(f, built.cmp_with(&other, order));
    }
    Ok(())
}

#[test]
fn records_with_mixed_field_types_are_comparable_field_by_field() {
    let ctx = ctx_for(ValueOrder::Tlc);
    let rcd = |fields: &[(&str, Value)]| {
        Value::record(
            fields
                .iter()
                .map(|(n, v)| (NAMES.intern(n), v.clone()))
                .collect(),
            ctx,
        )
        .expect("record")
    };
    let a = rcd(&[("fx", Value::string(&NAMES, "a")), ("fy", Value::Int(1))]);
    let b = rcd(&[
        ("fx", Value::string(&NAMES, "b")),
        ("fy", Value::Int(2)),
        ("c", Value::model(&NAMES, "m1")),
    ]);
    assert!(Value::elements_comparable(&[a.clone(), b.clone()]));
    // The same field holding a string in one record and an integer in the
    // other can fail in TLC.
    let c = rcd(&[("fx", Value::Int(3)), ("fy", Value::Int(1))]);
    assert!(!Value::elements_comparable(&[a.clone(), c]));
    // A record and a non-empty tuple fail in one direction.
    assert!(!Value::elements_comparable(&[
        a.clone(),
        Value::tuple(vec![Value::Int(1)])
    ]));
    assert!(Value::elements_comparable(&[a, Value::tuple(Vec::new())]));
}
