//! Candidate errors require actual ordered primary and diagnostic execution.

use dive_eval::{ErrorClass, EvalError, ExceptionKind, HostControl};
use dive_model::Model;

use crate::ordered::{Driver, Event, Site};
use crate::{CandidateErrorSite, CandidateStateEndpoint, Check, Failure, State, Validation};

#[allow(
    clippy::too_many_arguments,
    reason = "explicit private replay inputs and untrusted claims"
)]
pub(crate) fn replay(
    model: &Model,
    source: &State,
    expected: &State,
    terminal: usize,
    ec: &str,
    endpoint: &CandidateStateEndpoint,
    host: &crate::validate::TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<Validation, EvalError> {
    if let Some(refusal) = crate::next_error::preflight(model, false, control)? {
        return Ok(refusal);
    }
    let driver = Driver {
        initial: false,
        model,
        source,
        level: terminal,
        host,
        control,
    };
    let (site, primary, candidate) = match driver.primary(false)? {
        Event::Error {
            site,
            error,
            candidate: Some(candidate),
            ..
        } => (site, error, candidate),
        Event::Refused(reason) => return Ok(Validation::Unsupported(reason)),
        Event::Failed { check, detail } => {
            return Ok(Validation::Invalid(Failure {
                state: terminal,
                check,
                detail,
            }));
        }
        _ => {
            return Ok(invalid(
                terminal,
                "the first terminal event is not the claimed candidate error",
            ));
        }
    };
    let actual_site = match site {
        Site::Constraint => CandidateErrorSite::Constraint,
        Site::ActionConstraint => CandidateErrorSite::ActionConstraint,
        Site::ImpliedAction => CandidateErrorSite::ImpliedAction,
        _ => {
            return Ok(invalid(
                terminal,
                "an earlier error occurs at a different evaluation site",
            ));
        }
    };
    if actual_site != endpoint.site
        || primary.ec().name() != ec
        || !endpoint.next.matches_error_source(&model.ir, &primary)
    {
        return Ok(invalid(
            terminal,
            "the first candidate error has different site, source or underlying class",
        ));
    }
    if candidate.len() != expected.len() {
        return Ok(invalid(
            terminal,
            "the actual terminal candidate has different bindings",
        ));
    }
    for (actual, expected) in candidate.iter().zip(expected.iter()) {
        match (actual, expected) {
            (Some(actual), Some(expected)) if model.evaluator.equal_values(actual, expected)? => {}
            _ => {
                return Ok(invalid(
                    terminal,
                    "the actual terminal candidate has different bindings",
                ));
            }
        }
    }
    let Some((reported, deciding)) = endpoint.next.ordered_codes() else {
        return Ok(invalid(
            terminal,
            "candidate error lacks primary and deciding identities",
        ));
    };
    let primary_reported = match primary.exception_kind() {
        Some(_) if site == Site::ImpliedAction => 2113,
        Some(ExceptionKind::EvalException) => primary.ec().code(),
        Some(ExceptionKind::ToolRuntime) => 1000,
        None => {
            return Ok(Validation::Unsupported(
                "candidate primary exception constructor/source provenance is not certified".into(),
            ));
        }
    };
    if primary_reported != reported {
        return Ok(invalid(
            terminal,
            "reported wrapper differs from the actual candidate primary exception",
        ));
    }
    check_diagnostic(&driver, site, deciding, terminal)
}

fn check_diagnostic(
    driver: &Driver<'_>,
    site: Site,
    deciding: u32,
    terminal: usize,
) -> Result<Validation, EvalError> {
    let secondary = match driver.diagnostic()? {
        Event::Error {
            site: secondary_site,
            error,
            ..
        } if secondary_site == site || secondary_site == Site::Next => error,
        Event::Refused(reason) => return Ok(Validation::Unsupported(reason)),
        Event::Error { .. } => {
            return Ok(Validation::Unsupported(
                "different secondary candidate-check site is not certified".into(),
            ));
        }
        _ => {
            return Ok(invalid(
                terminal,
                "diagnostic execution does not raise the claimed deciding error",
            ));
        }
    };
    let actual_deciding = match secondary.exception_kind() {
        Some(ExceptionKind::EvalException) => secondary.ec().code(),
        Some(ExceptionKind::ToolRuntime) => 2103,
        None => {
            return Ok(Validation::Unsupported(
                "diagnostic candidate value-source exception classification is not certified"
                    .into(),
            ));
        }
    };
    if secondary.class() != ErrorClass::Eval || actual_deciding != deciding {
        return Ok(invalid(
            terminal,
            "deciding wrapper differs from the actual diagnostic exception",
        ));
    }
    driver.poll()?;
    Ok(Validation::Valid)
}

fn invalid(state: usize, detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state,
        check: Check::Ending,
        detail: detail.into(),
    })
}
