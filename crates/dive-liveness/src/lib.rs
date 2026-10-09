//! `dive-liveness`: Temporal properties, fairness, tableau construction and SCC analysis.
//!
//! [`Graph`] provides the validated, deterministic graph and iterative SCC
//! foundation for M6. Formula planning, accepting-component tests and model
//! checking are still planned; this crate does not produce model verdicts.
#![forbid(unsafe_code)]

mod graph;

pub use graph::{Graph, GraphError};
