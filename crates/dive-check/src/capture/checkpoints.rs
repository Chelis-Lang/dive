//! Opt-in reached checkpoints, without temporal demand or verdict authority.

use dive_eval::{ErrorClass, EvalError, next::StateVals};
use dive_model::Model;

use super::{ConcreteSuccessor, ControlledExploration, Edge, Limits, Observer};
use crate::{CheckOptions, ErrorSite, Stats, Verdict};

const DECISION: &str = "reached capture checkpoint stopped; temporal demand and error classification are not certified";

/// The actual search boundary which invoked an opt-in observer.
///
/// ```
/// use dive_check::capture::CheckpointPhase;
/// assert_ne!(CheckpointPhase::Init, CheckpointPhase::SourceRow);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointPhase {
    /// A fresh in-model root, already enqueued, before its invariant checks.
    Init,
    /// A source whose successors and implicit stutter were admitted successfully.
    SourceRow,
}

/// Borrowed facts at an actually reached checkpoint, never a demand certificate.
/// No graph completion, temporal truth or error-class parity is implied.
#[derive(Clone, Copy, Debug)]
pub struct ReachedCheckpoint<'a> {
    pub(crate) phase: CheckpointPhase,
    pub(crate) source: usize,
    pub(crate) state: &'a StateVals,
    pub(crate) stats: Stats,
    pub(crate) edges: &'a [Edge],
    pub(crate) concrete: &'a [ConcreteSuccessor],
}
impl<'a> ReachedCheckpoint<'a> {
    /// Actual Init or completed-source boundary.
    #[must_use]
    pub const fn phase(&self) -> CheckpointPhase {
        self.phase
    }
    /// Dense original root/source identity, not a predicate or tableau identity.
    #[must_use]
    pub const fn source(&self) -> usize {
        self.source
    }
    /// Original complete state being processed.
    #[must_use]
    pub const fn state(&self) -> &'a StateVals {
        self.state
    }
    /// Actual counters, including generated Init candidates so far and real queue.
    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }
    /// Admitted outgoing row including stutter; empty at Init.
    #[must_use]
    pub const fn edges(&self) -> &'a [Edge] {
        self.edges
    }
    /// Original accepted concrete successors, excluding implicit stutter.
    #[must_use]
    pub const fn concrete_successors(&self) -> &'a [ConcreteSuccessor] {
        self.concrete
    }
}

/// Explicit consumer of reached checkpoints; ordinary capture never calls this.
/// Implementations receive evidence only, not authority to classify temporal errors.
///
/// ```
/// use dive_check::capture::{CheckpointObserver, ReachedCheckpoint};
/// struct Observe;
/// impl CheckpointObserver for Observe {
///     fn reached(&mut self, event: ReachedCheckpoint<'_>) -> Result<(), dive_eval::EvalError> {
///         let _actual = (event.phase(), event.source(), event.state(), event.stats(),
///             event.edges(), event.concrete_successors());
///         Ok(())
///     }
/// }
/// ```
pub trait CheckpointObserver: Send {
    /// Called only at the actual admitted boundary under the search's control.
    ///
    /// # Errors
    /// A genuine Core `ErrorClass::Eval` cause stops as unsupported evidence.
    /// Other classes keep their existing mapping and produce no typed decision.
    /// Owned budget interruptions keep their existing stopped/measurement class;
    /// foreign interruptions and callback panics cannot produce a decision.
    fn reached(&mut self, event: ReachedCheckpoint<'_>) -> Result<(), EvalError>;
}

/// Producer-owned Core `ErrorClass::Eval` cause at a reached checkpoint.
/// The stopped result carries its checked diagnostic path.
/// This does not certify TLC's demand, class, trace ending or completed exploration.
#[derive(Clone, Debug)]
pub struct CheckpointDecision {
    phase: CheckpointPhase,
    source: usize,
    cause: EvalError,
    stats: Stats,
}
impl CheckpointDecision {
    /// Actual reached boundary, not a reconstructed earlier analysis site.
    #[must_use]
    pub const fn phase(&self) -> CheckpointPhase {
        self.phase
    }
    /// Original dense root/source identity.
    #[must_use]
    pub const fn source(&self) -> usize {
        self.source
    }
    /// Unformatted original core evaluation cause and its expression origin.
    #[must_use]
    pub const fn cause(&self) -> &EvalError {
        &self.cause
    }
    /// Actual search counters at this decision, never reconstructed graph counts.
    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }
}

/// Capture result and optional reached cause; no new ordinary verdict is introduced.
/// A decision is accompanied by the stopped result's checked finite diagnostic path.
#[derive(Clone, Debug)]
pub struct CheckpointExploration {
    outcome: ControlledExploration,
    decision: Option<Box<CheckpointDecision>>,
}
impl CheckpointExploration {
    /// Existing capture outcome, including original counters, workers and path.
    #[must_use]
    pub const fn outcome(&self) -> &ControlledExploration {
        &self.outcome
    }
    /// Actual core cause, absent on panic, interruption or failed path reconstruction.
    #[must_use]
    pub fn decision(&self) -> Option<&CheckpointDecision> {
        self.decision.as_deref()
    }
    /// Transfers the ordinary capture outcome and its separate typed evidence.
    #[must_use]
    pub fn into_parts(self) -> (ControlledExploration, Option<Box<CheckpointDecision>>) {
        (self.outcome, self.decision)
    }
}

pub(crate) struct Hooks<'a> {
    observer: &'a mut dyn CheckpointObserver,
    decision: Option<Box<CheckpointDecision>>,
}
impl Hooks<'_> {
    pub(crate) fn has_decision(&self) -> bool {
        self.decision.is_some()
    }
    pub(crate) fn invoke(
        &mut self,
        event: ReachedCheckpoint<'_>,
        control: &crate::budget::EvaluationControl,
    ) -> Result<(), Verdict> {
        // Record only after a normal Err return. A panic or foreign receipt cannot
        // leave a partially published or stale decision behind.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.observer.reached(event)
        })) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(cause)) => {
                if let Some(verdict) = control.verdict(&cause) {
                    return Err(verdict);
                }
                if cause.class() != ErrorClass::Eval {
                    // This existing branch handles only non-Eval classes. It
                    // cannot apply an ordinary Init/Next TLC mapping to this cause.
                    return Err(crate::class::search_error(&cause, ErrorSite::Next, None));
                }
                self.decision = Some(Box::new(CheckpointDecision {
                    phase: event.phase,
                    source: event.source,
                    cause,
                    stats: event.stats,
                }));
                Err(Verdict::Unsupported {
                    feature: DECISION.into(),
                })
            }
            Err(_) => Err(Verdict::InternalError {
                message: "capture checkpoint callback panicked".into(),
            }),
        }
    }
}

/// Opt-in W1 inline capture with actual Init/source-row checkpoints.
/// Existing capture entrypoints, old Observer callbacks and ordinary scheduling
/// are unchanged. Other requested widths (including zero) refuse before search.
/// No temporal predicate is evaluated by this API itself.
///
/// ```no_run
/// # fn example(model: &dive_model::Model,
/// #     checkpoints: &mut dyn dive_check::capture::CheckpointObserver)
/// #     -> Result<(), String> {
/// use dive_check::{capture, CheckOptions};
/// let run = capture::explore_checkpoints_with_evidence(model,
///     &CheckOptions::tlc(0, 0), Default::default(), None, checkpoints)?;
/// if let Some(decision) = run.decision() {
///     let _facts = (decision.phase(), decision.source(), decision.cause(), decision.stats());
/// }
/// let (_outcome, _decision) = run.into_parts();
/// # Ok(()) }
/// ```
///
/// # Errors
/// The dedicated exploration thread failed or panicked, losing execution evidence.
pub fn explore_checkpoints_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: Option<&mut dyn Observer>,
    checkpoints: &mut dyn CheckpointObserver,
) -> Result<CheckpointExploration, String> {
    let refuse = |outcome: super::Exploration| CheckpointExploration {
        outcome: outcome.into(),
        decision: None,
    };
    if opts.workers != 1 {
        let mut stop = super::stopped(
            Verdict::Unsupported {
                feature: "reached capture checkpoints require exactly one requested worker".into(),
            },
            model,
            opts,
        );
        if let super::Exploration::Stopped(result) = &mut stop {
            result.workers.requested = opts.workers;
            result.workers.used = 0;
            result.workers.reason =
                Some("checkpoint worker-width admission stopped before search".into());
        }
        return Ok(refuse(stop));
    }
    if let Some(stop) = super::context_stop(model, opts) {
        return Ok(refuse(stop));
    }
    let started = std::time::Instant::now();
    let (mut outcome, mut decision) = super::run_on_thread(|| {
        let mut hooks = Hooks {
            observer: checkpoints,
            decision: None,
        };
        let outcome = super::explore_inner(model, opts, limits, false, observer, Some(&mut hooks));
        (outcome, hooks.decision)
    })?;
    match &mut outcome {
        ControlledExploration::Complete(c) => c.safety_result.elapsed = started.elapsed(),
        ControlledExploration::Prefix(p) => p.result.elapsed = started.elapsed(),
        ControlledExploration::Stopped(s) => s.elapsed = started.elapsed(),
    }
    if let Some(record) = &decision {
        let usable = matches!(&outcome, ControlledExploration::Stopped(s)
            if matches!(&s.verdict, Verdict::Unsupported { feature } if feature == DECISION)
                && s.stats == Some(record.stats) && s.counterexample.is_some());
        if !usable {
            decision = None;
        }
    }
    Ok(CheckpointExploration { outcome, decision })
}

#[cfg(test)]
mod tests;
