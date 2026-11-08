//! Property tests for state fingerprints and exact-store keys (ADR 0015 §5).
//!
//! - Equal states have equal fingerprints and keys however their values are
//!   represented, and distinct states have distinct keys.
//! - Fingerprint and exact-key membership agree: no two distinct states share a
//!   fingerprint (`tests/state_fingerprint.rs` has the targeted collision
//!   properties).

#![allow(
    clippy::expect_used,
    reason = "test code: a failed expectation is a test failure"
)]

use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use dive_store::state::{StateFingerprinter, state_key};
use dive_value::{Ctx, Int, Interner, Value};
use proptest::prelude::*;

static NAMES: LazyLock<Interner> = LazyLock::new(|| {
    let names = Interner::new();
    for n in ["idle", "busy", "done", "p1", "p2"] {
        names.intern(n);
    }
    names
});

fn ctx() -> Ctx {
    Ctx::tlc()
}

/// A variable value: small, with several representations of equal sets.
fn var_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        (0i64..4).prop_map(Value::Int),
        prop::sample::select(vec!["idle", "busy", "done"]).prop_map(|s| Value::string(&NAMES, s)),
        prop::sample::select(vec!["p1", "p2"]).prop_map(|s| Value::model(&NAMES, s)),
        any::<bool>().prop_map(Value::Bool),
    ];
    leaf.prop_recursive(2, 12, 3, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Value::tuple),
            prop::collection::vec(inner.clone(), 0..3)
                .prop_map(|v| Value::set(v.clone(), ctx()).unwrap_or_else(|_| Value::tuple(v))),
            (0i64..3, 0i64..4).prop_map(|(a, b)| Value::interval(
                Int::from(a),
                Int::from(b),
                ctx()
            )),
            // The same sets as intervals, enumerated.
            (0i64..3, 0i64..4).prop_map(|(a, b)| {
                Value::set((a..=b).map(Value::Int).collect(), ctx()).unwrap_or(Value::FALSE)
            }),
            (inner.clone(), inner).prop_map(|(x, y)| {
                Value::record(vec![(NAMES.intern("pc"), x), (NAMES.intern("q"), y)], ctx())
                    .unwrap_or(Value::FALSE)
            }),
        ]
    })
}

fn state() -> impl Strategy<Value = Vec<Value>> {
    prop::collection::vec(var_value(), 1..4)
}

fn key_of(s: &[Value]) -> Box<[u8]> {
    let mut k = Vec::new();
    state_key(s, ctx(), &mut k).expect("finite values have keys");
    k.into_boxed_slice()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn equal_states_share_fingerprint_and_key(a in state(), b in state()) {
        let f = StateFingerprinter::new(0).expect("polynomial 0");
        let (fa, fb) = (f.fingerprint(&a, ctx()).expect("fp"), f.fingerprint(&b, ctx()).expect("fp"));
        let (ka, kb) = (key_of(&a), key_of(&b));
        prop_assert_eq!(ka == kb, a == b);
        if a == b {
            prop_assert_eq!(fa, fb);
        }
        // Cached and uncached fingerprints agree.
        prop_assert_eq!(f.fingerprint(&a, ctx()).expect("fp"), fa);
    }

    #[test]
    fn fingerprint_and_key_identify_the_same_states(
        states in prop::collection::vec(state(), 1..90),
    ) {
        let f = StateFingerprinter::new(0).expect("polynomial 0");
        // No two distinct states share a fingerprint (a collision would be a
        // finding: the per-pair probability is about 2^-64; issue #42), so
        // fingerprint and exact-key membership agree.
        let mut by_fp: HashMap<u64, Box<[u8]>> = HashMap::new();
        let mut keys = BTreeSet::new();
        for s in &states {
            let k = key_of(s);
            let fresh_key = keys.insert(k.clone());
            match by_fp.insert(f.fingerprint(s, ctx()).expect("fp"), k.clone()) {
                Some(other) => {
                    prop_assert_eq!(other, k);
                    prop_assert!(!fresh_key);
                }
                None => prop_assert!(fresh_key),
            }
        }
        prop_assert_eq!(by_fp.len(), keys.len());
    }
}

#[test]
fn intervals_and_enumerated_sets_are_one_state() {
    let f = StateFingerprinter::new(0).expect("polynomial 0");
    let a = [Value::interval(Int::from(1), Int::from(3), ctx())];
    let b = [Value::set(vec![Value::Int(3), Value::Int(2), Value::Int(1)], ctx()).expect("set")];
    assert_eq!(f.fingerprint(&a, ctx()), f.fingerprint(&b, ctx()));
    assert_eq!(key_of(&a), key_of(&b));
}

#[test]
fn tlc_fingerprint_is_tlcs_single_stream() {
    // A state of FpFull's shape: TLC's fingerprint is one FP64 stream over all
    // values, unlike dive's two-level fingerprint.
    let f = StateFingerprinter::new(0).expect("polynomial 0");
    let s = [Value::Int(1), Value::TRUE];
    let tlc = f.tlc_fingerprint(&s, ctx()).expect("fp");
    let dive = f.fingerprint(&s, ctx()).expect("fp");
    assert_ne!(tlc, dive);
    assert_eq!(f.poly_index(), 0);
}
