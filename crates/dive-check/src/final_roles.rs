//! Narrow single-binding dispatch for bounded final checking.
//!
//! This accepts source/configuration, never a caller-prebound model. The temporal
//! API calls it on its fresh thread with [`dive_eval::RECOMMENDED_STACK`]. Other
//! callers must likewise use a dedicated thread with that stack size.

use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::Ir;

use crate::{CheckOptions, CheckResult};

/// The result of binding and inspecting the actual configured property roles.
#[derive(Debug)]
pub enum Dispatch {
    /// All configured obligations are safety roles, checked by the ordinary
    /// engine with the same model and startup host. This can also be a refusal
    /// or failure; callers must inspect the complete result.
    Safety(Box<CheckResult>),
    /// At least one temporal obligation remains. No checking, assumptions or
    /// role removal occurred; continue with the existing temporal analyzer.
    Temporal(Box<dive_model::Model>),
}

fn eligible(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> bool {
    let mut baseline = match opts.profile {
        dive_eval::Profile::Dive => CheckOptions::dive(opts.seed),
        dive_eval::Profile::Tlc => CheckOptions::tlc(0, opts.seed),
    };
    if opts.profile == dive_eval::Profile::Tlc
        && opts.eval.action_composition == dive_eval::ActionComposition::Tlc
    {
        baseline.eval.action_composition = dive_eval::ActionComposition::Tlc;
    }
    !cfg.properties.is_empty()
        && opts.seen_disk.is_none()
        && opts.frontier_memory_bytes.is_none()
        && opts.frontier_directory.is_none()
        && opts.predecessor_memory_bytes.is_none()
        && opts.predecessor_directory.is_none()
        && opts.store == baseline.store
        && opts.fp_index == baseline.fp_index
        && opts.tlc_fingerprints == baseline.tlc_fingerprints
        && opts.eval == baseline.eval
        && opts.no_deadlock == baseline.no_deadlock
        && opts.sequential == baseline.sequential
        && opts.chaos == baseline.chaos
        && dive_eval::validate_ir(ir).is_ok()
        && dive_model::transformed_randomness(ir, cfg).is_none()
}

/// Binds once and dispatches only the default resident-memory option route.
///
/// `None` means no host was created and no binding or source evaluation ran:
/// the caller must retain its existing temporal route and error priority.
/// Eligible requests use profile-default evaluation/fingerprints, or TLC evaluation
/// with only [`dive_eval::ActionComposition::Tlc`] selected, and polynomial 0,
/// no storage/directory overrides, deadlock override, sequential flag or chaos,
/// and no source/configured randomness under VIEW or SYMMETRY. Seed and worker
/// requests and the independently selected symmetry/liveness policy are retained.
/// That policy is resolved only after actual role binding. Eligibility is not a
/// purity or model-truth claim.
///
/// The actual bound roles decide the route. Empty implied temporal roles retain
/// every configured initial/invariant/action obligation and the ordinary
/// checker's complete post-binding ordering, including symmetry, assumptions,
/// remaining refusals and postconditions. Genuine temporal roles are returned
/// unchanged before these phases for the temporal analyzer.
///
/// # Errors
/// An eligible request failed ordinary model binding. Its original diagnostic
/// and startup output are retained; callers keep their binding-stage interface.
#[must_use]
pub fn dispatch(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
) -> Option<Result<Dispatch, Box<dive_model::Failure>>> {
    if !eligible(ir, cfg, opts) {
        return None;
    }
    Some(
        crate::bind_for_run(ir, cfg, opts, None).map(|(model, host)| {
            if model.implied_temporals.is_empty() {
                Dispatch::Safety(Box::new(crate::finish_bound_check(
                    ir, cfg, opts, model, host, None,
                )))
            } else {
                Dispatch::Temporal(Box::new(model))
            }
        }),
    )
}

/// Like [`dispatch`], retaining typed run interruption during fresh binding.
///
/// The caller owns the already armed budget and classifies an interrupted error
/// only with this control's actual resource authority. Eligibility and single
/// startup evaluation are unchanged; this must use the dedicated stack above.
///
/// ```
/// # fn example(ir: &std::sync::Arc<dive_ir::Ir>, cfg: &dive_config::ModelConfig,
/// # opts: &dive_check::CheckOptions, control: std::sync::Arc<dive_check::budget::EvaluationControl>) {
/// let _ = dive_check::final_roles::dispatch_controlled(ir, cfg, opts, control);
/// # }
/// ```
#[must_use]
pub fn dispatch_controlled(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    control: Arc<crate::budget::EvaluationControl>,
) -> Option<Result<Dispatch, dive_model::ControlledBindError>> {
    if !eligible(ir, cfg, opts) {
        return None;
    }
    Some(
        crate::bind_for_run_controlled(ir, cfg, opts, None, control).map(|(model, host)| {
            if model.implied_temporals.is_empty() {
                Dispatch::Safety(Box::new(crate::finish_bound_check(
                    ir, cfg, opts, model, host, None,
                )))
            } else {
                Dispatch::Temporal(Box::new(model))
            }
        }),
    )
}

/// Controlled ordinary dispatch which preserves ordinary binding error reports.
///
/// Genuine binding failures retain their existing safety result. Typed run
/// interruptions remain errors so API callers retain actual binding-stage
/// evidence, rather than mislabelling a pre-search failure as capture work.
#[must_use]
pub fn dispatch_ordinary_controlled(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    control: Arc<crate::budget::EvaluationControl>,
) -> Option<Result<Dispatch, dive_model::ControlledBindError>> {
    dispatch_controlled(ir, cfg, opts, control).map(|result| match result {
        Err(dive_model::ControlledBindError::Binding(failure)) => Ok(Dispatch::Safety(Box::new(
            crate::binding_failure(ir, opts, &failure),
        ))),
        other => other,
    })
}

/// Like [`dispatch`], retaining ordinary binding failures as ordinary results.
///
/// This uses the same eligibility, startup host and single binding. It must run
/// on the dedicated evaluation stack described by this module. `None` retains
/// the caller's original ordinary route without binding or running effects.
#[must_use]
pub fn dispatch_ordinary(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> Option<Dispatch> {
    if !eligible(ir, cfg, opts) {
        return None;
    }
    let Ok(control) =
        crate::budget::EvaluationControl::new(&opts.budget, crate::budget::Phase::Binding)
    else {
        return Some(Dispatch::Safety(Box::new(crate::bare(
            crate::Verdict::InternalError {
                message: "binding control identity".into(),
            },
            opts,
            ir,
        ))));
    };
    dispatch_ordinary_controlled(ir, cfg, opts, control.clone()).map(|result| match result {
        Ok(bound) => bound,
        Err(failure) => Dispatch::Safety(Box::new(crate::controlled_binding_failure(
            ir, opts, &failure, &control,
        ))),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "tiny eligibility assertions")]
    use super::eligible;
    use crate::CheckOptions;
    use dive_eval::{ActionComposition, Comparisons, ExplorationOrder};
    use std::sync::Arc;

    fn input() -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
        let source =
            "---- MODULE Eligible ----\nVARIABLE x\nInit==x=0\nNext==x'=x\nP==<>(x=1)\n====\n";
        let analysis =
            dive_sema::analyze_source("Eligible.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted());
        (
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
                .into_config()
                .expect("configuration"),
        )
    }

    #[test]
    fn only_tlc_composition_adds_a_profile_match() {
        let (ir, cfg) = input();
        for (baseline, modes) in [
            (CheckOptions::tlc(0, 7), [true, true, false]),
            (CheckOptions::dive(7), [false, false, true]),
        ] {
            for (mode, expected) in [
                ActionComposition::Off,
                ActionComposition::Tlc,
                ActionComposition::Tla,
            ]
            .into_iter()
            .zip(modes)
            {
                let mut options = baseline.clone();
                options.eval.action_composition = mode;
                assert_eq!(eligible(&ir, &cfg, &options), expected, "{options:?}");
            }
        }
    }

    #[test]
    fn composition_preserves_other_evaluation_and_resident_guards() {
        type Change = fn(&mut CheckOptions);
        let changes: &[Change] = &[
            |o| o.eval.ctx.int_mode = dive_value::IntMode::Big,
            |o| o.eval.ctx.order = dive_value::ValueOrder::Canonical,
            |o| o.eval.exploration = ExplorationOrder::Value,
            |o| o.eval.comparisons = Comparisons::Tla,
            |o| o.eval.max_depth = 1,
            |o| o.eval.warnings = false,
            |o| o.store = dive_store::StoreMode::Exact,
            |o| o.fp_index = 1,
            |o| o.tlc_fingerprints = false,
            |o| o.frontier_memory_bytes = Some(0),
            |o| o.frontier_directory = Some("unused".into()),
            |o| o.predecessor_memory_bytes = Some(0),
            |o| o.predecessor_directory = Some("unused".into()),
            |o| o.no_deadlock = true,
            |o| o.sequential = true,
            |o| o.chaos = Some(0),
        ];
        let (ir, cfg) = input();
        let mut baseline = CheckOptions::tlc(0, 7);
        baseline.eval.action_composition = ActionComposition::Tlc;
        assert!(eligible(&ir, &cfg, &baseline));
        for change in changes {
            let mut options = baseline.clone();
            change(&mut options);
            assert!(!eligible(&ir, &cfg, &options), "{options:?}");
        }
    }
}
