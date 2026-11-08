//! State fingerprint and key micro-benchmarks. Run by hand, never in CI:
//! `cargo bench -p dive-store --bench store`.
//!
//! Per-state costs: the two-level fingerprint (cached and not), TLC's single-stream
//! fingerprint, and the canonical exact-store key.

#![allow(
    clippy::expect_used,
    missing_docs,
    reason = "benchmark setup: a failure is a broken benchmark"
)]

use std::sync::LazyLock;
use std::sync::atomic::{AtomicI64, Ordering};

use divan::{Bencher, black_box};
use dive_store::state::{StateFingerprinter, state_key};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Interner, Value};

fn main() {
    divan::main();
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
fn key_sample_state(b: Bencher<'_, '_>) {
    let s = sample_state(5);
    let ctx = Ctx::tlc();
    let mut out = Vec::with_capacity(256);
    b.bench_local(|| {
        out.clear();
        state_key(black_box(&s), ctx, &mut out).expect("keys");
        out.len()
    });
}
