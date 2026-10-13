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
