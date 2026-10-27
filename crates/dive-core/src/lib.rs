//! `dive-core`: the new execution core (ADR 0117; design in
//! `docs/developer/core/`).
//!
//! This is the M0 spike: the default profile only, and only the constructs
//! the spike models use. Everything else is refused before the run starts
//! with an `unsupported` result naming the construct.
//!
//! - [`handle`]: 64-bit tagged value words.
//! - [`store`]: the hash-consed, sharded persistent store.
//! - [`arena`]: the per-worker transient arena with epochs.
//! - [`heap`]: hashing, equality, order, builders and interning.
//! - [`mod@compile`]: the plan compiler from `dive_ir` plus the binder handoff.
//! - [`eval`]: the interpreter for value nodes.
//! - [`generate`]: TLC's successor-generation rules.
//! - [`memo`]: keyed result caches.
//! - [`opts`]: strategy switches for ablation.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_core::{Handle, Heap, Store};
//!
//! let store = Arc::new(Store::default());
//! let mut heap = Heap::new(store, false, false);
//! let one = Handle::int(1).unwrap();
//! let two = Handle::int(2).unwrap();
//! let a = heap.set_from(&mut vec![two, one]);
//! let b = heap.set_from(&mut vec![one, two, one]);
//! assert!(heap.eq(a, b));
//! let (pa, pb) = (heap.intern(a).unwrap(), heap.intern(b).unwrap());
//! assert_eq!(pa, pb); // hash-consed
//! ```
#![forbid(unsafe_code)]

pub mod arena;
mod compare;
pub mod compile;
pub mod convert;
pub mod eval;
pub mod generate;
pub mod handle;
pub mod heap;
pub mod memo;
pub mod node;
pub mod opts;
pub mod plan;
pub mod store;
pub mod strings;

pub use compile::{Handoff, Role, compile};
pub use eval::Machine;
pub use generate::Sink;
pub use handle::Handle;
pub use heap::Heap;
pub use opts::Opts;
pub use plan::Plan;
pub use store::Store;

/// What kind of failure an [`Error`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A construct or size the core does not implement: never a verdict.
    Unsupported,
    /// An evaluation error TLC would also raise. M0 does not classify
    /// errors; the run reports it as unsupported, naming it.
    Eval,
    /// A bug in the core.
    Internal,
    /// Not an error: the sink ended the enumeration.
    Stop,
}

/// An error of the core. Boxed, so that `Result<Handle, Error>` is two
/// words and returns in registers on the interpreter's hot path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(Box<ErrorData>);

/// The contents of an [`Error`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorData {
    /// The kind.
    pub kind: ErrorKind,
    /// What happened.
    pub message: String,
}

impl Error {
    fn new(kind: ErrorKind, message: String) -> Self {
        Self(Box::new(ErrorData { kind, message }))
    }

    /// An unsupported construct.
    #[must_use]
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unsupported, what.into())
    }

    /// An evaluation error.
    #[must_use]
    pub fn eval(what: impl Into<String>) -> Self {
        Self::new(ErrorKind::Eval, what.into())
    }

    /// An internal error.
    #[must_use]
    pub fn internal(what: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, what.into())
    }

    /// The sink's stop.
    #[must_use]
    pub fn stop() -> Self {
        Self::new(ErrorKind::Stop, String::new())
    }

    /// The kind.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.0.kind
    }

    /// The message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0.message
    }

    /// The kind and the message.
    #[must_use]
    pub fn into_parts(self) -> (ErrorKind, String) {
        (self.0.kind, self.0.message)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.message)
    }
}

impl std::error::Error for Error {}
