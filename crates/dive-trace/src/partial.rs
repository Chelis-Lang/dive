//! Partial terminal candidates are generated, never decoded as complete states.

use dive_eval::{EvalError, HostControl};
use dive_model::Model;

use crate::{Check, Failure, PartialNextEndpoint, State, Validation};

pub(crate) fn replay(
    model: &Model,
    state: &State,
    terminal: usize,
    endpoint: &PartialNextEndpoint,
    host: &crate::validate::TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<Validation, EvalError> {
    let poll = || control.map_or(Ok(()), |c| c.check().map_err(EvalError::interrupted));
    poll()?;
    if let Some(refusal) = crate::next_error::preflight(model, endpoint.initial(), control)? {
        return Ok(refusal);
    }
    let variables = crate::variables(model);
    let unassigned: std::collections::BTreeSet<_> = endpoint.unassigned().iter().collect();
    if unassigned.len() != endpoint.unassigned().len()
        || endpoint
            .unassigned()
            .iter()
            .any(|name| endpoint.assignments().contains_key(name))
        || endpoint.assignments().len() + endpoint.unassigned().len() != variables.len()
        || endpoint
            .assignments()
            .keys()
            .chain(endpoint.unassigned())
            .any(|name| !variables.contains(name))
    {
        return Ok(invalid(
            terminal,
            "partial bindings must partition the model's variables",
        ));
    }
    let mut expected = Vec::with_capacity(variables.len());
    for name in &variables {
        expected.push(if let Some(value) = endpoint.assignments().get(name) {
            match crate::value_json::from_json(value, &model.names, model.evaluator.options().ctx) {
                Ok(value) => Some(value),
                Err(_) => return Ok(invalid(terminal, "invalid assigned terminal value")),
            }
        } else {
            None
        });
    }
    let driver = crate::ordered::Driver {
        initial: endpoint.initial(),
        model,
        source: state,
        level: terminal,
        host,
        control,
    };
    match driver.primary(false)? {
        crate::ordered::Event::Partial { state, expression } => {
            if !endpoint.matches_source(&model.ir, expression) {
                return Ok(invalid(
                    terminal,
                    "first incomplete candidate has a different source",
                ));
            }
            for (actual, expected) in state.iter().zip(&expected) {
                match (actual, expected) {
                    (None, None) => {}
                    (Some(actual), Some(expected))
                        if model.evaluator.equal_values(actual, expected)? => {}
                    _ => {
                        return Ok(invalid(
                            terminal,
                            "first incomplete candidate has different assignments",
                        ));
                    }
                }
            }
            poll()?;
            Ok(Validation::Valid)
        }
        crate::ordered::Event::Error { .. } => Ok(invalid(
            terminal,
            if endpoint.initial() {
                "Init raises an evaluation error before the partial candidate"
            } else {
                "Next raises an evaluation error before the partial candidate"
            },
        )),
        crate::ordered::Event::Failed { check, detail } => Ok(Validation::Invalid(Failure {
            state: terminal,
            check,
            detail,
        })),
        crate::ordered::Event::Refused(reason) => Ok(Validation::Unsupported(reason)),
        crate::ordered::Event::End => Ok(invalid(
            terminal,
            "Next completed without the claimed partial candidate",
        )),
    }
}

fn invalid(state: usize, detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state,
        check: Check::Ending,
        detail: detail.into(),
    })
}
