//! `dive-liveness`: Temporal properties, fairness, tableau construction and SCC analysis.
//!
//! [`Graph`] provides the validated, deterministic graph and iterative SCC
//! foundation for M6. [`search::find_lasso`] checks acceptance labels and
//! constructs a deterministic witness over a supplied product graph.
//! [`formula::compile`] provides a bounded boolean/always/eventually tableau
//! kernel over supplied atoms. [`plan::lower`] retains pure bound predicates
//! and scopes. Fairness expansion and TLC-exact formula order,
//! behavior-graph capture, model coordination and property attribution remain
//! planned. [`product::Product`] combines supplied behavior graphs/tableaux
//! and projects witnesses; this crate does not produce model verdicts.
#![forbid(unsafe_code)]

mod graph;

pub use graph::{Graph, GraphError};

pub mod search;

pub mod formula;

pub mod plan;
pub mod product;

pub mod analyze;
