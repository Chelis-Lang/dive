//! `dive-stdlib`: Standard modules and TLC's Java overrides of them. Each operator references the Java implementation it matches.
//!
//! Status: partial. [`random`] holds TLC's randomness (`RandomElement` and the
//! `Randomization` module) and dive's replayable seeding. The other operators follow
//! in M5; see `docs/ROADMAP.md` and `docs/design/m5-plan.md`. Permitted internal
//! dependencies are listed in `xtask/src/layering.rs`.
#![forbid(unsafe_code)]

pub mod random;
