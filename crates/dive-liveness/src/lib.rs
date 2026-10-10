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
//! validation is required. TLC-exact planning/selection, parallel graph capture,
//! periodic scheduling and CLI verdict integration remain pending.
#![forbid(unsafe_code)]

mod graph;

pub use graph::{Graph, GraphError};

pub mod search;

pub mod formula;

pub mod plan;
pub mod product;

pub mod analyze;

/// Bounded projections of TLC normalization and ordered liveness grouping.
pub mod tlc_plan;
