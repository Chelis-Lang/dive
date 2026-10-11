//! Store micro-benchmarks. Run by hand, never in CI:
//! `cargo bench -p dive-store --bench store`.
//!
//! - Insert throughput: a level's resolve (phase 2) of 2^20 candidates into a set that
//!   already holds 2^20 states, at 1, 2, 4 and 8 resolve threads.
//! - Lookup throughput: phase-1 `contains` on a shared set from 1, 2, 4 and 8 threads
//!   at once.
//! - The same for the exact store, with 48-byte keys.
//! - Per-state costs: the two-level fingerprint (cached and not), and the state
//!   codec.
//!
//! Memory per state is printed by `cargo run -p dive-store --release --example memory`.

#![allow(
    clippy::expect_used,
    missing_docs,
    reason = "benchmark setup: a failure is a broken benchmark"
)]

use std::sync::LazyLock;
use std::sync::atomic::{AtomicI64, Ordering};

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
use dive_store::state::{StateDecoder, StateFingerprinter, encode_state};
use dive_store::{ExactSet, FpSet, ResolveOptions, StateSet, shard_bits_for};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Interner, Value};

fn main() {
    divan::main();
}

const N: u64 = 1 << 20;
const WORKERS: usize = 8;

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A set holding fingerprints `mix(0..N)`.
fn filled_fpset() -> FpSet {
    let mut s = FpSet::new(shard_bits_for(WORKERS));
    for i in 0..N {
        s.insert(mix(i));
    }
    s
}

static FPSET: LazyLock<FpSet> = LazyLock::new(filled_fpset);

#[divan::bench(args = [1, 2, 4, 8], sample_count = 10)]
fn fpset_resolve(b: Bencher<'_, '_>, threads: usize) {
    // Candidates mix(N/2 .. 3N/2), each found twice: half are already present and
    // half new, as in a level with many duplicate successors.
    b.counter(ItemsCount::new(2 * N))
        .with_inputs(|| {
            let set = FPSET.clone();
            let mut bufs: Vec<_> = (0..WORKERS).map(|_| set.candidates()).collect();
            for i in 0..2 * N {
                let k = mix(N / 2 + i % N);
                let w = usize::try_from(i % WORKERS as u64).unwrap_or(0);
                bufs[w].push(&set, k, i, ());
            }
            (set, bufs)
        })
        .bench_values(|(mut set, bufs)| {
            let won = set
                .resolve(bufs, &ResolveOptions::threads(threads))
                .expect("resolves");
            black_box(won.len())
        });
}

#[divan::bench(threads = [1, 2, 4, 8])]
fn fpset_contains(b: Bencher<'_, '_>) {
    let set = &*FPSET;
    let mut i = 0u64;
    b.counter(ItemsCount::new(1024usize)).bench_local(|| {
        let mut hits = 0u32;
        for _ in 0..1024 {
            // Half hits, half misses.
            i = i.wrapping_add(1);
            hits += u32::from(set.contains(&mix(i % (2 * N))));
        }
        black_box(hits)
    });
}

#[divan::bench(sample_count = 10)]
fn fpset_insert_sequential(b: Bencher<'_, '_>) {
    b.counter(ItemsCount::new(N)).bench(|| {
        let mut s = FpSet::new(0);
        for i in 0..N {
            s.insert(mix(i));
        }
        black_box(s.len())
    });
}

const EXACT_N: u64 = 1 << 18;

fn key(i: u64) -> Box<[u8]> {
    // A 48-byte key, typical of a small state's canonical encoding.
    let mut k = Vec::with_capacity(48);
    for j in 0..6 {
        k.extend_from_slice(&mix(i ^ (j << 40)).to_le_bytes());
    }
    k.into_boxed_slice()
}

static EXACT: LazyLock<ExactSet> = LazyLock::new(|| {
    let mut s = ExactSet::new(shard_bits_for(WORKERS));
    for i in 0..EXACT_N {
        s.insert(key(i));
    }
    s
});

#[divan::bench(args = [1, 2, 4, 8], sample_count = 10)]
fn exact_resolve(b: Bencher<'_, '_>, threads: usize) {
    b.counter(ItemsCount::new(2 * EXACT_N))
        .with_inputs(|| {
            let set = EXACT.clone();
            let mut bufs: Vec<_> = (0..WORKERS).map(|_| set.candidates()).collect();
            for i in 0..2 * EXACT_N {
                let w = usize::try_from(i % WORKERS as u64).unwrap_or(0);
                bufs[w].push(&set, key(EXACT_N / 2 + i % EXACT_N), i, ());
            }
            (set, bufs)
        })
        .bench_values(|(mut set, bufs)| {
            let won = set
                .resolve(bufs, &ResolveOptions::threads(threads))
                .expect("resolves");
            black_box(won.len())
        });
}

#[divan::bench(threads = [1, 2, 4, 8])]
fn exact_contains(b: Bencher<'_, '_>) {
    let set = &*EXACT;
    let keys: Vec<Box<[u8]>> = (0..1024).map(|i| key(i * 2 % (2 * EXACT_N))).collect();
    b.counter(ItemsCount::new(keys.len()))
        .bench_local(|| black_box(keys.iter().filter(|k| set.contains(k)).count()));
}

static NAMES: LazyLock<Interner> = LazyLock::new(Interner::new);

/// A state like a small protocol's: a program counter per process, a message
/// set of records, a counter and a queue.
fn sample_state(i: i64) -> Vec<Value> {
    let ctx = Ctx::tlc();
    let pcs = Value::fcn(
        (1..=3)
            .map(|p| {
                (
                    Value::Int(p),
                    Value::string(&NAMES, if (i + p) % 2 == 0 { "idle" } else { "busy" }),
                )
            })
            .collect(),
        ctx,
    )
    .expect("fcn");
    let msgs = Value::set(
        (0..10)
            .map(|m| {
                Value::record(
                    vec![
                        (NAMES.intern("src"), Value::Int(m % 3)),
                        (NAMES.intern("val"), Value::Int(m + i)),
                    ],
                    ctx,
                )
                .expect("record")
            })
            .collect(),
        ctx,
    )
    .expect("set");
    vec![
        pcs,
        msgs,
        Value::Int(i),
        Value::tuple(vec![Value::Int(1), Value::Int(2), Value::Int(i)]),
    ]
}

#[divan::bench]
fn fingerprint_two_level_cached(b: Bencher<'_, '_>) {
    let f = StateFingerprinter::new(0).expect("polynomial 0");
    let s = sample_state(5);
    let ctx = Ctx::tlc();
    f.fingerprint(&s, ctx).expect("fp");
    b.bench(|| f.fingerprint(black_box(&s), ctx).expect("fp"));
}

#[divan::bench]
fn fingerprint_two_level_one_var_changed(b: Bencher<'_, '_>) {
    // A successor that shares all but the counter with its parent.
    let f = StateFingerprinter::new(0).expect("polynomial 0");
    let parent = sample_state(5);
    let ctx = Ctx::tlc();
    f.fingerprint(&parent, ctx).expect("fp");
    let i = AtomicI64::new(0);
    b.with_inputs(|| {
        let mut s = parent.clone();
        s[2] = Value::Int(i.fetch_add(1, Ordering::Relaxed));
        s
    })
    .bench_refs(|s| f.fingerprint(s, ctx).expect("fp"));
}

#[divan::bench]
fn fingerprint_tlc_single_stream(b: Bencher<'_, '_>) {
    let f = TlcFingerprint::new(0).expect("polynomial 0");
    let s = sample_state(5);
    let ctx = Ctx::tlc();
    b.bench(|| f.fingerprint_state(black_box(&s), ctx).expect("fp"));
}

#[divan::bench]
fn encode_sample_state(b: Bencher<'_, '_>) {
    let s = sample_state(5);
    let ctx = Ctx::tlc();
    let mut out = Vec::with_capacity(256);
    b.bench_local(|| {
        out.clear();
        encode_state(black_box(&s), ctx, &mut out).expect("encodes");
        out.len()
    });
}

#[divan::bench]
fn decode_sample_state(b: Bencher<'_, '_>) {
    let s = sample_state(5);
    let ctx = Ctx::tlc();
    let mut bytes = Vec::new();
    encode_state(&s, ctx, &mut bytes).expect("encodes");
    let mut dec = StateDecoder::new(&NAMES);
    let mut out = Vec::new();
    b.bench_local(|| {
        dec.decode_into(black_box(&bytes), &mut out)
            .expect("decodes");
        out.len()
    });
}
