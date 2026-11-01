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
    View,
}

pub(crate) enum Event {
    Error {
        site: Site,
        error: EvalError,
        candidate: Option<State>,
        name: Option<String>,
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
            Site::Constraint | Site::Invariant | Site::ImpliedInit | Site::View => {
                self.level.saturating_add(1)
            }
            Site::Init | Site::Next | Site::ActionConstraint | Site::ImpliedAction => self.level,
        };
        self.host.set_replay_level(level);
    }

    pub(crate) fn poll(&self) -> Result<(), EvalError> {
        self.control
            .map_or(Ok(()), |c| c.check().map_err(EvalError::interrupted))
    }

    pub(crate) fn primary(&self, before_first: bool) -> Result<Event, EvalError> {
        self.primary_with_count(before_first)
            .map(|(event, _)| event)
    }

    pub(crate) fn primary_with_count(&self, before_first: bool) -> Result<(Event, u64), EvalError> {
        self.primary_history(before_first, None)
    }

    pub(crate) fn primary_history(
        &self,
        before_first: bool,
        mut history: Option<&mut History>,
    ) -> Result<(Event, u64), EvalError> {
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
                let result = if self.initial && history.is_some() && emitted > 1 {
                    Ok(Some(Event::Refused(
                        "ordered prefix requires exactly one Init emission".into(),
                    )))
                } else {
                    self.candidate_history(
                        candidate,
                        before_first,
                        known_new,
                        None,
                        history.as_deref_mut(),
                    )
                };
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
                return Ok((event, emitted));
            }
            if let Err(error) = outcome {
                return execution_error(
                    if self.initial { Site::Init } else { Site::Next },
                    error.error,
                )
                .map(|event| (event, emitted));
            }
        }
        Ok((Event::End, emitted))
    }

    /// Reconstructs the first slice's sole complete initial emission.
    pub(crate) fn initial_source(&self) -> Result<Event, EvalError> {
        let mut emitted = false;
        for action in &self.model.init_actions {
            self.poll()?;
            self.at_site(Site::Init);
            let mut decision = None;
            let mut stop = None;
            let outcome =
                self.model
                    .evaluator
                    .init_states(std::slice::from_ref(action), &mut |candidate| {
                        let result = (|| {
                            if emitted {
                                return Ok(Some(Event::Refused(
                                "temporal source-row replay requires exactly one initial emission"
                                    .into(),
                            )));
                            }
                            emitted = true;
                            if !candidate.complete()
                                || slots(candidate.state)?.as_ref() != self.source.as_ref()
                            {
                                return Ok(Some(Event::Refused(
                                    "initial emission differs from the single concrete source"
                                        .into(),
                                )));
                            }
                            let mut admitted = false;
                            let event = self.candidate_observed(
                                candidate,
                                false,
                                true,
                                Some(&mut admitted),
                            )?;
                            if event.is_none() && !admitted {
                                return Ok(Some(Event::Refused(
                                    "initial source is excluded by its constraints".into(),
                                )));
                            }
                            Ok(event)
                        })();
                        self.at_site(Site::Init);
                        match result {
                            Ok(None) => Flow::Continue,
                            Ok(Some(event)) => {
                                decision = Some(event);
                                Flow::Stop
                            }
                            Err(error) => {
                                stop = Some(error);
                                Flow::Stop
                            }
                        }
                    });
            self.poll()?;
            if let Some(error) = stop {
                return Err(error);
            }
            if let Some(event) = decision {
                return Ok(event);
            }
            if let Err(error) = outcome {
                return execution_error(Site::Init, error.error);
            }
        }
        Ok(if emitted {
            Event::End
        } else {
            Event::Refused("Init has no complete source emission".into())
        })
    }

    /// Replays Next and candidate checks, admitting only the concrete source
    /// as a row destination. The implicit stutter therefore has the same sole
    /// destination whether Next completes or fails before a full emission.
    pub(crate) fn source_row(&self) -> Result<Event, EvalError> {
        if self.initial {
            return Ok(Event::Refused(
                "post-Next row cannot use Init generation".into(),
            ));
        }
        let mut emitted = false;
        for action in &self.model.actions {
            self.poll()?;
            self.at_site(Site::Next);
            let mut decision = None;
            let mut stop = None;
            let outcome =
                self.model
                    .evaluator
                    .next_states(action, self.source, 0, &mut |candidate| {
                        emitted = true;
                        let result = (|| {
                            let mut admitted = false;
                            if let Some(event) = self.candidate_observed(
                                candidate,
                                false,
                                false,
                                Some(&mut admitted),
                            )? {
                                return Ok(Some(event));
                            }
                            if admitted && slots(candidate.state)?.as_ref() != self.source.as_ref()
                            {
                                return Ok(Some(Event::Refused(
                            "temporal source-row replay has a distinct concrete destination".into(),
                        )));
                            }
                            Ok(None)
                        })();
                        self.at_site(Site::Next);
                        match result {
                            Ok(None) => Flow::Continue,
                            Ok(Some(event)) => {
                                decision = Some(event);
                                Flow::Stop
                            }
                            Err(error) => {
                                stop = Some(error);
                                Flow::Stop
                            }
                        }
                    });
            self.poll()?;
            if let Some(error) = stop {
                return Err(error);
            }
            if let Some(event) = decision {
                return Ok(event);
            }
            if let Err(error) = outcome {
                return execution_error(Site::Next, error.error);
            }
        }
        if !emitted && self.model.settings.check_deadlock {
            return Ok(Event::Failed {
                check: Check::Ending,
                detail: "Next completes without a successor before temporal row checking".into(),
            });
        }
        Ok(Event::End)
    }

    /// TLC diagnostic doNext enumerates each action before checking that
    /// action's candidates. Retain only that action, with checked reservations.
    /// The runtime is the independently reconstructed primary runtime, whose
    /// own history is retained; no caller cache/seen facts enter this sequence.
    pub(crate) fn diagnostic(&self) -> Result<Event, EvalError> {
        self.diagnostic_mode(false, None)
    }

    /// The real diagnostic tool source hook, without relabelling cached values.
    pub(crate) fn diagnostic_sources(&self) -> Result<Event, EvalError> {
        self.diagnostic_mode(true, None)
    }

    pub(crate) fn diagnostic_history(&self, history: &mut History) -> Result<Event, EvalError> {
        self.diagnostic_mode(false, Some(history))
    }

    fn diagnostic_mode(
        &self,
        sources: bool,
        mut history: Option<&mut History>,
    ) -> Result<Event, EvalError> {
        for action in &self.model.actions {
            self.poll()?;
            self.at_site(Site::Next);
            let mut candidates = Vec::new();
            let mut stop = None;
            let mut capped = false;
            let mut sink = |candidate: &Successor<'_>| {
                if history.as_ref().is_some_and(|history| {
                    history.emissions.saturating_add(candidates.len()) >= 256
                }) {
                    capped = true;
                    return Flow::Stop;
                }
                if let Err(error) = self.poll().and_then(|()| {
                    candidates.try_reserve(1).map_err(|_| {
                        EvalError::resource_limit("diagnostic candidate storage unavailable")
                    })?;
                    candidates.push(slots(candidate.state)?);
                    Ok(())
                }) {
                    stop = Some(error);
                    Flow::Stop
                } else {
                    Flow::Continue
                }
            };
            let outcome = if sources {
                self.model
                    .evaluator
                    .next_states_diagnostic(action, self.source, 0, &mut sink)
            } else {
                self.model
                    .evaluator
                    .next_states(action, self.source, 0, &mut sink)
            };
            self.poll()?;
            if let Some(error) = stop {
                return Err(error);
            }
            if capped {
                return Ok(Event::Refused(
                    "ordered diagnostic exceeds 256 cumulative emissions".into(),
                ));
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
                if let Some(event) =
                    self.candidate_history(&candidate, false, false, None, history.as_deref_mut())?
                {
                    return Ok(event);
                }
            }
        }
        Ok(Event::End)
    }

    fn candidate_observed(
        &self,
        candidate: &Successor<'_>,
        before_first: bool,
        known_new: bool,
        admission: Option<&mut bool>,
    ) -> Result<Option<Event>, EvalError> {
        self.candidate_history(candidate, before_first, known_new, admission, None)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one ordered candidate operation retains its admission and check priority"
    )]
    fn candidate_history(
        &self,
        candidate: &Successor<'_>,
        before_first: bool,
        known_new: bool,
        admission: Option<&mut bool>,
        mut history: Option<&mut History>,
    ) -> Result<Option<Event>, EvalError> {
        self.poll()?;
        if let Some(history) = history.as_deref_mut() {
            history.emissions += 1;
            if history.emissions > 256 {
                return Ok(Some(Event::Refused(
                    "ordered prefix exceeds 256 emissions".into(),
                )));
            }
            if candidate.state.len() > 64
                || candidate.state.iter().any(|v| {
                    !matches!(
                        v,
                        Some(Value::Bool(_) | Value::Int(_) | Value::Str(_) | Value::Model(_))
                    )
                })
            {
                return Ok(Some(Event::Refused(
                    "ordered prefix requires complete scalar states before key demand".into(),
                )));
            }
        }
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
        let check_invariants = if inside {
            if let Some(history) = history {
                let key = match self.key(candidate.state)? {
                    Ok(key) => key,
                    Err(event) => return Ok(Some(with_candidate(event, candidate.state)?)),
                };
                match history.admit(key, candidate.state)? {
                    Ok(new) => new,
                    Err(event) => return Ok(Some(event)),
                }
            } else {
                if let Some(event) = self.fingerprint(candidate.state)? {
                    return Ok(Some(event));
                }
                if !known_new
                    && (!self.model.invariants.is_empty()
                        || self.initial && !self.model.implied_inits.is_empty())
                {
                    return Ok(Some(Event::Refused("ordered replay cannot infer global newness or invariant alias history for an in-model candidate".into())));
                }
                known_new
            }
        } else {
            true
        };
        self.at_site(Site::Invariant);
        if check_invariants {
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
        if let Some(admission) = admission {
            *admission = inside;
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
        match self.model.eval_boolean(&named.pred, state) {
            Ok(value) => Ok(Ok(value)),
            Err(error) => execution_error(site, error).map(|mut event| {
                if let Event::Error { name, .. } = &mut event {
                    *name = Some(named.name.clone());
                }
                Err(event)
            }),
        }
    }

    fn key(&self, state: &[Option<Value>]) -> Result<Result<Key, Event>, EvalError> {
        use dive_value::fp64::Fingerprinter;
        self.poll()?;
        let concrete = state
            .iter()
            .map(|v| {
                v.clone()
                    .ok_or_else(|| EvalError::bug("complete scalar key lacks a value"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ctx = self.model.evaluator.options().ctx;
        let values = if let Some(view) = &self.model.view {
            if let Err(error) = dive_model::symmetry::Reduction::normalize_concrete(&concrete, ctx)
            {
                return Ok(Err(Event::Refused(format!(
                    "ordered concrete normalization: {error}"
                ))));
            }
            self.at_site(Site::View);
            let value = match self.model.eval(&view.pred, &St::state(Arc::from(state))) {
                Ok(value) => value,
                Err(error) => return execution_error(Site::View, error).map(Err),
            };
            let mut work = 0;
            if !scalar_tuple(&value, 0, &mut work) {
                return Ok(Err(Event::Refused(
                    "ordered VIEW key exceeds scalar/tuple shape bounds".into(),
                )));
            }
            vec![value]
        } else if ctx.order == ValueOrder::Tlc {
            self.model
                .ir
                .tlc_var_order()
                .iter()
                .map(|var| concrete[var.index()].clone())
                .collect()
        } else {
            concrete
        };
        // All values were independently shown scalar or bounded materialized tuples.
        // The ordinary value helper therefore has no lazy or unbounded traversal.
        let index = if let Some(config) = self
            .host
            .run_config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            let polynomial = u64::from_ne_bytes(config.fingerprint.to_ne_bytes());
            let Some(index) = dive_value::fp64::POLYS
                .iter()
                .position(|candidate| *candidate == polynomial)
            else {
                return Ok(Err(Event::Refused(
                    "ordered prefix fingerprint polynomial is unproved".into(),
                )));
            };
            index
        } else {
            return Ok(Err(Event::Refused(
                "ordered prefix requires the actual run fingerprint polynomial".into(),
            )));
        };
        let fp = dive_value::fp64::TlcFingerprint::new(index)
            .ok_or_else(|| EvalError::bug("fingerprint polynomial missing"))?
            .fingerprint_state(&values, ctx)?;
        self.poll()?;
        Ok(Ok(Key { values, fp }))
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
            name: None,
        }),
        ErrorClass::Unsupported => Ok(Event::Refused(error.to_string())),
        _ => Err(error),
    }
}

fn with_candidate(event: Event, values: &[Option<Value>]) -> Result<Event, EvalError> {
    Ok(match event {
        Event::Error {
            site, error, name, ..
        } => Event::Error {
            site,
            error,
            name,
            candidate: Some(slots(values)?),
        },
        other => other,
    })
}

// This is a bounded history of the two independently executed source rows,
// never a checker queue or caller-supplied global seen set.
#[derive(Default)]
pub(crate) struct History {
    keys: Vec<Key>,
    pub(crate) admitted: Vec<State>,
    emissions: usize,
}
struct Key {
    values: Vec<Value>,
    fp: u64,
}
impl History {
    fn admit(
        &mut self,
        key: Key,
        state: &[Option<Value>],
    ) -> Result<Result<bool, Event>, EvalError> {
        for old in &self.keys {
            if old.values == key.values {
                return Ok(Ok(false));
            }
            if old.fp == key.fp {
                return Ok(Err(Event::Refused(
                    "ordered prefix key fingerprint collision".into(),
                )));
            }
        }
        if self.keys.len() >= 64 {
            return Ok(Err(Event::Refused(
                "ordered prefix exceeds 64 admitted keys".into(),
            )));
        }
        self.keys
            .try_reserve(1)
            .map_err(|_| EvalError::resource_limit("ordered key storage unavailable"))?;
        self.admitted
            .try_reserve(1)
            .map_err(|_| EvalError::resource_limit("ordered source storage unavailable"))?;
        self.admitted.push(slots(state)?);
        self.keys.push(key);
        Ok(Ok(true))
    }
}
fn scalar_tuple(value: &Value, depth: usize, work: &mut usize) -> bool {
    *work += 1;
    if depth > 8 || *work > 64 {
        return false;
    }
    match value {
        Value::Bool(_) | Value::Int(_) | Value::Str(_) | Value::Model(_) => true,
        Value::Fcn(function) => function.as_tuple().is_some_and(|values| {
            values
                .iter()
                .all(|value| scalar_tuple(value, depth + 1, work))
        }),
        _ => false,
    }
}

#[cfg(test)]
mod history_controls {
    use super::*;
    #[test]
    fn equal_fingerprints_cannot_merge_distinct_keys() -> Result<(), EvalError> {
        let mut history = History::default();
        assert!(matches!(
            history.admit(
                Key {
                    values: vec![Value::Int(0)],
                    fp: 17
                },
                &[Some(Value::Int(0))]
            )?,
            Ok(true)
        ));
        assert!(
            matches!(history.admit(Key { values: vec![Value::Int(1)], fp: 17 }, &[Some(Value::Int(1))])?, Err(Event::Refused(reason)) if reason.contains("collision"))
        );
        assert_eq!(history.admitted.len(), 1);
        Ok(())
    }
}
