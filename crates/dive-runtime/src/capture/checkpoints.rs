//! Opt-in reached checkpoints, without temporal demand or verdict authority.

use dive_eval::{ErrorClass, EvalError, next::StateVals};

use super::{ConcreteSuccessor, ControlledExploration, Edge};
use crate::{ErrorSite, Stats, Verdict};

/// The unsupported feature a reached checkpoint decision stops with.
#[doc(hidden)]
pub const DECISION: &str = "reached capture checkpoint stopped; temporal demand and error classification are not certified";

/// The actual search boundary which invoked an opt-in observer.
///
/// ```
/// use dive_runtime::capture::CheckpointPhase;
/// assert_ne!(CheckpointPhase::Init, CheckpointPhase::SourceRow);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointPhase {
    /// A fresh in-model root, already enqueued, before its invariant checks.
    Init,
    /// A source whose successors and implicit stutter were admitted successfully.
    SourceRow,
    /// A source whose Next generator failed, before stutter or row completion.
    /// The original failure and any already admitted concrete targets survive.
    TerminalSourceRow,
}

/// Borrowed facts at an actually reached checkpoint, never a demand certificate.
/// No graph completion, temporal truth or error-class parity is implied.
#[derive(Clone, Copy, Debug)]
pub struct ReachedCheckpoint<'a> {
    #[doc(hidden)]
    pub phase: CheckpointPhase,
    #[doc(hidden)]
    pub source: usize,
    #[doc(hidden)]
    pub state: &'a StateVals,
    #[doc(hidden)]
    pub stats: Stats,
    #[doc(hidden)]
    pub edges: &'a [Edge],
    #[doc(hidden)]
    pub concrete: &'a [ConcreteSuccessor],
    #[doc(hidden)]
    pub next_error: Option<&'a EvalError>,
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
    /// Original generator failure at a terminal source boundary. This is
    /// absent at Init and successfully completed source rows.
    #[must_use]
    pub const fn next_error(&self) -> Option<&'a EvalError> {
        self.next_error
    }
}

/// Explicit consumer of reached checkpoints; ordinary capture never calls this.
/// Implementations receive evidence only, not authority to classify temporal errors.
///
/// ```
/// use dive_runtime::capture::{CheckpointObserver, ReachedCheckpoint};
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
    next_error: Option<EvalError>,
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
    /// Original Next failure preceding the observer's row error, if any.
    /// It is retained as typed execution evidence, not a deciding class.
    #[must_use]
    pub const fn next_error(&self) -> Option<&EvalError> {
        self.next_error.as_ref()
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
    /// A driver's outcome and decision (drivers only).
    #[doc(hidden)]
    #[must_use]
    pub fn from_parts(
        outcome: ControlledExploration,
        decision: Option<Box<CheckpointDecision>>,
    ) -> Self {
        Self { outcome, decision }
    }
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

/// The producer-side state of an opt-in checkpoint observer (drivers only).
#[doc(hidden)]
pub struct Hooks<'a> {
    observer: &'a mut dyn CheckpointObserver,
    decision: Option<Box<CheckpointDecision>>,
}
impl<'a> Hooks<'a> {
    /// Hooks with no decision recorded yet.
    #[must_use]
    pub fn new(observer: &'a mut dyn CheckpointObserver) -> Self {
        Self {
            observer,
            decision: None,
        }
    }
    /// The recorded decision, if the observer returned one.
    #[must_use]
    pub fn into_decision(self) -> Option<Box<CheckpointDecision>> {
        self.decision
    }
    /// Whether the observer has returned a decision.
    #[must_use]
    pub fn has_decision(&self) -> bool {
        self.decision.is_some()
    }
    /// Invokes the observer at a reached checkpoint.
    ///
    /// # Errors
    /// The stopping verdict of an observer decision, interruption or panic.
    pub fn invoke(
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
                    next_error: event.next_error.cloned(),
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
