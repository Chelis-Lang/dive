//! Bounded behavior-graph capture through the reference safety engine.
//!
//! A complete graph is exploration evidence, not a temporal verdict. This
//! capture path uses the shared ordered safety engines. It retains every accepted
//! generated edge, including duplicates, and adds explicit implicit stutters.
//! The graph, evidence and admission types live in `dive-runtime`
//! (ADR 0155) and are re-exported here; this module holds the drivers.

use std::sync::Arc;

use dive_model::Model;

use crate::engine;
use crate::{CheckOptions, Verdict};

pub use dive_runtime::capture::*;

mod checkpoints;
pub use checkpoints::{explore_checkpoints_with_evidence, explore_checkpoints_with_native_hooks};

/// Explores a freshly bound model through the shared safety engine. The
/// model must use the same evaluator options; callers must not concurrently
/// evaluate it or its clones. This installs the run host, checks assumptions,
/// and includes the model's recorded startup output.
///
/// Temporal property refusals are deferred explicitly; every other refusal
/// is retained. VIEW, active quotient symmetry, randomness and stateful checker
/// reads remain unsupported; D-005 may select full concrete identities instead. Capture uses ordered, bounded source waves with the requested
/// pool width, or the explicitly requested sequential mode. The ordinary
/// safety checker is unchanged.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{capture::{explore, Exploration, Limits}, CheckOptions};
/// let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().expect("config");
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let Exploration::Complete(c) = explore(&model, &CheckOptions::tlc(0, 0), Limits::default()) else {
///     panic!("the supported finite model must explore completely");
/// };
/// assert_eq!(c.graph.states.len(), 1);
/// assert_eq!(c.graph.edges[0].len(), 2); // generated step and implicit stutter
/// assert_eq!(c.options.seed, 0);
/// ```
#[must_use]
pub fn explore(model: &Model, opts: &CheckOptions, limits: Limits) -> Exploration {
    let started = std::time::Instant::now();
    explore_with_evidence(model, opts, limits).unwrap_or_else(|message| {
        let mut result = stopped(bug(&message), model, opts);
        if let Exploration::Stopped(stop) = &mut result {
            stop.elapsed = started.elapsed();
        }
        result
    })
}

/// Like [`explore`], distinguishing a thread failure that lost execution
/// evidence from an observed safety stop. Unknown work must not be reported
/// as a proved zero-worker search.
///
/// # Errors
/// The exploration thread failed to start or panicked; actual work is unknown.
pub fn explore_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Exploration, String> {
    explore_with_native_hooks(model, opts, limits, dive_eval::NativeHooks::Callback)
}

/// Explicit operative native-hook selection for an owning driver.
///
/// Legacy capture APIs always use callbacks. This selects execution policy only;
/// it grants no private model ownership, purity or permission to skip work.
///
/// # Errors
/// The existing exploration thread failure; execution evidence may be lost.
pub fn explore_with_native_hooks(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    native_hooks: dive_eval::NativeHooks,
) -> Result<Exploration, String> {
    let started = std::time::Instant::now();
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    let run = || explore_inner(model, opts, limits, false, None, None, native_hooks).legacy();
    let mut result = run_on_thread(run)?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Capture accepted concrete successors through the actual deterministic
/// safety worker pool, preserving original seen-state values before replacement.
/// This supplies exploration evidence only; it evaluates no temporal predicates.
///
/// Per-level speculative quotas divide the remaining entry/generation bounds by
/// source count, independently of worker width. These conservative named bounds
/// may refuse a skewed level that fits the inline capture's global budget.
/// VIEW, active quotient symmetry, randomness and checker-state reads remain
/// named refusals. Whole-run D-005 unreduced symmetry uses concrete identities.
/// The staged pool has a fixed structural maximum of 64 workers.
///
/// Captured operator/function/lazy-set values are refused by a conservative
/// no-evaluation scoped proof; no provider is forced to guess purity.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{capture, CheckOptions};
/// let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n";
/// let a = dive_sema::analyze_source("M.tla", source, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().expect("config");
/// let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let options = CheckOptions { workers: 4, ..CheckOptions::tlc(0, 0) };
/// let result = capture::explore_concrete_parallel_with_evidence(&m, &options, Default::default())?;
/// let capture::Exploration::Complete(c) = result else { return Err("incomplete".into()) };
/// assert_eq!(c.workers.used, 4);
/// assert_eq!(c.graph.concrete[0].len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// A worker or exploration thread fails and loses execution evidence.
pub fn explore_concrete_parallel_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Exploration, String> {
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    if opts.workers > 64 {
        let mut result = crate::bare(
            limit("concrete pool worker width exceeds 64"),
            opts,
            &model.ir,
        );
        result.workers.requested = opts.workers;
        return Ok(Exploration::Stopped(Box::new(result)));
    }
    if opts.workers == 0 {
        let mut result = crate::bare(
            Verdict::Unsupported {
                feature: "concrete parallel capture worker width is zero".into(),
            },
            opts,
            &model.ir,
        );
        result.workers.requested = 0;
        return Ok(Exploration::Stopped(Box::new(result)));
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| {
        explore_inner(
            model,
            opts,
            limits,
            true,
            None,
            None,
            dive_eval::NativeHooks::Callback,
        )
        .legacy()
    })?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Capture and observe successful initialization and completed levels at one worker.
/// Both the inline and ordered pool executors deliver the same events. Failure
/// stops exploration without exposing a completed graph; no temporal verdict is implied.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{capture::{self, BehaviorGraph, Observer}, CheckOptions, Stats};
/// #[derive(Default)]
/// struct Levels(usize);
/// impl Observer for Levels {
///     fn level_done(&mut self, _: u64, _: &BehaviorGraph, _: Stats) -> Result<(), String> {
///         self.0 += 1;
///         Ok(())
///     }
/// }
/// let analysis = dive_sema::analyze_source("M.tla",
///     "---- MODULE M ----\nVARIABLE x\nInit == x=TRUE\nNext == x'=x\n====\n",
///     &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().expect("config");
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let mut levels = Levels::default();
/// let result = capture::explore_observed_with_evidence(
///     &model, &CheckOptions::tlc(0, 7), Default::default(), &mut levels)?;
/// assert!(matches!(result, capture::Exploration::Complete(_)));
/// assert_eq!(levels.0, 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// An exploration thread or worker fails, losing actual execution evidence.
pub fn explore_observed_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
) -> Result<Exploration, String> {
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    if opts.workers != 1 {
        return Ok(stopped(
            Verdict::Unsupported {
                feature: "level observation currently requires one worker".into(),
            },
            model,
            opts,
        ));
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| {
        explore_inner(
            model,
            opts,
            limits,
            false,
            Some(observer),
            None,
            dive_eval::NativeHooks::Callback,
        )
        .legacy()
    })?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Captures with completed-level prefix control at the requested worker width.
/// Every callback follows joined, committed safety work. A prefix is never
/// complete; callback errors retain the existing safety stop behavior.
///
/// # Errors
/// The dedicated exploration thread failed or panicked; work is unknown.
pub fn explore_controlled_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
) -> Result<ControlledExploration, String> {
    explore_controlled_with_native_hooks(
        model,
        opts,
        limits,
        observer,
        dive_eval::NativeHooks::Callback,
    )
}

/// Explicit operative native-hook selection for an owning driver.
///
/// Legacy capture APIs always use callbacks. This selects execution policy only;
/// it grants no private model ownership, purity or permission to skip work.
///
/// # Errors
/// The existing exploration thread failure; execution evidence may be lost.
pub fn explore_controlled_with_native_hooks(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
    native_hooks: dive_eval::NativeHooks,
) -> Result<ControlledExploration, String> {
    if let Some(stop) = context_stop(model, opts) {
        return Ok(stop.into());
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| {
        explore_inner(
            model,
            opts,
            limits,
            false,
            Some(observer),
            None,
            native_hooks,
        )
    })?;
    result.set_elapsed(started.elapsed());
    Ok(result)
}

fn run_on_thread<T: Send>(run: impl FnOnce() -> T + Send) -> Result<T, String> {
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, run)
        {
            Ok(thread) => thread
                .join()
                .map_err(|_| "exploration thread panicked".to_owned()),
            Err(e) => Err(format!("cannot start exploration thread: {e}")),
        }
    })
}

fn context_stop(model: &Model, opts: &CheckOptions) -> Option<Exploration> {
    if let Err(error) = model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
    {
        // No binding output belongs to a foreign arena/name table. Return a
        // checked no-work report before reading any model roles or starting host effects.
        return Some(Exploration::Stopped(Box::new(crate::bare(
            crate::context_verdict(&error),
            opts,
            &model.ir,
        ))));
    }
    if let Err(error) = model.validate_context() {
        let verdict = if error.class() == dive_eval::ErrorClass::ResourceLimit {
            Verdict::ResourceLimit {
                message: error.to_string(),
            }
        } else {
            Verdict::Unsupported {
                feature: error.to_string(),
            }
        };
        return Some(Exploration::Stopped(Box::new(crate::bare(
            verdict, opts, &model.ir,
        ))));
    }
    if opts.candidate_memory_bytes.is_some() || opts.candidate_directory.is_some() {
        return Some(stopped(
            Verdict::Unsupported {
                feature: "candidate spill with behavior graph/source capture is not implemented"
                    .into(),
            },
            model,
            opts,
        ));
    }
    if opts.seen_disk.is_some() {
        return Some(stopped(Verdict::Unsupported { feature: "disk seen-state retention with behavior graph/source capture is not implemented".into() }, model, opts));
    }
    None
}

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "keep admission, startup, explicit operative policy and failed execution evidence in order"
)]
fn explore_inner<'obs>(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    parallel: bool,
    observer: Option<&'obs mut dyn Observer>,
    primary: Option<&mut CheckpointHooks<'obs>>,
    native_hooks: dive_eval::NativeHooks,
) -> ControlledExploration {
    if let Some(s) = refusal(model, opts, parallel) {
        return ControlledExploration::from(stopped(
            Verdict::Unsupported { feature: s },
            model,
            opts,
        ));
    }
    let host = Arc::new(engine::run_host(opts));
    model
        .evaluator
        .set_host(Arc::new(crate::host::EngineHost::new(host.clone())));
    // Keep the original fresh capture-host installation/reset above. The
    // owned scope restores Callback on THIS host, never the discarded bind host.
    let _native_hooks = if native_hooks == dive_eval::NativeHooks::Unrestricted {
        model.evaluator.native_hook_scope(native_hooks)
    } else {
        None
    };
    model.evaluator.intern_checker_names();
    let config = engine::run_config(opts, model.settings.check_deadlock);
    model.evaluator.intern_bfs_config(&config);
    host.set_config(Some(config));
    engine::startup_counters(&host);
    let mut result = crate::bare(Verdict::Success, opts, &model.ir);
    result.symmetry_liveness = Some(model.symmetry_liveness().clone());
    crate::messages(&model.output, &mut result);
    let assumption_failure = crate::budget::check_assumptions(model, &opts.budget);
    result.output.extend(model.take_output());
    if let Some(verdict) = assumption_failure {
        result.verdict = verdict;
        return ControlledExploration::Stopped(Box::new(result));
    }
    let outcome = engine::search(
        model,
        opts,
        &host,
        engine::SearchHooks {
            capture: Some((limits, parallel)),
            observer,
            primary,
            ..Default::default()
        },
    );
    result.verdict = outcome.verdict;
    result.stats = outcome.stats;
    result.tlc_prints_stats = outcome.tlc_prints_stats;
    result.counterexample = outcome.counterexample;
    result.collision = outcome.collision;
    result.workers = outcome.workers;
    result.output.extend(outcome.output);
    result.notes = outcome.notes;
    refuse_unproved_temporal_next_error(model, &mut result);
    crate::note_canonical_stop(opts, &result.verdict, &mut result.notes);
    if result.verdict != Verdict::Success {
        return ControlledExploration::Stopped(Box::new(result));
    }
    let (Some(graph), Some(stats)) = (outcome.graph, result.stats) else {
        result.verdict = bug("completed exploration did not return graph/statistics");
        return ControlledExploration::Stopped(Box::new(result));
    };
    ControlledExploration::from_successful_search(
        graph,
        result,
        opts.clone(),
        stats,
        outcome.prefix_level,
        limits,
    )
}

fn stopped(verdict: Verdict, model: &Model, opts: &CheckOptions) -> Exploration {
    let mut result = crate::bare(verdict, opts, &model.ir);
    result.symmetry_liveness = Some(model.symmetry_liveness().clone());
    crate::messages(&model.output, &mut result);
    Exploration::Stopped(Box::new(result))
}

#[cfg(test)]
#[allow(clippy::panic, reason = "controlled internal thread failure")]
mod thread_controls {
    #[test]
    fn panic_does_not_fabricate_a_zero_worker_stop() {
        let result: Result<super::Exploration, String> =
            super::run_on_thread(|| panic!("controlled capture panic"));
        assert!(matches!(result, Err(message) if message == "exploration thread panicked"));
    }
}
