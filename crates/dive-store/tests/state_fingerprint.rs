//! dive's state fingerprint (ADR 0015 §5, issue #42): the per-variable FP64
//! value fingerprints are combined by a non-linear, position-keyed chain, so
//! that structured changes to several variables do not cancel.
//!
//! - The structured collision of the former FP64-over-FP64 combination (the
//!   same model value changed in two variables, at offsets 64 bits apart)
//!   is reproduced against that combination and is gone.
//! - Equal changes in two variables, swaps of values between variables, and
//!   permutations of model values across a state never collide.
//! - On a large sample of random structured states, fingerprint-set and
//!   exact-set membership agree exactly.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use dive_store::state::{StateFingerprinter, combine, state_key};
use dive_value::fp64::TlcFingerprint;
use dive_value::{Ctx, Interner, Value};
use proptest::prelude::*;

const MODEL_VALUES: [&str; 5] = ["p1", "p2", "p3", "a1", "v1"];

static NAMES: LazyLock<Interner> = LazyLock::new(|| {
    let names = Interner::new();
    for n in MODEL_VALUES {
        names.intern(n);
    }
    for n in ["rdy", "busy", "Wr", "Rd", "adr", "op", "val"] {
        names.intern(n);
    }
    names
});

fn ctx() -> Ctx {
    Ctx::tlc()
}

fn fingerprinter() -> StateFingerprinter {
    StateFingerprinter::new(0).expect("polynomial 0")
}

fn fp(s: &[Value]) -> u64 {
    fingerprinter()
        .fingerprint(s, ctx())
        .expect("fingerprintable")
}

fn key(s: &[Value]) -> Vec<u8> {
    let mut k = Vec::new();
    state_key(s, ctx(), &mut k).expect("finite values have keys");
    k
}

fn model(name: &str) -> Value {
    Value::model(&NAMES, name)
}

/// The former default-profile combination: FP64 over the FP64 value
/// fingerprints, in variable order.
fn former_two_level(s: &[Value]) -> u64 {
    let values = TlcFingerprint::new(0).expect("polynomial 0");
    let f = values.fp64();
    s.iter().fold(f.empty(), |acc, v| {
        let vf = values
            .fingerprint_cached(v, ctx())
            .expect("fingerprintable");
        f.extend_i64(acc, i64::from_le_bytes(vf.to_le_bytes()))
    })
}

/// `<<m, 0, …, 0, TRUE, …, TRUE>>`: `m` followed by `ints` integers and
/// `bools` booleans, so that `m`'s bytes lie at a chosen distance from the end
/// of the value's FP64 stream.
fn padded(m: &Value, ints: usize, bools: usize) -> Value {
    let mut items = vec![m.clone()];
    items.extend(std::iter::repeat_n(Value::Int(0), ints));
    items.extend(std::iter::repeat_n(Value::TRUE, bools));
    Value::tuple(items)
}

#[test]
fn former_combination_collided_and_the_new_one_does_not() {
    // An integer element adds 10 bytes after `m`, a boolean 7, so the second
    // variable's `m` is 8 bytes (one outer FP64 step) further from the end:
    // the two changes are the same polynomial at the same power and cancel.
    let state = |m: &Value| vec![padded(m, 2, 0), padded(m, 0, 4)];
    let (s, t) = (state(&model("p1")), state(&model("p2")));
    assert_ne!(key(&s), key(&t));
    assert_eq!(
        former_two_level(&s),
        former_two_level(&t),
        "the regression case reproduces issue #42"
    );
    assert_ne!(fp(&s), fp(&t));

    // Every padding with such an alignment, which the former scheme hit.
    let (mut former, mut now) = (0, 0);
    for ai in 0..6 {
        for bi in 0..6 {
            for aj in 0..6 {
                for bj in 0..6 {
                    let st = |m: &Value| vec![padded(m, ai, bi), padded(m, aj, bj)];
                    let (s, t) = (st(&model("p1")), st(&model("p2")));
                    former += usize::from(former_two_level(&s) == former_two_level(&t));
                    now += usize::from(fp(&s) == fp(&t));
                }
            }
        }
    }
    assert_eq!(former, 8);
    assert_eq!(now, 0);
}

#[test]
fn issue_42_states_differ() {
    // The shape of the colliding pair reported on MCWriteThroughCache: the
    // same model value, changed in two variables.
    let ctx = ctx();
    let rec = |p: &str| {
        Value::tuple(vec![Value::tuple(vec![
            model(p),
            Value::record(
                vec![
                    (NAMES.intern("adr"), model("a1")),
                    (NAMES.intern("op"), Value::string(&NAMES, "Wr")),
                    (NAMES.intern("val"), model("v1")),
                ],
                ctx,
            )
            .expect("record"),
        ])])
    };
    let ctl = Value::fcn(
        vec![
            (model("p1"), Value::string(&NAMES, "rdy")),
            (model("p2"), Value::string(&NAMES, "rdy")),
        ],
        ctx,
    )
    .expect("function");
    let mem = Value::fcn(vec![(model("a1"), model("v1"))], ctx).expect("function");
    let state = |p: &str| {
        vec![
            Value::tuple(vec![model(p), Value::string(&NAMES, "NoVal")]),
            mem.clone(),
            ctl.clone(),
            rec(p),
        ]
    };
    assert_ne!(fp(&state("p1")), fp(&state("p2")));
}

#[test]
fn single_variable_changes_never_collide() {
    // The chain is a bijection in each value fingerprint, so states differing
    // in one variable collide only if those values' FP64 fingerprints do.
    let base: Vec<u64> = (0..6).map(|i| i * 0x1234_5678).collect();
    let mut seen = HashSet::new();
    for pos in 0..base.len() {
        for x in 0..2000u64 {
            let mut v = base.clone();
            v[pos] = x.wrapping_mul(0xDEAD_BEEF_0000_0001);
            seen.insert((pos, v[pos], combine(v.iter().copied())));
        }
    }
    let fps: HashSet<u64> = seen.iter().map(|t| t.2).collect();
    // Only the base state itself recurs (once per position, at `x` = 0 when
    // the base holds 0 there).
    assert!(fps.len() + base.len() >= seen.len());
}

/// A value that may hold model values, at varying depth and offset.
fn value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        (0i64..4).prop_map(Value::Int),
        prop::sample::select(MODEL_VALUES.to_vec()).prop_map(model),
        any::<bool>().prop_map(Value::Bool),
        prop::sample::select(vec!["rdy", "busy"]).prop_map(|s| Value::string(&NAMES, s)),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::tuple),
            prop::collection::vec(inner.clone(), 0..4)
                .prop_map(|v| Value::set(v.clone(), ctx()).unwrap_or_else(|_| Value::tuple(v))),
            (inner.clone(), inner).prop_map(|(x, y)| {
                Value::record(
                    vec![(NAMES.intern("adr"), x), (NAMES.intern("val"), y)],
                    ctx(),
                )
                .unwrap_or(Value::FALSE)
            }),
        ]
    })
}

/// A one-hole context: `hole` placed inside a tuple with values before and
/// after it, nested `depth` times.
#[derive(Clone, Debug)]
struct Ctx1 {
    layers: Vec<(Vec<Value>, Vec<Value>)>,
}

impl Ctx1 {
    /// `<<pre…, hole, "xx…x">>`, with a string of `len` characters.
    fn trailing(pre: &[Value], len: usize) -> Self {
        let tail = Value::string(&NAMES, &"x".repeat(len));
        Self {
            layers: vec![(pre.to_vec(), vec![tail])],
        }
    }

    fn fill(&self, hole: &Value) -> Value {
        self.layers.iter().fold(hole.clone(), |v, (pre, post)| {
            let mut items = pre.clone();
            items.push(v);
            items.extend(post.iter().cloned());
            Value::tuple(items)
        })
    }
}

/// Two values to change one into the other: often two model values (equal
/// encoded lengths, the case that cancelled under the former combination),
/// otherwise any two values.
fn changed_pair() -> impl Strategy<Value = (Value, Value)> {
    let names = prop::sample::select(MODEL_VALUES.to_vec());
    prop_oneof![
        (names.clone(), names).prop_map(|(a, b)| (model(a), model(b))),
        (value(), value()),
    ]
}

fn hole_context() -> impl Strategy<Value = Ctx1> {
    let pad = prop::collection::vec(
        prop_oneof![
            (0i64..3).prop_map(Value::Int),
            any::<bool>().prop_map(Value::Bool)
        ],
        0..8,
    );
    prop::collection::vec((pad.clone(), pad), 0..4).prop_map(|layers| Ctx1 { layers })
}

/// Applies a permutation of the model values to every model value in `v`.
fn rename(v: &Value, perm: &HashMap<&str, &str>) -> Value {
    if let Some(m) = MODEL_VALUES.iter().find(|m| *v == model(m)) {
        return model(perm.get(m).copied().unwrap_or(m));
    }
    match v {
        Value::Set(s) => {
            let items = s.elements(ctx()).expect("finite");
            Value::set(items.iter().map(|x| rename(x, perm)).collect(), ctx()).expect("set")
        }
        Value::Fcn(f) => {
            if let Some(items) = f.as_tuple() {
                Value::tuple(items.iter().map(|x| rename(x, perm)).collect())
            } else if let Some(fields) = f.as_record() {
                let fields = fields
                    .iter()
                    .map(|(a, x)| (a.clone(), rename(x, perm)))
                    .collect();
                Value::record(fields, ctx()).expect("record")
            } else {
                let pairs = f
                    .iter()
                    .map(|(d, x)| (rename(&d, perm), rename(x, perm)))
                    .collect();
                Value::fcn(pairs, ctx()).expect("function")
            }
        }
        _ => v.clone(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The same change (`a` to `b`) made in two variables, each in its own
    /// context, never collides; nor does it in one variable only. Half the
    /// cases align the two changes as the former combination needed: the
    /// later variable's change lies 8 bytes per position further from the end
    /// of its value's FP64 stream, which a trailing string arranges.
    #[test]
    fn equal_changes_in_two_variables_do_not_cancel(
        others in prop::collection::vec(value(), 0..4),
        (a, b) in changed_pair(),
        ci in hole_context(),
        cj in hole_context(),
        aligned in any::<bool>(),
        pre in prop::collection::vec(value(), 0..3),
        pad in 0usize..12,
        i in 0usize..6,
        j in 0usize..6,
    ) {
        prop_assume!(a != b);
        let n = others.len() + 2;
        let (i, j) = (i % n, j % n);
        prop_assume!(i != j);
        let (ci, cj) = if aligned {
            // `x` then a string: the byte distance from `x` to the end is
            // the string's length plus a constant.
            let shift = 8 * i.abs_diff(j);
            let (li, lj) = if i < j { (pad, pad + shift) } else { (pad + shift, pad) };
            (Ctx1::trailing(&pre, li), Ctx1::trailing(&pre, lj))
        } else {
            (ci, cj)
        };
        let state = |x: &Value| {
            let mut rest = others.iter().cloned();
            (0..n).map(|k| {
                if k == i { ci.fill(x) } else if k == j { cj.fill(x) } else { rest.next().unwrap_or(Value::TRUE) }
            }).collect::<Vec<_>>()
        };
        let (s, t) = (state(&a), state(&b));
        prop_assert_ne!(key(&s), key(&t));
        prop_assert_ne!(fp(&s), fp(&t));
        let mut one = s.clone();
        one[i] = ci.fill(&b);
        prop_assert_ne!(fp(&s), fp(&one));
    }

    /// Swapping two unequal values between two variables changes the
    /// fingerprint.
    #[test]
    fn swapping_values_between_variables(
        others in prop::collection::vec(value(), 0..5),
        a in value(),
        b in value(),
        i in 0usize..6,
        j in 0usize..6,
    ) {
        prop_assume!(a != b);
        let n = others.len() + 2;
        let (i, j) = (i % n, j % n);
        prop_assume!(i != j);
        let mut s: Vec<Value> = others.clone();
        s.insert(i.min(j), Value::FALSE);
        s.insert(i.max(j), Value::FALSE);
        let mut t = s.clone();
        s[i] = a.clone();
        s[j] = b.clone();
        t[i] = b;
        t[j] = a;
        prop_assert_ne!(fp(&s), fp(&t));
    }

    /// Permuting the model values across a whole state (as a symmetry
    /// permutation would) gives a different fingerprint exactly when it gives
    /// a different state.
    #[test]
    fn permuting_model_values_across_variables(
        s in prop::collection::vec(value(), 1..6),
        perm in Just(MODEL_VALUES.to_vec()).prop_shuffle(),
    ) {
        let perm: HashMap<&str, &str> = MODEL_VALUES.iter().copied().zip(perm).collect();
        let t: Vec<Value> = s.iter().map(|v| rename(v, &perm)).collect();
        prop_assert_eq!(fp(&s) == fp(&t), key(&s) == key(&t));
    }
}

/// A deterministic generator of structured states (xorshift64*), with small
/// domains so that the sample has many duplicates and many near neighbours.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A string of 0 to 31 characters, so that model values lie at varying
    /// distances from the end of their variable's FP64 stream.
    fn tag(&mut self) -> Value {
        let len = usize::try_from(self.below(32)).unwrap_or(0);
        Value::string(&NAMES, &"x".repeat(len))
    }

    fn pick(&mut self) -> Value {
        let k = usize::try_from(self.below(3)).unwrap_or(0);
        model(MODEL_VALUES[k])
    }

    /// A state shaped like `MCWriteThroughCache`'s: a request pair, a memory
    /// function, a control function, and a queue of (processor, request)
    /// pairs, over three processors, one address and two values.
    fn state(&mut self) -> Vec<Value> {
        let ctx = ctx();
        let req = |g: &mut Self| {
            Value::record(
                vec![
                    (NAMES.intern("adr"), model("a1")),
                    (
                        NAMES.intern("op"),
                        Value::string(&NAMES, if g.below(2) == 0 { "Wr" } else { "Rd" }),
                    ),
                    (
                        NAMES.intern("val"),
                        if g.below(2) == 0 {
                            model("v1")
                        } else {
                            Value::Int(0)
                        },
                    ),
                ],
                ctx,
            )
            .expect("record")
        };
        let p = self.pick();
        let reply = Value::tuple(vec![
            p,
            if self.below(2) == 0 {
                model("v1")
            } else {
                Value::string(&NAMES, "NoVal")
            },
            self.tag(),
        ]);
        let mem = Value::fcn(
            vec![(
                model("a1"),
                if self.below(2) == 0 {
                    model("v1")
                } else {
                    Value::Int(0)
                },
            )],
            ctx,
        )
        .expect("function");
        let ctl = Value::fcn(
            ["p1", "p2", "p3"]
                .iter()
                .map(|p| {
                    (
                        model(p),
                        Value::string(&NAMES, if self.below(2) == 0 { "rdy" } else { "busy" }),
                    )
                })
                .collect(),
            ctx,
        )
        .expect("function");
        let len = self.below(3);
        let queue = Value::tuple(
            (0..len)
                .map(|_| {
                    let p = self.pick();
                    Value::tuple(vec![p, req(self), self.tag()])
                })
                .collect(),
        );
        let n = Value::Int(i64::try_from(self.below(4)).unwrap_or(0));
        vec![reply, mem, ctl, queue, n]
    }
}

#[test]
fn fingerprint_and_exact_membership_agree_on_a_large_sample() {
    let mut gen_states = Gen(0x9E37_79B9_7F4A_7C15);
    let fpr = fingerprinter();
    let mut by_key: HashMap<Vec<u8>, u64> = HashMap::new();
    let mut by_fp: HashMap<u64, Vec<u8>> = HashMap::new();
    let mut former: HashSet<u64> = HashSet::new();
    for _ in 0..200_000 {
        let state = gen_states.state();
        let exact = key(&state);
        let fp = fpr.fingerprint(&state, ctx()).expect("fp");
        former.insert(former_two_level(&state));
        if let Some(&old) = by_key.get(&exact) {
            assert_eq!(old, fp, "equal states, equal fingerprints");
            continue;
        }
        if let Some(other) = by_fp.insert(fp, exact.clone()) {
            panic!("distinct states collide: {other:?} and {exact:?}");
        }
        by_key.insert(exact, fp);
    }
    assert_eq!(by_key.len(), by_fp.len());
    assert!(
        by_key.len() > 10_000,
        "a sample with {} distinct states",
        by_key.len()
    );
    // The sample is one the former combination fails on (it merged distinct
    // states), so agreement here is evidence.
    assert!(former.len() < by_key.len());
    eprintln!(
        "{} distinct states; former combination: {} fingerprints",
        by_key.len(),
        former.len()
    );
}
