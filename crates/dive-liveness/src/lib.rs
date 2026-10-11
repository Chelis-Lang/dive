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

/// Bounded pinned TLC particle-tableau and promise projections.
pub mod tlc_tableau;

/// Bounded concrete successor receipts and ordered diagnostic phase-L checks.
pub mod phase_l;

/// Bounded concrete worker capture and actual null-tableau graph commits.
pub mod phase_graph;

/// Actual ordered safety/source callbacks and multiple OOS/tableau graph execution.
pub mod phase_execution;

/// Bounded pinned final SCC visitation over supplied product labels.
pub mod tlc_scc;
/// Pinned bounded product witness selection; no concrete validation claim.
pub mod tlc_witness;

/// Constructed-model final-only pinned SCC/lasso analysis; candidates require validation.
pub mod tlc_final;

/// Source-owned pointer growth and explicit partial-check stopping receipts.
pub mod periodic_backend;

/// Pinned controller/ratio primitives; native lifecycle integration remains separate.
pub mod periodic;

pub mod periodic_hook;
/// Native producer-owned commit-boundary clock/pool diagnostics.
pub mod periodic_lifecycle;
pub mod periodic_snapshot;
