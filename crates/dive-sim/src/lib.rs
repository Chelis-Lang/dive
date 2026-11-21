//! The seeded finite safety simulation contract: its options,
//! report, static admission and the run host that owns one simulation.
//!
//! Core's finite simulation is the backend (`dive-api`), run through
//! [`Runtime::simulate_with_backend`]. A finished sample is explicitly not a
//! model-checking proof. Temporal properties are checked on each trace's own
//! behaviors (ADR 0162 §4) and never reported as holding. Unsupported runtime
//! effects and unsupported options refuse by name. The independent trace validator remains
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

/// [`Options::traces`] for a run that samples until a violation, an error
/// or its budget stops it.
pub const UNLIMITED_TRACES: u64 = u64::MAX;

/// [`Options::keep_traces`] that keeps every trace.
pub const KEEP_ALL_TRACES: u64 = u64::MAX;

/// How simulation treats a set Init or Next would have to enumerate but
/// cannot because it is infinite (`Nat`, `Int`, `Seq(S)`, D-040).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleUnbounded {
    /// TLC's behavior: the run ends with GENERAL 1000, "not enumerable".
    Off,
    /// Draw one member per binding: `Nat` geometric with this mean, `Int` a
    /// geometric magnitude with a uniform sign, `Seq(S)` a geometric length
    /// with members drawn from `S`, tuples, records and functions over
    /// finite domains component by component.
    Geometric {
        /// The mean of the geometric distribution, from 1 to 65,536.
        mean: u64,
    },
}

/// The default mean of [`SampleUnbounded::Geometric`].
pub const DEFAULT_SAMPLE_MEAN: u64 = 8;

/// Times a step (or Init) whose draws from unbounded sets led to no admitted
/// state is drawn again before its trace ends, counted in
/// [`Report::traces_stalled`]. A step that drew is never a deadlock.
pub const SAMPLE_ATTEMPTS: u64 = 8;

/// A simulation request.
///
/// Trace `id` is sampled from its own stream, so its states, its generated
/// count and its verdict are a function of the seed and `id` alone, whatever
/// worker runs it. The run reports the lowest-id trace that decides it (a
/// violation, an error or a refusal), with the statistics of the traces
/// before it, so the report is the same at any worker count.
#[derive(Clone, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent simulation switches, each a separate documented option"
)]
pub struct Options {
    /// Run seed, always recorded by callers.
    pub seed: i64,
    /// TLC's `-aril`: skip this many doubles before deriving the worker seed.
    /// Currently only supported in TLC mode; zero is required in dive mode.
    pub aril: u64,
    /// Maximum number of transitions in each trace (TLC's `-depth`).
    pub depth: usize,
    /// Total number of traces to sample at any worker count, stopping on the
    /// first deciding trace; [`UNLIMITED_TRACES`] samples until a budget stops.
    pub traces: u64,
    /// The id of the first trace: ids `first_trace` to
    /// `first_trace + traces - 1` are sampled. A trace's states depend only
    /// on the seed and its id, so a range regenerates exactly those traces
    /// of a larger run (`--trace-ids`).
    pub first_trace: u64,
    /// Sampling threads. Traces are committed in id order, so the report
    /// does not depend on this.
    pub workers: usize,
    /// Traces kept in the report besides the deciding trace: the lowest ids.
    /// [`KEEP_ALL_TRACES`] keeps every trace.
    pub keep_traces: u64,
    /// TLC's worker stream or dive's stable trace identities.
    pub mode: RandomMode,
    /// Maximum initial states retained by Init enumeration. Unbounded by
    /// default: the run's memory budget governs.
    pub max_candidates: usize,
    /// Maximum states retained across the kept trace documents. Unbounded
    /// by default; [`Options::keep_traces`] bounds what is kept.
    pub max_retained: usize,
    /// Maximum generated states, including discarded candidates, counted in
    /// trace order: a trace that would cross it ends the run with the
    /// statistics of the traces before it. Unbounded by default.
    pub max_generated: u64,
    /// Collect chosen action flows, combining source locations as TLC `stats=basic`.
    pub action_stats: bool,
    /// Construct one admitted successor with randomized Next choices instead
    /// of retaining every successor of a selected action. Sampling is never proof.
    /// `true` is TLC `-generate num=N`; the default `false` is `-simulate num=N`.
    pub generation: bool,
    /// Each trace constructs its initial state with randomized Init choices
    /// instead of drawing one from Init enumerated once (opt-in, D-041). Init
    /// is never enumerated, so an initial state that violates an invariant is
    /// found only if a trace constructs it.
    pub constructive_init: bool,
    /// Sets Init or Next cannot enumerate because they are infinite: drawn
    /// from a stated distribution, or TLC's GENERAL 1000 error. Draws come
    /// from a stream keyed by the trace, step, action and attempt, so they
    /// are reproducible at any worker count. A step whose draws lead to no
    /// admitted state is drawn again a few times, then ends the trace
    /// without a deadlock claim ([`Report::traces_stalled`]).
    pub sample_unbounded: SampleUnbounded,
    /// Maximum retained runtime Print/PrintT events (startup output is separate).
    pub max_output_events: usize,
    /// Maximum UTF-8 bytes retained in runtime Print/PrintT text. This does not
    /// bound temporary rendering allocations or total process memory.
    pub max_output_bytes: usize,
    /// Extra steps a trace that reached the depth may take while a temporal
    /// property is still unchecked on it, stopping when it closes a loop or
    /// has no successor (`--liveness-extend`, D-039). Zero, the default here
    /// and under `--compat=tlc`, ends every trace at the depth.
    pub liveness_extend: usize,
    /// Work units one trace's temporal check may spend before the trace's
    /// undecided properties count as unchecked (`budget`); `None` is
    /// [`liveness_budget`]'s default for the trace's length. A budget stop is
    /// never a verdict.
    pub liveness_budget: Option<u64>,
    /// Refute a temporal property on a finite prefix that no extension can
    /// satisfy (`bad-prefix`), as TLC does only for `[]P` and `[][A]_v`
    /// parts. Off under `--compat=tlc`.
    pub bad_prefix: bool,
}

/// The default per-trace temporal work budget for a trace of `states`
/// states: at least a million units, and 256 per state.
///
/// ```
/// assert_eq!(dive_sim::liveness_budget(10), 1_000_000);
/// assert_eq!(dive_sim::liveness_budget(10_000), 2_560_000);
/// ```
#[must_use]
pub const fn liveness_budget(states: usize) -> u64 {
    let scaled = (states as u64).saturating_mul(256);
    if scaled > 1_000_000 {
        scaled
    } else {
        1_000_000
    }
}

impl Options {
    /// One past the last trace id sampled: `first_trace + traces`, or
    /// `u64::MAX` for [`UNLIMITED_TRACES`].
    #[must_use]
    pub const fn end_trace(&self) -> u64 {
        if self.traces == UNLIMITED_TRACES {
            u64::MAX
        } else {
            self.first_trace.saturating_add(self.traces)
        }
    }

    /// One worker, the default retention of 10 traces and no state bounds:
    /// the caller's time and memory budgets govern.
    #[must_use]
    pub const fn new(seed: i64, mode: RandomMode, depth: usize, traces: u64) -> Self {
        Self {
            seed,
            aril: 0,
            depth,
            traces,
            first_trace: 0,
            workers: 1,
            keep_traces: 10,
            mode,
            max_candidates: usize::MAX,
            max_retained: usize::MAX,
            max_generated: u64::MAX,
            action_stats: false,
            generation: false,
            constructive_init: false,
            // The default profile samples unbounded sets (owner ruling,
            // 2026-11-20); TLC's seeding keeps TLC's error.
            sample_unbounded: match mode {
                RandomMode::Dive => SampleUnbounded::Geometric {
                    mean: DEFAULT_SAMPLE_MEAN,
                },
                RandomMode::Tlc => SampleUnbounded::Off,
            },
            max_output_events: 100_000,
            max_output_bytes: 16 * 1024 * 1024,
            liveness_extend: 0,
            liveness_budget: None,
            bad_prefix: false,
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
    /// For each step, the index into the model's `actions` of the action
    /// that produced it: `actions[k]` leads from `states[k]` to
    /// `states[k + 1]`. Shorter than the steps only when a trace ends at a
    /// violating candidate, whose action is the last entry.
    pub actions: Vec<usize>,
    /// States this trace generated: its successor candidates, and under
    /// constructive Init its initial candidates.
    pub generated: u64,
    /// Members drawn from unbounded sets for the existentials of the step
    /// into a state, as (state index, members), for the states that have
    /// any. The validator uses them only to prove those existentials.
    pub witnesses: Vec<(usize, Vec<dive_value::Value>)>,
    /// For a temporal violation, how the word in `states` continues forever
    /// and the configured properties false on it, in configuration order.
    /// `None` for a finite sampled trace.
    pub temporal: Option<(TemporalEnding, Vec<String>)>,
}

/// How a temporal counterexample word continues after its last state.
/// Every loop closes on exactly equal states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemporalEnding {
    /// The last state's successor is `states[back_to]`.
    Lasso {
        /// Zero-based index of the state the loop returns to.
        back_to: usize,
    },
    /// The behavior stutters forever at the last state.
    Stuttering,
    /// No infinite extension of the states satisfies the named property.
    BadPrefix(String),
}

/// How one committed trace ended (ADR 0162 §4.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceClasses {
    /// A sampled step reached a state already in the trace.
    pub closed: u64,
    /// The last state has no admitted successor.
    pub terminal: u64,
    /// The depth was reached without closing or terminating.
    pub truncated: u64,
    /// Closed only during `--liveness-extend` steps.
    pub extended_closed: u64,
    /// Terminated only during `--liveness-extend` steps.
    pub extended_terminal: u64,
    /// A violation, deadlock or error ended the trace.
    pub violated: u64,
    /// The run's budget or the caller stopped the trace.
    pub cancelled: u64,
}

/// Why a temporal property was not checked on a trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unchecked {
    /// A trace cut by the depth whose behaviors are all unfair.
    Truncated,
    /// A closed or terminal trace none of whose behaviors is fair.
    NoFairBehavior,
    /// The per-trace temporal budget ran out.
    Budget,
}

impl Unchecked {
    /// The report's reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Truncated => "truncated",
            Self::NoFairBehavior => "no-fair-behavior",
            Self::Budget => "budget",
        }
    }
}

/// How many committed traces checked one configured temporal property.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyCoverage {
    /// Configured `PROPERTY` name.
    pub name: String,
    /// Traces whose behaviors include a fair one, none violating the property.
    pub checked: u64,
    /// Traces that could not decide the property, by reason.
    pub unchecked: std::collections::BTreeMap<Unchecked, u64>,
    /// Id of the trace that refuted the property, if one did.
    pub refuted_by_trace: Option<u64>,
}

/// Temporal-property coverage of the committed traces, present when the
/// model has temporal properties.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemporalCoverage {
    /// Trace classes, counted for every committed trace.
    pub classes: TraceClasses,
    /// One entry per configured property, in configuration order.
    pub properties: Vec<PropertyCoverage>,
}

/// The result of a simulation; a sample never proves absence of violations.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Requested sampling completed without finding an error.
    Sampled,
    /// An invariant, action property or deadlock was found.
    Violation {
        /// `invariant`, `property`, `deadlock` or `temporal`.
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
    /// Traces committed in id order, including the deciding trace.
    pub traces_started: u64,
    /// The kept traces in id order: the [`Options::keep_traces`] lowest ids
    /// and the deciding trace.
    pub traces: Vec<Trace>,
    /// Optional chosen-action counts; absent when disabled or preflight refused.
    pub action_flow: Option<ActionFlow>,
    /// Actual backend output, separate from binding and independent replay.
    /// Warning events preserve their existing distinction from Print events.
    pub runtime_output: Vec<dive_eval::capture::Captured>,
    /// Every unbounded set the committed traces drew from, with the action
    /// that drew (`Nat in Next (line 6, ...)`), sorted.
    pub sampled_sets: Vec<String>,
    /// Committed traces that ended early because no drawn member of an
    /// unbounded set led to an admitted state; none is a deadlock claim.
    pub traces_stalled: u64,
    /// Temporal-property coverage; `None` without temporal properties.
    pub temporal: Option<TemporalCoverage>,
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
        sampled_sets: Vec::new(),
        traces_stalled: 0,
        temporal: None,
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
            sampled_sets: Vec::new(),
            traces_stalled: 0,
            temporal: None,
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
            sampled_sets: Vec::new(),
            traces_stalled: 0,
            temporal: None,
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
        return refusal("simulation requires a positive trace count");
    }
    if opts.workers == 0 {
        return refusal("simulation requires a positive worker count");
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
    // Simulation never deduplicates or canonicalizes states, so an unprepared
    // SYMMETRY changes nothing it reports (ADR 0162); its declaration is
    // still prepared at startup as `dive check` prepares it.
    if let Some(u) = model.unsupported.iter().find(|u| {
        !matches!(
            u.kind,
            dive_model::UnsupportedKind::TemporalProperty | dive_model::UnsupportedKind::Symmetry
        )
    }) {
        return refusal(&u.feature);
    }
    // Temporal properties (`model.implied_temporals`) are checked by the
    // backend on each trace's own behaviors; what their planning refuses is
    // refused there, by name, before any state is generated.
    // Implied initial predicates are checked on every initial state (2108),
    // and implied action properties on every candidate before the
    // constraints, as in `dive check`; the validator accepts a last state
    // outside the constraints for both. SYMMETRY and VIEW identify states
    // only for deduplication, which simulation never does: neither changes
    // a sampled trace or its verdict.
    // ALIAS only renders a separate view of the validated states
    // (`dive_api::simulate`). A failed lookup is TLC's error when it prints
    // a trace, which is not reproduced.
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
