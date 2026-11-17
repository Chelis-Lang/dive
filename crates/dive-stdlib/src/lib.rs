//! `dive-stdlib`: Standard modules and TLC's Java overrides of them. Each operator references the Java implementation it matches.
//!
//! Status: partial. [`random`] holds TLC's randomness (`RandomElement` and the
//! `Randomization` module) and dive's replayable seeding; [`host`] is the
//! model checker's [`dive_eval::Host`], which wires that randomness, the level,
//! the live counters and `TLCGet("config")` into evaluation. The value-level
//! overrides (the standard modules, `Bags` and the CommunityModules operators)
//! are in `dive-eval`, which needs their evaluation context.
#![forbid(unsafe_code)]

pub mod inputs;

pub mod host;
pub mod random;
