//! Native commit-boundary clock/controller execution, with no ordinary verdict.
//!
//! The producer owns the clock and real dispatch pause. Polling at ordered source
//! commits remains distinct from TLC's asynchronous progress thread/queue.

use crate::{periodic, periodic_backend, phase_execution, phase_graph};
use dive_check::{CheckOptions, Stats};
use dive_model::Model;

/// Internally sampled commit-boundary clock policy; not a caller clock hook.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Interval in milliseconds. Zero checks every source commit; negative
    /// intervals refuse before graph execution. TLC normally uses 3000.
    pub progress_interval: i32,
    /// Pinned runtime-ratio upper boundary, normally 0.2.
    pub ratio_limit: f64,
    /// Initial forced request; cleared by the actual controller only on success.
    pub force: bool,
    /// Actual backend growth and default/final policy.
    pub backend: periodic_backend::Policy,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            progress_interval: 3000,
            ratio_limit: 0.2,
            force: false,
            backend: periodic_backend::Policy::default(),
        }
    }
}
/// An actual internally sampled controller invocation under dispatch authority.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub(crate) source: u64,
    pub(crate) publication: dive_check::capture::SourceBarrier,
    pub(crate) stats: Stats,
    pub(crate) workers: usize,
    pub(crate) generation: u64,
    pub(crate) now: i64,
    pub(crate) ratio_before: f64,
    pub(crate) ratio_after: f64,
    pub(crate) force_before: bool,
    pub(crate) force_after: bool,
    pub(crate) outcome: Result<periodic::Outcome, String>,
    pub(crate) resume_requested: bool,
}
impl Invocation {
    /// Source-owned publication transaction at this invocation. Its pending
    /// frontier flag records the actual pre-append callback stage even when
    /// a hook/candidate stops the diagnostic before append can succeed.
    #[must_use]
    pub fn publication(&self) -> dive_check::capture::SourceBarrier {
        self.publication
    }
    /// Actual logical source identity; speculative jobs remain separate.
    #[must_use]
    pub fn source(&self) -> u64 {
        self.source
    }
    /// Actual logical commit counters.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Actual pool width whose dispatch authority was held.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.workers
    }
    /// Joined job generation under the real pool mutex.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Producer-sampled UNIX milliseconds for timer admission.
    #[must_use]
    pub fn time_millis(&self) -> i64 {
        self.now
    }
    /// Actual ratio before and after the controller; IEEE values are retained.
    #[must_use]
    pub fn runtime_ratio(&self) -> (f64, f64) {
        (self.ratio_before, self.ratio_after)
    }
    /// Forced request before and after the controller.
    #[must_use]
    pub fn force(&self) -> (bool, bool) {
        (self.force_before, self.force_after)
    }
    /// Whether the controller reached its resume hook. The actual mutex pause
    /// ends when the source callback returns; errors stop future dispatch.
    #[must_use]
    pub fn resume_requested(&self) -> bool {
        self.resume_requested
    }
    /// Actual controller result; diagnostic candidates remain unvalidated.
    #[must_use]
    pub fn outcome(&self) -> &Result<periodic::Outcome, String> {
        &self.outcome
    }
}
/// One actual final SCC/witness selection after successful safety EOF.
#[derive(Clone, Debug)]
pub struct FinalCheck {
    pub(crate) group: usize,
    pub(crate) pem: usize,
    pub(crate) selection: crate::tlc_witness::Selection,
}
impl FinalCheck {
    /// Original OOS/PEM indices in source order.
    #[must_use]
    pub fn order(&self) -> (usize, usize) {
        (self.group, self.pem)
    }
    /// Actual bounded final selection. A candidate requires independent replay.
    #[must_use]
    pub fn selection(&self) -> &crate::tlc_witness::Selection {
        &self.selection
    }
}
/// Actual request quiescence and returned-prefix publication under the pool mutex.
/// The receipt distinguishes the selected returned prefix, successful physical
/// frontier append and later unreturned outputs; it does not assert that every
/// submitted source has completed or that TLC's queue suspension was reproduced.
#[derive(Clone, Debug)]
pub struct ExpansionPause {
    pub(crate) receipt: dive_check::capture::ExpansionQuiescence,
    pub(crate) workers: usize,
    pub(crate) generation: u64,
}
impl ExpansionPause {
    /// Actual claims/completed output and committed/frontier-appended prefix.
    #[must_use]
    pub fn receipt(&self) -> &dive_check::capture::ExpansionQuiescence {
        &self.receipt
    }
    /// Actual joined pool width and generation held during service.
    #[must_use]
    pub fn pool(&self) -> (usize, u64) {
        (self.workers, self.generation)
    }
}
/// Closed native diagnostic run. No outcome reports ordinary Holds or violation.
#[derive(Clone, Debug)]
pub struct Run {
    pub(crate) backend: periodic_backend::Run,
    pub(crate) options: Options,
    pub(crate) expansion_pauses: Vec<ExpansionPause>,
    pub(crate) request_options: Option<dive_check::progress::Options>,
    pub(crate) requests_cancelled: bool,
    pub(crate) aborted_request: Option<dive_check::progress::Request>,
    pub(crate) invocations: Vec<Invocation>,
    pub(crate) final_checks: Vec<FinalCheck>,
    pub(crate) eof: Option<Stats>,
    pub(crate) hook_evaluations: Vec<crate::periodic_hook::Evaluation>,
    pub(crate) request_services: Vec<crate::periodic_service::Invocation>,
    pub(crate) service_evaluations: Vec<crate::periodic_service::Evaluation>,
}
impl Run {
    /// Request-owned services after actual prefix append. Empty services retain
    /// no invented source; these never establish ordinary temporal verdicts.
    #[must_use]
    pub fn request_services(&self) -> &[crate::periodic_service::Invocation] {
        &self.request_services
    }
    /// Actual configured-hook calls in request service, separate from ordinal calls.
    #[must_use]
    pub fn service_evaluations(&self) -> &[crate::periodic_service::Evaluation] {
        &self.service_evaluations
    }
    /// Source-owned graph/partial-check evidence and actual deciding stop.
    #[must_use]
    pub fn backend(&self) -> &periodic_backend::Run {
        &self.backend
    }
    /// Actual internally sampled timer policy used by this run.
    #[must_use]
    pub fn options(&self) -> Options {
        self.options
    }
    /// Actual admitted timer invocations, retaining failed attempts.
    #[must_use]
    pub fn invocations(&self) -> &[Invocation] {
        &self.invocations
    }
    /// Actual final checks, absent unless safety execution genuinely reached EOF.
    #[must_use]
    pub fn final_checks(&self) -> &[FinalCheck] {
        &self.final_checks
    }
    /// Actual extra claim-gate policy; absent for the existing commit-only path.
    #[must_use]
    pub fn request_options(&self) -> Option<dive_check::progress::Options> {
        self.request_options
    }
    /// An actual expansion cut disabled requests while leaving earlier claims open.
    #[must_use]
    pub fn requests_cancelled(&self) -> bool {
        self.requests_cancelled
    }
    /// The one pending request cancelled at that cut, if any. This is a bounded
    /// stopping scalar, not a successful pause/hook/SCC or ordinary stop receipt.
    #[must_use]
    pub fn aborted_request(&self) -> Option<dive_check::progress::Request> {
        self.aborted_request
    }
    /// Producer-clock claim pauses retaining actual successful prefix publication.
    /// Per-source graph/controller callbacks precede physical frontier append;
    /// this receipt is retained after the selected transaction's append succeeds.
    /// Later unreturned outputs remain separate from the committed graph. An
    /// empty prefix is not an EOF or a model-completion certificate.
    #[must_use]
    pub fn expansion_pauses(&self) -> &[ExpansionPause] {
        &self.expansion_pauses
    }
    /// Actual supplied-expression calls. These assert no configured `_PERIODIC` identity.
    #[must_use]
    pub fn hook_evaluations(&self) -> &[crate::periodic_hook::Evaluation] {
        &self.hook_evaluations
    }
    /// Actual successful safety EOF, separate from any temporal result.
    #[must_use]
    pub fn safety_eof(&self) -> Option<Stats> {
        self.eof
    }
    /// This commit-boundary diagnostic does not certify ordinary scheduling.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}
/// Execute actual native pauses, internal clock/controller polling and EOF SCCs.
///
/// Polling happens only at logical source commits after fixed-wave jobs join.
/// It is not TLC's asynchronous queue schedule and grants no strict temporal
/// verdict. Existing graph/session bounds apply cumulatively. PERIODIC and
/// checkpoint transactions remain unsupported; no successful hooks replace them.
/// The supplied already-bound model may have executed startup effects before
/// this call; this function cannot undo them. Fresh facade admission is separate.
/// Do not evaluate this model or its evaluator-sharing clones concurrently with
/// execution; a bound evaluator carries mutable run state.
#[must_use]
pub fn execute(
    model: &Model,
    options: &CheckOptions,
    limits: phase_graph::Limits,
    policy: Options,
) -> Run {
    phase_execution::execute_periodic_lifecycle(model, options, limits, policy)
}

pub(crate) struct State {
    pub request_service: bool,
    pub initial_stats: Option<Stats>,
    pub graph_source: Option<u64>,
    pub request_services: Vec<crate::periodic_service::Invocation>,
    pub service_evaluations: Vec<crate::periodic_service::Evaluation>,
    pub options: Options,
    pub requests: Option<dive_check::progress::Options>,
    pub requests_cancelled: bool,
    pub aborted_request: Option<dive_check::progress::Request>,
    pub expansion_pauses: Vec<ExpansionPause>,
    pub start: i64,
    pub last: i64,
    pub ratio: f64,
    pub force: bool,
    pub invocations: Vec<Invocation>,
    pub final_checks: Vec<FinalCheck>,
    pub eof: Option<Stats>,
    pub hook: Option<(dive_model::Pred, usize)>,
    pub hook_evaluations: Vec<crate::periodic_hook::Evaluation>,
}
pub(crate) fn time_millis() -> Result<i64, phase_graph::Error> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| phase_graph::Error::Execution("native wall clock precedes UNIX epoch".into()))?
        .as_millis();
    i64::try_from(millis)
        .map_err(|_| phase_graph::Error::Execution("native wall clock millisecond range".into()))
}
impl State {
    pub fn new(options: Options) -> Result<Self, phase_graph::Error> {
        if options.progress_interval < 0 {
            return Err(phase_graph::Error::Input(
                "negative native progress interval".into(),
            ));
        }
        if options.backend.force {
            return Err(phase_graph::Error::Input(
                "use lifecycle force instead of repeated backend force".into(),
            ));
        }
        let start = time_millis()?;
        Ok(Self {
            request_service: false,
            initial_stats: None,
            graph_source: None,
            request_services: Vec::new(),
            service_evaluations: Vec::new(),
            options,
            requests: None,
            requests_cancelled: false,
            aborted_request: None,
            expansion_pauses: Vec::new(),
            start,
            last: start,
            ratio: 0.0,
            force: options.force,
            invocations: Vec::new(),
            final_checks: Vec::new(),
            eof: None,
            hook: None,
            hook_evaluations: Vec::new(),
        })
    }
}
