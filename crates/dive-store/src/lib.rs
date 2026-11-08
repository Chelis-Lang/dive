//! `dive-store`: state storage for the model checker (ADR 0015). It provides the
//! state fingerprint and the exact-store key, the disk-backed seen-state set, the
//! ordered frontier spill and the predecessor log Core's search uses.
//!
//! The crate depends only on `dive-value` (`xtask/src/layering.rs`) and has no
//! `unsafe` code.
//!
//! # Overview
//!
//! - [`state`]: the state fingerprint (dive's two-level fingerprint in the default
//!   profile, cached FP64 value fingerprints combined by a non-linear mixer chain;
//!   TLC's own, in TLC's variable order, under `--compat=tlc`) and the canonical
//!   exact-store key.
//! - [`StoreMode`]: whether two states are the same by fingerprint or by
//!   canonical key.
//! - [`seen`]: the disk-backed seen-state set (`--seen disk`).
//! - [`spill`]: the ordered, disk-spilled frontier.
//! - [`predecessor`]: parent pointers and successor ordinals for counterexample
//!   replay, in memory or on disk.
//! - [`trace`]: the state ids and path steps the predecessor log returns.
//!
//! # Fingerprints and keys
//!
//! ```
//! use dive_store::state::{StateFingerprinter, state_key};
//! use dive_value::{Ctx, Int, Value};
//!
//! let ctx = Ctx::tlc();
//! let fp = StateFingerprinter::new(0).ok_or("polynomial 0")?;
//! // `1..3` and `{3, 2, 1}` are one state, whatever their representation.
//! let a = [Value::interval(Int::from(1), Int::from(3), ctx)];
//! let b = [Value::set(vec![Value::Int(3), Value::Int(2), Value::Int(1)], ctx)?];
//! assert_eq!(fp.fingerprint(&a, ctx)?, fp.fingerprint(&b, ctx)?);
//! let (mut ka, mut kb) = (Vec::new(), Vec::new());
//! state_key(&a, ctx, &mut ka)?;
//! state_key(&b, ctx, &mut kb)?;
//! assert_eq!(ka, kb);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]

mod collision;
mod error;
pub mod predecessor;
pub mod seen;
pub mod spill;
pub mod state;
pub mod trace;

pub use error::StoreError;

/// How a store decides that two states are the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreMode {
    /// 64-bit fingerprints: two distinct states may collide.
    Fingerprint,
    /// Full canonical state encodings: exhaustiveness is unconditional (charter
    /// soundness rule 4).
    Exact,
}

impl StoreMode {
    /// The name used in output (`store: "fingerprint"` or `store: "exact"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fingerprint => "fingerprint",
            Self::Exact => "exact",
        }
    }
}
