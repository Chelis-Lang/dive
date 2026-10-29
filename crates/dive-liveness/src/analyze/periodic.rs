//! Deterministic checks over borrowed graphs at committed expansion boundaries.
//!
//! Capture limits apply cumulatively to the run. Formula planning happens once;
//! label and product bounds retain their per-property, per-check scope. Repeated
//! checks are additional work, without a total-run work or byte guarantee.

use super::{
    Analysis, AnalysisError, CandidateWord, DetailedFailure, EvaluationErrors, ExplorationEvidence,
    Failure, GraphLabelPolicy, Limits, Stage, analyze_captured, prepare, validate_analysis_context,
};
use crate::product::AcceptancePolicy;
use dive_check::{CheckOptions, Stats, capture};
use dive_eval::next::StateVals;
use dive_model::Model;

/// When a completed, nonexhausted level is checked (ADR 0018 section 7, D-014).
///
/// A level is checked once the graph has at least `min_states` distinct states
/// and at least `growth_factor` times as many as at the previous check. With
/// the default doubling, all prefix checks together cost about one final check.
/// The schedule depends only on the graph, so results are the same at every
/// worker count. TLC's wall-clock trigger has no deterministic equivalent; the
/// TLC-equivalent setting is final checking (`-lncheck final`).
///
/// ```
/// use dive_liveness::analyze::periodic::Schedule;
/// let schedule = Schedule::DEFAULT;
/// assert!(!schedule.due(65_535, 0));
/// assert!(schedule.due(65_536, 0));
/// assert!(!schedule.due(100_000, 65_536));
/// assert!(schedule.due(131_072, 65_536));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    /// Smallest distinct-state count at which a prefix is checked.
    pub min_states: u64,
    /// Required growth factor over the previous check's distinct-state count.
    pub growth_factor: u64,
}

impl Schedule {
    /// The documented default: at least 2^16 states, doubling between checks.
    pub const DEFAULT: Self = Self {
        min_states: 1 << 16,
        growth_factor: 2,
    };

    /// Whether a completed level with `distinct` states is due, given the
    /// distinct-state count `last` at the previous check (zero before any).
    #[must_use]
    pub fn due(self, distinct: u64, last: u64) -> bool {
        distinct >= self.min_states.max(1)
            && distinct > last
            && distinct >= last.saturating_mul(self.growth_factor)
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Actual deterministic-check execution facts; not evidence of a holding verdict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checks {
    /// The schedule that selected the checks.
    pub schedule: Schedule,
    /// Number of completed-level analysis calls, excluding the final check.
    pub prefixes: u64,
    /// Last level at which a nonexhausted graph was checked.
    pub last_level: Option<u64>,
    /// Distinct-state count at that check, zero before any.
    pub last_size: u64,
    /// Whether exploration actually exhausted its frontier.
    pub exhausted: bool,
    /// Whether the complete graph's final analysis was attempted.
    pub final_check: bool,
}

/// A candidate found on a real committed prefix. Independent validation is required.
#[derive(Clone, Debug)]
pub struct PrefixCandidate {
    /// Original property whose negation selected the word.
    pub target_property: String,
    /// Complete directly attributed false-property set.
    pub properties: Vec<String>,
    /// Actual graph states along the word.
    pub states: Vec<StateVals>,
    /// Zero-based closing target.
    pub back_to: usize,
    /// Actual transitions, including the closing edge.
    pub edges: Vec<capture::Edge>,
    /// Producer-owned prefix; never complete exploration.
    pub exploration: capture::Prefix,
}

/// Analysis with explicit prefix versus complete evidence.
#[derive(Clone, Debug)]
pub enum PeriodicAnalysis {
    /// Real exhaustion followed by the existing final analysis.
    Final(Analysis),
    /// An unvalidated word found before exploration finished.
    Prefix(PrefixCandidate),
}

impl PeriodicAnalysis {
    /// Releases legacy graph storage while retaining the actual complete or
    /// prefix facts and any selected word. It does not validate a candidate or
    /// promote incomplete exploration to a holding result.
    ///
    /// ```
    /// use dive_liveness::analyze::periodic::{PeriodicAnalysis, PeriodicFailure, Checks};
    /// use dive_liveness::analyze::periodic::external::NativeAnalysis;
    /// fn release_graph(result: Result<(PeriodicAnalysis, Checks), PeriodicFailure>)
    ///     -> Result<(NativeAnalysis, Checks), PeriodicFailure> {
    ///     result.map(|(analysis, checks)| (analysis.into_native(), checks))
    /// }
    /// ```
    #[must_use]
    pub fn into_native(self) -> external::NativeAnalysis {
        match self {
            Self::Final(analysis) => external::NativeAnalysis::Final(analysis.into_native()),
            Self::Prefix(candidate) => external::NativeAnalysis::Prefix(super::finish::Prefix {
                word: super::finish::NativeWord {
                    target_property: candidate.target_property,
                    properties: candidate.properties,
                    states: candidate.states,
                    back_to: candidate.back_to,
                    edges: candidate.edges,
                },
                exploration: candidate.exploration.into_evidence(),
            }),
        }
    }
}

/// A failure with explicit capture-completion truth and actual scheduling facts.
#[derive(Clone, Debug)]
pub struct PeriodicFailure {
    /// Original analysis error and typed action cause, where applicable.
    pub failure: Box<DetailedFailure>,
    /// Actual prefix facts, separate from legacy completed-exploration evidence.
    pub prefix: Option<Box<ExplorationEvidence>>,
    /// Whether capture completed; a lost thread remains unknown.
    pub capture_completed: Option<bool>,
    /// Actual checks attempted before the failure.
    pub checks: Checks,
}

/// The fixed inputs of a level or final check.
#[derive(Clone, Copy)]
struct Inputs<'a> {
    budget: &'a dive_check::budget::Budget,
    control: &'a std::sync::Arc<dive_check::budget::EvaluationControl>,
    model: &'a Model,
    plans: &'a [super::PreparedProperty],
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
}

/// What the checks of one run have done and decided so far.
struct Decisions {
    checks: Checks,
    stage: Stage,
    evaluations: EvaluationErrors,
    decision: Option<Result<CandidateWord, AnalysisError>>,
}

impl Decisions {
    fn new(checks: Checks) -> Self {
        Self {
            checks,
            stage: Stage::Capture,
            evaluations: EvaluationErrors {
                periodic: true,
                ..EvaluationErrors::default()
            },
            decision: None,
        }
    }

    /// Checks the graph after the completed `level`, if the schedule says so.
    fn level(
        &mut self,
        inputs: Inputs<'_>,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: Stats,
    ) {
        if stats.left_on_queue == 0
            || !self
                .checks
                .schedule
                .due(stats.distinct, self.checks.last_size)
        {
            return;
        }
        self.checks.prefixes += 1;
        self.checks.last_level = Some(level);
        self.checks.last_size = stats.distinct;
        // Capture invokes this callback on its own thread. Thread-local run
        // budgets do not cross that boundary; install the same armed signal
        // only while a due prefix analysis runs.
        let _scope = crate::budget::enter(inputs.budget);
        // Due prefixes retain legacy frontier labels and attribution. A row
        // cache covers expanded sources only and cannot replace that demand.
        // Only the first property can decide a prefix: a word for it on a
        // subgraph is a word on the complete graph, which the final search
        // would also report first. Later properties wait for the final check.
        match analyze_captured(
            inputs.model,
            inputs.plans,
            1,
            graph,
            inputs.limits,
            inputs.policy,
            inputs.graph_labels,
            &mut self.stage,
            &mut self.evaluations,
            inputs.control,
            None,
        ) {
            Ok(None) => {}
            Ok(Some(word)) => self.decision = Some(Ok(word)),
            Err(error) => self.decision = Some(Err(error)),
        }
    }

    /// The result of a stop after a completed level, with the kept decision.
    fn prefix(
        self,
        prefix: capture::Prefix,
    ) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
        let word = self.decision.unwrap_or_else(|| {
            Err(AnalysisError::Inconsistent(
                "prefix stopped without an analysis decision",
            ))
        });
        let result = super::finish::prefix(word, prefix.evidence().clone()).map_err(|error| {
            failure(
                error,
                self.stage,
                Some(Box::new((&prefix).into())),
                self.evaluations,
                Some(false),
                self.checks,
            )
        })?;
        Ok((
            PeriodicAnalysis::Prefix(result.reference(prefix)),
            self.checks,
        ))
    }

    /// The final check of a complete exploration.
    fn complete(
        mut self,
        inputs: Inputs<'_>,
        complete: capture::Complete,
        cache: Option<&std::sync::Mutex<super::row_demand::Cache>>,
    ) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
        let _control = inputs.control.enter(&inputs.model.evaluator).map_err(|_| {
            failure(
                AnalysisError::Inconsistent("another driver owns evaluator control"),
                Stage::Product,
                Some(Box::new((&complete).into())),
                EvaluationErrors::default(),
                Some(true),
                self.checks,
            )
        })?;
        self.checks.exhausted = true;
        self.checks.final_check = true;
        self.evaluations.periodic = false;
        let cached = cache
            .map(std::sync::Mutex::lock)
            .transpose()
            .map_err(|_| {
                failure(
                    AnalysisError::Inconsistent("temporal cache lock poisoned"),
                    Stage::Product,
                    Some(Box::new((&complete).into())),
                    EvaluationErrors::default(),
                    Some(true),
                    self.checks,
                )
            })?
            .filter(|cache| cache.active());
        let word = analyze_captured(
            inputs.model,
            inputs.plans,
            usize::MAX,
            &complete.graph,
            inputs.limits,
            inputs.policy,
            inputs.graph_labels,
            &mut self.stage,
            &mut self.evaluations,
            inputs.control,
            cached.as_deref(),
        );
        let result =
            super::finish::complete(word, complete.evidence().clone()).map_err(|error| {
                failure(
                    error,
                    self.stage,
                    Some(Box::new((&complete).into())),
                    self.evaluations,
                    Some(true),
                    self.checks,
                )
            })?;
        let analysis = result.reference(complete);
        Ok((PeriodicAnalysis::Final(analysis), self.checks))
    }
}

struct Observe<'a> {
    inputs: Inputs<'a>,
    d: Decisions,
}
impl capture::Observer for Observe<'_> {
    fn level_done(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        self.d.level(self.inputs, level, graph, stats);
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        self.d.decision.is_some()
    }
}

impl From<&capture::Prefix> for ExplorationEvidence {
    fn from(prefix: &capture::Prefix) -> Self {
        Self {
            stats: prefix.stats,
            workers: prefix.workers.clone(),
            collision: prefix.collision.clone(),
            output: prefix.output.clone(),
            warnings: prefix.warnings.clone(),
        }
    }
}

fn failure(
    error: AnalysisError,
    stage: Stage,
    exploration: Option<Box<ExplorationEvidence>>,
    evaluations: EvaluationErrors,
    capture_completed: Option<bool>,
    checks: Checks,
) -> PeriodicFailure {
    let (exploration, prefix) = if capture_completed == Some(false) {
        (None, exploration)
    } else {
        (exploration, None)
    };
    PeriodicFailure {
        prefix,
        failure: Box::new(DetailedFailure {
            failure: Failure {
                error,
                stage,
                exploration,
            },
            evaluations,
        }),
        capture_completed,
        checks,
    }
}

/// Plans once, then checks borrowed graphs only after fully committed levels.
/// `schedule` selects the completed levels to check ([`Schedule::DEFAULT`] for
/// the documented default). Exhausted levels are never checked as prefixes. Only real exhaustion can produce `Holds`.
/// Candidates still need fresh independent typed and serialized validation.
///
/// Capture bounds are cumulative. Existing formula, label and product bounds
/// retain their actual planning/per-property/per-check scopes; checks add work.
///
/// # Errors
/// All existing planning, capture, atom, attribution and product errors are
/// retained. Prefix failures explicitly report incomplete capture.
pub fn analyze_periodic(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    schedule: Schedule,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    analyze_periodic_with_policies(
        model,
        options,
        limits,
        policy,
        GraphLabelPolicy::Cells(limits.atom_cells),
        schedule,
    )
}

/// Like [`analyze_periodic`], explicitly choosing only graph-label admission.
/// All existing entrypoints retain every caller-supplied logical cell ceiling.
///
/// ```no_run
/// # fn example(model: &dive_model::Model) {
/// use dive_liveness::{analyze::{GraphLabelPolicy, Limits, periodic}, product::AcceptancePolicy};
/// let result = periodic::analyze_periodic_with_policies(model,
///     &dive_check::CheckOptions::dive(0), Limits::default(),
///     AcceptancePolicy::compact_default(), GraphLabelPolicy::Cells(31),
///     periodic::Schedule::DEFAULT);
/// # let _ = result;
/// # }
/// ```
///
/// # Errors
/// The same typed failures and actual prefix evidence as [`analyze_periodic`].
pub fn analyze_periodic_with_policies(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    schedule: Schedule,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    analyze_periodic_bound(model, options, limits, policy, graph_labels, schedule, None)
}

pub(super) fn analyze_periodic_bound(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    schedule: Schedule,
    authority: Option<&super::owned::Authority<'_>>,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    // One budget for capture and every level check (ADR 0107); only the
    // budget's own limit becomes a stop.
    let (mut watchdog, armed) = dive_check::budget::arm(options).map_err(|error| {
        failure(
            match error {
                dive_check::budget::StartError::Measurement(error) => {
                    AnalysisError::BudgetMeasurement(error)
                }
                error @ dive_check::budget::StartError::Thread(_) => {
                    AnalysisError::BudgetStartup(error.to_string())
                }
            },
            Stage::Planning,
            None,
            EvaluationErrors::default(),
            Some(false),
            Checks {
                schedule,
                ..Checks::default()
            },
        )
    })?;
    let _scope = crate::budget::enter(&armed.budget);
    let result = periodic_unbudgeted(
        model,
        &armed,
        limits,
        policy,
        graph_labels,
        schedule,
        authority,
    )
    .map_err(|failure| budget_failure(failure, &armed.budget));
    close_budget(watchdog.as_mut(), result)
}

/// The run's final budget check for the entry point that armed it: a
/// result is replaced by a stop when the budget ran out while finishing.
fn close_budget(
    watchdog: Option<&mut dive_check::budget::Watchdog>,
    result: Result<(PeriodicAnalysis, Checks), PeriodicFailure>,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    if let Some(watchdog) = watchdog {
        let finished = watchdog.finish();
        if let Ok((analysis, checks)) = &result {
            let error = match finished {
                Err(error) => Some(AnalysisError::BudgetMeasurement(error)),
                Ok(Some(exhausted)) => Some(AnalysisError::Stopped(Box::new(
                    exhausted.during(dive_check::budget::Phase::Finalization),
                ))),
                Ok(None) => None,
            };
            if let Some(error) = error {
                let (evidence, completed) = match analysis {
                    PeriodicAnalysis::Final(Analysis::Holds(capture)) => {
                        (ExplorationEvidence::from(capture), true)
                    }
                    PeriodicAnalysis::Final(Analysis::Candidate(candidate)) => {
                        (ExplorationEvidence::from(&candidate.exploration), true)
                    }
                    PeriodicAnalysis::Prefix(candidate) => {
                        let capture = &candidate.exploration;
                        (
                            ExplorationEvidence {
                                stats: capture.stats,
                                workers: capture.workers.clone(),
                                collision: capture.collision.clone(),
                                output: capture.output.clone(),
                                warnings: capture.warnings.clone(),
                            },
                            false,
                        )
                    }
                };
                return Err(failure(
                    error,
                    Stage::Finalization,
                    Some(Box::new(evidence)),
                    EvaluationErrors::default(),
                    Some(completed),
                    *checks,
                ));
            }
        }
    }
    result
}

fn budget_failure(
    mut failure: PeriodicFailure,
    budget: &dive_check::budget::Budget,
) -> PeriodicFailure {
    if super::budget_limit(&failure.failure.failure.error) {
        match budget.poll() {
            Ok(Some(exhausted)) => {
                failure.failure.failure.error = AnalysisError::Stopped(Box::new(
                    exhausted.during(dive_check::budget::Phase::Temporal),
                ));
            }
            Err(error) => failure.failure.failure.error = AnalysisError::BudgetMeasurement(error),
            Ok(None) => return failure,
        }
        failure.failure.evaluations = EvaluationErrors::default();
    }
    failure
}

/// Validates the context and plans every property once, before capture.
fn planned(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    checks: Checks,
) -> Result<
    (
        std::sync::Arc<dive_check::budget::EvaluationControl>,
        Vec<super::PreparedProperty>,
    ),
    PeriodicFailure,
> {
    // Before lowering, touching bound scopes or starting a capture host.
    validate_analysis_context(model).map_err(|error| {
        failure(
            error,
            Stage::Planning,
            None,
            EvaluationErrors::default(),
            Some(false),
            checks,
        )
    })?;
    let control = super::evaluator_control(options).map_err(|error| {
        failure(
            error,
            Stage::Planning,
            None,
            EvaluationErrors::default(),
            Some(false),
            checks,
        )
    })?;
    let plans = {
        let _scope = control.enter(&model.evaluator).map_err(|_| {
            failure(
                AnalysisError::Inconsistent("another driver owns evaluator control"),
                Stage::Planning,
                None,
                EvaluationErrors::default(),
                Some(false),
                checks,
            )
        })?;
        prepare(model, limits, &control).map_err(|error| {
            failure(
                error,
                Stage::Planning,
                None,
                EvaluationErrors::default(),
                Some(false),
                checks,
            )
        })?
    };
    Ok((control, plans))
}

#[allow(
    clippy::too_many_lines,
    reason = "keep producer outcomes and their distinct failure evidence together"
)]
fn periodic_unbudgeted(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    schedule: Schedule,
    authority: Option<&super::owned::Authority<'_>>,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    let checks = Checks {
        schedule,
        ..Checks::default()
    };
    let (control, plans) = planned(model, options, limits, checks)?;
    let demand = super::row_demand::prepare(
        model,
        &plans,
        options,
        model.temporals.len(),
        limits.formula,
        graph_labels,
        (authority, &control),
    )
    .map_err(|error| {
        failure(
            error,
            Stage::Planning,
            None,
            EvaluationErrors::default(),
            Some(false),
            checks,
        )
    })?;
    let cache = demand
        .as_ref()
        .map(|program| std::sync::Mutex::new(super::row_demand::Cache::new(program)));
    let mut observer = Observe {
        inputs: Inputs {
            budget: &options.budget,
            control: &control,
            model,
            plans: &plans,
            limits,
            policy,
            graph_labels,
        },
        d: Decisions::new(checks),
    };
    let native_hooks = super::owned::native_hooks(authority, model);
    let captured = if let (Some(program), Some(cache)) = (&demand, &cache) {
        let mut errors = EvaluationErrors {
            periodic: true,
            ..EvaluationErrors::default()
        };
        let mut exploration = None;
        super::row_demand::Observe::new(model, &plans, program, cache)
            .explore(
                options,
                limits.capture,
                Some(&mut observer),
                &mut errors,
                &mut exploration,
                native_hooks,
            )
            .map_err(|error| {
                failure(
                    error,
                    Stage::Capture,
                    exploration,
                    errors,
                    Some(false),
                    observer.d.checks,
                )
            })?
    } else {
        capture::explore_controlled_with_native_hooks(
            model,
            options,
            limits.capture,
            &mut observer,
            native_hooks,
        )
        .map_err(|message| {
            failure(
                AnalysisError::CaptureThread(message),
                Stage::Unknown,
                None,
                EvaluationErrors::default(),
                None,
                observer.d.checks,
            )
        })?
    };
    match captured {
        capture::ControlledExploration::Stopped(result) => Err(failure(
            AnalysisError::Exploration(result),
            Stage::Capture,
            None,
            EvaluationErrors::default(),
            Some(false),
            observer.d.checks,
        )),
        capture::ControlledExploration::Prefix(prefix) => observer.d.prefix(*prefix),
        capture::ControlledExploration::Complete(complete) => {
            observer
                .d
                .complete(observer.inputs, *complete, cache.as_ref())
        }
    }
}

pub mod external;

#[cfg(test)]
mod budget_tests;
