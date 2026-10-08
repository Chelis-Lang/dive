//! Property tests for the stores (ADR 0015 §2, §5).
//!
//! - The fingerprint set and the exact set equal a reference model (`BTreeSet` plus
//!   "earliest position wins") over random multi-level runs, whatever the shard
//!   count, the split of candidates among workers, the number of resolve threads
//!   and the chaos schedule (random shard-claim order and random yields).
//! - A resolve gives byte-identical winners and an identical set under every
//!   schedule, and equals sequential insertion in position order.
//! - Exact-store and fingerprint-store membership agree, and no two distinct
//!   states share a fingerprint (`tests/state_fingerprint.rs` has the targeted
//!   collision properties).
//! - States round-trip through the codec, and equal states have equal fingerprints
//!   and keys however their values are represented.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use dive_store::frontier::{FrontierBuilder, StateBlock};
use dive_store::state::{StateDecoder, StateFingerprinter, encode_state, state_key};
use dive_store::trace::TraceLog;
use dive_store::{ExactSet, FpSet, ResolveOptions, StateSet, StoreError};
use dive_value::{Ctx, Int, Interner, Value};
use proptest::prelude::*;

/// A position: (state index in the level, successor ordinal).
type Pos = (u32, u32);

/// One level's candidates: key, position, payload. Positions are unique.
type Level<K> = Vec<(K, Pos, u64)>;

/// Each level's winners, in position order.
type Won<K> = Vec<Vec<(K, Pos, u64)>>;

/// A schedule: how candidates are spread over workers and how resolve runs.
#[derive(Clone, Debug)]
struct Schedule {
    workers: usize,
    /// For each candidate, the worker that produced it (indexes modulo `workers`).
    owner_seed: u64,
    threads: usize,
    chaos: Option<u64>,
    shard_bits: u32,
}

fn schedule() -> impl Strategy<Value = Schedule> {
    (
        1usize..6,
        any::<u64>(),
        1usize..9,
        prop::option::of(any::<u64>()),
        0u32..7,
    )
        .prop_map(
            |(workers, owner_seed, threads, chaos, shard_bits)| Schedule {
                workers,
                owner_seed,
                threads,
                chaos,
                shard_bits,
            },
        )
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Fingerprints drawn from a small pool (so duplicates within and across levels are
/// common), spread over the whole 64-bit range, including 0 and the extremes.
fn fp_pool() -> impl Strategy<Value = u64> {
    prop_oneof![
        8 => (0u64..48).prop_map(mix),
        1 => Just(0u64),
        1 => Just(u64::MAX),
        1 => Just(1u64 << 63),
    ]
}

fn levels<K: Clone + std::fmt::Debug>(
    key: impl Strategy<Value = K>,
) -> impl Strategy<Value = Vec<Level<K>>> {
    prop::collection::vec(
        prop::collection::vec((key, any::<u64>()), 0..40).prop_map(|cands| {
            // Unique positions: shuffle-free, (index / 4, index % 4) is unique.
            cands
                .into_iter()
                .enumerate()
                .map(|(i, (k, payload))| {
                    let i = u32::try_from(i).unwrap_or(u32::MAX);
                    (k, (i / 4, i % 4), payload)
                })
                .collect()
        }),
        1..5,
    )
}

/// Runs `levels` through a set with the given schedule; returns each level's winners
/// in position order, and the set.
fn run<S: StateSet>(mut set: S, levels: &[Level<S::Key>], sch: &Schedule) -> (Won<S::Key>, S)
where
    S::Key: Clone,
{
    let mut out = Vec::new();
    for (li, level) in levels.iter().enumerate() {
        let mut bufs: Vec<_> = (0..sch.workers).map(|_| set.candidates()).collect();
        // Phase 1: each worker pre-filters with the read-only set; candidates are
        // pushed in a schedule-dependent order.
        let mut order: Vec<usize> = (0..level.len()).collect();
        order.sort_by_key(|&i| mix(sch.owner_seed ^ (i as u64) ^ ((li as u64) << 32)));
        for i in order {
            let (k, pos, payload) = &level[i];
            if set.contains(k) {
                continue;
            }
            let w =
                usize::try_from(mix(sch.owner_seed.wrapping_add(i as u64)) % sch.workers as u64)
                    .unwrap_or(0);
            bufs[w].push(&set, k.clone(), *pos, *payload);
        }
        let opts = ResolveOptions {
            threads: sch.threads,
            chaos: sch.chaos,
        };
        let won = set
            .resolve(bufs, &opts)
            .expect("unique positions")
            .into_position_order();
        out.push(won.into_iter().map(|c| (c.key, c.pos, c.payload)).collect());
    }
    (out, set)
}

/// The reference: a `BTreeSet` and "earliest position wins".
fn model<K: Ord + Clone>(levels: &[Level<K>]) -> (Won<K>, BTreeSet<K>) {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for level in levels {
        let mut best: BTreeMap<K, (Pos, u64)> = BTreeMap::new();
        for (k, pos, payload) in level {
            if seen.contains(k) {
                continue;
            }
            let e = best.entry(k.clone()).or_insert((*pos, *payload));
            if *pos < e.0 {
                *e = (*pos, *payload);
            }
        }
        let mut won: Vec<_> = best.into_iter().map(|(k, (p, x))| (k, p, x)).collect();
        won.sort_by_key(|w| w.1);
        for (k, _, _) in &won {
            seen.insert(k.clone());
        }
        out.push(won);
    }
    (out, seen)
}

fn sorted_fps(set: &FpSet) -> Vec<u64> {
    let mut v: Vec<u64> = set.iter().collect();
    v.sort_unstable();
    v
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn fpset_equals_reference_model(levels in levels(fp_pool()), sch in schedule()) {
        let (won, set) = run(FpSet::new(sch.shard_bits), &levels, &sch);
        let (want, seen) = model(&levels);
        prop_assert_eq!(&won, &want);
        prop_assert_eq!(set.len(), seen.len() as u64);
        prop_assert_eq!(sorted_fps(&set), seen.iter().copied().collect::<Vec<_>>());
        for k in 0..64u64 {
            let k = mix(k);
            prop_assert_eq!(set.contains(&k), seen.contains(&k));
        }
    }

    #[test]
    fn exact_set_equals_reference_model(
        levels in levels(prop::collection::vec(0u8..3, 0..5).prop_map(Vec::into_boxed_slice)),
        sch in schedule(),
    ) {
        let (won, set) = run(ExactSet::new(sch.shard_bits), &levels, &sch);
        let (want, seen) = model(&levels);
        prop_assert_eq!(&won, &want);
        prop_assert_eq!(set.len(), seen.len() as u64);
        let mut keys: Vec<Box<[u8]>> = set.iter().map(Box::from).collect();
        keys.sort();
        prop_assert_eq!(keys, seen.iter().cloned().collect::<Vec<_>>());
    }

    #[test]
    fn resolve_is_schedule_independent(
        levels in levels(fp_pool()),
        a in schedule(),
        b in schedule(),
    ) {
        // Same shard count for both, so the sets compare slot for slot too.
        let b = Schedule { shard_bits: a.shard_bits, ..b };
        let (wa, sa) = run(FpSet::new(a.shard_bits), &levels, &a);
        let (wb, sb) = run(FpSet::new(b.shard_bits), &levels, &b);
        prop_assert_eq!(wa, wb);
        prop_assert_eq!(sa.iter().collect::<Vec<_>>(), sb.iter().collect::<Vec<_>>());
    }

    #[test]
    fn resolve_equals_sequential_insertion(levels in levels(fp_pool()), sch in schedule()) {
        let (won, _) = run(FpSet::new(sch.shard_bits), &levels, &sch);
        let mut seq = FpSet::new(0);
        for (level, won) in levels.iter().zip(&won) {
            let mut sorted = level.clone();
            sorted.sort_by_key(|c| c.1);
            let mut inline = Vec::new();
            for (k, pos, payload) in sorted {
                if seq.insert(k) {
                    inline.push((k, pos, payload));
                }
            }
            prop_assert_eq!(&inline, won);
        }
    }
}

#[test]
fn chaos_runs_at_eight_threads_are_identical() {
    // A larger level, many schedules: every run must give the same bytes.
    let level: Level<u64> = (0..20_000u32)
        .map(|i| (mix(u64::from(i % 7_919)), (i / 16, i % 16), u64::from(i)))
        .collect();
    let levels = vec![level.clone(), level];
    let mut first: Option<(Won<u64>, Vec<u64>)> = None;
    for seed in 0..10u64 {
        for (threads, workers) in [(1, 1), (2, 3), (4, 4), (8, 8), (8, 5)] {
            let sch = Schedule {
                workers,
                owner_seed: seed,
                threads,
                chaos: Some(seed),
                shard_bits: 5,
            };
            let (won, set) = run(FpSet::new(5), &levels, &sch);
            let snapshot = (won, set.iter().collect());
            match &first {
                None => first = Some(snapshot),
                Some(f) => assert_eq!(f, &snapshot, "seed {seed}, {threads} threads"),
            }
        }
    }
}

#[test]
fn duplicate_positions_are_reported() {
    let mut set = FpSet::new(1);
    let mut c = set.candidates();
    c.push(&set, 9, (0u32, 0u32), ());
    c.push(&set, 9, (0u32, 0u32), ());
    let r = set.resolve(vec![c], &ResolveOptions::threads(2));
    assert!(matches!(r, Err(StoreError::DuplicatePosition { .. })));
}

#[test]
fn mismatched_buffers_are_reported() {
    let mut set = FpSet::new(3);
    let other = FpSet::new(1);
    let mut c = other.candidates();
    c.push(&other, 9, 0u32, ());
    let r = set.resolve(vec![c], &ResolveOptions::threads(1));
    assert!(matches!(r, Err(StoreError::ShardMismatch { .. })));
}

// ---------------------------------------------------------------------------------
// States: codec, fingerprints, exact keys
// ---------------------------------------------------------------------------------

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
    fn states_round_trip(s in state()) {
        let mut bytes = Vec::new();
        encode_state(&s, ctx(), &mut bytes).expect("encodes");
        let mut dec = StateDecoder::new(&NAMES);
        let back = dec.decode(&bytes).expect("decodes");
        let mut again = Vec::new();
        encode_state(&back, ctx(), &mut again).expect("encodes");
        prop_assert_eq!(again, bytes);
        prop_assert_eq!(back, s);
    }

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
    fn exact_and_fingerprint_membership_agree(
        states in prop::collection::vec(state(), 1..60),
        queries in prop::collection::vec(state(), 0..30),
        sch in schedule(),
    ) {
        let f = StateFingerprinter::new(0).expect("polynomial 0");
        let fp = |s: &[Value]| f.fingerprint(s, ctx()).expect("fp");
        // No two distinct states share a fingerprint (a collision would be a
        // finding: the per-pair probability is about 2^-64; issue #42).
        let mut by_fp: HashMap<u64, Box<[u8]>> = HashMap::new();
        for s in states.iter().chain(&queries) {
            let k = key_of(s);
            if let Some(other) = by_fp.insert(fp(s), k.clone()) {
                prop_assert_eq!(other, k);
            }
        }
        let fp_level: Level<u64> = states.iter().enumerate()
            .map(|(i, s)| (fp(s), (u32::try_from(i).unwrap_or(0), 0), i as u64)).collect();
        let ex_level: Level<Box<[u8]>> = states.iter().enumerate()
            .map(|(i, s)| (key_of(s), (u32::try_from(i).unwrap_or(0), 0), i as u64)).collect();
        let (wf, fs) = run(FpSet::new(sch.shard_bits), &[fp_level], &sch);
        let (we, es) = run(ExactSet::new(sch.shard_bits), &[ex_level], &sch);
        // Same winners (by position and payload), same sizes, same membership.
        let pf: Vec<_> = wf[0].iter().map(|w| (w.1, w.2)).collect();
        let pe: Vec<_> = we[0].iter().map(|w| (w.1, w.2)).collect();
        prop_assert_eq!(pf, pe);
        prop_assert_eq!(fs.len(), es.len());
        for q in states.iter().chain(&queries) {
            prop_assert_eq!(fs.contains(&fp(q)), es.contains(&key_of(q)));
        }
    }
}

#[test]
fn intervals_and_enumerated_sets_are_one_state() {
    let f = StateFingerprinter::new(0).expect("polynomial 0");
    let a = [Value::interval(Int::from(1), Int::from(3), ctx())];
    let b = [Value::set(vec![Value::Int(3), Value::Int(2), Value::Int(1)], ctx()).expect("set")];
    assert_eq!(f.fingerprint(&a, ctx()), f.fingerprint(&b, ctx()));
    let mut set = ExactSet::new(2);
    assert!(set.insert(key_of(&a)));
    assert!(!set.insert(key_of(&b)));
    // The frontier keeps each representation as it was.
    let (mut ea, mut eb) = (Vec::new(), Vec::new());
    encode_state(&a, ctx(), &mut ea).expect("encodes");
    encode_state(&b, ctx(), &mut eb).expect("encodes");
    assert_ne!(ea, eb);
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

// ---------------------------------------------------------------------------------
// Frontier and trace log
// ---------------------------------------------------------------------------------

proptest! {
    #[test]
    fn frontier_blocks_cover_in_order(
        sizes in prop::collection::vec(prop::collection::vec(0u8..255, 0..6), 0..300),
        split in prop::collection::vec(0usize..20, 0..12),
        workers in 1usize..10,
    ) {
        // States pushed one by one or as blocks appear in one order.
        let mut b = FrontierBuilder::new(3, 100);
        let mut it = sizes.iter();
        for n in &split {
            let mut block = StateBlock::new();
            for s in it.by_ref().take(*n) {
                block.push(s).expect("small");
            }
            b.append_block(block);
            if let Some(s) = it.next() {
                b.push(s).expect("small");
            }
        }
        for s in it {
            b.push(s).expect("small");
        }
        let f = b.finish();
        prop_assert_eq!(f.len(), sizes.len());
        let all: Vec<&[u8]> = f.iter().collect();
        let want: Vec<&[u8]> = sizes.iter().map(Vec::as_slice).collect();
        prop_assert_eq!(&all, &want);
        for (i, s) in want.iter().enumerate() {
            prop_assert_eq!(f.get(i), Some(*s));
            prop_assert_eq!(f.id(i), Some(100 + i as u64));
        }
        prop_assert_eq!(f.get(sizes.len()), None);
        let blocks = f.blocks(workers);
        let mut next = 0;
        for r in &blocks {
            prop_assert_eq!(r.start, next);
            prop_assert!(!r.is_empty() && r.len() <= 256);
            next = r.end;
        }
        prop_assert_eq!(next, f.len());
    }

    #[test]
    fn trace_paths_follow_parents(parents in prop::collection::vec(any::<u16>(), 0..200), roots in 1usize..5) {
        let mut log = TraceLog::new();
        for r in 0..roots {
            log.push_initial(u32::try_from(r).unwrap_or(0));
        }
        let mut want_depth = vec![1usize; roots];
        for (i, p) in parents.iter().enumerate() {
            let id = roots + i;
            let parent = usize::from(*p) % id;
            let got = log.push(parent as u64, u32::from(*p)).expect("earlier parent");
            prop_assert_eq!(got, id as u64);
            want_depth.push(want_depth[parent] + 1);
        }
        for (id, d) in want_depth.iter().enumerate() {
            let path = log.path(id as u64).expect("recorded");
            prop_assert_eq!(path.len(), *d);
            prop_assert!(path[0].parent.is_none());
            for w in path.windows(2) {
                prop_assert_eq!(w[1].parent, Some(w[0].id));
            }
        }
        prop_assert!(log.push(log.len(), 0).is_err());
        prop_assert!(log.path(log.len()).is_err());
    }
}
