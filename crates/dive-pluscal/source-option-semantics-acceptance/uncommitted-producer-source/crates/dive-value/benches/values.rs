//! Micro-benchmarks for the value layer's hot paths. Run by hand, never in CI:
//! `cargo bench -p dive-value --bench values`.

#![allow(
    clippy::expect_used,
    reason = "benchmark setup: a failure is a broken benchmark"
)]

use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use divan::{Bencher, black_box};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::print::print_value;
use dive_value::{Ctx, Interner, Value, tlc};

fn main() {
    divan::main();
}

static NAMES: LazyLock<Interner> = LazyLock::new(Interner::new);

fn ctx() -> Ctx {
    Ctx::tlc()
}

fn ints(n: i64) -> Vec<Value> {
    // A fixed pseudo-random permutation, so normalization has work to do.
    (0..n).map(|i| Value::Int((i * 7919) % n)).collect()
}

fn record(i: i64) -> Value {
    Value::record(
        vec![
            (
                NAMES.intern("pc"),
                Value::string(&NAMES, if i % 2 == 0 { "idle" } else { "busy" }),
            ),
            (NAMES.intern("count"), Value::Int(i)),
            (
                NAMES.intern("queue"),
                Value::tuple(vec![Value::Int(i), Value::Int(i + 1)]),
            ),
        ],
        ctx(),
    )
    .expect("record")
}

fn records(n: i64) -> Vec<Value> {
    (0..n).rev().map(record).collect()
}

#[divan::bench(args = [10, 100, 1000])]
fn normalize_int_set(b: Bencher<'_, '_>, n: i64) {
    b.with_inputs(|| ints(n))
        .bench_values(|v| Value::set(v, ctx()).expect("set"));
}

#[divan::bench(args = [10, 100])]
fn normalize_record_set(b: Bencher<'_, '_>, n: i64) {
    b.with_inputs(|| records(n))
        .bench_values(|v| Value::set(v, ctx()).expect("set"));
}

#[divan::bench]
fn clone_set_of_records(b: Bencher<'_, '_>) {
    let s = Value::set(records(100), ctx()).expect("set");
    b.bench(|| black_box(&s).clone());
}

#[divan::bench]
fn hash_set_of_records(b: Bencher<'_, '_>) {
    let s = Value::set(records(100), ctx()).expect("set");
    b.bench(|| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        black_box(&s).hash(&mut h);
        h.finish()
    });
}

#[divan::bench]
fn equal_sets_distinct_allocations(b: Bencher<'_, '_>) {
    let s = Value::set(records(100), ctx()).expect("set");
    let t = Value::set(records(100), ctx()).expect("set");
    b.bench(|| black_box(&s) == black_box(&t));
}

#[divan::bench]
fn tlc_compare_records(b: Bencher<'_, '_>) {
    let (x, y) = (record(5), record(6));
    b.bench(|| tlc::compare(black_box(&x), black_box(&y), ctx()).expect("comparable"));
}

#[divan::bench]
fn member_enum_1000(b: Bencher<'_, '_>) {
    let s = Value::set(ints(1000), ctx()).expect("set");
    let x = Value::Int(777);
    b.bench(|| tlc::member(black_box(&x), black_box(&s), ctx()).expect("member"));
}

#[divan::bench]
fn member_interval(b: Bencher<'_, '_>) {
    let s = Value::interval(0.into(), 1_000_000.into(), ctx());
    let x = Value::Int(777);
    b.bench(|| tlc::member(black_box(&x), black_box(&s), ctx()).expect("member"));
}

#[divan::bench]
fn fp64_state(b: Bencher<'_, '_>) {
    let fp = TlcFingerprint::new(0).expect("poly");
    let state = vec![
        record(3),
        Value::set(ints(10), ctx()).expect("set"),
        Value::tuple(ints(5)),
    ];
    b.bench(|| fp.fingerprint_state(black_box(&state), ctx()).expect("fp"));
}

#[divan::bench]
fn enumerate_subset_10(b: Bencher<'_, '_>) {
    b.with_inputs(|| Value::subset(Value::set(ints(10), ctx()).expect("set"), ctx()))
        .bench_values(|s| {
            s.as_set()
                .expect("set")
                .elements(ctx())
                .expect("enum")
                .len()
        });
}

#[divan::bench]
fn print_set_of_records(b: Bencher<'_, '_>) {
    let s = Value::set(records(20), ctx()).expect("set");
    b.bench(|| print_value(black_box(&s), ctx()).expect("print"));
}
