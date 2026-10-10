//! `dive-store`: state storage for the model checker. It provides fingerprint sets,
//! exact sets, per-level frontiers and the trace log (ADR 0015).
//!
//! The crate depends only on `dive-value` (`xtask/src/layering.rs`) and has no
//! `unsafe` code. Concurrency comes from exclusive ownership: during a level's
//! resolve phase each shard of the seen-state set is borrowed mutably by exactly one
//! thread, and during the expand phase the set is shared read-only.
//!
//! # Overview
//!
//! - [`state`]: states as bytes (a lossless encoding for frontiers and spill, a
//!   canonical key for the exact store) and the state fingerprint: dive's two-level
//!   fingerprint in the default profile (cached FP64 value fingerprints combined by
//!   a non-linear mixer chain), TLC's own (in TLC's variable order) under
//!   `--compat=tlc`.
//! - [`StateSet`]: the seen-state set, with its deterministic, level-synchronous
//!   [`resolve`](StateSet::resolve). It is implemented by [`FpSet`] (fingerprint
//!   mode) and [`ExactSet`] (`--store exact`).
//! - [`frontier`]: a level's encoded states in reference order, split into blocks
//!   for workers.
//! - [`trace`]: parent pointers and successor ordinals for counterexample replay.
//! - [`collision`]: TLC's two collision-probability estimates, exactly as TLC prints
//!   them.
//!
//! # One level of search
//!
//! ```
//! use dive_store::frontier::FrontierBuilder;
//! use dive_store::state::{StateFingerprinter, encode_state};
//! use dive_store::trace::TraceLog;
//! use dive_store::{FpSet, ResolveOptions, StateSet};
//! use dive_value::{Ctx, Value};
//!
//! let ctx = Ctx::tlc();
//! let fp = StateFingerprinter::new(0).ok_or("polynomial 0")?;
//! let mut seen = FpSet::new(dive_store::shard_bits_for(2));
//! let mut log = TraceLog::new();
//!
//! // Level 1: one initial state, `x = 0`.
//! let init = vec![Value::Int(0)];
//! seen.insert(fp.fingerprint(&init, ctx)?);
//! let root = log.push_initial(0);
//!
//! // Phase 1 (two workers): successors `x = 1` and `x = 2`, each found twice.
//! // A position is (state index in the level, successor ordinal).
//! let mut buffers = vec![seen.candidates(), seen.candidates()];
//! for (worker, ordinal, x) in [(0, 0, 1), (1, 1, 2), (1, 2, 1), (0, 3, 2)] {
//!     let state = vec![Value::Int(x)];
//!     let key = fp.fingerprint(&state, ctx)?;
//!     if !seen.contains(&key) {
//!         buffers[worker].push(&seen, key, (0u64, ordinal), state);
//!     }
//! }
//!
//! // Phase 2: resolve; the earliest occurrence of each state wins.
//! let winners = seen.resolve(buffers, &ResolveOptions::threads(2))?.into_position_order();
//! let ordinals: Vec<u32> = winners.iter().map(|w| w.pos.1).collect();
//! assert_eq!(ordinals, [0, 1]);
//!
//! // Phase 3: the next frontier, in position order, and its trace records.
//! let mut next = FrontierBuilder::new(2, log.len());
//! for w in &winners {
//!     next.push_with(|buf| encode_state(&w.payload, ctx, buf))?;
//!     log.push(root, w.pos.1)?;
//! }
//! assert_eq!(next.finish().len(), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]

pub mod collision;
mod error;
mod exact;
mod fpset;
pub mod frontier;
pub mod predecessor;
mod set;
pub mod spill;
pub mod state;
pub mod trace;

pub use error::StoreError;
pub use exact::{ExactSet, key_hash};
pub use fpset::FpSet;
pub use set::{
    Candidate, Candidates, ResolveOptions, Resolved, StateSet, StoreMode, shard_bits_for,
};
