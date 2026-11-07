//! Reference capture drivers with reached checkpoints; the checkpoint
//! types live in `dive-runtime` (ADR 0155).

use dive_model::Model;

use super::{
    CheckpointExploration, CheckpointHooks as Hooks, CheckpointObserver, ControlledExploration,
    Limits, Observer,
};
use crate::{CheckOptions, Verdict};
use dive_runtime::capture::CHECKPOINT_DECISION as DECISION;

/// Opt-in W1 inline capture with actual Init/source-row checkpoints.
/// Existing capture entrypoints, old Observer callbacks and ordinary scheduling
/// are unchanged. Other requested widths (including zero) refuse before search.
/// No temporal predicate is evaluated by this API itself.
/// Both observer borrows cover this call; the owned result borrows neither.
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
pub fn explore_checkpoints_with_evidence<'observer>(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: Option<&'observer mut dyn Observer>,
    checkpoints: &'observer mut dyn CheckpointObserver,
) -> Result<CheckpointExploration, String> {
    explore_checkpoints_with_native_hooks(
        model,
        opts,
        limits,
        observer,
        checkpoints,
        dive_eval::NativeHooks::Callback,
    )
}

/// Explicit operative hook selection for a source-owning row consumer.
///
/// The ordinary public checkpoint API retains callbacks. This is an execution
/// choice, not a certificate of ownership or authority to evaluate row atoms.
///
/// # Errors
/// The existing dedicated-thread failure, with unknown lost work.
pub fn explore_checkpoints_with_native_hooks<'observer>(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: Option<&'observer mut dyn Observer>,
    checkpoints: &'observer mut dyn CheckpointObserver,
    native_hooks: dive_eval::NativeHooks,
) -> Result<CheckpointExploration, String> {
    let refuse =
        |outcome: super::Exploration| CheckpointExploration::from_parts(outcome.into(), None);
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
        let mut hooks = Hooks::new(checkpoints);
        let outcome = super::explore_inner(
            model,
            opts,
            limits,
            false,
            observer,
            Some(&mut hooks),
            native_hooks,
        );
        (outcome, hooks.into_decision())
    })?;
    outcome.set_elapsed(started.elapsed());
    if let Some(record) = &decision {
        let usable = matches!(&outcome, ControlledExploration::Stopped(s)
            if matches!(&s.verdict, Verdict::Unsupported { feature } if feature == DECISION)
                && s.stats == Some(record.stats()) && s.counterexample.is_some());
        if !usable {
            decision = None;
        }
    }
    Ok(CheckpointExploration::from_parts(outcome, decision))
}

#[cfg(test)]
mod tests;
