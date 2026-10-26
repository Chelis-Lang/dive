//! Persistent-view microbenchmark (M1, `03-value-model.md` §12 step 2).
//!
//! Interns 1,000,000 tuples of 4, 8 and 32 integers, then reads them back
//! through `Heap::view`: 20,000,000 reads at random (cache-missing), and
//! runs of nearby reads. Prints the nanoseconds per read.
//!
//! ```sh
//! cargo run -p dive-core --release --example store_views
//! ```
#![allow(clippy::print_stdout, reason = "a benchmark prints its results")]

use std::sync::Arc;
use std::time::Instant;

use dive_core::{Handle, Heap, Store};

const NODES: usize = 1_000_000;
const READS: u32 = 20_000_000;

fn main() {
    for width in [4i64, 8, 32] {
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(Arc::clone(&store), false, false);
        let mut hs = Vec::with_capacity(NODES);
        for i in 0..NODES {
            let base = i64::try_from(i).unwrap_or(0) * 31;
            let vals: Vec<Handle> = (0..width)
                .map(|k| Handle::int(base + k).unwrap_or(Handle::TRUE))
                .collect();
            let t = heap.tuple(&vals);
            hs.push(heap.intern(t).unwrap_or(Handle::EMPTY));
            if i % 1024 == 0 {
                heap.reset();
            }
        }
        let mid = usize::try_from(width / 2).unwrap_or(0);
        let mut x = 0x1234_5678u64;
        let mut acc = 0u64;
        let t = Instant::now();
        for _ in 0..READS {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let h = hs[usize::try_from(x % NODES as u64).unwrap_or(0)];
            let v = heap.view(h);
            acc = acc.wrapping_add(v.hash).wrapping_add(v.data[mid].bits());
        }
        let random = t.elapsed().as_secs_f64() * 1e9 / f64::from(READS);
        let t = Instant::now();
        let mut reads = 0u32;
        for r in 0..20 {
            for h in hs.iter().skip(r * 1000).take(50_000) {
                acc = acc.wrapping_add(heap.view(*h).data[0].bits());
                reads += 1;
            }
        }
        let near = t.elapsed().as_secs_f64() * 1e9 / f64::from(reads);
        println!(
            "{{\"width\":{width},\"random_ns\":{random:.1},\"near_ns\":{near:.1},\"check\":{}}}",
            acc & 1
        );
    }
}
