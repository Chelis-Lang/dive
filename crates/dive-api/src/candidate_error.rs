//! Candidate classification requires actual controlled diagnostic-word replay.

use std::sync::Arc;

use dive_check::budget::{EvaluationControl, Phase as BudgetPhase};
use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_eval::HostControl;
use dive_trace::{CandidateStateEndpoint, Session, TraceDoc, Validation, ValidationRunError};

use super::{CheckReport, Phase};

pub(super) fn certify(
    report: &mut CheckReport,
    ir: &Arc<dive_ir::Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
) {
    let Some(endpoint) = report.candidate_error_endpoint.as_ref() else {
        return;
    };
    let Phase::Checked(result) = &report.phase else {
        return;
    };
    let validation = verify(result, endpoint, ir, config, options);
    if let Err(verdict) = validation {
        let Phase::Checked(result) = &mut report.phase else {
            return;
        };
        if let Verdict::EvaluationError { site, cause, .. } = &result.verdict {
            result.notes.push(format!(
                "Observed {site:?} primary {} before unproved candidate diagnostic replay: {}",
                cause.ec().name(),
                cause.tool_text()
            ));
        }
        result.verdict = verdict;
        result.counterexample = None;
        result.tlc_prints_stats = false;
        report.candidate_error_endpoint = None;
    }
}

fn unsupported(detail: impl std::fmt::Display) -> Verdict {
    Verdict::Unsupported {
        feature: format!("candidate primary/diagnostic lifecycle is unproved: {detail}"),
    }
}

fn checkpoint(control: &EvaluationControl) -> Result<(), Verdict> {
    control.check().map_err(|interrupted| {
        control
            .verdict(&dive_eval::EvalError::interrupted(interrupted))
            .unwrap_or_else(|| Verdict::InternalError {
                message: "candidate validation interruption lacks this run's exhausted budget"
                    .into(),
            })
    })
}

fn verify(
    result: &CheckResult,
    endpoint: &CandidateStateEndpoint,
    ir: &Arc<dive_ir::Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
) -> Result<(), Verdict> {
    let control =
        EvaluationControl::new(&options.budget, BudgetPhase::Validation).map_err(|error| {
            Verdict::InternalError {
                message: error.to_string(),
            }
        })?;
    checkpoint(&control)?;
    let path = result
        .counterexample
        .as_ref()
        .ok_or_else(|| unsupported("the actual candidate word is absent"))?;
    let wire = super::trace_doc(
        result,
        &path.states,
        options.eval.ctx,
        None,
        None,
        Some(endpoint),
    )
    .map_err(unsupported)?;
    let document = TraceDoc::from_json(&wire).map_err(unsupported)?;
    let recipe = dive_model::Recipe::new(
        ir.clone(),
        config.clone(),
        options.eval,
        options.no_deadlock,
    )
    .with_symmetry_liveness(options.symmetry_liveness);
    checkpoint(&control)?;
    let validation = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                Session::new(recipe, None).read_controlled(&document, control.clone())
            })
            .map_err(|error| Verdict::InternalError {
                message: format!("could not start candidate validation: {error}"),
            })?
            .join()
            .map_err(|_| Verdict::InternalError {
                message: "candidate validation thread panicked".into(),
            })
    })?;
    match validation {
        Ok((report, _)) if report.validation == Validation::Valid => checkpoint(&control),
        Ok((report, _)) => Err(unsupported(format!("{:?}", report.validation))),
        Err(error) => Err(failed(&control, error)),
    }
}

fn failed(control: &EvaluationControl, error: ValidationRunError) -> Verdict {
    match error {
        ValidationRunError::Interrupted { error, .. } => match control.verdict(&error) {
            Some(verdict) => verdict,
            None => Verdict::InternalError {
                message:
                    "candidate replay returned an interruption without this run's exhausted budget"
                        .into(),
            },
        },
        ValidationRunError::ResourceLimit { error, .. } => Verdict::ResourceLimit {
            message: error.to_string(),
        },
        error => Verdict::InternalError {
            message: format!("candidate diagnostic replay did not complete: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "actual owned validation interruption controls"
    )]
    fn validation_stops_require_the_actual_expired_owner_and_keep_their_phase() {
        let watchdog = dive_check::budget::Budget::new(Some(std::time::Duration::ZERO), None)
            .start()
            .expect("watchdog starts")
            .expect("armed budget");
        let owner =
            EvaluationControl::new(watchdog.budget(), BudgetPhase::Validation).expect("owner");
        assert!(
            matches!(checkpoint(&owner), Err(Verdict::Stopped(ref stopped)) if stopped.phase == BudgetPhase::Validation)
        );
        let error = dive_eval::EvalError::interrupted(owner.check().expect_err("expired"));
        let progress = dive_trace::ValidationProgress {
            checks_passed: 0,
            state: None,
            check: None,
        };
        let own = ValidationRunError::Interrupted {
            error: error.clone(),
            progress: progress.clone(),
        };
        assert!(
            matches!(failed(&owner, own.clone()), Verdict::Stopped(ref stopped) if stopped.phase == BudgetPhase::Validation)
        );
        let foreign = EvaluationControl::new(
            &dive_check::budget::Budget::unlimited(),
            BudgetPhase::Validation,
        )
        .expect("other owner");
        assert!(matches!(
            failed(&foreign, own),
            Verdict::InternalError { .. }
        ));
        let resource = ValidationRunError::ResourceLimit {
            error: dive_eval::EvalError::resource_limit("actual allocation refusal"),
            progress,
        };
        assert!(matches!(
            failed(&owner, resource),
            Verdict::ResourceLimit { .. }
        ));
    }
}
