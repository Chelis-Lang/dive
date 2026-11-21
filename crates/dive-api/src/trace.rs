//! Trace validation: checking a trace against a specification.
//!
//! A [`SpecModel`] is a specification bound to its TLC configuration. It
//! validates traces in dive's format ([`dive_trace::format`]) or TLC's
//! `-dumpTrace json`, with the independent validator of `dive-trace`, which
//! evaluates the next-state relation as a predicate over supplied states and
//! never runs the model checker's search. Source-bound Next endpoints may use
//! the shared evaluator's ordered generator before its first candidate.
//! Register-dependent Init
//! uses an independent ordered single-candidate operation from empty slots.
//!
//! Named invariant evaluation errors use native trace version 2 with
//! `Ending::InvariantError { name, ec }`: `ec` is the underlying cause, while
//! the check report retains TLC's deciding wrapper 2111. Independent replay
//! requires that exact final invariant/cause after earlier checks pass. All
//! existing ending kinds retain native version 1; readers also accept version 3
//! partial candidates and version 4 independently replayed candidate errors.
//! Neither native payload version changes the report schema version.
//!
//! This is also how a Rust project checks its *own implementation's*
//! traces: record each state of a run as a JSON object from variable names
//! to values (in dive's JSON form, [`dive_trace::value_json`]), and validate
//! the sequence as a [`Ending::Behavior`]. The trace must assign every
//! variable in every complete state. A version-3 terminal partial candidate
//! is separate from those rows and requires fresh fallible replay.
//!
//! # Example
//!
//! ```
//! use dive_api::trace::{SpecModel, TraceOptions, TraceDoc, Validation};
//! use serde_json::json;
//!
//! let spec = "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE n\n\
//!             Init == n = 0\nNext == n' = n + 1 \\/ n' = 0\n====\n";
//! let model = SpecModel::from_source("Counter.tla", spec, "INIT Init\nNEXT Next\n", &TraceOptions::default())?;
//!
//! // States observed in the implementation, in order.
//! let observed = vec![json!({"n": 0}), json!({"n": 1}), json!({"n": 2}), json!({"n": 0})];
//! let report = model.validate(&TraceDoc::behavior(observed).unwrap());
//! assert_eq!(report.validation, Validation::Valid);
//!
//! // A bug in the implementation: the counter skipped a value.
//! let report = model.validate(&TraceDoc::behavior(vec![json!({"n": 0}), json!({"n": 2})]).unwrap());
//! let Validation::Invalid(f) = &report.validation else { panic!() };
//! assert_eq!(f.state, 1);
//! # Ok::<(), dive_api::ApiError>(())
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use dive_trace::mutate::{ControlMode, Mutant};
pub use dive_trace::{
    Check, DEADLOCK_UNSUPPORTED, Ending, Failure, InitialErrorStage, InitialInvariantClass, Report,
    State, Trace, TraceDoc, Validation, ValidationRunError,
};
use serde_json::{Value, json};

use crate::ApiError;

/// Version of the JSON document produced by [`report_json`].
pub const TRACE_SCHEMA_VERSION: u32 = crate::REPORT_SCHEMA_VERSION;

/// How the specification is bound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceOptions {
    /// Explicit recorded external assumptions. Source binding replays exact
    /// operation/argument order without operating-system access. Categories
    /// come from this context; absent inputs never grant host access.
    pub inputs: Option<dive_eval::inputs::RecordedInputs>,
    /// `--compat=tlc`: evaluate with every option at its TLC setting (32-bit
    /// integers, TLC's value order and comparisons). Otherwise dive's
    /// default profile.
    pub compat_tlc: bool,
    /// Explicit independent symmetry/liveness binding policy; omitted selects
    /// the semantic profile default. No trace annotation supplies this policy.
    pub symmetry_liveness: Option<dive_model::SymmetryLiveness>,
    /// TLC's `-deadlock` flag: the model does not check for deadlock.
    pub no_deadlock: bool,
    /// The checking run's `-seed` and `-fp` index, when known (the TLC
    /// goldens use `-seed 0 -fp 0`): `TLCGet("config")` reports them, and
    /// constant definitions draw from the run's start-up stream (TLC's
    /// seeding under `compat_tlc`, dive's replayable seeding otherwise).
    /// Without them,
    /// startup random operations need a producer host to bind. Native finite
    /// validation independently proves finite source-defined choice support,
    /// rebinding every alternative; it does not reproduce the run's RNG draws.
    /// Deferred domains, register-dependent random prefixes, random ENABLED and
    /// temporal/error-endpoint interactions retain named refusal boundaries.
    pub run: Option<(i64, usize)>,
    /// Modules searched before the standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Modules searched after the standard modules (TLC's classpath).
    pub module_paths: Vec<PathBuf>,
}

/// A specification bound to its configuration, with immutable original inputs
/// for private startup reconstruction on every validation.
#[derive(Debug, Clone)]
pub struct SpecModel {
    model: dive_model::Model,
    model_values: Vec<String>,
    session: dive_trace::Session,
    recipe: dive_model::Recipe,
}

pub(crate) fn make_run_config(
    model: &dive_model::Model,
    seed: i64,
    fp_index: usize,
) -> dive_eval::RunConfig {
    dive_eval::RunConfig {
        deadlock: model.settings.check_deadlock,
        workers: 1,
        seed,
        fingerprint: i64::from_ne_bytes(
            dive_value::fp64::POLYS
                .get(fp_index)
                .copied()
                .unwrap_or(0)
                .to_ne_bytes(),
        ),
        install: std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    }
}

impl SpecModel {
    /// Loads the specification rooted at `spec` and binds it to the
    /// configuration file `cfg`.
    ///
    /// # Errors
    /// [`ApiError::Io`] if a file cannot be read, [`ApiError::Rejected`] if
    /// the specification or configuration has errors, and
    /// [`ApiError::Unsupported`] if dive cannot bind it exactly.
    pub fn load(spec: &Path, cfg: &Path, opts: &TraceOptions) -> Result<Self, ApiError> {
        Self::load_hosted(spec, cfg, opts, None)
    }

    pub(crate) fn load_hosted(
        spec: &Path,
        cfg: &Path,
        opts: &TraceOptions,
        host: Option<Arc<dyn dive_eval::Host>>,
    ) -> Result<Self, ApiError> {
        let sema = sema_options(opts);
        let shown = spec.display().to_string();
        let analysis = dive_sema::analyze_file(spec, &sema).map_err(|error| ApiError::Io {
            path: shown.clone(),
            error,
        })?;
        let text = std::fs::read(cfg).map_err(|error| ApiError::Io {
            path: cfg.display().to_string(),
            error,
        })?;
        let (_, parsed) = dive_config::parse_bytes(&text);
        Self::bind_hosted(&shown, &analysis, parsed, opts, host)
    }

    /// Binds specification source text (the root module; others come from
    /// the search path) to configuration text.
    ///
    /// # Errors
    /// As [`SpecModel::load`].
    pub fn from_source(
        file: &str,
        source: &str,
        cfg: &str,
        opts: &TraceOptions,
    ) -> Result<Self, ApiError> {
        let analysis = dive_sema::analyze_source(file, source, &sema_options(opts));
        Self::bind(file, &analysis, dive_config::parse(cfg), opts)
    }

    fn bind(
        file: &str,
        analysis: &dive_sema::Analysis,
        cfg: dive_config::ConfigParse,
        opts: &TraceOptions,
    ) -> Result<Self, ApiError> {
        Self::bind_hosted(file, analysis, cfg, opts, None)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one ordered startup transaction retains source, host, replay cursor and config ownership"
    )]
    fn bind_hosted(
        file: &str,
        analysis: &dive_sema::Analysis,
        cfg: dive_config::ConfigParse,
        opts: &TraceOptions,
        owned_host: Option<Arc<dyn dive_eval::Host>>,
    ) -> Result<Self, ApiError> {
        if owned_host.is_some() && opts.inputs.is_some() {
            return Err(ApiError::Unsupported {
                feature: "owned simulation external input replay is not implemented".into(),
            });
        }
        if !analysis.accepted() {
            return Err(ApiError::Rejected {
                message: format!("{file} has errors; `dive analyze` lists them"),
            });
        }
        let Some(cfg) = cfg.into_config() else {
            return Err(ApiError::Rejected {
                message: "the configuration has errors; `dive parse --config` lists them"
                    .to_owned(),
            });
        };
        let ir = dive_ir::lower(analysis).map_err(|e| ApiError::Unsupported {
            feature: e.to_string(),
        })?;
        let ir = Arc::new(ir);
        dive_eval::validate_ir(&ir).map_err(|error| ApiError::Unsupported {
            feature: error.to_string(),
        })?;
        // A simulation (the only owner of a host here) never fingerprints,
        // views or canonicalizes states, so randomness under VIEW or SYMMETRY
        // needs nothing extra there (ADR 0162); other loads keep the refusal.
        let concrete = owned_host.is_some();
        if !concrete && let Some(feature) = dive_model::transformed_randomness(&ir, &cfg) {
            return Err(ApiError::Unsupported {
                feature: feature.to_owned(),
            });
        }
        let mode = if opts.compat_tlc {
            dive_stdlib::random::RandomMode::Tlc
        } else {
            dive_stdlib::random::RandomMode::Dive
        };
        let streams = match opts.run {
            Some((seed, fp_index)) => {
                if fp_index >= dive_value::fp64::POLYS.len() {
                    return Err(ApiError::Rejected {
                        message: format!(
                            "fingerprint polynomial index {fp_index} is out of range (0..{})",
                            dive_value::fp64::POLYS.len()
                        ),
                    });
                }
                Some(dive_stdlib::random::Streams::new(seed, mode))
            }
            None => None,
        };
        let replay = opts
            .inputs
            .as_ref()
            .map(|inputs| dive_eval::inputs::InputReplay::new(inputs.clone()).map(Arc::new))
            .transpose()
            .map_err(|error| ApiError::Unsupported {
                feature: error.message,
            })?;
        let host = streams.map(|s| {
            let host = dive_stdlib::host::RunHost::new(s, None);
            Arc::new(if let Some(replay) = &replay {
                host.with_inputs(replay.clone())
            } else {
                host
            })
        });
        let mut bind_opts = dive_model::BindOptions {
            no_deadlock: opts.no_deadlock,
            host: owned_host
                .or_else(|| host.clone().map(|h| -> Arc<dyn dive_eval::Host> { h }))
                .or_else(|| {
                    replay
                        .clone()
                        .map(|replay| -> Arc<dyn dive_eval::Host> { replay })
                }),
            ..if opts.compat_tlc {
                dive_model::BindOptions::tlc()
            } else {
                dive_model::BindOptions::dive()
            }
        };
        if let Some(inputs) = &opts.inputs {
            bind_opts.eval.host_access = inputs.grants;
        }
        if let Some(policy) = opts.symmetry_liveness {
            bind_opts.symmetry_liveness = policy;
        }
        let model_values = cfg.model_values.clone();
        let recipe =
            dive_model::Recipe::new(ir.clone(), cfg.clone(), bind_opts.eval, opts.no_deadlock)
                .with_symmetry_liveness(bind_opts.symmetry_liveness);
        let recipe = if concrete {
            recipe.with_concrete_states()
        } else {
            recipe
        };
        let compat_tlc = opts.compat_tlc;
        let bound = on_big_stack(move || {
            if concrete && !compat_tlc {
                // Simulation runs on Core, which evaluates the pure constant
                // definitions its roles use only as values when it compiles
                // its plan, as `dive check` binds in the default profile
                // (issue #1142). This model's own evaluator computes them
                // lazily, with the same values, where it needs them.
                dive_model::bind_deferring_constants(ir, &cfg, &bind_opts)
            } else if concrete {
                dive_model::bind(ir, &cfg, &bind_opts)
            } else {
                dive_model::bind_for_trace(ir, &cfg, &bind_opts)
            }
        })?;
        if let Some(replay) = &replay {
            replay
                .finish_binding()
                .map_err(|error| ApiError::Unsupported {
                    feature: error.message,
                })?;
        }
        let mut run_config = None;
        if let (Ok(model), Some((seed, fp_index))) = (&bound, opts.run) {
            model.evaluator.intern_checker_names();
            let config = make_run_config(model, seed, fp_index);
            model.evaluator.intern_bfs_config(&config);
            if let Some(host) = &host {
                host.set_config(Some(config.clone()));
            }
            run_config = Some(config);
        }
        match bound {
            Ok(model) => Ok(Self {
                model,
                model_values,
                recipe: recipe.clone(),
                session: {
                    let session = dive_trace::Session::new(
                        recipe.clone(),
                        run_config.or_else(|| {
                            host.as_ref()
                                .and_then(|host| dive_eval::Host::config(&**host))
                        }),
                    );
                    if let Some(inputs) = &opts.inputs {
                        session.with_inputs(inputs.clone())
                    } else {
                        session
                    }
                },
            }),
            Err(f) => match &f.error.kind {
                dive_model::ErrorKind::Unsupported { feature } => Err(ApiError::Unsupported {
                    feature: feature.clone(),
                }),
                _ => Err(ApiError::Rejected {
                    message: f.error.to_string(),
                }),
            },
        }
    }

    /// The bound model.
    #[must_use]
    pub fn model(&self) -> &dive_model::Model {
        &self.model
    }

    /// The bound configuration.
    pub(crate) const fn config(&self) -> &dive_config::ModelConfig {
        self.recipe.config()
    }

    /// Private feasibility runtime: fixed inputs, fresh names/caches and no ASSUMEs.
    pub(crate) fn preparation_model(
        &self,
        seed: i64,
        mode: dive_stdlib::random::RandomMode,
        control: Arc<dyn dive_eval::HostControl>,
    ) -> Result<dive_model::Model, String> {
        let host = Arc::new(dive_stdlib::host::RunHost::new(
            dive_stdlib::random::Streams::new(seed, mode),
            None,
        ));
        // Core compiles its plan from this model, so in the default profile
        // it evaluates the pure constant definitions its roles use only as
        // values, as `dive check` does (issue #1142).
        let model = on_big_stack(|| {
            if mode == dive_stdlib::random::RandomMode::Dive {
                self.recipe
                    .bind_next_fallible_for_core(host.clone(), Some(control))
            } else {
                self.recipe.bind_next_fallible(host.clone(), Some(control))
            }
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        if let Some(config) = self.model.evaluator.host().and_then(|host| host.config()) {
            model.evaluator.intern_checker_names();
            model.evaluator.intern_bfs_config(&config);
            host.set_config(Some(config));
        }
        Ok(model)
    }

    /// The configuration's model value names.
    #[must_use]
    pub fn model_values(&self) -> &[String] {
        &self.model_values
    }

    /// Validates a trace document (either format). A TLC document needs its
    /// ending set ([`TraceDoc::with_ending`]).
    #[must_use]
    pub fn validate(&self, doc: &TraceDoc) -> Report {
        self.read(doc).0
    }

    /// Replays a finite mode's native word on a fresh source-bound runtime
    /// under the caller's existing run control.
    pub(crate) fn validate_mode(
        &self,
        doc: &TraceDoc,
        control: Arc<dyn dive_eval::HostControl>,
    ) -> Result<Report, ValidationRunError> {
        on_big_stack(|| {
            self.session
                .read_controlled(doc, control)
                .map(|(report, _)| report)
        })
        .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// [`Self::validate_mode`] of a finite word whose states `states`
    /// produces one at a time (`dive_trace::stream`).
    pub(crate) fn validate_source(
        &self,
        header: &TraceDoc,
        states: &dyn dive_trace::stream::StateSource,
        control: Arc<dyn dive_eval::HostControl>,
    ) -> Result<Report, ValidationRunError> {
        on_big_stack(|| self.session.read_controlled_source(header, states, control))
            .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Validates a simulation word whose states `states` produces one at a
    /// time (`dive_trace::stream`), so the word is never held as JSON. A
    /// behavior is checked as a finite sample, a simulation deadlock as a
    /// state with no successor inside the model's constraints, and a
    /// violation by the ordinary finite check.
    pub(crate) fn validate_sample_source(
        &self,
        header: &TraceDoc,
        states: &dyn dive_trace::stream::StateSource,
        control: Arc<dyn dive_eval::HostControl>,
    ) -> Result<Report, ValidationRunError> {
        on_big_stack(|| {
            self.session
                .read_sampled_source(header, states, Some(control))
        })
        .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Independently checks every finite sampled row, including configured
    /// constraints, invariants and implied finite properties, on fresh source.
    /// Unlike ordinary behavior validity, this operation rejects a prefix that
    /// passed a deciding safety violation. It proves no exhaustive result.
    /// A `deadlock` ending is checked as a simulation deadlock: no successor
    /// of the last state satisfies every constraint.
    ///
    /// ```no_run
    /// # fn sample(source: &dive_api::trace::SpecModel,
    /// # word: &dive_api::trace::TraceDoc)
    /// # -> Result<(), dive_api::trace::ValidationRunError> {
    /// let report = source.try_validate_sample(word)?;
    /// # let _ = report; Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Typed interruption, resource exhaustion or isolated validation failure.
    pub fn try_validate_sample(&self, doc: &TraceDoc) -> Result<Report, ValidationRunError> {
        on_big_stack(|| {
            if doc.ending == Some(Ending::Deadlock) {
                self.session.read_sampled_deadlock(doc, None)
            } else {
                self.session.read_sampled(doc, None)
            }
            .map(|(report, _)| report)
        })
        .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Validates a trace document and also returns the trace as read (see
    /// [`dive_trace::validate_doc`]).
    #[must_use]
    pub fn read(&self, doc: &TraceDoc) -> (Report, Option<Trace>) {
        on_big_stack(|| {
            let (report, plain) = self.session.read(doc);
            let trace = plain.and_then(|doc| Trace::decode(&self.model, &doc).ok());
            (report, trace)
        })
        .unwrap_or_else(|e| (thread_failed(&e, doc.states.len()), None))
    }

    /// Validates with resource/execution exceptions outside the report.
    /// Endpoint documents require this fallible entrypoint; no resource failure
    /// is relabeled as a completed unsupported validation result.
    ///
    /// ```no_run
    /// # fn check(model: &dive_api::trace::SpecModel, doc: &dive_api::trace::TraceDoc)
    /// # -> Result<(), dive_api::trace::ValidationRunError> {
    /// let report = model.try_validate(doc)?;
    /// # let _ = report; Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Resource exhaustion, typed interruption, control conflict or isolated
    /// thread failure; in each case there is no completed validation report.
    pub fn try_validate(&self, doc: &TraceDoc) -> Result<Report, ValidationRunError> {
        self.try_read(doc).map(|(report, _)| report)
    }

    /// Reads through fresh private source reconstruction with fallible transport.
    /// Other document forms retain their existing reading semantics.
    ///
    /// # Errors
    /// As [`Self::try_validate`].
    pub fn try_read(&self, doc: &TraceDoc) -> Result<(Report, Option<Trace>), ValidationRunError> {
        if !matches!(
            doc.ending,
            Some(
                Ending::TemporalStateError { .. }
                    | Ending::TemporalSourceError { .. }
                    | Ending::NextStateError { .. }
                    | Ending::ViewSourceError { .. }
                    | Ending::CandidateStateError { .. }
                    | Ending::PartialNext { .. }
                    | Ending::PartialInit { .. }
            )
        ) {
            return Ok(self.read(doc));
        }
        on_big_stack(|| {
            let (report, plain) = self.session.try_read(doc)?;
            let trace = plain.and_then(|doc| Trace::decode(&self.model, &doc).ok());
            Ok((report, trace))
        })
        .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Validates a named invariant-error word and separately reconstructs its
    /// actual first-Init diagnostic classification on a fresh private runtime.
    /// A valid word alone does not grant a deciding-wrapper certificate.
    ///
    /// ```no_run
    /// # fn check(model: &dive_api::trace::SpecModel, doc: &dive_api::trace::TraceDoc)
    /// # -> Result<(), dive_api::trace::ValidationRunError> {
    /// let (word, class) = model.initial_invariant_class(doc, None)?;
    /// if let Some(class) = class {
    ///     assert_eq!(class.deciding_code(), 2147);
    /// }
    /// # let _ = word; Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Resource exhaustion, actual host interruption, control conflict or
    /// isolated thread failure; no completed class proof is returned then.
    pub fn initial_invariant_class(
        &self,
        doc: &TraceDoc,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<dive_trace::InitialInvariantClass>), ValidationRunError> {
        on_big_stack(|| self.session.initial_invariant_class(doc, control))
            .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Validates a named successor invariant error and separately executes its
    /// bounded Init/source admission and diagnostic lifecycle on a fresh runtime.
    /// Ordinary word validity alone grants no diagnostic identity.
    ///
    /// # Errors
    /// Actual interruption, resource/internal failure, control or thread failure.
    pub fn invariant_error_class(
        &self,
        doc: &TraceDoc,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<dive_trace::InvariantErrorClass>), ValidationRunError> {
        on_big_stack(|| self.session.invariant_error_class(doc, control))
            .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Independently reconstructs the actual first initial error stage on a
    /// fresh source-bound runtime. Header fields do not select this route.
    /// An invariant-receiver stage still needs its auxiliary word and separate
    /// [`Self::initial_invariant_class`] proof before a wrapper is established.
    ///
    /// # Errors
    /// Actual interruption, resource, binding, control or isolated-thread failure.
    pub fn initial_error_stage(
        &self,
        control: Option<Arc<dyn dive_eval::HostControl>>,
    ) -> Result<(Report, Option<dive_trace::InitialErrorStage>), ValidationRunError> {
        on_big_stack(|| self.session.initial_error_stage(control))
            .map_err(|error| ValidationRunError::ThreadFailure(error.to_string()))?
    }

    /// Validates a trace already read against this model.
    #[must_use]
    pub fn validate_trace(&self, trace: &Trace) -> Report {
        on_big_stack(|| {
            // Preserve the typed-input ownership guard before transport by text.
            for state in &trace.states {
                for value in state.iter().flatten() {
                    if let Err(error) = value.validate_names(&self.model.names) {
                        return thread_failed(
                            &ApiError::Unsupported {
                                feature: error.to_string(),
                            },
                            trace.states.len(),
                        );
                    }
                }
            }
            match trace.to_doc(&self.model) {
                Ok(doc) => self.session.read(&doc).0,
                Err(message) => thread_failed(&ApiError::Rejected { message }, trace.states.len()),
            }
        })
        .unwrap_or_else(|e| thread_failed(&e, trace.states.len()))
    }

    /// The by-construction invalid mutants of a valid shortest trace
    /// ([`dive_trace::mutate`]): negative controls, without random redraws.
    #[must_use]
    pub fn mutants(&self, trace: &Trace) -> Vec<Mutant> {
        self.mutants_with_mode(trace, ControlMode::ShortestBfs)
    }

    /// The invalid-by-construction mutants supported by the known valid
    /// counterexample's explicit producer contract. This does not infer
    /// shortestness or explore alternative paths.
    #[must_use]
    pub fn mutants_with_mode(&self, trace: &Trace, mode: ControlMode) -> Vec<Mutant> {
        on_big_stack(|| dive_trace::mutate::mutants_with_mode(&self.model, trace, mode))
            .unwrap_or_default()
    }

    /// The trace in dive's format.
    ///
    /// # Errors
    /// If a value has no JSON form.
    pub fn to_doc(&self, trace: &Trace) -> Result<TraceDoc, ApiError> {
        trace
            .to_doc(&self.model)
            .map_err(|message| ApiError::Rejected { message })
    }
}

fn sema_options(opts: &TraceOptions) -> dive_sema::Options {
    dive_sema::Options {
        search: dive_sema::SearchPath {
            library_paths: opts.library_paths.clone(),
            after_standard: opts.module_paths.clone(),
        },
        ..dive_sema::Options::default()
    }
}

fn thread_failed(e: &ApiError, length: usize) -> Report {
    Report {
        validation: Validation::Unsupported(format!("internal error: {e}")),
        notes: Vec::new(),
        length,
    }
}

/// Runs `f` on a thread with the evaluator's recommended stack.
fn on_big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> Result<T, ApiError> {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, f)
            .ok()
            .and_then(|h| h.join().ok())
    })
    .ok_or_else(|| ApiError::Unsupported {
        feature: "internal error: the evaluation thread could not run or panicked".to_owned(),
    })
}

/// The JSON report of a validation, as `dive validate-trace --output json`
/// prints it.
#[must_use]
pub fn report_json(report: &Report) -> Value {
    let mut j = json!({
        "schema_version": TRACE_SCHEMA_VERSION,
        "command": "validate-trace",
        "result": report.validation.kind(),
        "length": report.length,
        "notes": report.notes,
    });
    match &report.validation {
        Validation::Invalid(f) => {
            j["failure"] = json!({"state": f.state, "check": f.check.name(), "detail": f.detail});
        }
        Validation::Unsupported(u) => j["unsupported"] = Value::from(u.as_str()),
        // Valid: the result kind says everything.
        _ => {}
    }
    j
}

/// The report in a human-readable form.
#[must_use]
pub fn render_report(report: &Report) -> String {
    use std::fmt::Write as _;
    let mut s = match &report.validation {
        Validation::Valid => format!("valid: the {}-state trace validates\n", report.length),
        Validation::Invalid(f) => format!(
            "invalid: state {} of {} (numbered from 1, as TLC prints traces), check `{}`: {}\n",
            f.state + 1,
            report.length,
            f.check.name(),
            f.detail
        ),
        Validation::Unsupported(u) => format!("unsupported: {u}\n"),
        other => format!("{}\n", other.kind()),
    };
    for n in &report.notes {
        let _ = writeln!(s, "note: {n}");
    }
    s
}

/// The result of running the validator's controls on one counterexample
/// known to be valid (TLC's, or dive's): the positive control, and every
/// by-construction mutant.
#[derive(Debug, Clone)]
pub struct Controls {
    /// The original trace's report.
    pub positive: Report,
    /// The trace as read, written in dive's format and validated again: it
    /// must agree with the positive control.
    pub dive_format: Option<Validation>,
    /// Each mutant's name and verdict. Every one must be invalid.
    pub mutants: Vec<(String, Validation)>,
}

impl Controls {
    /// Whether the positive control validated: `valid`, or, for a deadlock,
    /// every check but the unverifiable claim.
    #[must_use]
    pub fn positive_ok(&self) -> bool {
        match &self.positive.validation {
            Validation::Valid => true,
            Validation::Unsupported(u) => u == dive_trace::DEADLOCK_UNSUPPORTED,
            _ => false,
        }
    }

    /// Every failed control, described.
    #[must_use]
    pub fn failures(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.positive_ok() {
            out.push(format!(
                "positive control: {}",
                render_report(&self.positive).trim_end()
            ));
        }
        if let Some(v) = &self.dive_format
            && *v != self.positive.validation
        {
            out.push(format!(
                "the trace in dive's format gives {}, not {}",
                v.kind(),
                self.positive.validation.kind()
            ));
        }
        if self.mutants.is_empty() {
            out.push("no negative controls generated".to_owned());
        }
        for (name, v) in &self.mutants {
            if !matches!(v, Validation::Invalid(_)) {
                out.push(format!("mutant {name} is not rejected: {}", v.kind()));
            }
        }
        out
    }
}

impl SpecModel {
    /// Runs the controls on a known valid shortest BFS counterexample.
    ///
    /// Compatibility wrapper for [`Self::controls_with_mode`] with
    /// [`ControlMode::ShortestBfs`]. Other producers must select their mode
    /// explicitly.
    #[must_use]
    pub fn controls(&self, doc: &TraceDoc) -> Controls {
        self.controls_with_mode(doc, ControlMode::ShortestBfs)
    }

    /// Runs positive, document round-trip and negative controls using the
    /// known valid counterexample's explicit producer contract. Every
    /// generated mutant must be invalid both directly and through dive's
    /// format; an empty negative suite receives no coverage credit.
    ///
    /// ```
    /// use dive_api::trace::{ControlMode, Ending, SpecModel, TraceDoc, TraceOptions};
    /// use serde_json::json;
    ///
    /// let model = SpecModel::from_source("Loop.tla",
    ///     "---- MODULE Loop ----\nVARIABLE x\nInit == x=0\nNext == x'=0 \\/ x'=1\nInv == x=0\n====\n",
    ///     "INIT Init\nNEXT Next\nINVARIANT Inv\n", &TraceOptions::default())?;
    /// let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":0}), json!({"x":1})])?
    ///     .with_ending(Ending::Invariant("Inv".to_owned()));
    /// let controls = model.controls_with_mode(&doc, ControlMode::Counterexample);
    /// assert!(controls.failures().is_empty());
    /// let (_, trace) = model.read(&doc);
    /// let mutants = model.mutants_with_mode(&trace.ok_or("trace missing")?, ControlMode::Counterexample);
    /// assert_eq!(mutants.len(), 3);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn controls_with_mode(&self, doc: &TraceDoc, mode: ControlMode) -> Controls {
        let (positive, trace) = self.read(doc);
        let mut mutants = Vec::new();
        let mut dive_format = None;
        if let Some(trace) = trace.filter(|_| {
            matches!(
                positive.validation,
                Validation::Valid | Validation::Unsupported(_)
            )
        }) {
            dive_format = Some(match self.to_doc(&trace) {
                Ok(d) => self.validate(&d).validation,
                Err(e) => Validation::Unsupported(e.to_string()),
            });
            for m in self.mutants_with_mode(&trace, mode) {
                let direct = self.validate_trace(&m.trace).validation;
                let via_format = match self.to_doc(&m.trace) {
                    Ok(d) => self.validate(&d).validation,
                    Err(e) => Validation::Unsupported(e.to_string()),
                };
                let v = if matches!(direct, Validation::Invalid(_)) {
                    via_format
                } else {
                    direct
                };
                mutants.push((m.name, v));
            }
        }
        Controls {
            positive,
            dive_format,
            mutants,
        }
    }
}
