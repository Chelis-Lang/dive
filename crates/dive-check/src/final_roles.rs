//! Narrow single-binding dispatch for explicitly selected final checking.
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
    let baseline = match opts.profile {
        dive_eval::Profile::Dive => CheckOptions::dive(opts.seed),
        dive_eval::Profile::Tlc => CheckOptions::tlc(0, opts.seed),
    };
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
/// Eligible requests use profile-default evaluation/fingerprints, polynomial 0,
/// no storage/directory overrides, deadlock override, sequential flag or chaos,
/// and no source/configured randomness under VIEW or SYMMETRY. Seed and worker
/// requests are retained. Eligibility is not a purity or model-truth claim.
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
