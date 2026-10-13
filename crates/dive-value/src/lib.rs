//! `dive-value`: TLA+ values with TLC's canonical ordering, equality, printing and
//! fingerprints; model values; lazy and enumerable sets.
//!
//! The crate has no internal dependencies (`xtask/src/layering.rs`). Its design is
//! recorded in ADR 0012 (`docs/decisions/0012-value-representation.md`).
//!
//! # Overview
//!
//! - [`Value`]: an immutable, 16-byte, `Send + Sync` value; compound values are shared
//!   behind `Arc`s, so cloning is cheap.
//! - [`Ctx`]: the semantics of a run: [`IntMode`] (TLC's 32-bit integers or
//!   arbitrary precision) and [`ValueOrder`] (TLC's intern order or dive's canonical
//!   order). Defaults are dive's; [`Ctx::tlc`] is `--compat=tlc`.
//! - [`Interner`]: the run's table of names (strings, model values, record fields).
//! - [`tlc`]: TLC's checked equality, comparison and membership, with TLC's errors.
//! - [`mod@print`]: TLC's value printing and pretty-printer.
//! - [`fp64`]: TLC's FP64 fingerprints.
//! - [`json`]: TLC's JSON form of values.
//! - [`codec`]: compact, deterministic byte encodings of values for state storage.
//!
//! ```
//! use dive_value::{Ctx, Interner, Value, print::print_value, tlc};
//!
//! let ctx = Ctx::tlc();
//! let names = Interner::new();
//! let s = Value::set(
//!     vec![Value::string(&names, "b"), Value::string(&names, "a")],
//!     ctx,
//! )?;
//! // TLC orders strings by intern order: "b" was interned first.
//! assert_eq!(print_value(&s, ctx)?, r#"{"b", "a"}"#);
//!
//! // TLC refuses to compare an integer with a string.
//! let err = tlc::equals(&Value::from(1), &Value::string(&names, "a"), ctx).unwrap_err();
//! assert_eq!(err.message(), "Attempted to check equality of integer 1 with non-integer:\n\"a\"");
//! # Ok::<(), dive_value::ValueError>(())
//! ```
#![forbid(unsafe_code)]

mod atom;
pub mod codec;
mod ctx;
mod depth;
mod enumerate;
mod error;
pub mod fp64;
mod int;
pub mod json;
mod names;
pub mod ops;
mod order;
mod pprint;
pub mod print;
mod stream;
pub mod tlc;
mod value;

pub use atom::{Atom, Interner};
pub use ctx::{Ctx, IntMode, ValueOrder};
pub use depth::MAX_DEPTH;
pub use enumerate::MAX_ENUMERATION;
pub use error::{ErrorKind, Result, ValueError};
pub use int::Int;
pub use names::NameError;
pub use value::{DeferredKind, DeferredSet, Fcn, FcnKind, Set, SetKind, Value};
