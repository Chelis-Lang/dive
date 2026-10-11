//! Request-owned diagnostic service after real contiguous-prefix publication.
//!
//! Source callbacks and physical frontier append precede this service. Empty
//! requests inspect only genuine existing registrations; neither an empty range
//! nor a successful partial check establishes EOF or a model proof.

use dive_check::{Stats, progress::Request};
use dive_eval::EvalError;
use dive_ir::ExprId;

/// Privately produced controller service under a real borrowed dispatch pause.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub(crate) request: Request,
    pub(crate) submitted: std::ops::Range<u64>,
    pub(crate) committed: std::ops::Range<u64>,
    pub(crate) graph_source: Option<u64>,
    pub(crate) initial: Stats,
    pub(crate) stats: Stats,
    pub(crate) workers: usize,
    pub(crate) generation: u64,
    pub(crate) predicates: usize,
    pub(crate) now: i64,
    pub(crate) ratio: (f64, f64),
    pub(crate) force: (bool, bool),
    pub(crate) resumed: bool,
    pub(crate) outcome: Result<crate::periodic::Outcome, String>,
}
impl Invocation {
    /// Actual producer-clock or initial claim-lane request, not a caller token.
    #[must_use]
    pub fn request(&self) -> Request {
        self.request
    }
    /// Full submitted source range; completion/publication is separately recorded.
    #[must_use]
    pub fn submitted(&self) -> std::ops::Range<u64> {
        self.submitted.clone()
    }
    /// Actual newly committed and successfully frontier-appended source range.
    #[must_use]
    pub fn committed(&self) -> std::ops::Range<u64> {
        self.committed.clone()
    }
    /// Last actually published graph source; absent before any source publication.
    #[must_use]
    pub fn graph_source(&self) -> Option<u64> {
        self.graph_source
    }
    /// Actual successful initialization counters, never a supplied empty state.
    #[must_use]
    pub fn initial_stats(&self) -> Stats {
        self.initial
    }
    /// Actual counters after this request's completed prefix and physical append.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Real paused pool width and joined generation.
    #[must_use]
    pub fn pool(&self) -> (usize, u64) {
        (self.workers, self.generation)
    }
    /// Inline invariant width, distinct from the expansion pool width.
    #[must_use]
    pub fn predicate_workers(&self) -> usize {
        self.predicates
    }
    /// Actual producer-sampled service time, separate from request publication.
    #[must_use]
    pub fn time_millis(&self) -> i64 {
        self.now
    }
    /// Runtime ratio before and after the actual controller.
    #[must_use]
    pub fn runtime_ratio(&self) -> (f64, f64) {
        self.ratio
    }
    /// Liveness force before and after service; separate from an initial request.
    #[must_use]
    pub fn force(&self) -> (bool, bool) {
        self.force
    }
    /// Whether the controller reached resume; actual dispatch resumes on return.
    #[must_use]
    pub fn resume_requested(&self) -> bool {
        self.resumed
    }
    /// Controller result; a candidate still requires fresh independent replay.
    #[must_use]
    pub fn outcome(&self) -> &Result<crate::periodic::Outcome, String> {
        &self.outcome
    }
    /// Partial service never certifies model absence or ordinary scheduling.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}

/// Actual configured-hook evaluation in a request-owned service.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub(crate) request: Request,
    pub(crate) graph_source: Option<u64>,
    pub(crate) expression: ExprId,
    pub(crate) result: Result<bool, EvalError>,
}
impl Evaluation {
    /// Actual request whose dispatch pause was held.
    #[must_use]
    pub fn request(&self) -> Request {
        self.request
    }
    /// Genuine last graph source, including `None` for initial empty service.
    #[must_use]
    pub fn graph_source(&self) -> Option<u64> {
        self.graph_source
    }
    /// Retained configured-hook body, not a caller replacement predicate.
    #[must_use]
    pub fn expression(&self) -> ExprId {
        self.expression
    }
    /// Actual `FALSE.equals` result or original typed evaluation error.
    #[must_use]
    pub fn result(&self) -> &Result<bool, EvalError> {
        &self.result
    }
}
