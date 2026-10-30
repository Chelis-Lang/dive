//! Partial terminal candidates are generated, never decoded as complete states.

use dive_eval::next::Flow;
use dive_eval::{ErrorClass, EvalError, HostControl};
use dive_model::Model;

use crate::{Check, Failure, PartialNextEndpoint, State, Validation};

pub(crate) fn replay(
    model: &Model,
    state: &State,
    terminal: usize,
    endpoint: &PartialNextEndpoint,
    control: Option<&dyn HostControl>,
) -> Result<Validation, EvalError> {
    let poll = || control.map_or(Ok(()), |c| c.check().map_err(EvalError::interrupted));
    poll()?;
    if let Some(refusal) = crate::next_error::preflight(model, control)? {
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
    for action in &model.actions {
        poll()?;
        let mut decision = None;
        let mut error = None;
        let outcome = model
            .evaluator
            .next_states(action, state, 0, &mut |candidate| {
                let checked = (|| {
                    poll()?;
                    if candidate.complete() {
                        return Ok(Validation::Unsupported(
                            "partial terminal replay requires earlier complete-candidate checks"
                                .into(),
                        ));
                    }
                    if !endpoint.matches_source(&model.ir, candidate.action.expr()) {
                        return Ok(invalid(
                            terminal,
                            "first incomplete candidate has a different source",
                        ));
                    }
                    for (actual, expected) in candidate.state.iter().zip(&expected) {
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
                    Ok(Validation::Valid)
                })();
                match checked {
                    Ok(validation) => decision = Some(validation),
                    Err(failure) => error = Some(failure),
                }
                Flow::Stop
            });
        if let Some(error) = error {
            return Err(error);
        }
        poll()?;
        if let Some(decision) = decision {
            return Ok(decision);
        }
        if let Err(error) = outcome {
            return match error.error.class() {
                ErrorClass::Eval => Ok(invalid(
                    terminal,
                    "Next raises an evaluation error before the partial candidate",
                )),
                ErrorClass::Unsupported => Ok(Validation::Unsupported(error.error.to_string())),
                _ => Err(error.error),
            };
        }
    }
    Ok(invalid(
        terminal,
        "Next completed without the claimed partial candidate",
    ))
}

fn invalid(state: usize, detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state,
        check: Check::Ending,
        detail: detail.into(),
    })
}
