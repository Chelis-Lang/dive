//! Fresh source-bound Next replay with independently proved demand order.

use dive_eval::{ErrorClass, EvalError};
use dive_model::Model;

use crate::{Check, Failure, NextStateEndpoint, State, Validation};

pub(crate) fn replay(
    model: &Model,
    state: &State,
    state_index: usize,
    ec: &str,
    endpoint: &NextStateEndpoint,
    host: &crate::validate::TraceHost,
    control: Option<&dyn dive_eval::HostControl>,
) -> Result<Validation, EvalError> {
    let poll = || {
        control.map_or(Ok(()), |control| {
            control.check().map_err(EvalError::interrupted)
        })
    };
    poll()?;
    if crate::next_temporal::needed(model) {
        return crate::next_temporal::replay(
            model,
            state,
            state_index,
            ec,
            endpoint,
            host,
            control,
        );
    }
    if let Some(refusal) = preflight(model, false, control)? {
        return Ok(refusal);
    }
    // A diagnostic receiver catch requires its actual first-source history.
    // Prefix validation has a separate runtime; reconstruct Init here before
    // ordinary Next, retaining this runtime into diagnostic Next execution.
    if endpoint.deciding_code() == Some(2147) {
        if state_index != 0 {
            return Ok(Validation::Unsupported(
                "Next receiver replay covers only the first source row".into(),
            ));
        }
        if let Some(refusal) = preflight(model, true, control)? {
            return Ok(refusal);
        }
        let initial = crate::ordered::Driver {
            initial: true,
            model,
            source: state,
            level: 0,
            host,
            control,
        };
        match initial.initial_source()? {
            crate::ordered::Event::End => {}
            crate::ordered::Event::Refused(reason) => return Ok(Validation::Unsupported(reason)),
            _ => {
                return Ok(invalid(
                    state_index,
                    "Init fails before the claimed Next receiver",
                ));
            }
        }
    }
    let driver = crate::ordered::Driver {
        initial: false,
        model,
        source: state,
        level: state_index.saturating_add(1),
        host,
        control,
    };
    let error = match driver
        .primary(endpoint.ordered_codes().is_none() || endpoint.deciding_code() == Some(2147))?
    {
        crate::ordered::Event::Error {
            site: crate::ordered::Site::Next,
            error,
            ..
        } => error,
        crate::ordered::Event::Error { .. } => {
            return Ok(invalid(
                state_index,
                "an earlier candidate check fails at a different evaluation site",
            ));
        }
        crate::ordered::Event::Partial { .. } => {
            return Ok(invalid(
                state_index,
                "Next emits an incomplete candidate before the claimed error",
            ));
        }
        crate::ordered::Event::Failed { check, detail } => {
            return Ok(Validation::Invalid(Failure {
                state: state_index,
                check,
                detail,
            }));
        }
        crate::ordered::Event::Refused(reason) => return Ok(Validation::Unsupported(reason)),
        crate::ordered::Event::End => {
            return Ok(invalid(
                state_index,
                "Next completed without the claimed error",
            ));
        }
    };
    let validation = match_error(model, &error, state_index, ec, endpoint)?;
    if validation != Validation::Valid {
        return Ok(validation);
    }
    let result = check_diagnostic(&driver, &error, endpoint, state_index)?;
    poll()?;
    Ok(result)
}

fn check_diagnostic(
    driver: &crate::ordered::Driver<'_>,
    error: &EvalError,
    endpoint: &NextStateEndpoint,
    state_index: usize,
) -> Result<Validation, EvalError> {
    if let Some((reported, deciding)) = endpoint.ordered_codes() {
        let actual_reported = match error.exception_kind() {
            Some(dive_eval::ExceptionKind::EvalException) => error.ec().code(),
            Some(dive_eval::ExceptionKind::ToolRuntime) => 1000,
            None => {
                return Ok(Validation::Unsupported(
                    "primary exception constructor/source provenance is not certified".into(),
                ));
            }
        };
        if reported != actual_reported {
            return Ok(invalid(
                state_index,
                "reported wrapper differs from the actual primary exception",
            ));
        }
        let secondary = match if deciding == 2147 {
            driver.diagnostic_sources()?
        } else {
            driver.diagnostic()?
        } {
            crate::ordered::Event::Error {
                site: crate::ordered::Site::Next,
                error,
                ..
            } => error,
            crate::ordered::Event::Refused(reason) => return Ok(Validation::Unsupported(reason)),
            crate::ordered::Event::Error { .. } => {
                return Ok(Validation::Unsupported(
                    "secondary candidate-check error site is not certified".into(),
                ));
            }
            _ => {
                return Ok(invalid(
                    state_index,
                    "diagnostic replay completes or stops quietly instead of the claimed error",
                ));
            }
        };
        if deciding == 2147 {
            return Ok(if same_function_receiver(error, &secondary) {
                Validation::Valid
            } else {
                Validation::Unsupported(
                    "Next diagnostic function receiver construction/source lifetime is not certified".into(),
                )
            });
        }
        let actual_deciding = match secondary.exception_kind() {
            Some(dive_eval::ExceptionKind::EvalException) => secondary.ec().code(),
            Some(dive_eval::ExceptionKind::ToolRuntime) => 2103,
            None => {
                return Ok(Validation::Unsupported(
                    "diagnostic value-source exception classification is not certified".into(),
                ));
            }
        };
        if actual_deciding != deciding {
            return Ok(invalid(
                state_index,
                "deciding wrapper differs from the actual diagnostic exception",
            ));
        }
    }
    Ok(Validation::Valid)
}

fn same_function_receiver(primary: &EvalError, diagnostic: &EvalError) -> bool {
    primary.class() == ErrorClass::Eval
        && diagnostic.class() == ErrorClass::Eval
        && primary.ec() == diagnostic.ec()
        && primary.is_eval_exception() == diagnostic.is_eval_exception()
        && primary.exception_kind().is_some()
        && primary.exception_kind() == diagnostic.exception_kind()
        && primary.origin().is_some()
        && primary.origin() == diagnostic.origin()
        && primary.value_method().is_some_and(|receiver| {
            receiver.method() == dive_eval::ValueMethod::FunctionMaterialize
                && receiver.source().is_none()
                && diagnostic.value_method().is_some_and(|actual| {
                    actual.method() == receiver.method()
                        && actual.constructor() == receiver.constructor()
                        && actual.source() == Some(actual.constructor())
                })
        })
}

fn match_error(
    model: &Model,
    error: &EvalError,
    state_index: usize,
    ec: &str,
    endpoint: &NextStateEndpoint,
) -> Result<Validation, EvalError> {
    if error.class() != ErrorClass::Eval {
        return if error.class() == ErrorClass::Unsupported {
            Ok(Validation::Unsupported(error.to_string()))
        } else {
            Err(error.clone())
        };
    }
    if error.ec().name() != ec {
        return Ok(invalid(
            state_index,
            "the first Next error has a different underlying class",
        ));
    }
    let Some(observed) = NextStateEndpoint::from_error(&model.ir, error) else {
        return Ok(Validation::Unsupported(
            "Next error has no original source context".into(),
        ));
    };
    if observed.instantiated() && endpoint.ordered_codes().is_none() {
        return Ok(Validation::Unsupported(
            "instantiated Next error source context is not certified yet".into(),
        ));
    }
    if !endpoint.matches_error_source(&model.ir, error) {
        return Ok(invalid(
            state_index,
            "the first Next error has a different source context",
        ));
    }
    Ok(Validation::Valid)
}

fn invalid(state: usize, detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state,
        check: Check::Ending,
        detail: detail.into(),
    })
}

pub(crate) fn preflight(
    model: &Model,
    initial: bool,
    control: Option<&dyn dive_eval::HostControl>,
) -> Result<Option<Validation>, EvalError> {
    if let Some(feature) = model.unsupported.first() {
        return Ok(Some(Validation::Unsupported(format!(
            "Next error replay with an unsupported model feature: {}",
            feature.feature,
        ))));
    }
    if model.view.is_some()
        || model.symmetry.is_some()
        || !model.temporals.is_empty()
        || !model.implied_temporals.is_empty()
    {
        return Ok(Some(Validation::Unsupported(
            "Next error replay with VIEW, symmetry or temporal obligations".into(),
        )));
    }
    for action in if initial {
        &model.init_actions
    } else {
        &model.actions
    } {
        if let Some(control) = control {
            control.check().map_err(EvalError::interrupted)?;
        }
        if model
            .ir
            .node(action.expr())
            .effects
            .intersects(dive_ir::Effects::IMPURE)
            || model.evaluator.action_shares_side_effects(action)
        {
            return Ok(Some(Validation::Unsupported(
                "effects or dynamically shared scopes in Next error replay".into(),
            )));
        }
    }
    Ok(None)
}
