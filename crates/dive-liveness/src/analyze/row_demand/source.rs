//! Actual singleton source-row demand before an opaque temporal residual.

use dive_eval::{Ec, ErrorClass, EvalError, ExceptionKind, St};
use dive_model::Model;
use dive_runtime::{CheckOptions, CheckResult};
use dive_value::Value;

use super::{Failed, stable_producers};
use crate::analyze::{
    AnalysisError, GraphLabelPolicy, PreparedProperty, SourceRowEvidence, StateEvaluation,
    StateEvaluationSite,
};
use crate::formula::{AtomKind, FormulaError};
use crate::tlc_plan::demand::{self, SourceProgram};

#[allow(
    clippy::too_many_arguments,
    reason = "same source-bound row preparation boundary"
)]
pub(crate) fn prepare_source(
    model: &Model,
    plans: &[PreparedProperty],
    options: &CheckOptions,
    fairness: usize,
    limits: crate::formula::Limits,
    graph_labels: GraphLabelPolicy,
    owned_control: (
        Option<&crate::analyze::owned::Authority<'_>>,
        &dive_runtime::budget::EvaluationControl,
    ),
) -> Result<Option<SourceProgram>, AnalysisError> {
    let (authority, control) = owned_control;
    if !authority.is_some_and(|authority| authority.owns(model)) {
        return Ok(None);
    }
    inspect_source(
        model,
        plans,
        options,
        fairness,
        limits,
        graph_labels,
        control,
    )
}

pub(in crate::analyze) fn source_possible(
    model: &Model,
    plans: &[PreparedProperty],
    options: &CheckOptions,
    limits: crate::formula::Limits,
    graph_labels: GraphLabelPolicy,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<bool, AnalysisError> {
    inspect_source(
        model,
        plans,
        options,
        model.temporals.len(),
        limits,
        graph_labels,
        control,
    )
    .map(|program| program.is_some())
}

#[allow(
    clippy::too_many_arguments,
    reason = "same optional bounded row inspection"
)]
fn inspect_source(
    model: &Model,
    plans: &[PreparedProperty],
    options: &CheckOptions,
    fairness: usize,
    limits: crate::formula::Limits,
    graph_labels: GraphLabelPolicy,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Option<SourceProgram>, AnalysisError> {
    if options.workers != 1
        || graph_labels != GraphLabelPolicy::NoCellLimit
        || model.view.is_some()
        || model.symmetry.is_some()
    {
        return Ok(None);
    }
    let Some(stability) = stable_producers(model, plans, control).map_err(|error| {
        crate::analyze::evaluator_interruption(control, &error).unwrap_or_else(|| {
            if error.class() == ErrorClass::ResourceLimit {
                AnalysisError::Limit("source-row canonical inspection allocation")
            } else {
                AnalysisError::Inconsistent("source-row inspection lost control provenance")
            }
        })
    })?
    else {
        return Ok(None);
    };
    let inputs = plans
        .iter()
        .map(|prepared| (&prepared.plan.formula, prepared.plan.atoms.len()))
        .collect::<Vec<_>>();
    match demand::compile_source(&inputs, fairness, limits) {
        Ok(mut program) => {
            if let Some(program) = &mut program {
                program.bind_stability(stability);
            }
            Ok(program)
        }
        Err(FormulaError::Limit(crate::budget::EXHAUSTED)) => {
            if let Err(stop) = dive_eval::HostControl::check(control) {
                return Err(crate::analyze::evaluator_interruption(
                    control,
                    &EvalError::interrupted(stop),
                )
                .unwrap_or(AnalysisError::Inconsistent(
                    "source-row normalization lost its control owner",
                )));
            }
            Err(AnalysisError::Limit(crate::budget::EXHAUSTED))
        }
        Err(FormulaError::Limit(_)) => Ok(None),
        Err(error) => Err(AnalysisError::Tableau(error)),
    }
}

fn general(error: &EvalError) -> bool {
    error.class() == ErrorClass::Eval
        && error.exception_kind() == Some(ExceptionKind::ToolRuntime)
        && error.ec() == Ec::General
        && error.value_method().is_none()
        && error.origin().is_some()
}

fn scalar(state: &[Option<Value>]) -> bool {
    state.iter().all(|value| {
        matches!(
            value,
            Some(
                Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_)
            )
        )
    })
}

struct SourceFailure {
    failed: Failed,
    primary: EvalError,
    diagnostic: EvalError,
}

pub(crate) struct SourceObserve<'a> {
    model: &'a Model,
    plans: &'a [PreparedProperty],
    program: &'a SourceProgram,
    failed: Option<SourceFailure>,
}

impl<'a> SourceObserve<'a> {
    pub(crate) fn new(
        model: &'a Model,
        plans: &'a [PreparedProperty],
        program: &'a SourceProgram,
    ) -> Self {
        Self {
            model,
            plans,
            program,
            failed: None,
        }
    }

    fn row(&self, state: &dive_eval::next::StateVals, diagnostic: bool) -> Result<(), Failed> {
        self.program.evaluate(|global| {
            let slot = self.program.slots[global];
            let atom = &self.plans[slot.property].plan.atoms[slot.atom];
            let context = if atom.kind == AtomKind::Action {
                St::action(state.clone(), state.clone())
            } else {
                St::state(state.clone())
            };
            let context = if diagnostic {
                context.diagnostic()
            } else {
                context
            };
            atom.evaluate_detailed(self.model, &context)
                .map_err(|failure| Failed {
                    global,
                    core: matches!(&failure, crate::plan::AtomError::Core(_)),
                    cause: failure.into_error(),
                })
        })
    }
}

impl SourceObserve<'_> {
    fn observe_row(
        &mut self,
        state: &dive_eval::next::StateVals,
        next_error: Option<&EvalError>,
    ) -> Result<(), EvalError> {
        let failed = match self.row(state, false) {
            Ok(()) => return Ok(()), // Opaque barrier: no completion or absence claim.
            Err(failed) => failed,
        };
        if !failed.core || !general(&failed.cause) {
            return if matches!(
                failed.cause.class(),
                ErrorClass::Eval | ErrorClass::Unsupported
            ) {
                Err(EvalError::unsupported(
                    "singleton temporal source-row primary constructor is not certified",
                ))
            } else {
                Err(failed.cause)
            };
        }
        let primary = next_error.unwrap_or(&failed.cause);
        if !general(primary) {
            return Err(EvalError::unsupported(
                "retained Next primary of the singleton temporal row is not certified",
            ));
        }
        let diagnostic = match self.row(state, true) {
            Err(replay)
                if replay.core
                    && replay.cause.class() == ErrorClass::Eval
                    && replay.cause.exception_kind().is_some()
                    && replay.cause.value_method().is_none()
                    && replay.cause.origin().is_some() =>
            {
                replay.cause
            }
            Err(replay)
                if !matches!(
                    replay.cause.class(),
                    ErrorClass::Eval | ErrorClass::Unsupported
                ) =>
            {
                return Err(replay.cause);
            }
            _ => {
                return Err(EvalError::unsupported(
                    "actual temporal row diagnostic suppression is not certified",
                ));
            }
        };
        let cause = failed.cause.clone();
        self.failed = Some(SourceFailure {
            primary: primary.clone(),
            diagnostic,
            failed,
        });
        Err(cause)
    }
}

/// Hidden source-bound evaluator for the native producer's bounded row-error
/// lane. It can only be constructed by consuming an owned source dispatch;
/// no model, evaluator, host or arbitrary provider is accepted or exposed.
///
/// ```no_run
/// # fn finish(driver: dive_liveness::analyze::owned::SourceRowDriver,
/// # actual: &dive_runtime::CheckResult) -> Result<(), dive_liveness::analyze::AnalysisError> {
/// let evidence = driver.finish(actual)?;
/// // Evidence still requires fresh independent source/word validation.
/// # let _ = evidence; Ok(()) }
/// ```
pub struct SourceRowDriver {
    model: Box<Model>,
    plans: Vec<PreparedProperty>,
    program: SourceProgram,
    control: std::sync::Arc<dive_runtime::budget::EvaluationControl>,
    pending: Option<(
        usize,
        dive_eval::next::StateVals,
        dive_runtime::Stats,
        SourceFailure,
    )>,
    active: bool,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl SourceRowDriver {
    pub(in crate::analyze) fn new(
        model: Box<Model>,
        plans: Vec<PreparedProperty>,
        program: SourceProgram,
        control: std::sync::Arc<dive_runtime::budget::EvaluationControl>,
    ) -> Self {
        Self {
            model,
            plans,
            program,
            control,
            pending: None,
            active: true,
            _thread: std::marker::PhantomData,
        }
    }

    /// Observes an actual native completed row. Fresh local generation must
    /// reproduce its complete scalar self emissions before row demand runs.
    /// A true result means a typed row failure is pending, not a verdict.
    ///
    /// # Errors
    /// Original owned interruption/resource failure, or unproved runtime facts.
    pub fn completed_source(
        &mut self,
        source: usize,
        state: &dive_eval::next::StateVals,
        targets: &[dive_eval::next::StateVals],
        emitted: u64,
        counts: dive_runtime::Stats,
    ) -> Result<bool, AnalysisError> {
        self.observe(source, state, targets, emitted, counts, false)
    }

    /// Observes an actual native Next-error boundary. Its original complete
    /// emissions and failure are reproduced before post-Next row demand runs;
    /// no implicit stutter or successful row completion is published.
    ///
    /// # Errors
    /// As [`Self::completed_source`].
    pub fn failed_source(
        &mut self,
        source: usize,
        state: &dive_eval::next::StateVals,
        targets: &[dive_eval::next::StateVals],
        emitted: u64,
        counts: dive_runtime::Stats,
    ) -> Result<bool, AnalysisError> {
        self.observe(source, state, targets, emitted, counts, true)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "actual native source event fields remain distinct"
    )]
    fn observe(
        &mut self,
        source: usize,
        state: &dive_eval::next::StateVals,
        targets: &[dive_eval::next::StateVals],
        emitted: u64,
        counts: dive_runtime::Stats,
        next_failed: bool,
    ) -> Result<bool, AnalysisError> {
        if self.pending.is_some() {
            return Err(AnalysisError::Inconsistent(
                "row driver continued after its deciding event",
            ));
        }
        if !self.active {
            return Ok(false);
        }
        self.active = false;
        if source != 0
            || counts.initial_generated != 1
            || !scalar(state)
            || targets.iter().any(|target| target != state)
            || u64::try_from(targets.len()).ok() != Some(emitted)
        {
            return Ok(false);
        }
        let state = self.transfer(state).ok_or(AnalysisError::Inconsistent(
            "native scalar source namespace",
        ))?;
        let _control = self
            .control
            .enter(&self.model.evaluator)
            .map_err(|_| AnalysisError::Inconsistent("source-row evaluator control conflict"))?;
        let _hooks = self
            .model
            .evaluator
            .native_hook_scope(dive_eval::NativeHooks::Unrestricted);
        let mut count = 0_u64;
        let mut self_only = true;
        let generated =
            self.model
                .evaluator
                .successors(&self.model.actions, &state, &mut |candidate| {
                    count = count.saturating_add(1);
                    self_only &= candidate.state == state.as_ref();
                    dive_eval::next::Flow::Continue
                });
        let next = match generated {
            Ok(outcome)
                if !next_failed && self_only && count == emitted && outcome.intermediate == 0 =>
            {
                None
            }
            Err(failure)
                if next_failed
                    && self_only
                    && count == emitted
                    && failure.intermediate == 0
                    && failure.error.class() == ErrorClass::Eval =>
            {
                Some(failure.error)
            }
            Err(failure)
                if !matches!(
                    failure.error.class(),
                    ErrorClass::Eval | ErrorClass::Unsupported
                ) =>
            {
                return Err(self.error(&failure.error));
            }
            _ => return Ok(false),
        };
        let mut observe = SourceObserve::new(&self.model, &self.plans, &self.program);
        match observe.observe_row(&state, next.as_ref()) {
            Ok(()) => Ok(false),
            Err(error) => {
                if let Some(failed) = observe.failed.take() {
                    self.pending = Some((source, state, counts, failed));
                    Ok(true)
                } else {
                    Err(self.error(&error))
                }
            }
        }
    }

    fn error(&self, error: &EvalError) -> AnalysisError {
        crate::analyze::evaluator_interruption(&self.control, error).unwrap_or_else(|| match error
            .class()
        {
            ErrorClass::ResourceLimit => {
                AnalysisError::Limit("native source-row evaluation allocation")
            }
            ErrorClass::Unsupported | ErrorClass::Eval => {
                AnalysisError::UnsupportedValue(error.to_string())
            }
            _ => AnalysisError::Inconsistent("native source-row error lost typed authority"),
        })
    }

    fn transfer(&self, state: &[Option<Value>]) -> Option<dive_eval::next::StateVals> {
        state
            .iter()
            .map(|value| {
                Some(Some(match value.as_ref()? {
                    value @ (Value::Bool(_) | Value::Int(_) | Value::BigInt(_)) => value.clone(),
                    Value::Str(name) | Value::Model(name) => {
                        let own = self.model.names.resolve(name.tok())?;
                        if own.as_str() != name.as_str() {
                            return None;
                        }
                        if matches!(value, Some(Value::Str(_))) {
                            Value::Str(own)
                        } else {
                            Value::Model(own)
                        }
                    }
                    _ => return None,
                }))
            })
            .collect::<Option<Vec<_>>>()
            .map(Into::into)
    }

    /// Whether actual row execution selected an error. This is not independent
    /// validation, a TLC class or completed graph evidence.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.pending.is_some()
    }

    /// Binds the pending actual row to the producer's reconstructed source word
    /// and counters. A missing/mismatching word provides no certificate.
    ///
    /// # Errors
    /// The producer reconstruction does not match the selected source event.
    pub fn finish(
        mut self,
        result: &CheckResult,
    ) -> Result<Option<StateEvaluation>, AnalysisError> {
        let Some((source_id, state, counts, failure)) = self.pending.take() else {
            return Ok(None);
        };
        if result.stats != Some(counts)
            || result
                .counterexample
                .as_ref()
                .and_then(|path| path.states.last())
                .is_none_or(|last| {
                    last.text.is_some()
                        || last.values.iter().cloned().map(Some).collect::<Vec<_>>()
                            != state.as_ref()
                })
        {
            return Err(AnalysisError::Inconsistent(
                "native row error lost exact source reconstruction",
            ));
        }
        let slot = self.program.slots[failure.failed.global];
        let atom = &self.plans[slot.property].plan.atoms[slot.atom];
        Ok(Some(StateEvaluation {
            cause: failure.failed.cause,
            property: self.model.property_claims()[slot.property].name.clone(),
            source: Box::new(crate::plan::Source::new(&self.model, atom.pred.location)),
            core: true,
            site: StateEvaluationSite::CaptureSourceRow,
            row: None,
            source_row: Some(Box::new(SourceRowEvidence {
                source: source_id,
                result: Box::new(result.clone()),
                primary: failure.primary,
                diagnostic: failure.diagnostic,
                _proof: self
                    .program
                    .certificate()
                    .ok_or(AnalysisError::Inconsistent(
                        "source-row certificate missing",
                    ))?,
            })),
        }))
    }
}
