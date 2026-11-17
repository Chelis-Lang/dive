//! `dive-liveness`: temporal properties, fairness, tableau construction and
//! SCC analysis for the model checker.
//!
//! The pieces, in the order a temporal check uses them:
//!
//! - [`plan::lower`] turns the configured temporal properties into a plan: pure
//!   bound state and action predicates, their scopes, and `WF`/`SF` fairness
//!   expanded with the checker's `ENABLED`.
//! - [`formula::compile`] builds a bounded tableau for the boolean, always and
//!   eventually structure over those atoms.
//! - [`Graph`] is the validated, deterministic behavior graph with an iterative
//!   SCC decomposition.
//! - The product of the graph and the tableau is explored on the fly
//!   ([`product`]), and a lasso search ([`search`]) checks its acceptance
//!   labels and builds a deterministic counterexample.
//! - [`analyze::periodic::external::GraphCheck`] checks a producer's graph at
//!   completed level boundaries, on the deterministic
//!   [`analyze::periodic::Schedule`], and once more when it is complete,
//!   reporting each candidate with its complete set of violated properties.
//!
//! Every counterexample still has to pass the independent validator in
//! `dive-trace`. [`tlc_plan::classify`] reproduces TLC's refusals of
//! liveness formulas for `--compat=tlc`. dive does not emulate TLC's tableau
//! numbering or its choice of SCC and lasso: it matches TLC's verdicts and
//! reports a counterexample that validates.
#![forbid(unsafe_code)]

mod budget;
mod graph;
#[cfg(test)]
mod test_capture;

pub use graph::{Graph, GraphError};

pub mod search;

pub mod formula;

pub mod plan;
pub mod product;

pub mod analyze;

/// TLC's 2213/2214 liveness formula refusals, for `--compat=tlc`.
pub mod tlc_plan;
