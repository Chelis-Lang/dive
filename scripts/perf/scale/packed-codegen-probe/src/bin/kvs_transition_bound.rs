//! Model-specific throughput diagnostic for the pinned Examples KeyValueStore.
//!
//! This is a direct translation of Init, Next and TxLifecycle into a 32-bit
//! layout. It measures the room left for general lowering. It accepts only
//! the four named configurations below and cannot check edited TLA+ source.

use std::time::Instant;

struct Search<const K: u32, const V: u32, const T: u32, const SYM: bool> {
    seen: Vec<u64>,
    frontier: Vec<u32>,
    next: Vec<u32>,
    generated: u64,
    distinct: u64,
    depth: u64,
}

impl<const K: u32, const V: u32, const T: u32, const SYM: bool> Search<K, V, T, SYM> {
    // The tested domains have two value bits with zero representing NoVal.
    const STORE_WIDTH: u32 = 2 * K;
    const STORE_MASK: u32 = (1 << Self::STORE_WIDTH) - 1;
    const WRITTEN: u32 = Self::STORE_WIDTH;
    const MISSED: u32 = Self::WRITTEN + K;
    const OPEN: u32 = 1 << (Self::MISSED + K);
    const LANE_WIDTH: u32 = Self::MISSED + K + 1;
    const LANE_MASK: u32 = (1 << Self::LANE_WIDTH) - 1;
    const KEY_MASK: u32 = (1 << K) - 1;

    fn run() -> Self {
        let width = Self::STORE_WIDTH + T * Self::LANE_WIDTH;
        let mut seen = vec![0; 1_usize << (width - 6)];
        seen[0] = 1;
        let mut search = Self {
            seen,
            frontier: vec![0],
            next: Vec::new(),
            generated: 1,
            distinct: 1,
            depth: 0,
        };
        while !search.frontier.is_empty() {
            search.depth += 1;
            for index in 0..search.frontier.len() {
                let before = search.generated;
                search.expand(search.frontier[index]);
                assert!(before < search.generated, "deadlock");
            }
            search.frontier.clear();
            std::mem::swap(&mut search.frontier, &mut search.next);
        }
        search
    }

    fn offset(tx: u32) -> u32 {
        Self::STORE_WIDTH + tx * Self::LANE_WIDTH
    }

    fn lane(row: u32, tx: u32) -> u32 {
        (row >> Self::offset(tx)) & Self::LANE_MASK
    }

    fn with_lane(row: u32, tx: u32, lane: u32) -> u32 {
        let offset = Self::offset(tx);
        (row & !(Self::LANE_MASK << offset)) | (lane << offset)
    }

    fn key(row: u32) -> u32 {
        if !SYM {
            return row;
        }
        let mut lanes = [0; 3];
        for tx in 0..T {
            lanes[tx as usize] = Self::lane(row, tx);
        }
        lanes[..T as usize].sort_unstable_by(|a, b| b.cmp(a));
        let mut key = row & Self::STORE_MASK;
        for tx in 0..T {
            key |= lanes[tx as usize] << Self::offset(tx);
        }
        key
    }

    fn emit(&mut self, row: u32) {
        self.generated += 1;
        let key = Self::key(row);
        let word = &mut self.seen[key as usize / 64];
        let mask = 1 << (key % 64);
        if *word & mask != 0 {
            return;
        }
        *word |= mask;
        self.distinct += 1;
        self.check_lifecycle(row);
        self.next.push(row);
    }

    fn check_lifecycle(&self, row: u32) {
        let store = row & Self::STORE_MASK;
        for tx in 0..T {
            let lane = Self::lane(row, tx);
            if lane & Self::OPEN == 0 {
                assert_eq!(lane, 0, "closed transaction has retained fields");
                continue;
            }
            let unequal = (lane ^ store) & Self::STORE_MASK;
            let covered = (lane >> Self::WRITTEN) | (lane >> Self::MISSED);
            for key in 0..K {
                assert!(unequal & (3 << (2 * key)) == 0 || covered & (1 << key) != 0);
            }
        }
    }

    fn expand(&mut self, row: u32) {
        let store = row & Self::STORE_MASK;
        for tx in 0..T {
            let lane = Self::lane(row, tx);
            if lane & Self::OPEN == 0 {
                self.emit(Self::with_lane(row, tx, Self::OPEN | store));
                continue;
            }
            for key in 0..K {
                let shift = 2 * key;
                let current = (lane >> shift) & 3;
                let marked = (lane & !(3 << shift)) | (1 << (Self::WRITTEN + key));
                // Add if absent, update to any different value otherwise.
                for value in 1..=V {
                    if current != value {
                        self.emit(Self::with_lane(row, tx, marked | (value << shift)));
                    }
                }
                if current != 0 {
                    self.emit(Self::with_lane(row, tx, marked));
                }
            }
            let cleared = Self::with_lane(row, tx, 0);
            self.emit(cleared); // rollback
            let written = (lane >> Self::WRITTEN) & Self::KEY_MASK;
            let missed = (lane >> Self::MISSED) & Self::KEY_MASK;
            if written & missed == 0 {
                let mut cells = 0;
                for key in 0..K {
                    if written & (1 << key) != 0 {
                        cells |= 3 << (2 * key);
                    }
                }
                let mut closed = (cleared & !cells) | (lane & cells);
                for other in 0..T {
                    if Self::lane(cleared, other) & Self::OPEN != 0 {
                        closed |= written << (Self::offset(other) + Self::MISSED);
                    }
                }
                self.emit(closed);
            }
        }
    }
}

fn measure<const K: u32, const V: u32, const T: u32, const SYM: bool>(name: &str) {
    let start = Instant::now();
    let result = Search::<K, V, T, SYM>::run();
    let elapsed_ns = start.elapsed().as_nanos();
    println!(
        "{{\"case\":{name:?},\"elapsed_ns\":{elapsed_ns},\"distinct\":{},\"generated\":{},\"depth\":{},\"identity\":\"exact\",\"symmetry\":{SYM}}}",
        result.distinct, result.generated, result.depth,
    );
}

fn main() -> std::process::ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("reduced") => measure::<2, 2, 2, true>("reduced"),
        Some("reduced3") => measure::<2, 2, 3, true>("reduced3"),
        Some("small") => measure::<2, 2, 3, false>("small"),
        Some("medium") => measure::<3, 3, 2, true>("medium"),
        _ => {
            eprintln!("usage: kvs_transition_bound reduced|reduced3|small|medium");
            return std::process::ExitCode::FAILURE;
        }
    }
    std::process::ExitCode::SUCCESS
}
