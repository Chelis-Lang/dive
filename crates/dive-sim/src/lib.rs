//! The seeded finite safety simulation contract (ADR 0020): its options,
//! report, static admission and the run host that owns one simulation.
//!
//! Core's finite simulation is the backend (`dive-api`), run through
//! [`Runtime::simulate_with_backend`]. A finished sample is explicitly not a
//! model-checking proof. Temporal checking, unsupported runtime effects, and
//! unsupported options refuse by name. The independent trace validator remains
//! outside this crate.
#![forbid(unsafe_code)]

mod statistics;

pub use statistics::{ActionFlow, ActionGroup};

use dive_eval::next::StateVals;
mod host;
use dive_model::Model;
use dive_stdlib::random::RandomMode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A finite simulation request. No unbounded trace count is accepted.
#[derive(Clone, Debug)]
pub struct Options {
    /// Run seed, always recorded by callers.
    pub seed: i64,
    /// TLC's `-aril`: skip this many doubles before deriving the worker seed.
    /// Currently only supported in TLC mode; zero is required in dive mode.
    pub aril: u64,
    /// Maximum number of transitions in each trace (TLC's `-depth`).
    pub depth: usize,
    /// Number of traces to sample, stopping on the first error.
    pub traces: u64,
    /// TLC's worker stream or dive's stable trace identities.
    pub mode: RandomMode,
    /// Maximum initial states or candidates retained for one action.
    pub max_candidates: usize,
    /// Maximum states retained across all trace documents.
    pub max_retained: usize,
    /// Maximum generated states, including discarded candidates.
    pub max_generated: u64,
    /// Collect chosen action flows, combining source locations as TLC `stats=basic`.
    pub action_stats: bool,
    /// Construct one admitted successor with randomized Next choices instead
    /// of retaining every successor of a selected action. Sampling is never proof.
    /// `true` is TLC `-generate num=N`; the default `false` is `-simulate num=N`.
    pub generation: bool,
    /// Maximum retained runtime Print/PrintT events (startup output is separate).
    pub max_output_events: usize,
    /// Maximum UTF-8 bytes retained in runtime Print/PrintT text. This does not
    /// bound temporary rendering allocations or total process memory.
    pub max_output_bytes: usize,
}

impl Options {
    /// Bounds with a finite, explicit number of traces and transitions.
    #[must_use]
    pub const fn new(seed: i64, mode: RandomMode, depth: usize, traces: u64) -> Self {
        Self {
            seed,
            aril: 0,
            depth,
            traces,
            mode,
            max_candidates: 1_000_000,
            max_retained: 1_000_000,
            max_generated: 10_000_000,
            action_stats: false,
            generation: false,
            max_output_events: 100_000,
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}

/// An independently checkable trace of concrete states.
#[derive(Clone, Debug)]
pub struct Trace {
    /// Stable identity, starting at zero.
    pub id: u64,
    /// Initial state and chosen transitions; the final state is included.
    pub states: Vec<StateVals>,
}

/// The result of a simulation; a sample never proves absence of violations.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Requested sampling completed without finding an error.
    Sampled,
    /// An invariant, action property or deadlock was found.
    Violation {
        /// `invariant`, `property`, or `deadlock`.
        kind: &'static str,
        /// Configured property name, if any.
        name: Option<String>,
        /// TLC's deciding message code.
        code: u32,
        /// Index into the report's traces.
        trace: usize,
    },
    /// An assumption or evaluated predicate failed, with a measured class.
    Error {
        /// TLC's deciding message code.
        code: u32,
        /// Diagnostic detail.
        message: String,
    },
    /// An unimplemented behavior; no simulation conclusion.
    Unsupported(String),
    /// A configured resource bound was reached; no conclusion.
    Limit(String),
    /// The caller's cancellation checkpoint stopped execution; no conclusion.
    /// The caller must separately establish its actual resource/stop reason.
    Cancelled,
    /// An internal error; no conclusion.
    Internal(String),
}

/// A deciding violation observed without retaining its full bounded trace.
/// This is evidence for diagnosis, not independently certified violation data.
#[derive(Clone, Debug)]
pub struct PendingViolation {
    /// Invariant or action property kind.
    pub kind: &'static str,
    /// Configured property name.
    pub name: Option<String>,
    /// TLC's deciding message code.
    pub code: u32,
}

/// A finite simulation result and complete sampled trace documents.
#[derive(Clone, Debug)]
pub struct Report {
    /// Last reached phase; preflight refusals perform no model evaluation.
    pub stage: &'static str,
    /// Result, explicitly distinguishing samples from proofs.
    pub outcome: Outcome,
    /// Generated initial states plus all complete successor candidates checked.
    pub generated: u64,
    /// Initial candidates collected, including a prefix when enumeration fails.
    pub initial_candidates: u64,
    /// Observed violation whose complete trace could not be retained.
    pub pending_violation: Option<PendingViolation>,
    /// Traces started, including a trace that ends with a violation.
    pub traces_started: u64,
    /// Concrete trace data, retained subject to the request's bound.
    pub traces: Vec<Trace>,
    /// Optional chosen-action counts; absent when disabled or preflight refused.
    pub action_flow: Option<ActionFlow>,
    /// Actual backend output, separate from binding and independent replay.
    /// Warning events preserve their existing distinction from Print events.
    pub runtime_output: Vec<dive_eval::capture::Captured>,
}

/// Owns one simulation's host from source binding through execution.
///
/// Install [`Self::host`] in `dive_model::BindOptions` before binding, then
/// execute with this same runtime. Startup and reached random operations share
/// one stream; the simulator never restarts a caller's partially consumed RNG.
/// A runtime admits one execution, including failed/cancelled executions.
/// Run execution on a thread with [`dive_eval::RECOMMENDED_STACK`].
///
/// ```
/// use dive_sim::{Options, Runtime};
/// use dive_stdlib::random::RandomMode;
/// let runtime = Runtime::new(Options::new(7, RandomMode::Dive, 100, 10));
/// let binding = dive_model::BindOptions { host: Some(runtime.host()), ..Default::default() };
/// assert!(binding.host.is_some());
/// // Bind the source with `binding`, then run a backend with
/// // `runtime.simulate_with_backend`.
/// ```
#[derive(Debug)]
pub struct Runtime {
    options: Options,
    host: Arc<host::SimulationHost>,
    used: AtomicBool,
}

impl Runtime {
    /// Creates the run context before source binding or startup evaluation.
    #[must_use]
    pub fn new(options: Options) -> Self {
        let host = host::SimulationHost::new(&options);
        host.binding(true);
        Self {
            host,
            options,
            used: AtomicBool::new(false),
        }
    }

    /// The owned host to install in the source binder.
    #[must_use]
    pub fn host(&self) -> Arc<dyn dive_eval::Host> {
        self.host.clone()
    }

    /// Supplies the actual bound configuration after startup binding finishes.
    /// Configuration remains unavailable during binding, as in ordinary TLC.
    pub fn set_config(&self, config: Option<dive_eval::RunConfig>) {
        self.host.set_config(config);
    }

    /// Runs an alternate finite backend under this same binding/runtime host.
    /// The callback receives the remaining retained-output allowance.
    #[must_use]
    pub fn simulate_with_backend(
        &self,
        model: &Model,
        driver: impl FnOnce(&dyn Fn() -> (usize, usize)) -> Report,
    ) -> Report {
        self.run(model, |host| driver(&|| host.remaining_output()))
    }

    fn run(&self, model: &Model, driver: impl FnOnce(&host::SimulationHost) -> Report) -> Report {
        if self.used.swap(true, Ordering::AcqRel) {
            return refused_runtime("simulation runtime was already used");
        }
        self.host.binding(false);
        with_host(model, &self.options, &self.host, driver)
    }
}

fn refused_runtime(reason: &str) -> Report {
    Report {
        stage: "preflight",
        outcome: Outcome::Unsupported(reason.to_owned()),
        generated: 0,
        initial_candidates: 0,
        pending_violation: None,
        traces_started: 0,
        traces: Vec::new(),
        action_flow: None,
        runtime_output: Vec::new(),
    }
}

fn with_host(
    model: &Model,
    opts: &Options,
    host: &Arc<host::SimulationHost>,
    driver: impl FnOnce(&host::SimulationHost) -> Report,
) -> Report {
    if let Some(outcome) = preflight(model, opts) {
        return Report {
            stage: "preflight",
            outcome,
            generated: 0,
            initial_candidates: 0,
            pending_violation: None,
            traces_started: 0,
            traces: Vec::new(),
            action_flow: None,
            runtime_output: Vec::new(),
        };
    }
    // Declared before HostRestore so reverse drop order restores the original
    // host before releasing shared-core simulation ownership, on every return.
    let Some(_lease) = model.evaluator.try_simulation_lease() else {
        return Report {
            stage: "preflight",
            outcome: Outcome::Unsupported(
                "concurrent or reentrant simulation on the same evaluator".to_owned(),
            ),
            generated: 0,
            initial_candidates: 0,
            pending_violation: None,
            traces_started: 0,
            traces: Vec::new(),
            action_flow: None,
            runtime_output: Vec::new(),
        };
    };
    let _restore = host::HostRestore::install(&model.evaluator, host);
    let capture = dive_eval::capture::PrintCapture::start();
    let mut report = driver(host);
    let mut output = model.evaluator.filter_captured_warnings(capture.take());
    output.append(&mut report.runtime_output);
    report.runtime_output = output;
    if let Some(reason) = host.limit() {
        report.outcome = Outcome::Limit(reason.to_owned());
    } else if let Some(reason) = host.denied() {
        report.outcome = Outcome::Unsupported(reason);
    }
    report
}

/// Checks finite-mode options and statically unsupported combinations without evaluation.
#[must_use]
pub fn preflight(model: &Model, opts: &Options) -> Option<Outcome> {
    let refusal = |s: &str| Some(Outcome::Unsupported(s.to_owned()));
    if opts.traces == 0 {
        return refusal("simulation requires a positive finite trace count");
    }
    if opts.aril > 1_000_000 {
        return Some(Outcome::Limit("simulation aril skip bound".to_owned()));
    }
    if opts.mode == RandomMode::Dive && opts.aril != 0 {
        return refusal("nonzero aril outside TLC simulation mode");
    }
    if model.runtime_name_order {
        return refusal(
            "simulation runtime string encounter order requires independent namespace provenance",
        );
    }
    // A temporal property's binding refusal names the liveness backend, which
    // simulation never uses; the simulation-specific refusal below names it.
    if let Some(u) = model
        .unsupported
        .iter()
        .find(|u| u.kind != dive_model::UnsupportedKind::TemporalProperty)
    {
        return refusal(&u.feature);
    }
    // Fairness (`model.temporals`) constrains only infinite behaviors and is
    // inert for the finite safety prefixes simulation checks (ADR 0116).
    // Simulation does not check temporal properties on its traces (deferred
    // by owner ruling, 2026-11-06).
    if !model.implied_temporals.is_empty() {
        let names: Vec<&str> = model
            .implied_temporals
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        return refusal(&format!(
            "simulation temporal properties: {}",
            names.join(", ")
        ));
    }
    if !model.implied_inits.is_empty() {
        return refusal(
            "simulation implied initial properties require a separate validation contract",
        );
    }
    if !model.implied_actions.is_empty()
        && (!model.constraints.is_empty() || !model.action_constraints.is_empty())
    {
        return refusal(
            "simulation implied action properties with state/action constraints require a separate validation contract",
        );
    }
    if model.symmetry.is_some() {
        return refusal("simulation SYMMETRY declarations");
    }
    if model.view.is_some() {
        return refusal("simulation VIEW declarations");
    }
    // ALIAS only projects printed traces; as in checking, the report shows
    // the states themselves and says so (`dive_api::simulate`). A failed
    // lookup is TLC's error when it prints a trace, which is not reproduced.
    if matches!(model.alias, Some(Err(_))) {
        return refusal("simulation ALIAS whose definition lookup fails");
    }
    if !matches!(&model.postconditions, Ok(p) if p.is_empty()) {
        return refusal("simulation POSTCONDITION");
    }
    if model.actions.len() > i32::MAX as usize {
        return Some(Outcome::Limit(
            "simulation action count exceeds TLC integer indexing".to_owned(),
        ));
    }
    None
}
