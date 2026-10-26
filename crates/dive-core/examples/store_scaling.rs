//! Interning-store scaling microbenchmark (M0, `10-plan.md` §2).
//!
//! Each thread builds small sets and functions of integers in its own arena
//! and interns them into one shared store. Half of each thread's values are
//! shared with the other threads (hits), half are its own (inserts).
//!
//! ```sh
//! cargo run -p dive-core --release --example store_scaling -- [ops-per-thread]
//! ```
//!
//! Prints one JSON line per thread count (1, 2, 4, 8) with the throughput
//! and the speedup over one thread.
#![allow(clippy::print_stdout, reason = "a benchmark prints its results")]

use std::sync::Arc;
use std::time::Instant;

use dive_core::{Handle, Heap, Store};

fn work(store: &Arc<Store>, thread: u64, ops: u64) -> u64 {
    let mut heap = Heap::new(Arc::clone(store), false, false);
    let mut acc = 0u64;
    let mut buf = Vec::with_capacity(8);
    for i in 0..ops {
        // Even ops: a value shared by every thread; odd: thread-private.
        let seed = if i % 2 == 0 {
            i / 2
        } else {
            (thread << 40) | i
        };
        buf.clear();
        for k in 0..4 {
            let v =
                i64::try_from((seed.wrapping_mul(0x9e37_79b9) >> (k * 8)) & 0xffff).unwrap_or(0);
            buf.push(Handle::int(v).unwrap_or(Handle::TRUE));
        }
        let set = heap.set_from(&mut buf.clone());
        let f = heap.tuple(&[set, buf[0], buf[1]]);
        if let Ok(p) = heap.intern(f) {
            acc ^= p.bits();
        }
        if i % 1024 == 1023 {
            heap.reset();
        }
    }
    acc
}

fn main() {
    let ops: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1_000_000);
    let mut base = None;
    for threads in [1u64, 2, 4, 8] {
        let store = Arc::new(Store::default());
        let start = Instant::now();
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let s = Arc::clone(&store);
                std::thread::spawn(move || work(&s, t, ops))
            })
            .collect();
        let mut x = 0;
        for h in handles {
            x ^= h.join().unwrap_or(0);
        }
        let secs = start.elapsed().as_secs_f64();
        #[allow(clippy::cast_precision_loss, reason = "a rate")]
        let rate = (threads * ops) as f64 / secs;
        let speedup = rate / *base.get_or_insert(rate);
        let st = store.stats();
        println!(
            "{{\"threads\":{threads},\"ops\":{},\"seconds\":{secs:.3},\"ops_per_sec\":{rate:.0},\"speedup\":{speedup:.2},\"hits\":{},\"inserts\":{},\"check\":{}}}",
            threads * ops,
            st.hits,
            st.misses,
            x & 1
        );
    }
}
