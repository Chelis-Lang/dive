//! `dive-check`: explicit-state safety model checking (ADR 0015).
//!
//! [`check`] binds a lowered specification to its TLC model configuration
//! through `dive-model` (ADR 0013 §8) and runs TLC's breadth-first search,
//! reproducing TLC's single-worker run exactly under `--compat=tlc`: the
//! same verdict and violated property, the same states generated, distinct
//! and left on the queue, the same depth, and the same shortest
//! counterexample with TLC's action labels.
//!
//! # What it checks
//!
//! - initial states and the next-state relation, through `dive-eval`'s
//!   successor generator, duplicates included;
//! - invariants on every new (or out-of-model) state, implied initial
//!   predicates on initial states, implied actions (`[][A]_v`) on every step
//!   including steps into seen states;
//! - `CONSTRAINT` and `ACTION_CONSTRAINT` (a constrained successor is still
//!   checked against the invariants, and prevents deadlock);
//! - deadlock: a state with no successor generated at all;
//! - pure safety symmetry, canonicalizing keys while retaining concrete
//!   explored states and independently checkable traces;
//! - `VIEW`, through the canonicalization hook (states are identified by the
//!   view's value; counterexamples show the concrete states explored);
//! - `ASSUME`s before the search, `POSTCONDITION`s after a complete one.
//!
//! The search stops at the first event in TLC's order (ADR 0015 §1) and
//! reports ADR 0015 §3's counts at that event; see [`mod@engine`].
//!
//! # Stores and profiles
//!
//! Fingerprint mode is the default. Under `--compat=tlc` it keys on TLC's own
//! state fingerprints (`StateFingerprinter::tlc`, TLC's variable order); the
//! default profile keys on ADR 0015 §5's cheaper two-level fingerprints
//! (`StateFingerprinter::new`), which reuse cached value fingerprints (see
//! [`CheckOptions::tlc_fingerprints`]). Exact mode (`--store exact`, D-007)
//! keys on canonical state encodings through the same engine.
//!
//! # What it refuses
//!
//! Anything not reproduced exactly is `unsupported`, naming the feature:
//! whatever `dive-model` lists in `Model::unsupported` (temporal properties,
//! unprepared symmetry, operators dive does not implement such as `IOUtils`),
//! and whatever the evaluator refuses during the search. Errors whose TLC
//! error class is not reproduced yet are reported as errors with an
//! unverified class ([`TlcClass::verified`]).
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_check::{CheckOptions, Verdict, check};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
//!            Next == x' = (x + 1) % 4\nInv == x < 3\n====\n";
//! let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
//! let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
//! let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n").into_config().expect("cfg");
//! let r = check(&ir, &cfg, &CheckOptions::tlc(0, 0));
//! assert_eq!(r.verdict, Verdict::InvariantViolated { property: "Inv".into(), initial: false });
//! let stats = r.stats.expect("statistics");
//! assert_eq!((stats.generated, stats.distinct, stats.left_on_queue, stats.depth), (4, 4, 0, 4));
//! assert_eq!(r.counterexample.expect("a trace").states.len(), 4);
//! ```
#![forbid(unsafe_code)]

// The run contract shared with Core and `dive-liveness` lives in
// `dive-runtime` (ADR 0155); it is re-exported at its historical paths.
pub(crate) use dive_runtime::class;
pub use dive_runtime::{budget, classify};
pub mod capture;
pub mod checkpoint;
pub mod dfid;
pub mod engine;
pub mod final_roles;
mod frontier;
pub use frontier::FrontierUsage;
mod host;
mod parallel;
mod pool;
mod seen;
pub use seen::{SeenFailure, SeenOptions, SeenPhase, SeenReplay, SeenUsage};

use std::sync::Arc;
use std::time::Instant;

use dive_config::ModelConfig;
use dive_eval::Profile;
use dive_ir::Ir;
use dive_model::{BindError, BindOptions};

pub(crate) use dive_runtime::admission::effectful;
pub use dive_runtime::{CheckOptions, format_trace_state, render_human};
pub use dive_runtime::{
    CheckResult, Collision, Counterexample, DiagnosticWitness, ErrorSite, PartialState,
    PredecessorUsage, Stats, TlcClass, TraceState, Verdict, Workers, tlc_exit_status,
};
pub(crate) use dive_runtime::{bare, context_verdict, messages, note_canonical_stop};

/// Checks the model `cfg` of the lowered specification `ir`.
///
/// Runs on its own thread with the evaluator's recommended stack.
#[must_use]
pub fn check(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> CheckResult {
    if opts.store_compact_bytes.is_some() {
        return bare(
            Verdict::Unsupported {
                feature: "Store reclamation requires the explicit Core engine".into(),
            },
            opts,
            ir,
        );
    }
    if let Err(error) = dive_eval::validate_ir(ir) {
        return bare(context_verdict(&error), opts, ir);
    }
    if opts.candidate_memory_bytes.is_some() || opts.candidate_directory.is_some() {
        return bare(
            Verdict::Unsupported {
                feature: "candidate spill requires the explicit Core engine".into(),
            },
            opts,
            ir,
        );
    }
    if opts.predecessor_directory.is_some() && opts.predecessor_memory_bytes.is_none() {
        return bare(
            Verdict::Unsupported {
                feature: "predecessor directory requires predecessor memory budget".into(),
            },
            opts,
            ir,
        );
    }
    let started = Instant::now();
    let (mut watchdog, armed) = match budget::arm(opts) {
        Ok(armed) => armed,
        Err(error) => {
            return bare(
                match error {
                    error @ budget::StartError::Thread(_) => Verdict::InternalError {
                        message: error.to_string(),
                    },
                    budget::StartError::Measurement(error) => Verdict::BudgetMeasurement { error },
                },
                opts,
                ir,
            );
        }
    };
    let opts = armed.as_ref();
    match opts.budget.poll() {
        Ok(Some(exhausted)) => {
            return bare(
                Verdict::Stopped(Box::new(exhausted.during(budget::Phase::Startup))),
                opts,
                ir,
            );
        }
        Err(error) => return bare(Verdict::BudgetMeasurement { error }, opts, ir),
        Ok(None) => {}
    }
    let run = || run_check(ir, cfg, opts);
    let outcome = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, run)
            .map_err(|e| format!("cannot start the checking thread: {e}"))
            .and_then(|h| h.join().map_err(|_| "the checker panicked".to_owned()))
    });
    let mut result = match outcome {
        Ok(r) => r,
        Err(message) => bare(Verdict::InternalError { message }, opts, ir),
    };
    if let Some(watchdog) = &mut watchdog {
        budget::finish_result(&mut result, watchdog.finish(), budget::Phase::Finalization);
    }
    result.elapsed = started.elapsed();
    result
}

/// The verdict for an error of the binding stage.
pub(crate) fn model_error(e: &BindError) -> Verdict {
    Verdict::from(e)
}

#[allow(clippy::too_many_lines, reason = "the phases of a run, in TLC's order")]
fn run_check(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> CheckResult {
    run_check_with_checkpoint(ir, cfg, opts, None)
}

#[allow(clippy::too_many_lines, reason = "the phases of a run, in TLC order")]
pub(crate) fn run_check_with_checkpoint(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    checkpoint: Option<&mut checkpoint::Session>,
) -> CheckResult {
    if let Some(disk) = &opts.seen_disk {
        if let Err(error) = disk.limits.validate() {
            return bare(seen::verdict(&error), opts, ir);
        }
        if checkpoint.is_some() {
            return bare(Verdict::Unsupported { feature: "disk seen-state retention with native checkpoint/recovery is not implemented".into() }, opts, ir);
        }
    }
    // Refuse transformed random streams before binding can execute effects.
    // Ordinary VIEW and non-VIEW per-state RNG retain their existing routes.
    if let Some(feature) = dive_model::transformed_randomness(ir, cfg) {
        return bare(
            Verdict::Unsupported {
                feature: feature.into(),
            },
            opts,
            ir,
        );
    }
    let Ok(control) = budget::EvaluationControl::new(&opts.budget, budget::Phase::Binding) else {
        return bare(
            Verdict::InternalError {
                message: "binding control identity".into(),
            },
            opts,
            ir,
        );
    };
    let bound = bind_for_run_controlled(
        ir,
        cfg,
        opts,
        checkpoint.as_ref().and_then(|s| s.names()),
        control.clone(),
    );
    let (m, host) = match bound {
        Ok(bound) => bound,
        Err(f) => return controlled_binding_failure(ir, opts, &f, &control),
    };
    finish_bound_check(ir, cfg, opts, m, host, checkpoint)
}

fn binding_failure(
    ir: &Arc<Ir>,
    opts: &CheckOptions,
    failure: &dive_model::Failure,
) -> CheckResult {
    let mut result = bare(model_error(&failure.error), opts, ir);
    result.binding_model_error = matches!(result.verdict, Verdict::ModelError { .. });
    messages(&failure.output, &mut result);
    result
}

fn controlled_binding_failure(
    ir: &Arc<Ir>,
    opts: &CheckOptions,
    failure: &dive_model::ControlledBindError,
    control: &budget::EvaluationControl,
) -> CheckResult {
    if let dive_model::ControlledBindError::Binding(failure) = failure {
        return binding_failure(ir, opts, failure);
    }
    let verdict = match failure {
        dive_model::ControlledBindError::Interrupted { error, .. } => control
            .verdict(error)
            .unwrap_or_else(|| Verdict::InternalError {
                message: "binding interruption lacks matching active resource authority".into(),
            }),
        _ => Verdict::InternalError {
            message: "model binding control ownership conflict".into(),
        },
    };
    let mut result = bare(verdict, opts, ir);
    messages(failure.output(), &mut result);
    result
}

fn bind_for_run_controlled(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    restored_names: Option<&[String]>,
    control: Arc<budget::EvaluationControl>,
) -> Result<(dive_model::Model, Arc<dive_stdlib::host::RunHost>), dive_model::ControlledBindError> {
    let host = Arc::new(engine::run_host(opts));
    let bind_opts = BindOptions {
        eval: opts.eval,
        symmetry_liveness: opts.symmetry_liveness,
        no_deadlock: opts.no_deadlock,
        host: Some(Arc::new(host::EngineHost::new(host.clone()))),
    };
    let bound = if let Some(coverage) = &opts.source_coverage {
        if restored_names.is_some() {
            return Err(Box::new(dive_model::Failure {
                error: dive_model::BindError::unsupported(
                    "source coverage with recovery is not implemented",
                ),
                output: Vec::new(),
            })
            .into());
        }
        dive_model::bind_controlled_source_covered(
            ir.clone(),
            cfg,
            &bind_opts,
            control,
            coverage.clone(),
        )
    } else if let Some(names) = restored_names {
        dive_model::bind_with_names_controlled(ir.clone(), cfg, &bind_opts, names, control)
    } else {
        dive_model::bind_controlled(ir.clone(), cfg, &bind_opts, control)
    };
    finish_input_binding(bound, opts).map(|model| (model, host))
}

fn bind_for_run(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    restored_names: Option<&[String]>,
) -> Result<(dive_model::Model, Arc<dive_stdlib::host::RunHost>), Box<dive_model::Failure>> {
    let host = Arc::new(engine::run_host(opts));
    let bind_opts = BindOptions {
        eval: opts.eval,
        symmetry_liveness: opts.symmetry_liveness,
        no_deadlock: opts.no_deadlock,
        host: Some(Arc::new(host::EngineHost::new(host.clone()))),
    };
    let bound = if let Some(coverage) = &opts.source_coverage {
        if restored_names.is_some() {
            return Err(Box::new(dive_model::Failure {
                error: dive_model::BindError::unsupported(
                    "source coverage with recovery is not implemented",
                ),
                output: Vec::new(),
            }));
        }
        dive_model::bind_source_covered(ir.clone(), cfg, &bind_opts, coverage.clone())
    } else if let Some(names) = restored_names {
        dive_model::bind_with_names(ir.clone(), cfg, &bind_opts, names)
    } else {
        dive_model::bind(ir.clone(), cfg, &bind_opts)
    };
    finish_input_binding(bound, opts).map(|model| (model, host))
}

fn finish_input_binding<E: From<Box<dive_model::Failure>>>(
    bound: Result<dive_model::Model, E>,
    opts: &CheckOptions,
) -> Result<dive_model::Model, E> {
    let bound = bound.and_then(|model| {
        if let Some(feature) = model.evaluator.external_input_refusal() {
            Err(Box::new(dive_model::Failure {
                error: dive_model::BindError::unsupported(feature),
                output: model.output,
            })
            .into())
        } else {
            Ok(model)
        }
    });
    let closed = opts.inputs.as_ref().map(|inputs| inputs.finish_binding());
    match (bound, closed) {
        (Ok(model), Some(Err(error))) => Err(Box::new(dive_model::Failure {
            error: dive_model::BindError::unsupported(error.message),
            output: model.output,
        })
        .into()),
        (bound, _) => bound,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the unchanged post-binding phases in TLC order"
)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "owns the original startup host through every post-binding return"
)]
fn finish_bound_check(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    mut m: dive_model::Model,
    host: Arc<dive_stdlib::host::RunHost>,
    mut checkpoint: Option<&mut checkpoint::Session>,
) -> CheckResult {
    let mut notes = Vec::new();
    let mut r = bare(Verdict::Success, opts, ir);
    r.symmetry_liveness = Some(m.symmetry_liveness().clone());
    messages(&m.output, &mut r);
    if m.symmetry.is_some() && m.implied_temporals.is_empty() {
        if engine::uses_random(ir) {
            r.verdict = Verdict::Unsupported {
                feature: "randomness under SYMMETRY is not supported".into(),
            };
            return r;
        }
        let mode = if opts.profile == Profile::Tlc {
            dive_model::symmetry::Mode::Tlc
        } else {
            dive_model::symmetry::Mode::Checked
        };
        match m.prepare_symmetry(cfg, mode, dive_model::symmetry::Limits::default()) {
            Ok(Some(prepared)) => {
                messages(&prepared.warnings, &mut r);
                m.symmetry_reduction = prepared.reduction;
                let symmetry = format!("SYMMETRY {}", m.symmetry.as_deref().unwrap_or_default());
                m.unsupported
                    .retain(|unsupported| unsupported.feature != symmetry);
                if opts.profile == Profile::Dive {
                    notes.push("The default profile canonicalizes symmetry keys in canonical value order (D-010). A non-invariant VIEW or asymmetric specification may differ under --compat=tlc; order relevance is not yet detected.".into());
                }
            }
            Ok(None) => {}
            Err(error) => {
                r.verdict = model_error(&error);
                r.binding_model_error = matches!(r.verdict, Verdict::ModelError { .. });
                return r;
            }
        }
    }
    if let Some(u) = m.unsupported.first() {
        r.verdict = Verdict::Unsupported {
            feature: u.feature.clone(),
        };
        r.notes = notes;
        return r;
    }
    // This source-bound Model has never published its evaluator. EngineHost's
    // actual native hook is the default Ok; keep the SAME startup run/state.
    // Persistent checkpoint/recovery retains callback execution.
    let _native_hooks = if checkpoint.is_none() {
        m.evaluator
            .native_hook_scope(dive_eval::NativeHooks::Unrestricted)
    } else {
        None
    };
    // TLC creates its model checker here: `TLCGet("config")` and the
    // checker's names exist from now on, and the counters read 0.
    m.evaluator.intern_checker_names();
    let config = engine::run_config(opts, m.settings.check_deadlock);
    m.evaluator.intern_bfs_config(&config);
    host.set_config(Some(config));
    engine::startup_counters(&host);
    if let Some(session) = checkpoint.as_mut()
        && let Err(verdict) = session.admit_model(&m)
    {
        r.verdict = verdict;
        return r;
    }
    let assumption_failure = budget::check_assumptions(&m, &opts.budget);
    r.output.extend(m.take_output());
    if let Some(verdict) = assumption_failure {
        r.verdict = verdict;
        r.notes = notes;
        return r;
    }
    let outcome = if let Some(session) = checkpoint.take() {
        engine::search(
            &m,
            opts,
            &host,
            engine::SearchHooks {
                checkpoint: Some(session),
                ..Default::default()
            },
        )
    } else {
        engine::search(&m, opts, &host, engine::SearchHooks::default())
    };
    r.seen = outcome.seen;
    r.frontier = outcome.frontier;
    r.predecessor = outcome.predecessor;
    r.verdict = outcome.verdict;
    r.stats = outcome.stats;
    r.tlc_prints_stats = outcome.tlc_prints_stats;
    r.counterexample = outcome.counterexample;
    if matches!(
        &r.verdict,
        Verdict::EvaluationError {
            site: ErrorSite::Invariant,
            code: 2147,
            reported: 2102,
            verified: true,
            invariant: Some(_),
            ..
        }
    ) {
        // TLC records this initial diagnostic state, not a behavior trace.
        // Preserve its original values separately without cloning or granting
        // a formal counterexample/independent-wrapper certificate.
        r.diagnostic_witness = r.counterexample.take().map(|context| DiagnosticWitness {
            states: context.states,
        });
    } else if matches!(
        &r.verdict,
        Verdict::EvaluationError {
            site: ErrorSite::Invariant,
            code: 2103,
            reported: 2102,
            verified: false,
            ..
        }
    ) {
        // The recorded initial context is neither a formal behavior word nor
        // ADR0133's separately proved receiver diagnostic. Do not serialize it
        // as an anonymous underlying error against an unproved wrapper.
        r.counterexample = None;
        notes.push("The initial invariant's diagnostic context has no proved deciding class or formal behavior word; its original cause/context, physical exit and stopping counts are retained.".into());
    }
    r.collision = outcome.collision;
    r.workers = outcome.workers;
    r.output.extend(outcome.output);
    notes.extend(outcome.notes);
    if r.counterexample.is_some()
        && let Some(alias) = &cfg.alias
    {
        notes.push(format!(
            "ALIAS {} is not applied: the counterexample shows the states themselves (not implemented yet).",
            alias.text
        ));
    }
    note_canonical_stop(opts, &r.verdict, &mut notes);
    r.notes = notes;
    r
}

#[cfg(test)]
mod binding_controls;
#[cfg(test)]
mod budget_controls;
