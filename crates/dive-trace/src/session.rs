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
        if matches!(doc.ending, Some(crate::Ending::TemporalStateError { .. })) {
            return (
                refusal(
                    "temporal error endpoints require Session::try_read or read_controlled".into(),
                    doc.states.len(),
                ),
                None,
            );
        }
        let (model, host) = match self.bind() {
            Ok(bound) => bound,
            Err(feature) => return (refusal(feature, doc.states.len()), None),
        };
        if let Some(feature) = Run::private(&model, host, false).refusal() {
            return (refusal(feature, doc.states.len()), None);
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
            Err(feature) => return (refusal(feature, doc.states.len()), None),
        };
        if let Some(feature) = Run::private(&model, host.clone(), initial).refusal() {
            return (refusal(feature, doc.states.len()), None);
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
    /// supports native temporal error endpoints only. Other native documents
    /// and TLC JSON interpretation retain their existing entrypoints.
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
        self.read_fallible(doc, Some(control))
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
        if !matches!(doc.ending, Some(crate::Ending::TemporalStateError { .. })) {
            return Ok(self.read(doc));
        }
        self.read_fallible(doc, None)
    }

    fn read_fallible(
        &self,
        doc: &TraceDoc,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<TraceDoc>), crate::ValidationRunError> {
        use crate::{ValidationProgress, ValidationRunError};
        let progress = || ValidationProgress {
            checks_passed: 0,
            state: None,
            check: None,
        };
        let poll = || {
            if let Some(control) = &control {
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
                    doc.states.len(),
                ),
                None,
            );
        }
        if !matches!(doc.ending, Some(crate::Ending::TemporalStateError { .. })) {
            return finish(refusal("controlled fresh-session validation currently requires a native temporal-state error endpoint; use existing validation entrypoints for other documents".into(),doc.states.len()),None);
        }
        let host = TraceHost::startup();
        host.startup.store(true, Ordering::Relaxed);
        let binding = if let Some(control) = &control {
            self.recipe.bind_controlled(host.clone(), control.clone())
        } else {
            self.recipe
                .bind(host.clone())
                .map_err(dive_model::ControlledBindError::Binding)
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
            Err(dive_model::ControlledBindError::Binding(failure)) => {
                return finish(
                    refusal(
                        format!("fresh validation binding: {}", failure.error),
                        doc.states.len(),
                    ),
                    None,
                );
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
        let trace = match Trace::decode(&model, doc) {
            Ok(trace) => trace,
            Err(error) => {
                return finish(
                    Report {
                        validation: Validation::Invalid(crate::Failure {
                            state: error.state.unwrap_or(0),
                            check: crate::Check::Shape,
                            detail: error.detail,
                        }),
                        notes: Vec::new(),
                        length: doc.states.len(),
                    },
                    None,
                );
            }
        };
        let report =
            crate::validate::validate_private_controlled(&model, host, &trace, control.clone())?;
        finish(report, trace.to_doc(&model).ok())
    }

    pub(crate) fn interpretation(&self, doc: &TraceDoc) -> (Report, Option<TraceDoc>) {
        self.plain(doc, true)
    }
}

fn refusal(feature: String, length: usize) -> Report {
    Report {
        validation: Validation::Unsupported(feature),
        notes: Vec::new(),
        length,
    }
}
