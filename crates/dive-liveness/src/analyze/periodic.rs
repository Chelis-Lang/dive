//! Deterministic checks over borrowed graphs at committed expansion boundaries.
//!
//! Capture limits apply cumulatively to the run. Formula planning happens once;
//! label and product bounds retain their per-property, per-check scope. Repeated
//! checks are additional work, without a total-run work or byte guarantee.

use super::{
    ActionEvaluation, Analysis, AnalysisError, CandidateWord, DetailedFailure, ExplorationEvidence,
    Failure, Limits, Stage, analyze_captured, prepare, validate_analysis_context,
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

struct Observe<'a> {
    budget: &'a dive_check::budget::Budget,
    model: &'a Model,
    plans: &'a [super::PreparedProperty],
    limits: Limits,
    policy: AcceptancePolicy,
    checks: Checks,
    stage: Stage,
    action: Option<Box<ActionEvaluation>>,
    decision: Option<Result<CandidateWord, AnalysisError>>,
}
impl capture::Observer for Observe<'_> {
    fn level_done(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        if stats.left_on_queue == 0
            || !self
                .checks
                .schedule
                .due(stats.distinct, self.checks.last_size)
        {
            return Ok(());
        }
        self.checks.prefixes += 1;
        self.checks.last_level = Some(level);
        self.checks.last_size = stats.distinct;
        // Capture invokes this callback on its own thread. Thread-local run
        // budgets do not cross that boundary; install the same armed signal
        // only while a due prefix analysis runs.
        let _scope = crate::budget::enter(self.budget);
        // Only the first property can decide a prefix: a word for it on a
        // subgraph is a word on the complete graph, which the final search
        // would also report first. Later properties wait for the final check.
        match analyze_captured(
            self.model,
            self.plans,
            1,
            graph,
            self.limits,
            self.policy,
            &mut self.stage,
            &mut self.action,
        ) {
            Ok(None) => {}
            Ok(Some(word)) => self.decision = Some(Ok(word)),
            Err(error) => self.decision = Some(Err(error)),
        }
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        self.decision.is_some()
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
    action: Option<Box<ActionEvaluation>>,
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
            action,
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
            None,
            Some(false),
            Checks {
                schedule,
                ..Checks::default()
            },
        )
    })?;
    let _scope = crate::budget::enter(&armed.budget);
    let result = periodic_unbudgeted(model, &armed, limits, policy, schedule)
        .map_err(|failure| budget_failure(failure, &armed.budget));
    if let Some(watchdog) = &mut watchdog {
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
                    None,
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
        failure.failure.action = None;
    }
    failure
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
    schedule: Schedule,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    let checks = Checks {
        schedule,
        ..Checks::default()
    };
    // Before lowering, touching bound scopes or starting a capture host.
    validate_analysis_context(model)
        .map_err(|error| failure(error, Stage::Planning, None, None, Some(false), checks))?;
    let plans = prepare(model, limits)
        .map_err(|error| failure(error, Stage::Planning, None, None, Some(false), checks))?;
    let mut observer = Observe {
        budget: &options.budget,
        model,
        plans: &plans,
        limits,
        policy,
        checks,
        stage: Stage::Capture,
        action: None,
        decision: None,
    };
    let captured =
        capture::explore_controlled_with_evidence(model, options, limits.capture, &mut observer)
            .map_err(|message| {
                failure(
                    AnalysisError::CaptureThread(message),
                    Stage::Unknown,
                    None,
                    None,
                    None,
                    observer.checks,
                )
            })?;
    match captured {
        capture::ControlledExploration::Stopped(result) => Err(failure(
            AnalysisError::Exploration(result),
            Stage::Capture,
            None,
            None,
            Some(false),
            observer.checks,
        )),
        capture::ControlledExploration::Prefix(prefix) => {
            let word = observer
                .decision
                .unwrap_or_else(|| {
                    Err(AnalysisError::Inconsistent(
                        "prefix stopped without an analysis decision",
                    ))
                })
                .map_err(|error| {
                    failure(
                        error,
                        observer.stage,
                        Some(Box::new((&*prefix).into())),
                        observer.action,
                        Some(false),
                        observer.checks,
                    )
                })?;
            Ok((
                PeriodicAnalysis::Prefix(PrefixCandidate {
                    target_property: word.target_property,
                    properties: word.properties,
                    states: word.states,
                    back_to: word.back_to,
                    edges: word.edges,
                    exploration: *prefix,
                }),
                observer.checks,
            ))
        }
        capture::ControlledExploration::Complete(complete) => {
            observer.checks.exhausted = true;
            observer.checks.final_check = true;
            let word = analyze_captured(
                model,
                &plans,
                usize::MAX,
                &complete.graph,
                limits,
                policy,
                &mut observer.stage,
                &mut observer.action,
            )
            .map_err(|error| {
                failure(
                    error,
                    observer.stage,
                    Some(Box::new((&*complete).into())),
                    observer.action,
                    Some(true),
                    observer.checks,
                )
            })?;
            let analysis = super::final_analysis(word, *complete);
            Ok((PeriodicAnalysis::Final(analysis), observer.checks))
        }
    }
}

#[cfg(test)]
mod budget_tests;
