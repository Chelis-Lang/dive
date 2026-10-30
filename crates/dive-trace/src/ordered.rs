//! Source-bound terminal demand order, without a queue, seen set or checker.

use crate::validate::TraceHost;
use crate::{Check, State};
use dive_eval::next::{Flow, Successor};
use dive_eval::{ErrorClass, EvalError, HostControl, St};
use dive_model::{Model, Named};
use dive_value::{ConstructionError, Value, ValueOrder};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Site {
    Init,
    Next,
    Constraint,
    ActionConstraint,
    Invariant,
    ImpliedAction,
    ImpliedInit,
}

pub(crate) enum Event {
    Error {
        site: Site,
        error: EvalError,
        candidate: Option<State>,
    },
    Partial {
        state: State,
        expression: dive_ir::ExprId,
    },
    Failed {
        check: Check,
        detail: String,
    },
    Refused(String),
    End,
}

pub(crate) struct Driver<'a> {
    pub(crate) initial: bool,
    pub(crate) model: &'a Model,
    pub(crate) source: &'a State,
    pub(crate) level: usize,
    pub(crate) host: &'a TraceHost,
    pub(crate) control: Option<&'a dyn HostControl>,
}

impl Driver<'_> {
    // Candidate predicates observe the generated state's level. Action
    // predicates and generation observe the predecessor (zero for Init).
    fn at_site(&self, site: Site) {
        let level = match site {
            Site::Constraint | Site::Invariant | Site::ImpliedInit => self.level.saturating_add(1),
            Site::Init | Site::Next | Site::ActionConstraint | Site::ImpliedAction => self.level,
        };
        self.host.set_replay_level(level);
    }

    pub(crate) fn poll(&self) -> Result<(), EvalError> {
        self.control
            .map_or(Ok(()), |c| c.check().map_err(EvalError::interrupted))
    }

    pub(crate) fn primary(&self, before_first: bool) -> Result<Event, EvalError> {
        let mut emitted = 0u64;
        let actions = if self.initial {
            &self.model.init_actions
        } else {
            &self.model.actions
        };
        for action in actions {
            self.poll()?;
            let generation = if self.initial { Site::Init } else { Site::Next };
            self.at_site(generation);
            let mut decision = None;
            let mut stop = None;
            let mut sink = |candidate: &Successor<'_>| {
                let known_new = self.initial && emitted == 0;
                emitted = emitted.saturating_add(1);
                let result = self.candidate(candidate, before_first, known_new);
                // The generator continues after a successful callback at its
                // original source level, even when Init properties used level1.
                self.at_site(generation);
                match result {
                    Ok(Some(event)) => decision = Some(event),
                    Ok(None) => return Flow::Continue,
                    Err(error) => stop = Some(error),
                }
                Flow::Stop
            };
            let outcome = if self.initial {
                self.model
                    .evaluator
                    .init_states(std::slice::from_ref(action), &mut sink)
            } else {
                self.model
                    .evaluator
                    .next_states(action, self.source, 0, &mut sink)
            };
            self.poll()?;
            if let Some(error) = stop {
                return Err(error);
            }
            if let Some(event) = decision {
                return Ok(event);
            }
            if let Err(error) = outcome {
                return execution_error(
                    if self.initial { Site::Init } else { Site::Next },
                    error.error,
                );
            }
        }
        Ok(Event::End)
    }

    /// TLC diagnostic doNext enumerates each action before checking that
    /// action's candidates. Retain only that action, with checked reservations.
    /// The runtime is the independently reconstructed primary runtime, whose
    /// own history is retained; no caller cache/seen facts enter this sequence.
    pub(crate) fn diagnostic(&self) -> Result<Event, EvalError> {
        for action in &self.model.actions {
            self.poll()?;
            self.at_site(Site::Next);
            let mut candidates = Vec::new();
            let mut stop = None;
            let outcome =
                self.model
                    .evaluator
                    .next_states(action, self.source, 0, &mut |candidate| {
                        if let Err(error) = self.poll().and_then(|()| {
                            candidates.try_reserve(1).map_err(|_| {
                                EvalError::resource_limit(
                                    "diagnostic candidate storage unavailable",
                                )
                            })?;
                            candidates.push(slots(candidate.state)?);
                            Ok(())
                        }) {
                            stop = Some(error);
                            Flow::Stop
                        } else {
                            Flow::Continue
                        }
                    });
            self.poll()?;
            if let Some(error) = stop {
                return Err(error);
            }
            if let Err(error) = outcome {
                return execution_error(Site::Next, error.error);
            }
            for (ordinal, state) in candidates.iter().enumerate() {
                let candidate = Successor {
                    state,
                    action,
                    ordinal: u64::try_from(ordinal)
                        .map_err(|_| EvalError::resource_limit("diagnostic ordinal unavailable"))?,
                };
                if let Some(event) = self.candidate(&candidate, false, false)? {
                    return Ok(event);
                }
            }
        }
        Ok(Event::End)
    }

    fn candidate(
        &self,
        candidate: &Successor<'_>,
        before_first: bool,
        known_new: bool,
    ) -> Result<Option<Event>, EvalError> {
        self.poll()?;
        if before_first {
            return Ok(Some(Event::Refused(
                "Next error replay emitted a candidate before the claimed error".into(),
            )));
        }
        if !candidate.complete() {
            return Ok(Some(Event::Partial {
                state: slots(candidate.state)?,
                expression: candidate.action.expr(),
            }));
        }
        let state = slots(candidate.state)?;
        let here = St::state(state.clone());
        let step = St::action(self.source.clone(), state);
        self.at_site(Site::Constraint);
        let mut inside = true;
        for constraint in &self.model.constraints {
            match self.holds(constraint, &here, Site::Constraint)? {
                Ok(true) => {}
                Ok(false) => {
                    inside = false;
                    break;
                }
                Err(event) => return Ok(Some(with_candidate(event, candidate.state)?)),
            }
        }
        if inside && !self.initial {
            self.at_site(Site::ActionConstraint);
            for constraint in &self.model.action_constraints {
                match self.holds(constraint, &step, Site::ActionConstraint)? {
                    Ok(true) => {}
                    Ok(false) => {
                        inside = false;
                        break;
                    }
                    Err(event) => return Ok(Some(with_candidate(event, candidate.state)?)),
                }
            }
        }
        self.at_site(Site::Invariant);
        if inside {
            if let Some(event) = self.fingerprint(candidate.state)? {
                return Ok(Some(event));
            }
            if !known_new
                && (!self.model.invariants.is_empty()
                    || self.initial && !self.model.implied_inits.is_empty())
            {
                return Ok(Some(Event::Refused("ordered replay cannot infer global newness or invariant alias history for an in-model candidate".into())));
            }
        }
        if !inside || known_new {
            // Excluded candidates are always checked, with no dedup decision.
            for invariant in &self.model.invariants {
                match self.holds(invariant, &here, Site::Invariant)? {
                    Ok(true) => {}
                    Ok(false) => {
                        return Ok(Some(Event::Failed {
                            check: Check::Invariant,
                            detail: format!(
                                "earlier excluded candidate violates {}",
                                invariant.name
                            ),
                        }));
                    }
                    Err(event) => return Ok(Some(with_candidate(event, candidate.state)?)),
                }
            }
        }
        self.at_site(if self.initial {
            Site::ImpliedInit
        } else {
            Site::ImpliedAction
        });
        let properties = if self.initial {
            &self.model.implied_inits
        } else {
            &self.model.implied_actions
        };
        for property in properties {
            let (context, site, check) = if self.initial {
                (&here, Site::ImpliedInit, Check::ImpliedInit)
            } else {
                (&step, Site::ImpliedAction, Check::ImpliedAction)
            };
            match self.holds(property, context, site)? {
                Ok(true) => {}
                Ok(false) => {
                    return Ok(Some(Event::Failed {
                        check,
                        detail: format!("earlier candidate violates {}", property.name),
                    }));
                }
                Err(event) => return Ok(Some(with_candidate(event, candidate.state)?)),
            }
        }
        Ok(None)
    }

    fn holds(
        &self,
        named: &Named,
        state: &St,
        site: Site,
    ) -> Result<Result<bool, Event>, EvalError> {
        self.poll()?;
        match self.model.eval(&named.pred, state) {
            Ok(Value::Bool(value)) => Ok(Ok(value)),
            Ok(_) => Ok(Err(Event::Refused(
                "ordered replay of a non-Boolean candidate check is not certified".into(),
            ))),
            Err(error) => execution_error(site, error).map(Err),
        }
    }

    fn fingerprint(&self, state: &[Option<Value>]) -> Result<Option<Event>, EvalError> {
        let fingerprint = dive_value::fp64::TlcFingerprint::new(0)
            .ok_or_else(|| EvalError::bug("fingerprint polynomial zero missing"))?;
        let mut interrupted = None;
        let mut proceed = || match self.poll() {
            Ok(()) => true,
            Err(error) => {
                interrupted = Some(error);
                false
            }
        };
        let order: Vec<_> = if self.model.evaluator.options().ctx.order == ValueOrder::Tlc {
            self.model
                .ir
                .tlc_var_order()
                .iter()
                .map(|var| var.index())
                .collect()
        } else {
            (0..state.len()).collect()
        };
        for index in order {
            let Some(Some(value)) = state.get(index) else {
                return Err(EvalError::bug("complete candidate variable missing"));
            };
            match fingerprint.fingerprint_materialized_controlled(
                value,
                self.model.evaluator.options().ctx,
                &mut proceed,
            ) {
                Ok(_) => {}
                Err(ConstructionError::Interrupted) => {
                    return Err(interrupted.take().unwrap_or_else(|| {
                        EvalError::bug("missing fingerprint control transport")
                    }));
                }
                Err(ConstructionError::Allocation) => {
                    return Err(EvalError::resource_limit(
                        "ordered fingerprint scratch unavailable",
                    ));
                }
                Err(ConstructionError::Value(error)) => {
                    return Ok(Some(Event::Refused(error.to_string())));
                }
            }
        }
        Ok(None)
    }
}

fn slots(values: &[Option<Value>]) -> Result<State, EvalError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(values.len())
        .map_err(|_| EvalError::resource_limit("terminal candidate bindings unavailable"))?;
    owned.extend_from_slice(values);
    Ok(Arc::from(owned))
}

fn execution_error(site: Site, error: EvalError) -> Result<Event, EvalError> {
    match error.class() {
        ErrorClass::Eval => Ok(Event::Error {
            site,
            error,
            candidate: None,
        }),
        ErrorClass::Unsupported => Ok(Event::Refused(error.to_string())),
        _ => Err(error),
    }
}

fn with_candidate(event: Event, values: &[Option<Value>]) -> Result<Event, EvalError> {
    Ok(match event {
        Event::Error { site, error, .. } => Event::Error {
            site,
            error,
            candidate: Some(slots(values)?),
        },
        other => other,
    })
}
