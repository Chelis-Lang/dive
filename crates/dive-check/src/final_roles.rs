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
    !cfg.properties.is_empty() && route_exclusion(ir, cfg, opts).is_none()
}

/// Why an ordinary request with temporal properties cannot use the temporal
/// route, or `None` when it can. Callers that refuse such a request name this
/// reason, so the user knows which option to drop.
///
/// ```
/// # fn example(ir: &std::sync::Arc<dive_ir::Ir>, cfg: &dive_config::ModelConfig) {
/// let mut opts = dive_check::CheckOptions::tlc(0, 0);
/// opts.chaos = Some(0);
/// assert_eq!(
///     dive_check::final_roles::route_exclusion(ir, cfg, &opts),
///     Some("--chaos")
/// );
/// # }
/// ```
#[must_use]
pub fn route_exclusion(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
) -> Option<&'static str> {
    let mut baseline = match opts.profile {
        dive_eval::Profile::Dive => CheckOptions::dive(opts.seed),
        dive_eval::Profile::Tlc => CheckOptions::tlc(0, opts.seed),
    };
    if opts.profile == dive_eval::Profile::Tlc
        && opts.eval.action_composition == dive_eval::ActionComposition::Tlc
    {
        baseline.eval.action_composition = dive_eval::ActionComposition::Tlc;
    }
    // The seen-state store, fingerprint polynomial, deadlock switch and
    // sequential request are honored by both continuations: the safety engine
    // reads them, and temporal capture retains concrete states with its own
    // deadlock check and worker plan.
    if opts.seen_disk.is_some() {
        Some("disk seen-state storage")
    } else if opts.frontier_memory_bytes.is_some() || opts.frontier_directory.is_some() {
        Some("frontier spill")
    } else if opts.predecessor_memory_bytes.is_some() || opts.predecessor_directory.is_some() {
        Some("predecessor spill")
    } else if opts.eval != baseline.eval {
        Some("evaluation options other than the profile defaults")
    } else if opts.chaos != baseline.chaos {
        Some("--chaos")
    } else if dive_eval::validate_ir(ir).is_err() {
        Some("an IR the evaluator does not accept")
    } else if dive_model::transformed_randomness(ir, cfg).is_some() {
        Some("randomness combined with VIEW or SYMMETRY")
    } else {
        None
    }
}

/// Binds once and dispatches only the default resident-memory option route.
///
/// `None` means no host was created and no binding or source evaluation ran:
/// the caller must retain its existing temporal route and error priority.
/// Eligible requests use profile-default evaluation/fingerprints, or TLC evaluation
/// with only [`dive_eval::ActionComposition::Tlc`] selected, any seen-state
/// store, polynomial, deadlock and sequential setting, no storage/directory
/// overrides or chaos,
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
            |o| o.frontier_memory_bytes = Some(0),
            |o| o.frontier_directory = Some("unused".into()),
            |o| o.predecessor_memory_bytes = Some(0),
            |o| o.predecessor_directory = Some("unused".into()),
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

    #[test]
    fn store_polynomial_deadlock_and_sequential_settings_remain_eligible() {
        type Change = fn(&mut CheckOptions);
        let changes: &[Change] = &[
            |o| o.store = dive_store::StoreMode::Exact,
            |o| o.fp_index = 5,
            |o| o.tlc_fingerprints = false,
            |o| o.no_deadlock = true,
            |o| o.sequential = true,
        ];
        let (ir, cfg) = input();
        for baseline in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
            for change in changes {
                let mut options = baseline.clone();
                change(&mut options);
                assert!(eligible(&ir, &cfg, &options), "{options:?}");
            }
        }
    }
}
