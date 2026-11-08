//! `dive-liveness`: Temporal properties, fairness, tableau construction and SCC analysis.
//!
//! [`Graph`] provides the validated, deterministic graph and iterative SCC
//! foundation for M6. [`search::find_lasso`] checks acceptance labels and
//! constructs a deterministic witness over a supplied product graph.
//! [`formula::compile`] provides a bounded boolean/always/eventually tableau
//! kernel over supplied atoms. [`plan::lower`] retains pure bound predicates
//! and scopes and expands WF/SF with checker-side enabledness.
//! [`analyze::periodic::external::GraphCheck`] checks a producer's behavior
//! graph at committed level boundaries, on the deterministic
//! [`analyze::periodic::Schedule`], and once it is complete, and finds
//! candidates with their complete attributed property set. Independent
//! validation is required. [`tlc_plan::classify`] reproduces TLC's
//! liveness formula refusals for `--compat=tlc`; dive does not emulate TLC's
//! tableau numbering or SCC and lasso choice.
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
