//! Independent post-Next row execution over one concrete scalar destination.

use dive_eval::{Ec, ErrorClass, EvalError, ExceptionKind, HostControl};
use dive_model::Model;
use dive_value::Value;

use crate::ordered::{Driver, Event, Site};
use crate::{Check, Failure, State, TemporalSourceEndpoint, Validation};

pub(crate) fn replay(
    model: &Model,
    state: &State,
    index: usize,
    ec: &str,
    endpoint: &TemporalSourceEndpoint,
    host: &crate::validate::TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<Validation, EvalError> {
    let invalid = |detail: &str| {
        Validation::Invalid(Failure {
            state: index,
            check: Check::Ending,
            detail: detail.into(),
        })
    };
    if state.iter().any(|value| {
        !matches!(
            value,
            Some(
                Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_)
            )
        )
    }) {
        return Ok(Validation::Unsupported(
            "temporal source-row replay requires concrete closed scalar state values".into(),
        ));
    }
    if model.view.is_some()
        || model.symmetry.is_some()
        || model
            .unsupported
            .iter()
            .any(|feature| feature.kind != dive_model::UnsupportedKind::TemporalProperty)
    {
        return Ok(Validation::Unsupported(
            "temporal source-row replay with VIEW, symmetry or unsupported model features".into(),
        ));
    }
    let program =
        match crate::row_demand::prepare_source_row(model, std::slice::from_ref(state), control) {
            Ok(program) => program,
            Err(error) => return refusal(error),
        };
    let driver = Driver {
        initial: false,
        model,
        source: state,
        level: index.saturating_add(1),
        host,
        control,
    };
    driver.poll()?;
    match driver.source_row()? {
        Event::End => {}
        Event::Error {
            site: Site::Next,
            error,
            ..
        } if general(&error) => {}
        Event::Error {
            site: Site::Next, ..
        } => {
            return Ok(Validation::Unsupported(
                "temporal source-row primary exception lifecycle is not proved GENERAL".into(),
            ));
        }
        Event::Refused(feature) => return Ok(Validation::Unsupported(feature)),
        _ => {
            return Ok(invalid(
                "an earlier candidate check or incomplete emission precedes the alleged temporal-row error",
            ));
        }
    }
    let error = match program.first_source_error(model, state, control) {
        Ok(Some(error)) => error,
        Ok(None) => {
            return Ok(invalid(
                "independent temporal source row completes without the alleged error",
            ));
        }
        Err(error) => return refusal(error),
    };
    if !general(&error) {
        return Ok(Validation::Unsupported(
            "temporal source-row exception lifecycle is not proved GENERAL".into(),
        ));
    }
    if error.ec().name() != ec || !endpoint.matches_error(model, &error) {
        return Ok(invalid(
            "the first independently executed row error has a different class or source",
        ));
    }
    // The row diagnostic must actually fail before ordinary doNext diagnostic
    // replay can be suppressed. The retained first primary is derived above,
    // not supplied by a document flag, error string, or a producer callback.
    let diagnostic = match program.first_source_error(model, state, control) {
        Ok(Some(error)) => error,
        Ok(None) => {
            return Ok(invalid(
                "independent row diagnostic does not suppress ordinary Next replay",
            ));
        }
        Err(error) => return refusal(error),
    };
    if !general(&diagnostic) || !endpoint.matches_error(model, &diagnostic) {
        return Ok(Validation::Unsupported(
            "temporal source-row diagnostic source lifecycle differs".into(),
        ));
    }
    driver.poll()?;
    Ok(Validation::Valid)
}

fn general(error: &EvalError) -> bool {
    error.class() == ErrorClass::Eval
        && error.ec() == Ec::General
        && error.exception_kind() == Some(ExceptionKind::ToolRuntime)
        && error.value_method().is_none()
}

fn refusal(error: crate::row_demand::FormulaError) -> Result<Validation, EvalError> {
    match error {
        crate::row_demand::FormulaError::Evaluation(error)
            if matches!(
                error.class(),
                ErrorClass::Interrupted | ErrorClass::ResourceLimit | ErrorClass::Bug
            ) =>
        {
            Err(error)
        }
        error => Ok(Validation::Unsupported(error.to_string())),
    }
}
