//! `dive-liveness`: Temporal properties, fairness, tableau construction and SCC analysis.
//!
//! [`Graph`] provides the validated, deterministic graph and iterative SCC
//! foundation for M6. [`search::find_lasso`] checks acceptance labels and
//! constructs a deterministic witness over a supplied product graph.
//! [`formula::compile`] provides a bounded boolean/always/eventually tableau
//! kernel over supplied atoms. [`plan::lower`] retains pure bound predicates
//! and scopes and expands WF/SF with checker-side enabledness. The bounded
//! final-only [`analyze::analyze_final`] coordinator captures model graphs and
//! finds candidates with their complete attributed property set. Independent
//! validation is required. Periodic checking at deterministic level boundaries
//! ([`periodic_backend`]) is pending. [`tlc_plan::classify`] reproduces TLC's
//! liveness formula refusals for `--compat=tlc`; dive does not emulate TLC's
//! tableau numbering or SCC and lasso choice.
#![forbid(unsafe_code)]

mod graph;

pub use graph::{Graph, GraphError};

pub mod search;

pub mod formula;

pub mod plan;
pub mod product;

pub mod analyze;

/// TLC's 2213/2214 liveness formula refusals, for `--compat=tlc`.
pub mod tlc_plan;

/// Growth-gated scheduling for checks over a growing graph.
pub mod periodic_backend;
