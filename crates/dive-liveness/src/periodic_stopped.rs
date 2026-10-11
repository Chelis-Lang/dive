//! Source-owned stopped-worker service before joins (ADR 0078).
//!
//! A stopped queue is not successful EOF. The original deciding safety/error
//! result remains authoritative; diagnostic failures are separate. Native graph
//! publication is only its committed prefix, not TLC's possible after-error
//! graph writes, and no receipt establishes ordinary scheduling or model proof.
use dive_check::{Stats, capture::WorkerStop, progress::Request};
/// Privately produced controller receipt for an actual ordered worker error.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub(crate) kind: WorkerStop,
    pub(crate) code: u32,
    pub(crate) source: u64,
    pub(crate) stats: Stats,
    pub(crate) initial: Stats,
    pub(crate) graph_source: Option<u64>,
    pub(crate) workers: usize,
    pub(crate) generation: u64,
    pub(crate) pending: Option<Request>,
    pub(crate) now: i64,
    pub(crate) ratio: (f64, f64),
    pub(crate) force: (bool, bool),
    pub(crate) suspended: bool,
    pub(crate) joined: Option<Stats>,
    pub(crate) outcome: Result<crate::periodic::Outcome, String>,
}
impl Invocation {
    /// Original eligible error family, never a successful temporal verdict.
    #[must_use]
    pub fn kind(&self) -> WorkerStop {
        self.kind
    }
    /// Original deciding TLC classification, separate from observer/service errors.
    #[must_use]
    pub fn code(&self) -> u32 {
        self.code
    }
    /// Actual discovery/source identity from the original predecessor path.
    #[must_use]
    pub fn source(&self) -> u64 {
        self.source
    }
    /// Actual prejoin logical source-stop counters; queue size need not be zero.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Successful original Init counters; failed Init cannot create this receipt.
    #[must_use]
    pub fn initial_stats(&self) -> Stats {
        self.initial
    }
    /// Last actually published graph source; the failing source need not be written.
    #[must_use]
    pub fn graph_source(&self) -> Option<u64> {
        self.graph_source
    }
    /// Actual live paused pool width/generation before its real closure.
    #[must_use]
    pub fn pool(&self) -> (usize, u64) {
        (self.workers, self.generation)
    }
    /// Genuine unserviced pending request removed while closing publication.
    #[must_use]
    pub fn pending_request(&self) -> Option<Request> {
        self.pending
    }
    /// Internally sampled wall-clock value, not supplied clock authority.
    #[must_use]
    pub fn time_millis(&self) -> i64 {
        self.now
    }
    /// Original IEEE runtime fraction before/after service.
    #[must_use]
    pub fn runtime_ratio(&self) -> (f64, f64) {
        self.ratio
    }
    /// Force flag before/after; failed finished suspension does not clear it.
    #[must_use]
    pub fn force(&self) -> (bool, bool) {
        self.force
    }
    /// Whether controller attempted suspension; a stopped queue cannot resume.
    #[must_use]
    pub fn suspension_attempted(&self) -> bool {
        self.suspended
    }
    /// Real postjoin result counters, potentially including ordinary error replay.
    /// None cannot establish a thread leak, EOF or completed exploration.
    #[must_use]
    pub fn joined_stats(&self) -> Option<Stats> {
        self.joined
    }
    /// Diagnostic controller outcome, never an alternate deciding model result.
    #[must_use]
    pub fn outcome(&self) -> &Result<crate::periodic::Outcome, String> {
        &self.outcome
    }
    /// No ordinary queue/graph/count/clock or whole-model certificate is produced.
    #[must_use]
    pub const fn model_proof(&self) -> bool {
        false
    }
}
