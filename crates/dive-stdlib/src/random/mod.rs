//! TLC's randomness, bit-exact, and dive's replayable randomness (M5 work package
//! C5; `docs/tlc-differences.md` D-002 and D-003).
//!
//! # Layers
//!
//! - [`RandomSource`]: a source of random bits with Java's derived draws
//!   (`nextDouble`, `nextInt(bound)`, `nextLong`) and TLC's `nextPrime`. Any
//!   `FnMut(u32) -> i32` is one, so the evaluator can carry a source without naming
//!   this crate.
//! - [`JavaRandom`]: `java.util.Random`, bit-exact.
//! - [`Streams`]: where each stream starts, under [`RandomMode::Tlc`] (TLC's seeding)
//!   or [`RandomMode::Dive`] (replayable at any worker count).
//! - [`random_element`], [`random_subset`], [`random_set_of_subsets`]: the operators
//!   over [`dive_value::Value`]s, drawing exactly as TLC's Java overrides draw.
//!
//! # Using it from the evaluator and the checker
//!
//! The checker owns the streams: it asks [`Streams::startup`] for constant
//! definitions, `ASSUME`s and `Init`, and [`Streams::state`] once per expanded state
//! (TLC: `fingerprint(state) ^ seed`). The evaluator passes the current stream down to
//! the operator as a `&mut dyn FnMut(u32) -> i32`:
//!
//! ```
//! use dive_stdlib::random::{random_element, RandomMode, RandomSource, Streams};
//! use dive_value::{Ctx, Int, Value, print::print_value};
//!
//! let ctx = Ctx::tlc();
//! let streams = Streams::new(7, RandomMode::Tlc);
//! let mut stream = streams.startup();
//! // What dive-eval would carry in its context:
//! let mut bits = |n| stream.next_bits(n);
//! let s = Value::interval(Int::from(1i64), Int::from(1_000_000i64), ctx);
//! let v = random_element(&s, ctx, &mut bits)?;
//! // TLC -seed 7: `RandomElement(1..1000000)` is 730700 (probe P9).
//! assert_eq!(print_value(&v, ctx)?, "730700");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Under [`RandomMode::Tlc`], [`Streams::state`] needs TLC's own state fingerprint,
//! which depends on TLC's variable order (ADR 0014); under [`RandomMode::Dive`] any
//! fixed 64-bit state fingerprint gives a replayable run.
//!
//! Provenance: `docs/provenance.md`, section "Randomness".

mod lcg;
mod ops;
mod primes;
mod seeding;
mod source;

pub use ops::{
    RANDOM_ELEMENT_METHOD, RANDOM_SET_OF_SUBSETS_METHOD, RANDOM_SUBSET_METHOD, RandomError,
    random_element, random_set_of_subsets, random_subset,
};
pub use seeding::{DiveStream, RandomMode, Stream, Streams, fresh_seed, parse_seed};
pub use source::{JavaRandom, RandomSource};
