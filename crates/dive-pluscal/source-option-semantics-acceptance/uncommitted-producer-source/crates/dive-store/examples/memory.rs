//! Prints the store's memory per state. Run by hand:
//! `cargo run -p dive-store --release --example memory`.
//!
//! Fingerprint-set memory depends on where the load factor sits between 3/8 and 3/4,
//! so it is printed at several sizes. The exact store is measured with 48-byte keys.

#![allow(
    clippy::expect_used,
    reason = "an example: a failure is a broken example"
)]

use dive_store::frontier::FrontierBuilder;
use dive_store::state::encode_state;
use dive_store::trace::TraceLog;
use dive_store::{ExactSet, FpSet, StateSet, shard_bits_for};
use dive_value::{Ctx, Interner, Value};

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[allow(clippy::cast_precision_loss, reason = "reporting ratios")]
fn per(bytes: usize, n: u64) -> f64 {
    bytes as f64 / n as f64
}

fn main() {
    println!("store                         states   bytes/state");
    for n in [100_000u64, 501_500, 1_000_000, 2_000_000, 10_000_000] {
        let mut s = FpSet::new(shard_bits_for(8));
        for i in 0..n {
            s.insert(mix(i));
        }
        println!(
            "fingerprint set (64 shards) {n:>10}   {:>8.1}",
            per(s.memory_bytes(), n)
        );
    }
    for n in [100_000u64, 1_000_000] {
        let mut s = ExactSet::new(shard_bits_for(8));
        for i in 0..n {
            let mut k = Vec::with_capacity(48);
            for j in 0..6 {
                k.extend_from_slice(&mix(i ^ (j << 40)).to_le_bytes());
            }
            s.insert(k.into_boxed_slice());
        }
        println!(
            "exact set, 48-byte keys     {n:>10}   {:>8.1}  (of which keys {:.1})",
            per(s.memory_bytes(), n),
            per(s.key_bytes(), n)
        );
    }
    let n = 1_000_000u64;
    let mut log = TraceLog::new();
    log.push_initial(0);
    for i in 1..n {
        log.push(i / 3, 0).expect("earlier parent");
    }
    log = log.clone(); // drop spare capacity
    println!(
        "trace log                   {n:>10}   {:>8.1}",
        per(log.memory_bytes(), n)
    );

    // Frontier: a state of four variables (pc function, a set of ten records, a
    // counter, a tuple), as in the benchmarks.
    let names = Interner::new();
    let ctx = Ctx::tlc();
    let mut b = FrontierBuilder::new(1, 0);
    let n = 100_000u64;
    for i in 0..n {
        let i = i64::try_from(i).unwrap_or(0);
        let pcs = Value::fcn(
            (1..=3)
                .map(|p| {
                    (
                        Value::Int(p),
                        Value::string(&names, if (i + p) % 2 == 0 { "idle" } else { "busy" }),
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
                            (names.intern("src"), Value::Int(m % 3)),
                            (names.intern("val"), Value::Int(m + i)),
                        ],
                        ctx,
                    )
                    .expect("record")
                })
                .collect(),
            ctx,
        )
        .expect("set");
        let s = [
            pcs,
            msgs,
            Value::Int(i),
            Value::tuple(vec![Value::Int(1), Value::Int(i)]),
        ];
        b.push_with(|buf| encode_state(&s, ctx, buf))
            .expect("encodes");
    }
    let f = b.finish();
    println!(
        "frontier (encoded states)   {n:>10}   {:>8.1}  (encoded {:.1})",
        per(f.memory_bytes(), n),
        per(f.encoded_bytes(), n)
    );
}
