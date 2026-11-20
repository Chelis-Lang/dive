//! `dive-core`: Core, dive's compiled execution core for explicit-state
//! model checking.
//!
//! Core turns a bound specification into a [`Plan`] and evaluates it over
//! compact value handles instead of tree-walking the IR. [`compile()`] takes
//! `dive_ir` plus the binder's [`Handoff`] (constants, roles and the
//! definitions left to evaluate) and compiles every value expression and
//! every action into flat node arrays. A [`Machine`] interprets those nodes
//! over a [`Heap`]: persistent values live in a hash-consed, sharded
//! [`Store`] shared by all workers, and each worker builds intermediate
//! values in its own transient arena, released once a state is accepted.
//! Successor generation follows TLC's `getNextStates` rules and hands each
//! initial state and successor to a [`Sink`]; the exploration engine in
//! `dive-explore` provides that sink and does the searching, storing and
//! checking.
//!
//! Core serves both profiles, dive's default and `--compat=tlc`: it keeps
//! TLC's evaluation order and error classes, supports either integer mode
//! and either value order, and implements `SYMMETRY` reduction. Anything it
//! does not implement exactly is refused before or during the run with an
//! `unsupported` result naming the construct, never approximated.
//!
//! - [`handle`]: 64-bit tagged value words.
//! - [`store`]: the hash-consed, sharded persistent store.
//! - [`arena`]: the per-worker transient arena with epochs.
//! - [`heap`]: hashing, equality, order, builders and interning.
//! - [`int`]: integers of any size, in both integer modes.
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
pub mod candidate_codec;
mod compare;
pub mod compile;
pub mod convert;
pub mod debug;
pub mod eval;
pub mod frontier_codec;
pub mod generate;
#[doc(hidden)]
pub mod guard_experiment;
pub mod handle;
pub mod heap;
pub mod int;
pub mod key;
mod member_index;
pub mod memo;
pub mod node;
pub mod opts;
pub mod owned_exchange;
pub mod plan;
pub mod print;
pub mod random;
pub mod stdlib;
pub mod store;
pub mod strings;
pub mod symmetry;

pub use compile::{Handoff, Role, compile};
pub use eval::Machine;
pub use generate::Sink;
pub use handle::Handle;
pub use heap::Heap;
pub use opts::Opts;
pub use plan::Plan;
pub use store::{Semantics, Store};

/// What kind of failure an [`Error`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A construct or size the core does not implement: never a verdict.
    Unsupported,
    /// An evaluation error TLC would also raise, with its TLC class
    /// ([`Error::tlc`]). The engine decides the reported site class.
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
    /// TLC's class of an [`ErrorKind::Eval`] error, when the core knows it.
    /// `None` is an unclassified error, which the engine must not report
    /// as a verdict: it names it as unsupported instead.
    pub tlc: Option<TlcClass>,
}

/// How TLC raises an evaluation error: its `EC` code and whether it is an
/// `EvalException` (which a Java override passes through, and which the
/// checker reports with its own code) or an `Assert.fail`.
/// The site rules stay with the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TlcClass {
    /// The `tlc2.output.EC` code: 1000 for `GENERAL`.
    pub code: u32,
    /// Whether TLC raises it as an `EvalException`.
    pub eval_exception: bool,
}

impl TlcClass {
    /// `Assert.fail(EC.GENERAL, ...)`, TLC's most common evaluation error.
    pub const GENERAL: Self = Self {
        code: 1000,
        eval_exception: false,
    };

    /// An `EvalException` with `code`.
    #[must_use]
    pub const fn eval_exception(code: u32) -> Self {
        Self {
            code,
            eval_exception: true,
        }
    }
}

impl Error {
    fn new(kind: ErrorKind, message: String) -> Self {
        Self(Box::new(ErrorData {
            kind,
            message,
            tlc: None,
        }))
    }

    /// An unsupported construct.
    #[must_use]
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unsupported, what.into())
    }

    /// An unclassified evaluation error: the core has not established that
    /// TLC raises it, or with which class.
    #[must_use]
    pub fn eval(what: impl Into<String>) -> Self {
        Self::new(ErrorKind::Eval, what.into())
    }

    /// An evaluation error of TLC class `tlc`.
    ///
    /// ```
    /// use dive_core::{Error, TlcClass};
    /// let e = Error::eval_class(TlcClass::eval_exception(2179), "The second argument of \\div is 0.");
    /// assert_eq!(e.tlc(), Some(TlcClass { code: 2179, eval_exception: true }));
    /// assert_eq!(Error::eval("x").tlc(), None);
    /// ```
    #[must_use]
    pub fn eval_class(tlc: TlcClass, what: impl Into<String>) -> Self {
        Self(Box::new(ErrorData {
            kind: ErrorKind::Eval,
            message: what.into(),
            tlc: Some(tlc),
        }))
    }

    /// TLC's class of an evaluation error the core has classified.
    #[must_use]
    pub fn tlc(&self) -> Option<TlcClass> {
        self.0.tlc
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
