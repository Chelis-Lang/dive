//! Adversarial review of `dive-value` (PR #21). Each test states the TLC behaviour it
//! relies on and where that was measured. Tests named `*_issue_*` failed on the
//! reviewed commit (9913d3e) and pin the fixes; the rest document behaviour that was
//! verified to be correct.
//!
//! TLC probes: `tests/fixtures/specs/Rv*` with transcripts under
//! `tests/fixtures/tlc/`; the FP64 probe is `tests/fixtures/specs/FpFull` with its
//! `-fp 0 -dump dot` output in `tests/fixtures/fp/FpFull.dot`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "test code"
)]

mod common;

use std::cmp::Ordering;
use std::sync::Arc;

use common::{Evaluator, seed_interner};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{
    Ctx, DeferredKind, DeferredSet, ErrorKind, Int, Interner, Value, ValueError, ValueOrder, tlc,
};

// ---------------------------------------------------------------------------------
// FP64: all 64 bits, verified against TLC's `-dump dot` node ids.
// ---------------------------------------------------------------------------------

/// The probe module (`scratchpad/fp/FpFull/FpFull.tla`), with the one `ā` escape
/// written as the character TLC decoded it to.
const FP_MODULE: &str = r#"---- MODULE FpFull ----
EXTENDS Integers, Sequences, FiniteSets, TLC
VARIABLES i, x
V == << 0, 1, -1, 2147483647, -2147483647, TRUE, FALSE, "qsq", "", "qé中", "ā",
        {3, 1, 2}, {}, 1..3, <<1, 2>>, <<>>, [qfa |-> 1, qfb |-> "qsr"],
        (5 :> TRUE @@ 7 :> FALSE), {<<1, {2}>>, <<3, {}>>}, SUBSET {1, 2},
        [{1, 2} -> {3}], {1, 2} \X {3}, [qfa : {1, 2}], {"qss", "qst"},
        (2 :> 1 @@ 3 :> 2), ({1} :> 2), 3..2, <<<<>>>> >>
===="#;

/// `(state fingerprint, i, x)` from TLC 2026.10.06.014338 run with `-fp 0 -dump dot`;
/// `DotStateWriter` names each node by `TLCState.fingerPrint()`, the full 64 bits.
const TLC_STATE_FPS: &[(i64, i64, &str)] = &[
    (6_980_719_983_254_260_540, 1, "0"),
    (-2_367_570_215_144_997_890, 2, "1"),
    (8_267_846_450_244_213_268, 3, "-1"),
    (-8_895_172_020_667_913_691, 4, "2147483647"),
    (2_961_109_922_930_166_735, 5, "-2147483647"),
    (7_771_953_958_146_015_014, 6, "TRUE"),
    (4_692_748_607_173_989_835, 7, "FALSE"),
    (6_455_259_641_076_684_023, 8, "\"qsq\""),
    (4_835_844_478_903_683_977, 9, "\"\""),
    (-6_380_261_763_894_610_784, 10, "\"qé中\""),
    (-8_703_921_184_652_659_579, 11, "\"ā\""),
    (-3_111_571_855_557_065_933, 12, "{3, 1, 2}"),
    (-7_372_879_826_077_772_697, 13, "{}"),
    (-7_244_293_707_493_851_643, 14, "1..3"),
    (-3_841_258_076_734_373_653, 15, "<<1, 2>>"),
    (1_818_696_155_677_368_078, 16, "<<>>"),
    (
        4_342_727_491_905_451_621,
        17,
        "[qfa |-> 1, qfb |-> \"qsr\"]",
    ),
    (-6_581_593_274_468_744_592, 18, "(5 :> TRUE @@ 7 :> FALSE)"),
    (-3_866_128_796_833_970_647, 19, "{<<1, {2}>>, <<3, {}>>}"),
    (9_185_438_606_914_341_595, 20, "SUBSET {1, 2}"),
    (5_938_541_637_243_326_451, 21, "[{1, 2} -> {3}]"),
    (-5_510_672_499_257_597_092, 22, "{1, 2} \\X {3}"),
    (-8_225_358_355_441_205_754, 23, "[qfa : {1, 2}]"),
    (-2_864_990_135_695_564_273, 24, "{\"qss\", \"qst\"}"),
    (-6_198_557_060_882_721_676, 25, "(2 :> 1 @@ 3 :> 2)"),
    (-7_681_716_527_844_961_722, 26, "({1} :> 2)"),
    (1_690_585_178_586_298_509, 27, "3..2"),
    (-2_977_504_914_679_722_261, 28, "<<<<>>>>"),
];

#[test]
fn fp64_state_fingerprints_match_tlc_in_all_64_bits() {
    let ctx = Ctx::tlc();
    let names = seed_interner(FP_MODULE);
    let ev = Evaluator {
        names: &names,
        ctx,
        constants: Vec::new(),
    };
    let fp = TlcFingerprint::new(0).unwrap();
    let mut bad = Vec::new();
    for &(want, i, expr) in TLC_STATE_FPS {
        let x = ev.eval(expr).unwrap();
        let got = fp.fingerprint_state(&[Value::from(i), x], ctx).unwrap();
        #[allow(clippy::cast_possible_wrap, reason = "Java long")]
        let got = got as i64;
        if got != want {
            bad.push(format!("{expr}: dive {got} TLC {want}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---------------------------------------------------------------------------------
// Unnormalized sets: Hash / Eq / Debug change TLC-observable membership.
// ---------------------------------------------------------------------------------

/// `{[qfa |-> 1, qfb |-> 1], [qfa |-> 2, qfb |-> 2]}` as TLC builds the literal.
fn raw_record_set(names: &Interner, ctx: Ctx) -> Value {
    let r = |a: i64, b: i64| {
        Value::record(
            vec![
                (names.intern("qfa"), Value::from(a)),
                (names.intern("qfb"), Value::from(b)),
            ],
            ctx,
        )
        .unwrap()
    };
    Value::set_raw(vec![r(1, 1), r(2, 2)], ctx)
}

/// TLC (probe `RvMemUnnorm`): `<<1>> \in {[qfa |-> 1, qfb |-> 1], [qfa |-> 2, qfb |-> 2]}`
/// is an error ("Attempted to check equality of record ... with non-record <<1>>");
/// once the set is normalized (probe `RvMemNorm`, the set read back from a state
/// variable) the same test is `FALSE`. So normalization state is observable, and only
/// TLC's own normalizing operations may change it. Hashing must not.
#[test]
fn hashing_a_raw_set_must_not_change_membership_issue_normalize_side_effect() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let s = raw_record_set(&names, ctx);
    let t = Value::tuple(vec![Value::from(1)]);
    assert!(
        tlc::member(&t, &s, ctx).is_err(),
        "TLC: error before normalization"
    );
    // What a HashMap/HashSet insertion, a memo cache, or an exact state store does:
    let _ = s.hash64();
    assert!(
        tlc::member(&t, &s, ctx).is_err(),
        "hash64() normalized the set: membership became {:?}",
        tlc::member(&t, &s, ctx)
    );
}

#[test]
fn debug_formatting_a_raw_set_must_not_change_membership_issue_normalize_side_effect() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let s = raw_record_set(&names, ctx);
    let t = Value::tuple(vec![Value::from(1)]);
    let _ = format!("{s:?}"); // a `tracing::debug!("{v:?}")` anywhere in dive-eval
    assert!(
        tlc::member(&t, &s, ctx).is_err(),
        "Debug normalized the set: membership became {:?}",
        tlc::member(&t, &s, ctx)
    );
}

#[test]
fn rust_eq_on_a_raw_set_must_not_change_membership_issue_normalize_side_effect() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let s = raw_record_set(&names, ctx);
    let other = raw_record_set(&names, ctx);
    let t = Value::tuple(vec![Value::from(1)]);
    let _ = s == other;
    assert!(
        tlc::member(&t, &s, ctx).is_err(),
        "PartialEq normalized the set: membership became {:?}",
        tlc::member(&t, &s, ctx)
    );
}

/// Issue #32: `==` on an unnormalized set whose elements TLC cannot compare (here an
/// integer and a set) must not mark the *nested* unnormalized set as normalized. TLC's
/// `SetEnumValue.normalize` would, through `compareTo`, but `==` is not a TLC
/// operation (ADR 0012).
#[test]
fn rust_eq_on_an_incomparable_raw_set_must_not_normalize_nested_sets_issue_32() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let inner = raw_record_set(&names, ctx);
    let outer = Value::set_raw(vec![Value::from(1), inner.clone()], ctx);
    let other = Value::set_raw(vec![Value::from(1), raw_record_set(&names, ctx)], ctx);
    let t = Value::tuple(vec![Value::from(1)]);
    assert!(
        tlc::member(&t, &inner, ctx).is_err(),
        "TLC: error before normalization"
    );
    let _ = outer == other;
    let _ = outer.hash64();
    let _ = format!("{outer:?}");
    let _ = outer.cmp_with(&other, ValueOrder::Tlc);
    assert!(
        tlc::member(&t, &inner, ctx).is_err(),
        "== / Hash / Debug / cmp_with normalized the nested set: membership became {:?}",
        tlc::member(&t, &inner, ctx)
    );
    // A TLC operation that normalizes the outer set still normalizes the nested one,
    // as TLC's `compareTo` does, even after the quiet operations above.
    assert!(outer.as_set().unwrap().elements(ctx).is_err());
    assert_eq!(
        tlc::member(&t, &inner, ctx).ok(),
        Some(false),
        "TLC's normalize of the outer set normalizes the nested set first"
    );
}

// ---------------------------------------------------------------------------------
// The element cache remembers the first Ctx's error.
// ---------------------------------------------------------------------------------

#[test]
fn enumeration_error_must_not_depend_on_who_enumerated_first_issue_cache() {
    let ctx = Ctx::tlc();
    let mk = || Value::subset(Value::interval(Int::ONE, Int::from(31i64), ctx), ctx);
    let fresh = mk();
    let want = fresh.as_set().unwrap().elements(ctx).unwrap_err();
    let hashed = mk();
    let _ = hashed.hash64(); // enumerates under IntMode::Big and caches the error
    let got = hashed.as_set().unwrap().elements(ctx).unwrap_err();
    assert_eq!(
        (got.kind(), got.message()),
        (want.kind(), want.message()),
        "TLC-mode enumeration returned the error cached by a Big-mode hash"
    );
}

// ---------------------------------------------------------------------------------
// Order / equality consistency.
// ---------------------------------------------------------------------------------

#[test]
fn cmp_with_equal_iff_eq_issue_unnormalizable_raw_sets() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let a = Value::set_raw(vec![Value::from(1), Value::string(&names, "a")], ctx);
    let b = Value::set_raw(vec![Value::from(2), Value::string(&names, "b")], ctx);
    assert_ne!(a, b);
    // Documented: "Consistent with `==`: a.cmp_with(b, o) == Equal iff a == b".
    assert_ne!(a.cmp_with(&b, ValueOrder::Tlc), Ordering::Equal);
}

#[test]
fn eq_is_reflexive_across_copies_issue_unnormalizable_raw_sets() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let a = Value::set_raw(vec![Value::from(1), Value::string(&names, "a")], ctx);
    let a2 = Value::set_raw(vec![Value::from(1), Value::string(&names, "a")], ctx);
    assert_eq!(a.hash64(), a2.hash64());
    assert_eq!(a, a2, "same construction, separate allocations");
}

// ---------------------------------------------------------------------------------
// API footguns.
// ---------------------------------------------------------------------------------

/// A set built under one order and queried under another gives a silently wrong
/// membership answer (binary search with the wrong comparator).
#[test]
fn member_with_mismatched_ctx_order_issue_silent_false() {
    let names = Interner::new();
    for n in ["c", "b", "a"] {
        names.intern(n);
    }
    let canon = Ctx::dive();
    let s = Value::set(
        ["a", "b", "c"]
            .iter()
            .map(|n| Value::string(&names, n))
            .collect(),
        canon,
    )
    .unwrap();
    let a = Value::string(&names, "a");
    // Either the set's own order is used, or the mismatch is an error.
    let r = tlc::member(&a, &s, Ctx::tlc());
    assert!(
        !matches!(r, Ok(false)),
        "\"a\" \\in {{\"a\", \"b\", \"c\"}} answered FALSE"
    );
}

/// `Value`'s variants are public, so the canonical-integer invariant can be broken.
#[test]
fn non_canonical_bigint_issue_public_variants() {
    let five_big = Value::BigInt(Arc::new(num_bigint::BigInt::from(5)));
    let five = Value::from(5);
    let ctx = Ctx::dive();
    assert!(tlc::equals(&five_big, &five, ctx).unwrap());
    assert_eq!(five_big, five, "Rust == disagrees with tlc::equals");
}

/// Interval-domain functions vs records: dive matches TLC (probes RvFcnRcd5..7).
#[test]
fn interval_domain_function_vs_record_matches_tlc() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let rec = Value::record(
        vec![
            (names.intern("qa"), Value::from(1)),
            (names.intern("qb"), Value::from(2)),
        ],
        ctx,
    )
    .unwrap();
    let t = Value::tuple(vec![Value::from(0), Value::from(0)]);
    let e = tlc::equals(&t, &rec, ctx).unwrap_err();
    assert_eq!(
        e.message(),
        "Attempted to compare an integer with non-integer:\n\"qa\"."
    );
    let e = Value::set(vec![rec, t], ctx).unwrap_err();
    assert_eq!(
        e.message(),
        "Attempted to compare integer with non-integer:\n\"qa\"."
    );
}

/// Big-integer mode: a function whose domain holds a big integer is called a
/// "non-integer" by the tuple/function comparison, and `Value::set` (fast path,
/// shapes say comparable) disagrees with `tlc::compare` (slow path, error).
#[test]
fn bigint_domain_is_an_integer_issue_non_int_domain() {
    let ctx = Ctx::dive();
    let big = Value::int(Int::from_big(num_bigint::BigInt::from(1u8) << 70));
    let f = Value::fcn(vec![(big, Value::from(1))], ctx).unwrap();
    let t = Value::tuple(vec![Value::from(1)]);
    assert!(Value::set(vec![t.clone(), f.clone()], ctx).is_ok());
    let r = tlc::compare(&t, &f, ctx);
    assert!(r.is_ok(), "{:?}", r.map_err(|e| e.message().to_owned()));
}

/// Debug builds: `Fcn::apply` subtracts 1 from the argument unchecked.
#[test]
fn apply_tuple_at_i64_min_does_not_panic_issue_overflow() {
    let t = Value::tuple(vec![Value::from(1)]);
    let f = t.as_fcn().unwrap();
    assert!(f.apply(&Value::Int(i64::MIN)).is_none());
}

// ---------------------------------------------------------------------------------
// Unenumerable deferred sets: ordered and compared by address.
// ---------------------------------------------------------------------------------

#[derive(Debug)]
struct Unenumerable;

impl DeferredSet for Unenumerable {
    fn kind(&self) -> DeferredKind {
        DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> Result<Vec<Value>, ValueError> {
        Err(ValueError::new(ErrorKind::NotEnumerable, "infinite"))
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<Result<bool, ValueError>> {
        None
    }
}

#[test]
fn deferred_sets_order_is_allocation_dependent_issue_nondeterminism() {
    let ctx = Ctx::dive();
    let p = Value::deferred(Arc::new(Unenumerable), ctx);
    let q = Value::deferred(Arc::new(Unenumerable), ctx);
    // Same definition: the order must not depend on which was allocated first.
    assert_eq!(p.cmp_with(&q, ValueOrder::Canonical), Ordering::Equal);
}

// ---------------------------------------------------------------------------------
// Huge lazy sets: Rust Hash / Eq materialize what TLC handles in O(1).
// ---------------------------------------------------------------------------------

/// TLC: `IntervalValue.equals(IntervalValue)` compares bounds and
/// `IntervalValue.fingerPrint` loops without allocating (a state `x = 0..2147483646`
/// checks in 3.5 min with 98 MB RSS, probe `RvBigIntvState`). dive's `==` and `Hash`
/// enumerate both intervals into `Vec<Value>` (16 bytes per element).
#[test]
fn rust_eq_on_large_intervals_materializes_issue_hash_enumerates() {
    let ctx = Ctx::dive();
    let n = Int::from(20_000_000i64);
    let a = Value::interval(Int::ZERO, n.clone(), ctx);
    let b = Value::interval(Int::ZERO, n, ctx);
    let t = std::time::Instant::now();
    assert_eq!(a, b);
    let dt = t.elapsed();
    assert!(dt.as_millis() < 10, "== took {dt:?}");
}

// ---------------------------------------------------------------------------------
// Deep nesting.
// ---------------------------------------------------------------------------------

fn nested(depth: usize) -> Value {
    let mut v = Value::tuple(Vec::new());
    for _ in 0..depth {
        v = Value::tuple(vec![v]);
    }
    v
}

/// Dropping a value nested 1e6 deep recurses once per level; on a 2 MiB worker
/// stack it overflows and aborts the process (no unwinding, no error report).
/// Measured threshold in release: ok at 10k, aborts at 30k.
#[test]
fn drop_deeply_nested_value_issue_stack_overflow() {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| drop(nested(1_000_000)))
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn compare_deeply_nested_values_issue_stack_overflow() {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| {
            let a = nested(200_000);
            let b = nested(200_000);
            let r = a == b;
            std::mem::forget((a, b)); // isolate compare from drop
            r
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn fingerprint_deeply_nested_value_issue_stack_overflow() {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| {
            let a = nested(200_000);
            let r = TlcFingerprint::new(0).unwrap().fingerprint(&a, Ctx::tlc());
            std::mem::forget(a);
            r.is_ok()
        })
        .unwrap()
        .join()
        .unwrap();
}

/// TLC keeps a function with a string domain built by `:>`/`@@` or `[x \in S |-> e]`
/// as a `FcnRcdValue`, whose `equals`/`compareTo` check sizes before anything else;
/// only a record literal is a `RecordValue`, which refuses non-records outright. dive
/// canonicalizes both to `FcnRepr::Record` and applies `RecordValue`'s rules to both.
/// TLC (probes RvFcnRcd1..4, 8):
/// - `("qa" :> 1 @@ "qb" :> 2) = <<1>>` is FALSE;
/// - `[x \in {"qa", "qb"} |-> 1] = <<1>>` is FALSE;
/// - `<<1>> \in {("qa" :> 1 @@ "qb" :> 2)}` is FALSE;
/// - `{<<1>>, ("qa" :> 1 @@ "qb" :> 2)}` is a set;
/// - `[qa |-> 1, qb |-> 2] = <<1>>` is an error (the only case dive gets right).
#[test]
fn string_domain_function_is_not_a_record_literal_issue_fcnrcd_vs_record() {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    let f = Value::fcn(
        vec![
            (Value::string(&names, "qa"), Value::from(1)),
            (Value::string(&names, "qb"), Value::from(2)),
        ],
        ctx,
    )
    .unwrap();
    let t = Value::tuple(vec![Value::from(1)]);
    let msg = |r: Result<String, dive_value::ValueError>| {
        r.unwrap_or_else(|e| format!("error: {}", e.message()))
    };
    let single = Value::set_raw(vec![f.clone()], ctx);
    let got = [
        msg(tlc::equals(&f, &t, ctx).map(|b| b.to_string())),
        msg(tlc::member(&t, &single, ctx).map(|b| b.to_string())),
        msg(Value::set(vec![t.clone(), f.clone()], ctx).map(|_| "set".to_owned())),
    ];
    assert_eq!(
        got,
        ["false".to_owned(), "false".to_owned(), "set".to_owned()]
    );
}

/// `Seq({})` is the finite set `{<<>>}`, but hashes structurally, so `==` (which
/// checks hashes first) says it differs from `{<<>>}` while `cmp_with` says Equal.
#[test]
fn seq_of_empty_equals_its_enumeration_issue_hash() {
    let ctx = Ctx::tlc();
    let seq = Value::seq_set(Value::empty_set(ctx), ctx);
    let enumerated = Value::set(vec![Value::tuple(Vec::new())], ctx).unwrap();
    assert_eq!(seq.cmp_with(&enumerated, ValueOrder::Tlc), Ordering::Equal);
    assert_eq!(seq.hash64(), enumerated.hash64());
    assert_eq!(seq, enumerated);
}

/// D-009 says the default profile reproduces TLC's comparability "exactly, in both
/// profiles". It cannot: TLC's errors depend on which pairs the insertion sort
/// compares, and that depends on the string order. `{[fx |-> "qzb", fy |-> 0], <<1>>,
/// [fx |-> "qza", fy |-> 0]}` is a set in TLC (probe `RvCanonAsym`) (and in dive
/// under `Ctx::tlc()`), but an error under dive's canonical order.
#[test]
fn canonical_profile_comparability_differs_from_tlc_issue_d009_claim() {
    let names = Interner::new();
    names.intern("fx");
    names.intern("fy");
    names.intern("qzb");
    names.intern("qza");
    let run = |ctx: Ctx| {
        let r = |x: &str| {
            Value::record(
                vec![
                    (names.intern("fx"), Value::string(&names, x)),
                    (names.intern("fy"), Value::from(0)),
                ],
                ctx,
            )
            .unwrap()
        };
        Value::set(
            vec![r("qzb"), Value::tuple(vec![Value::from(1)]), r("qza")],
            ctx,
        )
        .is_ok()
    };
    assert!(run(Ctx::tlc()), "TLC order: a set");
    assert_eq!(
        run(Ctx::dive()),
        run(Ctx::tlc()),
        "canonical order: an error"
    );
}

// ---------------------------------------------------------------------------------
// Fuzzing: richer value trees than tests/properties.rs (raw sets, all lazy kinds,
// typed model values, escapes and non-BMP strings, set-valued domains, big ints),
// every public operation, panic-freedom and cross-API consistency.
// Run with SKIP_SEQ=1 to skip the known Seq({}) hash issue; values containing
// unenumerable sets are skipped for the consistency checks (known issue class).
// ---------------------------------------------------------------------------------

mod fuzz {
    use std::cmp::Ordering;
    use std::sync::LazyLock;

    use dive_value::fp64::{Fingerprinter, TlcFingerprint};
    use dive_value::{Ctx, Int, Interner, Value, json, ops, print, tlc};
    use num_bigint::BigInt;
    use proptest::prelude::*;

    static NAMES: LazyLock<Interner> = LazyLock::new(|| {
        let n = Interner::new();
        for s in [
            "zz",
            "b",
            "a",
            "T_x",
            "T_y",
            "U_x",
            "m1",
            "q\"\\\n\u{1F600}é",
            "fy",
            "fx",
        ] {
            n.intern(s);
        }
        n
    });
    const CTX: Ctx = Ctx::tlc();

    fn leaf() -> impl Strategy<Value = Value> {
        prop_oneof![
            any::<bool>().prop_map(Value::Bool),
            (-3i64..4).prop_map(Value::Int),
            prop::sample::select(vec![
                i64::from(i32::MIN),
                i64::from(i32::MAX),
                i64::MIN,
                i64::MAX
            ])
            .prop_map(Value::Int),
            Just(Value::int(Int::from_big(BigInt::from(i64::MIN) * 3))),
            prop::sample::select(vec!["a", "b", "zz", "q\"\\\n\u{1F600}é", ""])
                .prop_map(|s| Value::string(&NAMES, s)),
            prop::sample::select(vec!["m1", "T_x", "T_y", "U_x"])
                .prop_map(|s| Value::model(&NAMES, s)),
        ]
    }

    fn value() -> impl Strategy<Value = Value> {
        leaf().prop_recursive(3, 20, 3, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(|v| Value::set_raw(v, CTX)),
                prop::collection::vec(inner.clone(), 0..4)
                    .prop_map(|v| Value::set(v.clone(), CTX).unwrap_or_else(|_| Value::tuple(v))),
                prop::collection::vec(inner.clone(), 0..3).prop_map(Value::tuple),
                (inner.clone(), inner.clone()).prop_map(|(x, y)| {
                    Value::record(vec![(NAMES.intern("fx"), x), (NAMES.intern("fy"), y)], CTX)
                        .unwrap_or(Value::FALSE)
                }),
                (inner.clone(), inner.clone(), inner.clone()).prop_map(|(d, x, y)| {
                    Value::fcn(vec![(d, x), (Value::Int(2), y)], CTX).unwrap_or(Value::TRUE)
                }),
                (-2i64..2, -2i64..3).prop_map(|(a, b)| Value::interval(a.into(), b.into(), CTX)),
                inner.clone().prop_map(|b| Value::subset(b, CTX)),
                (inner.clone(), inner.clone()).prop_map(|(d, r)| Value::fcn_set(d, r, CTX)),
                prop::collection::vec(inner.clone(), 0..3).prop_map(|v| Value::product(v, CTX)),
                (inner.clone(), inner.clone()).prop_map(|(x, y)| {
                    Value::record_set(vec![(NAMES.intern("fx"), x), (NAMES.intern("fy"), y)], CTX)
                        .unwrap_or(Value::FALSE)
                }),
                inner.clone().prop_map(|r| Value::seq_set(r, CTX)),
                Just(Value::nat(CTX)),
                Just(Value::string_set(CTX)),
            ]
        })
    }

    pub fn dump(v: &Value) -> String {
        match v {
            Value::Set(s) => match s.elements(CTX) {
                Ok(e) => format!(
                    "{:?}{{{}}}",
                    s.kind(),
                    e.iter().map(dump).collect::<Vec<_>>().join(", ")
                ),
                Err(err) => format!(
                    "{:?}<{:?}: {}>",
                    s.kind(),
                    err.kind(),
                    print::to_tlc_string_lenient(v, CTX)
                ),
            },
            Value::Fcn(f) => format!(
                "{:?}({})",
                f.kind(),
                f.iter()
                    .map(|(d, x)| format!("{} :> {}", dump(&d), dump(x)))
                    .collect::<Vec<_>>()
                    .join(" @@ ")
            ),
            other => print::to_tlc_string_lenient(other, CTX),
        }
    }

    /// Whether `v` contains a set that cannot be enumerated (known issue classes).
    fn has_unenumerable(v: &Value) -> bool {
        match v {
            Value::Set(s) => match s.elements(CTX) {
                Ok(e) => e.iter().any(has_unenumerable),
                Err(_) => true,
            },
            Value::Fcn(f) => f
                .iter()
                .any(|(d, x)| has_unenumerable(&d) || has_unenumerable(x)),
            _ => false,
        }
    }

    fn exercise(a: &Value) {
        let fp = TlcFingerprint::new(0).unwrap();
        let _ = print::to_tlc_string(a, CTX);
        let _ = print::to_tlc_string_lenient(a, CTX);
        let _ = print::print_value(a, CTX);
        let _ = json::to_tlc_json(a, CTX);
        let _ = fp.fingerprint(a, CTX);
        let _ = a.hash64();
        let _ = format!("{a:?}");
        if let Some(s) = a.as_set() {
            let _ = s.size(CTX);
            let _ = s.elements(CTX);
            let _ = s.is_infinite();
            let _ = tlc::is_empty(a, CTX);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(3000))]

        #[test]
        fn no_panics_and_consistent(a in value(), b in value()) {
            exercise(&a);
            exercise(&b);
            let fp = TlcFingerprint::new(0).unwrap();
            let _ = tlc::member(&a, &b, CTX);
            let _ = ops::union(&a, &b, CTX);
            let _ = ops::intersect(&a, &b, CTX);
            let _ = ops::difference(&a, &b, CTX);
            let _ = Value::set(vec![a.clone(), b.clone()], CTX);
            let _ = Value::fcn(vec![(a.clone(), b.clone()), (b.clone(), a.clone())], CTX);
            let ab = a.cmp_with(&b, CTX.order);
            if has_unenumerable(&a) || has_unenumerable(&b) || std::env::var("SKIP_SEQ").is_ok() && (dump(&a).contains("Seq") || dump(&b).contains("Seq")) { return Ok(()); }
            prop_assert_eq!(ab, b.cmp_with(&a, CTX.order).reverse());
            prop_assert_eq!(ab == Ordering::Equal, a == b, "cmp_with vs ==: {} {}", dump(&a), dump(&b));
            if a == b {
                prop_assert_eq!(a.hash64(), b.hash64());
                if let (Ok(x), Ok(y)) = (fp.fingerprint(&a, CTX), fp.fingerprint(&b, CTX)) {
                    prop_assert_eq!(x, y, "equal values, different FP: {} {}", dump(&a), dump(&b));
                }
            }
            if let Ok(o) = tlc::compare(&a, &b, CTX) {
                prop_assert_eq!(o, ab, "tlc::compare vs cmp_with: {} {}", dump(&a), dump(&b));
            }
            if let Ok(e) = tlc::equals(&a, &b, CTX) {
                prop_assert_eq!(e, a == b, "tlc::equals vs ==: {} {}", dump(&a), dump(&b));
            }
            // Lazy membership agrees with enumeration.
            if let Some(s) = b.as_set()
                && let (Ok(m), Ok(elems)) = (tlc::member(&a, &b, CTX), s.elements(CTX)) {
                    prop_assert_eq!(m, elems.iter().any(|e| e == &a), "member {} in {}", dump(&a), dump(&b));
                }
        }
    }
}
