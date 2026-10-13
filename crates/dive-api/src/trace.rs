//! Trace validation: checking a trace against a specification (ADR 0016).
//!
//! A [`SpecModel`] is a specification bound to its TLC configuration. It
//! validates traces in dive's format ([`dive_trace::format`]) or TLC's
//! `-dumpTrace json`, with the independent validator of `dive-trace`, which
//! evaluates `Init` and the next-state relation as predicates over the given
//! states and never runs the model checker's successor generator.
//!
//! This is also how a Rust project checks its *own implementation's*
//! traces: record each state of a run as a JSON object from variable names
//! to values (in dive's JSON form, [`dive_trace::value_json`]), and validate
//! the sequence as a [`Ending::Behavior`]. The trace must assign every
//! variable in every state; partial traces need a search and are not
//! supported yet (ADR 0016 §4).
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

pub use dive_trace::mutate::Mutant;
pub use dive_trace::{
    Check, DEADLOCK_UNSUPPORTED, Ending, Failure, Report, State, Trace, TraceDoc, Validation,
};
use serde_json::{Value, json};

use crate::ApiError;

/// Version of the JSON document produced by [`report_json`].
pub const TRACE_SCHEMA_VERSION: u32 = 1;

/// How the specification is bound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceOptions {
    /// `--compat=tlc`: evaluate with every option at its TLC setting (32-bit
    /// integers, TLC's value order and comparisons). Otherwise dive's
    /// default profile.
    pub compat_tlc: bool,
    /// TLC's `-deadlock` flag: the model does not check for deadlock.
    pub no_deadlock: bool,
    /// The checking run's `-seed` and `-fp` index, when known (the TLC
    /// goldens use `-seed 0 -fp 0`): `TLCGet("config")` reports them, and
    /// constant definitions draw from the run's start-up stream (TLC's
    /// seeding under `compat_tlc`, dive's otherwise; D-002). Without them,
    /// both are refused by name. The validator refuses every random draw in
    /// the checks themselves (`dive_trace::RANDOM_UNSUPPORTED`).
    pub run: Option<(i64, usize)>,
    /// Modules searched before the standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Modules searched after the standard modules (TLC's classpath).
    pub module_paths: Vec<PathBuf>,
}

/// A specification bound to its configuration, ready to validate traces.
#[derive(Debug, Clone)]
pub struct SpecModel {
    model: dive_model::Model,
    model_values: Vec<String>,
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
        Self::bind(&shown, &analysis, parsed, opts)
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
        let host = streams.map(|s| Arc::new(dive_stdlib::host::RunHost::new(s, None)));
        let bind_opts = dive_model::BindOptions {
            no_deadlock: opts.no_deadlock,
            host: host.clone().map(|h| -> Arc<dyn dive_eval::Host> { h }),
            ..if opts.compat_tlc {
                dive_model::BindOptions::tlc()
            } else {
                dive_model::BindOptions::dive()
            }
        };
        let model_values = cfg.model_values.clone();
        let bound = on_big_stack(move || dive_model::bind(ir, &cfg, &bind_opts))?;
        if let (Ok(model), Some(h), Some((seed, fp_index))) = (&bound, &host, opts.run) {
            model.evaluator.intern_checker_names();
            #[allow(clippy::cast_possible_wrap, reason = "Java's long: same bits")]
            let fingerprint = dive_value::fp64::POLYS.get(fp_index).copied().unwrap_or(0) as i64;
            let config = dive_eval::RunConfig {
                deadlock: model.settings.check_deadlock,
                workers: 1,
                seed,
                fingerprint,
                install: std::env::current_exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            };
            model.evaluator.intern_bfs_config(&config);
            h.set_config(Some(config));
        }
        match bound {
            Ok(model) => Ok(Self {
                model,
                model_values,
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

    /// Validates a trace document and also returns the trace as read (see
    /// [`dive_trace::validate_doc`]).
    #[must_use]
    pub fn read(&self, doc: &TraceDoc) -> (Report, Option<Trace>) {
        on_big_stack(|| dive_trace::validate_doc(&self.model, doc, &self.model_values))
            .unwrap_or_else(|e| (thread_failed(&e, doc.states.len()), None))
    }

    /// Validates a trace already read against this model.
    #[must_use]
    pub fn validate_trace(&self, trace: &Trace) -> Report {
        on_big_stack(|| dive_trace::validate(&self.model, trace))
            .unwrap_or_else(|e| thread_failed(&e, trace.states.len()))
    }

    /// The by-construction invalid mutants of a valid shortest trace
    /// ([`dive_trace::mutate`]): negative controls, without random redraws.
    #[must_use]
    pub fn mutants(&self, trace: &Trace) -> Vec<Mutant> {
        on_big_stack(|| dive_trace::mutate::mutants(&self.model, trace)).unwrap_or_default()
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
        Validation::Valid => {}
        Validation::Invalid(f) => {
            j["failure"] = json!({"state": f.state, "check": f.check.name(), "detail": f.detail});
        }
        Validation::Unsupported(u) => j["unsupported"] = Value::from(u.as_str()),
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
    };
    for n in &report.notes {
        let _ = writeln!(s, "note: {n}");
    }
    s
}

/// The result of running the validator's controls on one counterexample
/// known to be valid (TLC's, or dive's): the positive control, and every
/// by-construction mutant (ADR 0016 §3).
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
            Validation::Invalid(_) => false,
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
        for (name, v) in &self.mutants {
            if !matches!(v, Validation::Invalid(_)) {
                out.push(format!("mutant {name} is not rejected: {}", v.kind()));
            }
        }
        out
    }
}

impl SpecModel {
    /// Runs the controls on `doc`, a counterexample known to be valid: the
    /// document must validate, and each mutant of the trace it reads as
    /// must be rejected. Mutants are also round-tripped through dive's
    /// format, so the format is exercised as well.
    #[must_use]
    pub fn controls(&self, doc: &TraceDoc) -> Controls {
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
            for m in self.mutants(&trace) {
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
