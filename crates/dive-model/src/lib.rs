//! `dive-model`: The binding stage between the IR and every backend. It ports TLC's `SpecProcessor`: it binds the configuration's constants, model values and `<-` overrides, precomputes constant definitions, extracts the initial predicates, next-state actions and every other role, and reports configuration errors with TLC's codes (ADR 0013 §8). The checker, liveness checker, simulator, trace validator, compiler and API share it, so the binding is ported once.
//!
//! Status: placeholder. See `docs/ROADMAP.md` for when this crate is built
//! and `docs/CHARTER.md` for its responsibilities. Permitted internal
//! dependencies are listed in `xtask/src/layering.rs`. It must never need
//! `dive-eval`'s `enumerate` feature (ADR 0016 §2).
#![forbid(unsafe_code)]
