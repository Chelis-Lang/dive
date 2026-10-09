//! `dive-liveness`: Temporal properties, fairness, tableau construction and SCC analysis.
//!
//! [`Graph`] provides the validated, deterministic graph and iterative SCC
//! foundation for M6. [`search::find_lasso`] checks acceptance labels and
//! constructs a deterministic witness over a supplied product graph.
//! Formula planning, model integration, concrete-state projection and property
//! attribution are still planned; this crate does not produce model verdicts.
#![forbid(unsafe_code)]

mod graph;

pub use graph::{Graph, GraphError};

pub mod search;
