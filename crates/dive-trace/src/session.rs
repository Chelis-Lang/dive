//! Private reconstruction of startup, independent of the caller's evaluator.

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};

use dive_model::{Model, Recipe};

use crate::validate::{Run, TraceHost};
use crate::{Report, Source, Trace, TraceDoc, Validation};

/// Immutable validation inputs. Every interpretation gets a freshly bound
/// runtime. The caller's caches, registers, host and atoms are never copied.
///
/// ```
/// use std::sync::Arc;
/// use dive_trace::{Session, TraceDoc, Validation};
/// let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\n\
///               ASSUME TLCSet(1,42)\nInit == x=0 /\\ TLCGet(1)=42\n\
///               Next == x'=x\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", source, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
/// let config = dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
/// let recipe = dive_model::Recipe::new(ir, config, dive_eval::Options::tlc(), false);
/// let session = Session::new(recipe, None);
/// let doc = TraceDoc::behavior(vec![serde_json::json!({"x":0})]).unwrap();
/// assert_eq!(session.read(&doc).0.validation, Validation::Valid);
/// ```
#[derive(Debug, Clone)]
pub struct Session {
    recipe: Recipe,
    config: Option<dive_eval::RunConfig>,
}

#[derive(Clone, Copy)]
enum FreshMode {
    Temporal,
    Finite(bool),
    NextPrefix,
    NextTerminal,
    InvariantPrefix,
    InvariantTerminal,
}

impl Session {
    /// Retains source/config inputs and typed run metadata, with no callbacks.
    #[must_use]
    pub fn new(recipe: Recipe, config: Option<dive_eval::RunConfig>) -> Self {
        Self { recipe, config }
    }

    pub(crate) fn bind(&self) -> Result<(Model, Arc<TraceHost>), String> {
        self.bind_in(true)
    }

    fn bind_in(&self, registers: bool) -> Result<(Model, Arc<TraceHost>), String> {
        let host = TraceHost::startup();
        host.startup.store(registers, Ordering::Relaxed);
        let model = self
            .recipe
            .bind(host.clone())
            .map_err(|e| e.error.to_string())?;
        // Match the API/checker lifecycle: config is unavailable while binding.
        if let Some(mut config) = self.config.clone() {
            config.deadlock = model.settings.check_deadlock;
            model.evaluator.intern_checker_names();
            model.evaluator.intern_bfs_config(&config);
            *host
                .run_config
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(config);
        }
        Ok((model, host))
    }

    /// Reads and validates a document, returning a plain document for transport.
    /// Transport uses text/JSON; private interner atoms cannot escape the runtime.
    #[must_use]
    pub fn read(&self, doc: &TraceDoc) -> (Report, Option<TraceDoc>) {
        if matches!(
            doc.ending,
            Some(
                crate::Ending::TemporalStateError { .. }
                    | crate::Ending::TemporalSourceError { .. }
                    | crate::Ending::NextStateError { .. }
                    | crate::Ending::ViewSourceError { .. }
                    | crate::Ending::CandidateStateError { .. }
                    | crate::Ending::PartialNext { .. }
                    | crate::Ending::PartialInit { .. }
            )
        ) {
            return (
                refusal(
                    "source error endpoints require Session::try_read or read_controlled".into(),
                    doc.logical_length(),
                ),
                None,
            );
        }
        if doc.source == Source::Dive && finite_word(doc) {
            return self
                .read_finite_controlled(doc, None, false)
                .unwrap_or_else(|error| (refusal(error.to_string(), doc.logical_length()), None));
        }
        let (model, host) = match self.bind() {
            Ok(bound) => bound,
            Err(feature) => return (refusal(feature, doc.logical_length()), None),
        };
        if let Some(feature) = Run::private(&model, host, false).refusal() {
            return (refusal(feature, doc.logical_length()), None);
        }
        if doc.source == Source::Tlc {
            let reading =
                crate::tlc::validate_tlc_private(&model, doc, self.recipe.model_values(), self);
            let plain = reading.trace.and_then(|trace| trace.to_doc(&model).ok());
            (reading.report, plain)
        } else {
            self.plain(doc, true)
        }
    }

    fn plain(&self, doc: &TraceDoc, initial: bool) -> (Report, Option<TraceDoc>) {
        let (model, host) = match self.bind_in(initial) {
            Ok(bound) => bound,
            Err(feature) => return (refusal(feature, doc.logical_length()), None),
        };
        if let Some(feature) = Run::private(&model, host.clone(), initial).refusal() {
            return (refusal(feature, doc.logical_length()), None);
        }
        let Ok(trace) = Trace::decode(&model, doc) else {
            let (report, _) = crate::validate_doc(&model, doc, self.recipe.model_values());
            return (report, None);
        };
        let mut run = Run::private(&model, host.clone(), initial);
        let validation = run.validate(&trace);
        // Pure membership may use the conservative predicate path. It gets a
        // new runtime with registers refused even during binding and ASSUMEs;
        // any register use in this attempt disallows that fallback.
        if initial && run.init_incomplete && !host.register_used.load(Ordering::Relaxed) {
            drop(run);
            return self.plain(doc, false);
        }
        let report = Report {
            validation,
            notes: std::mem::take(&mut run.notes).into_iter().collect(),
            length: trace.states.len(),
        };
        (report, trace.to_doc(&model).ok())
    }

    /// Reads a native trace on a fresh privately bound runtime with typed control.
    /// Endpoint descriptors are independently compiled and replayed; no producer
    /// validation flag is authority. This new controlled fresh-session path
    /// supports native source error endpoints and finite behavior, invariant,
    /// action-property and deadlock words. TLC JSON interpretation retains its
    /// existing entrypoints.
    ///
    /// ```no_run
    /// # fn check(session: &dive_trace::Session, document: &dive_trace::TraceDoc,
    /// # control: std::sync::Arc<dyn dive_eval::HostControl>)
    /// # -> Result<(), dive_trace::ValidationRunError> {
    /// let (report, _) = session.read_controlled(document, control)?;
    /// # let _ = report; Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Typed binding or validation interruption with progress, or conflicting
    /// evaluator control ownership. Other refusals remain validation reports.
    pub fn read_controlled(
        &self,
        doc: &TraceDoc,
        control: Arc<dyn dive_eval::HostControl>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        // Retain the owned control until private model/trace owners and the final poll finish.
        let result = self.read_fallible(doc, Some(&control));
        drop(control);
        result
    }

    /// Independently checks a native finite sample on a freshly reconstructed
    /// runtime. The document must retain its ordinary `behavior` ending; this
    /// separate operation also checks every sampled constraint, invariant and
    /// implied finite property. It proves neither exhaustive safety nor liveness.
    /// Fairness-only specifications are allowed; configured temporal properties
    /// remain unsupported. No producer metadata selects this operation.
    ///
    /// # Errors
    /// The same typed resource, interruption and control failures as
    /// [`Self::read_controlled`].
    pub fn read_sampled(
        &self,
        doc: &TraceDoc,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        if let Some(control) = &control {
            control
                .check()
                .map_err(|error| crate::ValidationRunError::Interrupted {
                    error: dive_eval::EvalError::interrupted(error),
                    progress: crate::ValidationProgress {
                        checks_passed: 0,
                        state: None,
                        check: None,
                    },
                })?;
        }
        if doc.source != Source::Dive
            || doc.ending != Some(crate::Ending::Behavior)
            || !doc.properties.is_empty()
        {
            return Ok((Report {
                validation: Validation::Invalid(crate::Failure { state: 0,
                    check: crate::Check::Shape,
                    detail: "sample validation requires a native finite behavior with no property or error endpoint claim".into() }),
                notes: Vec::new(), length: doc.logical_length(),
            }, None));
        }
        let result = self.read_finite_controlled(doc, control.as_ref(), true);
        drop(control);
        result
    }

    /// Validates a document with resource exceptions outside the report.
    /// No budget or artificial control is installed by this uncontrolled path.
    /// Legacy documents retain the existing reader; temporal error endpoints
    /// use fresh private binding, ordered reconstruction and finite replay.
    ///
    /// ```no_run
    /// # fn read(session: &dive_trace::Session, doc: &dive_trace::TraceDoc)
    /// # -> Result<(), dive_trace::ValidationRunError> {
    /// let (report, _) = session.try_read(doc)?;
    /// # let _ = report; Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Allocation/resource failure or a typed evaluation-control interruption;
    /// neither is a completed validation verdict.
    pub fn try_read(
        &self,
        doc: &TraceDoc,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        if !matches!(
            doc.ending,
            Some(
                crate::Ending::TemporalStateError { .. }
                    | crate::Ending::TemporalSourceError { .. }
                    | crate::Ending::NextStateError { .. }
                    | crate::Ending::ViewSourceError { .. }
                    | crate::Ending::CandidateStateError { .. }
                    | crate::Ending::PartialNext { .. }
                    | crate::Ending::PartialInit { .. }
            )
        ) {
            return Ok(self.read(doc));
        }
        self.read_fallible(doc, None)
    }

    /// Reconstructs the first actual Init operation before selecting an error
    /// transport. Reported site/codes cannot disguise an emitted receiver error
    /// as a generation error to evade its required diagnostic witness.
    /// The no-word stages require matching actual diagnostic receiver catches.
    ///
    /// # Errors
    /// Typed interruption, resource/internal failure or control conflict;
    /// neither a stage nor an exception certificate exists in these cases.
    pub fn initial_error_stage(
        &self,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<crate::InitialErrorStage>), crate::ValidationRunError> {
        use crate::{ValidationProgress, ValidationRunError};
        let progress = || ValidationProgress {
            checks_passed: 0,
            state: None,
            check: Some(crate::Check::Init),
        };
        let poll = || {
            control.as_ref().map_or(Ok(()), |control| {
                control
                    .check()
                    .map_err(|error| ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: progress(),
                    })
            })
        };
        poll()?;
        let (model, host) =
            match self.fresh_binding(control.as_ref(), FreshMode::NextTerminal, 0)? {
                Ok(runtime) => runtime,
                Err(feature) => return Ok((refusal(feature, 0), None)),
            };
        let scope = control
            .as_ref()
            .map(|control| model.evaluator.host_control(Some(control.clone())))
            .transpose()
            .map_err(ValidationRunError::ControlConflict)?;
        let (validation, stage) = crate::invariant_error::stage(&model, &host, control.as_deref())
            .map_err(|error| match error.class() {
                dive_eval::ErrorClass::Interrupted => ValidationRunError::Interrupted {
                    error,
                    progress: progress(),
                },
                dive_eval::ErrorClass::ResourceLimit => ValidationRunError::ResourceLimit {
                    error,
                    progress: progress(),
                },
                _ => ValidationRunError::EvaluationFailure {
                    error,
                    progress: progress(),
                },
            })?;
        poll()?;
        let report = Report {
            validation,
            notes: Vec::new(),
            length: usize::from(
                stage.is_some_and(|stage| stage != crate::InitialErrorStage::Generation),
            ),
        };
        drop(scope);
        drop(control);
        Ok((report, stage))
    }

    /// Validates a named invariant-error word, then independently reconstructs
    /// its bounded first-Init receiver exception and diagnostic wrapper.
    /// Ordinary word validity alone never grants the optional class evidence.
    /// The document retains version 2 and names its original underlying cause.
    ///
    /// ```no_run
    /// # fn replay(session: &dive_trace::Session, word: &dive_trace::TraceDoc)
    /// # -> Result<(), dive_trace::ValidationRunError> {
    /// let (report, class) = session.initial_invariant_class(word, None)?;
    /// if let Some(class) = class {
    ///     assert!(report.validation.is_valid());
    ///     assert_eq!(class.deciding_code(), 2147);
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Original typed interruption, resource/internal failure or conflicting
    /// control ownership; none can produce a class certificate.
    #[allow(
        clippy::too_many_lines,
        reason = "fresh word and receiver runtimes retain distinct ownership"
    )]
    pub fn initial_invariant_class(
        &self,
        doc: &TraceDoc,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<crate::InitialInvariantClass>), crate::ValidationRunError> {
        use crate::{Check, Ending, ValidationProgress, ValidationRunError};
        let progress = || ValidationProgress {
            checks_passed: 0,
            state: Some(0),
            check: Some(Check::Ending),
        };
        let poll = || {
            control.as_ref().map_or(Ok(()), |control| {
                control
                    .check()
                    .map_err(|error| ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: progress(),
                    })
            })
        };
        let finish = |report, class| {
            poll()?;
            Ok((report, class))
        };
        poll()?;
        let Some(Ending::InvariantError { name, ec }) = &doc.ending else {
            return finish(
                refusal(
                    "initial invariant class requires a named original cause".into(),
                    doc.logical_length(),
                ),
                None,
            );
        };
        if doc.source != Source::Dive || doc.states.len() != 1 || !doc.properties.is_empty() {
            return finish(refusal("initial receiver class requires one native initial state without temporal claims".into(), doc.logical_length()), None);
        }
        let (model, host) = match self.fresh_binding(control.as_ref(), FreshMode::NextPrefix, 0)? {
            Ok(runtime) => runtime,
            Err(feature) => return finish(refusal(feature, doc.logical_length()), None),
        };
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => return finish(shape_refusal(error, doc.logical_length()), None),
        };
        let mut report =
            crate::validate::validate_private_controlled(&model, host, &trace, control.clone())?;
        if !report.validation.is_valid() {
            return finish(report, None);
        }
        // Predicate validation must not supply cached receivers or normalization
        // history to the separate ordinary/diagnostic Init reconstruction.
        drop(trace);
        drop(model);
        let (model, host) =
            match self.fresh_binding(control.as_ref(), FreshMode::NextTerminal, 0)? {
                Ok(runtime) => runtime,
                Err(feature) => return finish(refusal(feature, doc.logical_length()), None),
            };
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => return finish(shape_refusal(error, doc.logical_length()), None),
        };
        let control_scope = control
            .as_ref()
            .map(|control| model.evaluator.host_control(Some(control.clone())))
            .transpose()
            .map_err(ValidationRunError::ControlConflict)?;
        let replay = crate::invariant_error::replay(
            &model,
            &trace.states[0],
            name,
            ec,
            &host,
            control.as_deref(),
        );
        let class = match replay {
            Ok((validation, class)) => {
                report.validation = validation;
                class
            }
            Err(error) => {
                return Err(match error.class() {
                    dive_eval::ErrorClass::Interrupted => ValidationRunError::Interrupted {
                        error,
                        progress: progress(),
                    },
                    dive_eval::ErrorClass::ResourceLimit => ValidationRunError::ResourceLimit {
                        error,
                        progress: progress(),
                    },
                    _ => ValidationRunError::EvaluationFailure {
                        error,
                        progress: progress(),
                    },
                });
            }
        };
        let result = finish(report, class);
        drop(control_scope);
        drop(control);
        result
    }

    /// Validates a named successor-invariant word and independently executes
    /// one exhausted Init, each earlier admitted source and actual diagnostic
    /// replay with bounded private key history.
    /// No serialized metadata or ordinary word validity supplies this evidence.
    ///
    /// # Errors
    /// Actual interruption, resource/internal failure or control conflict;
    /// no completed class certificate is returned for these failures.
    #[allow(
        clippy::too_many_lines,
        reason = "fresh word and receiver runtimes retain distinct ownership"
    )]
    pub fn invariant_error_class(
        &self,
        doc: &TraceDoc,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<crate::InvariantErrorClass>), crate::ValidationRunError> {
        use crate::{Check, Ending, ValidationProgress, ValidationRunError};
        let progress = || ValidationProgress {
            checks_passed: 0,
            state: Some(0),
            check: Some(Check::Ending),
        };
        let poll = || {
            control.as_ref().map_or(Ok(()), |control| {
                control
                    .check()
                    .map_err(|error| ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: progress(),
                    })
            })
        };
        let finish = |report, class| {
            poll()?;
            Ok((report, class))
        };
        poll()?;
        let Some(Ending::InvariantError { .. }) = &doc.ending else {
            return finish(
                refusal(
                    "successor invariant class requires a named original cause".into(),
                    doc.logical_length(),
                ),
                None,
            );
        };
        if doc.source != Source::Dive || doc.states.len() < 2 || !doc.properties.is_empty() {
            return finish(refusal("successor invariant class requires a finite native successor word without temporal claims".into(), doc.logical_length()), None);
        }
        let (model, host) =
            match self.fresh_binding(control.as_ref(), FreshMode::InvariantPrefix, 0)? {
                Ok(runtime) => runtime,
                Err(feature) => return finish(refusal(feature, doc.logical_length()), None),
            };
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => return finish(shape_refusal(error, doc.logical_length()), None),
        };
        let mut report =
            crate::validate::validate_private_controlled(&model, host, &trace, control.clone())?;
        if !report.validation.is_valid() {
            return finish(report, None);
        }
        // Predicate validation must not supply cached receivers or normalization
        // history to the separate ordinary/diagnostic Init reconstruction.
        drop(trace);
        drop(model);
        let (model, host) =
            match self.fresh_binding(control.as_ref(), FreshMode::InvariantTerminal, 0)? {
                Ok(runtime) => runtime,
                Err(feature) => return finish(refusal(feature, doc.logical_length()), None),
            };
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => return finish(shape_refusal(error, doc.logical_length()), None),
        };
        let control_scope = control
            .as_ref()
            .map(|control| model.evaluator.host_control(Some(control.clone())))
            .transpose()
            .map_err(ValidationRunError::ControlConflict)?;
        let replay = crate::ordered_prefix::invariant(&model, &trace, &host, control.as_deref());
        let class = match replay {
            Ok((validation, class)) => {
                if let Some(reason) = Run::private(&model, host.clone(), false).refusal() {
                    report.validation = Validation::Unsupported(reason);
                    None
                } else {
                    report.validation = validation;
                    class
                }
            }
            Err(error) => {
                return Err(match error.class() {
                    dive_eval::ErrorClass::Interrupted => ValidationRunError::Interrupted {
                        error,
                        progress: progress(),
                    },
                    dive_eval::ErrorClass::ResourceLimit => ValidationRunError::ResourceLimit {
                        error,
                        progress: progress(),
                    },
                    _ => ValidationRunError::EvaluationFailure {
                        error,
                        progress: progress(),
                    },
                });
            }
        };
        let result = finish(report, class);
        drop(control_scope);
        drop(control);
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one source-admission boundary retains typed controls and endpoint dispatch order"
    )]
    fn read_fallible(
        &self,
        doc: &TraceDoc,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        use crate::{ValidationProgress, ValidationRunError};
        let progress = || ValidationProgress {
            checks_passed: 0,
            state: None,
            check: None,
        };
        let poll = || {
            if let Some(control) = control {
                control
                    .check()
                    .map_err(|error| ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: progress(),
                    })?;
            }
            Ok(())
        };
        let finish = |report, document| {
            poll()?;
            Ok((report, document))
        };
        poll()?;
        if doc.source != Source::Dive {
            return finish(
                refusal(
                    "controlled private TLC document interpretation".into(),
                    doc.logical_length(),
                ),
                None,
            );
        }
        if finite_word(doc) {
            return self.read_finite_controlled(doc, control, false);
        }
        if !matches!(
            doc.ending,
            Some(
                crate::Ending::TemporalStateError { .. }
                    | crate::Ending::TemporalSourceError { .. }
                    | crate::Ending::NextStateError { .. }
                    | crate::Ending::ViewSourceError { .. }
                    | crate::Ending::CandidateStateError { .. }
                    | crate::Ending::PartialNext { .. }
                    | crate::Ending::PartialInit { .. }
            )
        ) {
            return finish(refusal("controlled fresh-session validation currently requires a native source error endpoint; use existing validation entrypoints for other documents".into(),doc.states.len()),None);
        }
        let next_error = matches!(
            doc.ending,
            Some(
                crate::Ending::NextStateError { .. }
                    | crate::Ending::TemporalSourceError { .. }
                    | crate::Ending::ViewSourceError { .. }
                    | crate::Ending::CandidateStateError { .. }
                    | crate::Ending::PartialNext { .. }
                    | crate::Ending::PartialInit { .. }
            )
        );
        if next_error && !doc.properties.is_empty() {
            return finish(
                Report {
                    validation: Validation::Invalid(crate::Failure {
                        state: 0,
                        check: crate::Check::Shape,
                        detail: "finite Next error endpoint cannot claim temporal properties"
                            .into(),
                    }),
                    notes: Vec::new(),
                    length: doc.logical_length(),
                },
                None,
            );
        }
        let mode = if matches!(doc.ending, Some(crate::Ending::PartialInit { .. })) {
            FreshMode::NextTerminal
        } else if next_error {
            FreshMode::NextPrefix
        } else {
            FreshMode::Temporal
        };
        let (model, host) = match self.fresh_binding(control, mode, 0)? {
            Ok(runtime) => runtime,
            Err(feature) => return finish(refusal(feature, doc.logical_length()), None),
        };
        if matches!(doc.ending, Some(crate::Ending::PartialInit { .. })) {
            return Self::read_initial_partial_endpoint(doc, &model, host, control);
        }
        if matches!(doc.ending, Some(crate::Ending::PartialNext { .. })) {
            return self.read_partial_endpoint(doc, model, host, control);
        }
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => {
                return finish(shape_refusal(error, doc.states.len()), None);
            }
        };
        if next_error {
            return self.read_next_endpoint(doc, model, host, trace, control);
        }
        let report =
            crate::validate::validate_private_controlled(&model, host, &trace, control.cloned())?;
        finish(report, trace.to_doc(&model).ok())
    }

    fn read_finite_controlled(
        &self,
        doc: &TraceDoc,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
        sampled: bool,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        if !self.recipe.has_randomness() {
            return self.read_finite_attempt(doc, control, sampled, None);
        }
        let choices = Arc::new(crate::choices::Search::default());
        for _ in 0..crate::choices::MAX_ATTEMPTS {
            if let Some(control) = control {
                control
                    .check()
                    .map_err(|error| crate::ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: crate::ValidationProgress {
                            checks_passed: 0,
                            state: None,
                            check: None,
                        },
                    })?;
            }
            // The previous attempt, including every value and name it created,
            // is dropped before another source binding begins. A new host cache
            // domain alone would not isolate mutable normalization histories.
            let result = self.read_finite_attempt(doc, control, sampled, Some(&choices))?;
            if !matches!(result.0.validation, Validation::Invalid(_))
                || choices.refusal().is_some()
                || !choices.advance()
            {
                return Ok(result);
            }
        }
        Err(crate::ValidationRunError::ResourceLimit {
            error: dive_eval::EvalError::resource_limit(
                "independent finite random choice attempts",
            ),
            progress: crate::ValidationProgress {
                checks_passed: 0,
                state: None,
                check: None,
            },
        })
    }

    fn read_finite_attempt(
        &self,
        doc: &TraceDoc,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
        sampled: bool,
        choices: Option<&Arc<crate::choices::Search>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        for initial in [true, false] {
            if let Some(choices) = choices {
                choices.rewind();
            }
            let (model, host) = match self.fresh_binding_choices(
                control,
                FreshMode::Finite(initial),
                0,
                choices.cloned(),
            )? {
                Ok(runtime) => runtime,
                Err(feature) => return Ok((refusal(feature, doc.logical_length()), None)),
            };
            if sampled
                && (!model.implied_temporals.is_empty()
                    || model.view.is_some()
                    || model.symmetry.is_some()
                    || model.alias.is_some()
                    || !matches!(&model.postconditions, Ok(postconditions) if postconditions.is_empty()))
            {
                return Ok((refusal("finite sample validation does not discharge temporal PROPERTY, VIEW, symmetry, ALIAS or POSTCONDITION obligations".into(), doc.logical_length()), None));
            }
            let trace = match Trace::decode(&model, doc) {
                Ok(trace) => trace,
                Err(error) => {
                    return Ok((
                        Report {
                            validation: Validation::Invalid(crate::Failure {
                                state: error.state.unwrap_or(0),
                                check: crate::Check::Shape,
                                detail: error.detail,
                            }),
                            notes: Vec::new(),
                            length: doc.logical_length(),
                        },
                        None,
                    ));
                }
            };
            let (report, retry) = crate::validate::validate_finite_private_controlled(
                &model,
                &host,
                &trace,
                initial,
                control.cloned(),
                sampled,
            )?;
            if !retry {
                return Ok((report, trace.to_doc(&model).ok()));
            }
        }
        Ok((
            refusal(
                "finite initial replay is unproved".into(),
                doc.logical_length(),
            ),
            None,
        ))
    }

    fn read_next_endpoint(
        &self,
        doc: &TraceDoc,
        model: Model,
        host: Arc<TraceHost>,
        trace: Trace,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        use crate::{ValidationProgress, ValidationRunError};
        let (prefix, checks_passed) = crate::validate::validate_next_prefix_controlled(
            &model,
            host,
            &trace,
            control.cloned(),
        )?;
        let finish_next = |report, document| {
            if let Some(control) = control {
                control
                    .check()
                    .map_err(|error| ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: ValidationProgress {
                            checks_passed,
                            state: doc.states.len().checked_sub(1),
                            check: Some(crate::Check::Ending),
                        },
                    })?;
            }
            Ok((report, document))
        };
        if prefix.validation != Validation::Valid {
            return finish_next(prefix, trace.to_doc(&model).ok());
        }
        // Prefix predicates must not warm the terminal generator's lazy
        // values or normalization state. No private atom is transferred.
        drop(trace);
        drop(model);
        let (model, host) =
            match self.fresh_binding(control, FreshMode::NextTerminal, checks_passed)? {
                Ok(runtime) => runtime,
                Err(feature) => return finish_next(refusal(feature, doc.logical_length()), None),
            };
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => return finish_next(shape_refusal(error, doc.states.len()), None),
        };
        let Some(state) = trace.states.last() else {
            return finish_next(refusal("Next endpoint has no state".into(), 0), None);
        };
        let mut report = crate::validate::replay_next_controlled(
            &model,
            host,
            &trace,
            state,
            checks_passed,
            control.cloned(),
        )?;
        report.notes.extend(prefix.notes);
        let plain = trace.to_doc(&model).ok();
        finish_next(report, plain)
    }

    fn read_partial_endpoint(
        &self,
        doc: &TraceDoc,
        model: Model,
        host: Arc<TraceHost>,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        let invalid = |detail: &str| {
            (
                Report {
                    validation: Validation::Invalid(crate::Failure {
                        state: doc.states.len(),
                        check: crate::Check::Shape,
                        detail: detail.into(),
                    }),
                    notes: Vec::new(),
                    length: doc.logical_length(),
                },
                None,
            )
        };
        if matches!(&doc.ending,Some(crate::Ending::PartialNext {endpoint}) if endpoint.initial()) {
            return Ok(invalid("partial Next descriptor has an Init phase"));
        }
        if doc.states.is_empty() {
            return Ok(invalid("partial Next needs a complete source prefix"));
        }
        if doc
            .length
            .is_some_and(|length| length != doc.logical_length())
        {
            return Ok(invalid(
                "logical word length must include the partial terminal candidate",
            ));
        }
        // Retain the error-witness goal: ordinary Behavior deliberately omits
        // configured safety obligations, while every complete prefix row here
        // must pass them before a terminal event can be certified.
        let mut prefix = doc.clone();
        prefix.length = Some(prefix.states.len());
        let trace = match Trace::decode(&model, &prefix) {
            Ok(trace) => trace,
            Err(error) => return Ok(invalid(&error.detail)),
        };
        let (mut report, passed) = crate::validate::validate_next_prefix_controlled(
            &model,
            host,
            &trace,
            control.cloned(),
        )?;
        report.length = doc.logical_length();
        if report.validation != Validation::Valid {
            return Ok((report, None));
        }
        drop(trace);
        drop(model);
        let (model, host) = match self.fresh_binding(control, FreshMode::NextTerminal, passed)? {
            Ok(runtime) => runtime,
            Err(feature) => return Ok((refusal(feature, doc.logical_length()), None)),
        };
        let trace = match Trace::decode(&model, &prefix) {
            Ok(trace) => trace,
            Err(error) => return Ok(invalid(&error.detail)),
        };
        let Some(crate::Ending::PartialNext { endpoint }) = &doc.ending else {
            return Ok(invalid("missing partial terminal descriptor"));
        };
        let report = crate::validate::replay_partial_controlled(
            &model,
            host,
            &trace,
            endpoint,
            passed,
            control.cloned(),
        )?;
        Ok((report, Some(doc.clone())))
    }

    fn read_initial_partial_endpoint(
        doc: &TraceDoc,
        model: &Model,
        host: Arc<TraceHost>,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        let invalid = |detail: &str| {
            (
                Report {
                    validation: Validation::Invalid(crate::Failure {
                        state: 0,
                        check: crate::Check::Shape,
                        detail: detail.into(),
                    }),
                    notes: Vec::new(),
                    length: doc.logical_length(),
                },
                None,
            )
        };
        if !doc.states.is_empty()
            || !doc.labels.is_empty()
            || doc.length.is_some_and(|length| length != 1)
        {
            return Ok(invalid(
                "partial Init has no complete prefix and logical length one",
            ));
        }
        let variables = crate::variables(model);
        let header: std::collections::BTreeSet<_> = doc.variables.iter().collect();
        if header.len() != variables.len()
            || doc.variables.len() != variables.len()
            || variables.iter().any(|name| !header.contains(name))
        {
            return Ok(invalid(
                "partial Init variable header differs from the model",
            ));
        }
        let Some(crate::Ending::PartialInit { endpoint }) = &doc.ending else {
            return Ok(invalid("missing Init partial descriptor"));
        };
        if !endpoint.initial() {
            return Ok(invalid("partial Init descriptor has a Next phase"));
        }
        let report = crate::validate::replay_initial_partial_controlled(
            model,
            host,
            endpoint,
            control.cloned(),
        )?;
        Ok((report, Some(doc.clone())))
    }

    fn fresh_binding(
        &self,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
        mode: FreshMode,
        checks_passed: u64,
    ) -> Result<Result<(Model, Arc<TraceHost>), String>, crate::ValidationRunError> {
        self.fresh_binding_choices(control, mode, checks_passed, None)
    }

    fn fresh_binding_choices(
        &self,
        control: Option<&Arc<dyn dive_eval::HostControl>>,
        mode: FreshMode,
        checks_passed: u64,
        choices: Option<Arc<crate::choices::Search>>,
    ) -> Result<Result<(Model, Arc<TraceHost>), String>, crate::ValidationRunError> {
        use crate::{ValidationProgress, ValidationRunError};
        let progress = || ValidationProgress {
            checks_passed,
            state: None,
            check: None,
        };
        let poll = || {
            if let Some(control) = control {
                control
                    .check()
                    .map_err(|error| ValidationRunError::Interrupted {
                        error: dive_eval::EvalError::interrupted(error),
                        progress: progress(),
                    })?;
            }
            Ok(())
        };
        poll()?;
        let source_actions = choices.is_some() && matches!(mode, FreshMode::Finite(_));
        let host = TraceHost::startup_choices(choices);
        host.startup.store(
            matches!(mode, FreshMode::Temporal | FreshMode::Finite(true)),
            Ordering::Relaxed,
        );
        // Invariant certification executes deterministic Print value semantics
        // with the existing suppressed sink throughout its actual lifecycle.
        // Other error endpoints keep their earlier effect refusal; random,
        // registers and I/O retain their independent host guards.
        host.next_error.store(
            matches!(mode, FreshMode::NextPrefix | FreshMode::NextTerminal),
            Ordering::Relaxed,
        );
        let binding = if source_actions
            || matches!(mode, FreshMode::NextTerminal | FreshMode::InvariantTerminal)
        {
            self.recipe
                .bind_next_fallible(host.clone(), control.cloned())
        } else {
            self.recipe.bind_fallible(host.clone(), control.cloned())
        };
        let model = match binding {
            Ok(model) => model,
            Err(dive_model::ControlledBindError::Interrupted { error, .. }) => {
                return Err(ValidationRunError::Interrupted {
                    error,
                    progress: progress(),
                });
            }
            Err(dive_model::ControlledBindError::ControlConflict { error, .. }) => {
                return Err(ValidationRunError::ControlConflict(error));
            }
            Err(dive_model::ControlledBindError::EvaluationFailure { error, .. }) => {
                return Err(if error.class() == dive_eval::ErrorClass::ResourceLimit {
                    ValidationRunError::ResourceLimit {
                        error,
                        progress: progress(),
                    }
                } else {
                    ValidationRunError::EvaluationFailure {
                        error,
                        progress: progress(),
                    }
                });
            }
            Err(dive_model::ControlledBindError::Binding(failure)) => {
                if failure.error.kind == dive_model::ErrorKind::Bug {
                    return Err(ValidationRunError::BindingFailure {
                        error: failure.error,
                        progress: progress(),
                    });
                }
                return Ok(Err(format!("fresh validation binding: {}", failure.error)));
            }
        };
        poll()?;
        if let Some(mut config) = self.config.clone() {
            config.deadlock = model.settings.check_deadlock;
            model.evaluator.intern_checker_names();
            model.evaluator.intern_bfs_config(&config);
            *host
                .run_config
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(config);
        }
        poll()?;
        Ok(Ok((model, host)))
    }

    pub(crate) fn interpretation(&self, doc: &TraceDoc) -> (Report, Option<TraceDoc>) {
        self.plain(doc, true)
    }
}

fn finite_word(doc: &TraceDoc) -> bool {
    doc.properties.is_empty()
        && matches!(
            doc.ending,
            None | Some(
                crate::Ending::Behavior
                    | crate::Ending::Invariant(_)
                    | crate::Ending::Property(_)
                    | crate::Ending::Deadlock
            )
        )
}

fn refusal(feature: String, length: usize) -> Report {
    Report {
        validation: Validation::Unsupported(feature),
        notes: Vec::new(),
        length,
    }
}

fn shape_refusal(error: crate::ShapeError, length: usize) -> Report {
    Report {
        validation: Validation::Invalid(crate::Failure {
            state: error.state.unwrap_or(0),
            check: crate::Check::Shape,
            detail: error.detail,
        }),
        notes: Vec::new(),
        length,
    }
}
