//! `dive-liveness`: Temporal properties, fairness, tableau construction and SCC analysis.
//!
//! [`Graph`] provides the validated, deterministic graph and iterative SCC
//! foundation for M6. [`search::find_lasso`] checks acceptance labels and
//! constructs a deterministic witness over a supplied product graph.
//! [`formula::compile`] provides a bounded boolean/always/eventually tableau
//! kernel over supplied atoms. Bound-model lowering, TLC-exact formula order,
//! model integration, concrete-state projection and property attribution remain
//! planned; this crate does not produce model verdicts.
#![forbid(unsafe_code)]

mod graph;

pub use graph::{Graph, GraphError};

pub mod search;

pub mod formula;
