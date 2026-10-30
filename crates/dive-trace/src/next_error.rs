//! Fresh source-bound Next replay, stopped before any successor-side check.

use dive_eval::next::Flow;
use dive_eval::{ErrorClass, EvalError};
use dive_model::Model;

use crate::{Check, Failure, NextStateEndpoint, State, Validation};

pub(crate) fn replay(
    model: &Model,
    state: &State,
    state_index: usize,
    ec: &str,
    endpoint: &NextStateEndpoint,
    control: Option<&dyn dive_eval::HostControl>,
) -> Result<Validation, EvalError> {
    let poll = || {
        control.map_or(Ok(()), |control| {
            control.check().map_err(EvalError::interrupted)
        })
    };
    poll()?;
    if let Some(refusal) = preflight(model, control)? {
        return Ok(refusal);
    }
    let mut actions = model.actions.iter();
    let error = loop {
        let Some(action) = actions.next() else {
            return Ok(invalid(
                state_index,
                "Next completed without the claimed error",
            ));
        };
        poll()?;
        let mut candidate = false;
        let mut interruption = None;
        let outcome = model.evaluator.next_states(action, state, 0, &mut |_| {
            candidate = true;
            interruption = poll().err();
            Flow::Stop
        });
        if let Some(error) = interruption {
            return Err(error);
        }
        poll()?;
        match outcome {
            Ok(result) if candidate || result.intermediate != 0 => {
                return Ok(Validation::Unsupported(
                    "Next error replay emitted a candidate before the claimed error".into(),
                ));
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.error.class(),
                    ErrorClass::Interrupted | ErrorClass::ResourceLimit | ErrorClass::Bug
                ) =>
            {
                return Err(error.error);
            }
            Err(error) if candidate || error.emitted != 0 || error.intermediate != 0 => {
                return Ok(Validation::Unsupported(
                    "Next error replay needs successor-side ordering before its error".into(),
                ));
            }
            Err(error) => break error.error,
        }
    };
    let validation = match_error(model, error, state_index, ec, endpoint)?;
    poll()?;
    Ok(validation)
}

fn match_error(
    model: &Model,
    error: EvalError,
    state_index: usize,
    ec: &str,
    endpoint: &NextStateEndpoint,
) -> Result<Validation, EvalError> {
    if error.class() != ErrorClass::Eval {
        return if error.class() == ErrorClass::Unsupported {
            Ok(Validation::Unsupported(error.to_string()))
        } else {
            Err(error)
        };
    }
    if error.ec().name() != ec {
        return Ok(invalid(
            state_index,
            "the first Next error has a different underlying class",
        ));
    }
    let Some(observed) = NextStateEndpoint::from_error(&model.ir, &error) else {
        return Ok(Validation::Unsupported(
            "Next error has no original source context".into(),
        ));
    };
    if observed.instantiated() {
        return Ok(Validation::Unsupported(
            "instantiated Next error source context is not certified yet".into(),
        ));
    }
    if observed != *endpoint {
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
    for action in &model.actions {
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
