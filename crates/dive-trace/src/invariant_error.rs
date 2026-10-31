//! Actual first-Init receiver reconstruction, separate from named-cause validity.

use std::sync::Arc;

use dive_eval::{Ec, ErrorClass, EvalError, HostControl, St, ValueMethod};
use dive_model::Model;

use crate::ordered::{Driver, Event, Site};
use crate::{Check, Failure, State, Validation};

/// Independently reconstructed initial-invariant exception identities.
///
/// Only a fresh [`crate::Session::initial_invariant_class`] can construct this
/// result. A valid named error word alone proves neither identity. This is
/// execution evidence, never a serialized field or a producer certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitialInvariantClass {
    reported: u32,
    deciding: u32,
}

impl InitialInvariantClass {
    /// Initial-state report produced by the actual ordinary invariant failure.
    #[must_use]
    pub const fn reported_code(self) -> u32 {
        self.reported
    }

    /// Wrapper established by actual diagnostic receiver reconstruction.
    #[must_use]
    pub const fn deciding_code(self) -> u32 {
        self.deciding
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one actual primary/diagnostic receiver lifecycle retains private history"
)]
pub(crate) fn replay(
    model: &Model,
    state: &State,
    name: &str,
    ec: &str,
    host: &crate::validate::TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<(Validation, Option<InitialInvariantClass>), EvalError> {
    let driver = Driver {
        initial: true,
        model,
        source: state,
        level: 0,
        host,
        control,
    };
    driver.poll()?;
    if let Some(refusal) = crate::next_error::preflight(model, true, control)? {
        return Ok((refusal, None));
    }
    let Some(first) = model.invariants.first().filter(|first| first.name == name) else {
        return Ok(refused(
            "initial receiver replay requires the first configured invariant",
        ));
    };
    if !model.constraints.is_empty()
        || model
            .ir
            .node(first.pred.expr)
            .effects
            .intersects(dive_ir::Effects::IMPURE)
        || model.evaluator.scope_has_effects(&first.pred.scope)
    {
        return Ok(refused(
            "initial receiver replay with constraints or invariant effects",
        ));
    }
    let (primary, original) = match driver.primary(false)? {
        Event::Error {
            site: Site::Invariant,
            error,
            candidate: Some(candidate),
        } => (error, candidate),
        Event::Refused(reason) => return Ok((Validation::Unsupported(reason), None)),
        Event::Failed { check, detail } => {
            return Ok((
                Validation::Invalid(Failure {
                    state: 0,
                    check,
                    detail,
                }),
                None,
            ));
        }
        _ => {
            return Ok(refused(
                "the first Init operation does not fail at the claimed invariant",
            ));
        }
    };
    if !closed_state(model, &original, &driver)? || original != *state {
        return Ok(refused(
            "the first actual Init emission differs or retains unproved value history",
        ));
    }
    if primary.class() != ErrorClass::Eval || primary.ec().name() != ec {
        return Ok((
            invalid("the actual first invariant has a different underlying cause"),
            None,
        ));
    }
    let Some(receiver) = primary.value_method().filter(|receiver| {
        matches!(
            receiver.method(),
            ValueMethod::TupleEquals | ValueMethod::RecordEquals
        )
    }) else {
        return Ok(refused(
            "the initial invariant has no certified tuple/record receiver catch",
        ));
    };
    // The sealed value-layer catch identifies this constructor independently
    // of the optional generic tool/module exception marker. Initial reporting
    // uses the actual EvalException bit; it must take the recorded-state arm.
    if primary.ec() != Ec::General || primary.is_eval_exception() {
        return Ok(refused(
            "the original initial receiver exception constructor is unproved",
        ));
    }
    // Reuse this privately reconstructed primary runtime: constants/caches
    // retain their actual lifetime, while the diagnostic Init generator tags
    // only newly constructed receivers. No witness metadata supplies sources.
    host.set_replay_level(0);
    let mut diagnostic = None;
    let mut stopped = None;
    let generated = model
        .evaluator
        .init_states_diagnostic(&model.init_actions, &mut |candidate| {
            let attempt = (|| {
                driver.poll()?;
                if candidate.ordinal != 0
                    || !candidate.complete()
                    || !closed_state(model, candidate.state, &driver)?
                    || candidate.state != original.as_ref()
                {
                    return Ok(None);
                }
                host.set_replay_level(1);
                let context = St::state(Arc::from(candidate.state)).diagnostic();
                match model.eval(&first.pred, &context) {
                    Err(error)
                        if matches!(
                            error.class(),
                            ErrorClass::Interrupted | ErrorClass::ResourceLimit | ErrorClass::Bug
                        ) =>
                    {
                        Err(error)
                    }
                    Err(error) => Ok(Some(error)),
                    Ok(_) => Ok(None),
                }
            })();
            match attempt {
                Ok(error) => diagnostic = error,
                Err(error) => stopped = Some(error),
            }
            dive_eval::next::Flow::Stop
        });
    driver.poll()?;
    if let Some(error) = stopped {
        return Err(error);
    }
    if let Err(failure) = generated {
        return if matches!(
            failure.error.class(),
            ErrorClass::Interrupted | ErrorClass::ResourceLimit | ErrorClass::Bug
        ) {
            Err(failure.error)
        } else {
            Ok(refused(
                "diagnostic Init fails before the original invariant receiver",
            ))
        };
    }
    let Some(diagnostic) = diagnostic else {
        return Ok(refused(
            "diagnostic Init cannot reconstruct the first invariant failure",
        ));
    };
    if diagnostic.class() != ErrorClass::Eval
        || diagnostic.ec() != primary.ec()
        || diagnostic.is_eval_exception() != primary.is_eval_exception()
        || diagnostic.exception_kind() != primary.exception_kind()
        || primary.origin().is_none()
        || diagnostic.origin() != primary.origin()
        || !diagnostic.value_method().is_some_and(|actual| {
            actual.method() == receiver.method()
                && actual.constructor() == receiver.constructor()
                && actual.source().is_some()
        })
    {
        return Ok(refused(
            "diagnostic receiver construction/source lifetime is not certified",
        ));
    }
    driver.poll()?;
    Ok((
        Validation::Valid,
        Some(InitialInvariantClass {
            reported: 2102,
            deciding: 2147,
        }),
    ))
}

fn closed_state(
    model: &Model,
    state: &[Option<dive_value::Value>],
    driver: &Driver<'_>,
) -> Result<bool, EvalError> {
    let mut stopped = None;
    let mut proceed = || match driver.poll() {
        Ok(()) => true,
        Err(error) => {
            stopped = Some(error);
            false
        }
    };
    let closed = state.iter().all(|value| {
        value.as_ref().is_some_and(|value| {
            value
                .closed_function_cache_heap_bytes(&model.names, &mut proceed)
                .is_some()
        })
    });
    if let Some(error) = stopped {
        Err(error)
    } else {
        Ok(closed)
    }
}

fn refused(reason: &str) -> (Validation, Option<InitialInvariantClass>) {
    (Validation::Unsupported(reason.into()), None)
}

fn invalid(detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state: 0,
        check: Check::Ending,
        detail: detail.into(),
    })
}
