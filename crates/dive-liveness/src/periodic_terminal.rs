//! Source-owned exhausted-queue service before timer/pool join (ADR 0076).
//!
//! Actual frontier exhaustion differs from a joined dispatch pause. TLC's
//! finished queue refuses suspension; its terminal controller then skips the
//! configured hook and partial check. Final selection still requires independent
//! replay and does not establish ordinary scheduling or temporal absence.

use dive_check::{Stats, progress::Request};

/// Privately produced terminal controller invocation; not a supplied EOF fact.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub(crate) stats: Stats,
    pub(crate) initial: Stats,
    pub(crate) graph_source: Option<u64>,
    pub(crate) workers: usize,
    pub(crate) generation: u64,
    pub(crate) pending: Option<Request>,
    pub(crate) now: i64,
    pub(crate) ratio: (f64, f64),
    pub(crate) force: (bool, bool),
    pub(crate) suspension_attempted: bool,
    pub(crate) joined: bool,
    pub(crate) outcome: Result<crate::periodic::Outcome, String>,
}
impl Invocation {
    /// Actual successful exhausted-frontier counters after physical append.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Actual successful initialization, never synthesized from an empty graph.
    #[must_use]
    pub fn initial_stats(&self) -> Stats {
        self.initial
    }
    /// Genuine last graph/source publication, absent when no source was expanded.
    #[must_use]
    pub fn graph_source(&self) -> Option<u64> {
        self.graph_source
    }
    /// Actual pool width and last joined-job generation, before its close/join.
    #[must_use]
    pub fn pool(&self) -> (usize, u64) {
        (self.workers, self.generation)
    }
    /// Actual unserviced request removed while closing the producer lane.
    /// EOF never supplies an invented timed request or source range.
    #[must_use]
    pub fn pending_request(&self) -> Option<Request> {
        self.pending
    }
    /// Producer-sampled terminal time; no caller clock establishes authority.
    #[must_use]
    pub fn time_millis(&self) -> i64 {
        self.now
    }
    /// Original IEEE runtime ratio before/after the actual controller.
    #[must_use]
    pub fn runtime_ratio(&self) -> (f64, f64) {
        self.ratio
    }
    /// Liveness force is not cleared by a finished queue's failed suspension.
    #[must_use]
    pub fn force(&self) -> (bool, bool) {
        self.force
    }
    /// Whether the controller reached suspension; a finished queue returns false.
    #[must_use]
    pub fn suspension_attempted(&self) -> bool {
        self.suspension_attempted
    }
    /// Actual final callback confirmed timer/pool joins after successful service.
    /// False on a terminal service error; it does not assert native threads leaked.
    #[must_use]
    pub fn joins_confirmed(&self) -> bool {
        self.joined
    }
    /// Controller completion is not temporal Holds or a model proof.
    #[must_use]
    pub fn outcome(&self) -> &Result<crate::periodic::Outcome, String> {
        &self.outcome
    }
    /// No invocation certifies ordinary TLC clock/queue/stop-count equivalence.
    #[must_use]
    pub const fn model_proof(&self) -> bool {
        false
    }
}
