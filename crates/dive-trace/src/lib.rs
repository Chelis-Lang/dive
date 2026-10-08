//! `dive-trace`: The trace format, the independent trace validator, and the API for checking implementation traces against a spec. It must not share evaluation shortcuts with the checker beyond the core evaluator, so it stays a genuine second check.
//!
//! Status: placeholder. See `docs/ROADMAP.md` for when this crate is built
//! and `docs/CHARTER.md` for its responsibilities. Permitted internal
//! dependencies are listed in `xtask/src/layering.rs`.
#![forbid(unsafe_code)]
