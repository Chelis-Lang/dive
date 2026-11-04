//! Error endings (`docs/developer/core/07-engine.md` §5,
//! `02-semantic-contract.md` §3(e)).
//!
//! The explorer records where an evaluation error was raised, with the
//! states involved, and stops. It does not classify the error: the caller
//! does, by the site, as TLC does (`dive_check`'s `class.rs`).
//!
//! An invariant that fails to evaluate on a new successor inside the model
//! is followed by TLC's call-stack replay (`ModelChecker.modelCheckImpl`,
//! `keepCallStack`; the reference engine's `Search::replay_after`). The
//! replay expands the parent again, each action in full before its
//! successors are checked. Past the failing successor, it inserts new states
//! (counted as distinct, not as generated), checks them and queues them,
//! until a violation or an incomplete state ends it quietly, or a further
//! error ends it with that error's site.

use dive_core::plan::Code;
use dive_core::{Error, ErrorKind, Handle, Machine, Sink};

use crate::{Explorer, state_identity};

/// Where an evaluation error was raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// While generating the initial states.
    Init,
    /// While generating the successors of a state, in the action at this
    /// index of [`dive_core::Plan::next`].
    Next(usize),
    /// In the `CONSTRAINT` at this index, on [`Failure::state`].
    Constraint(usize),
    /// In the `ACTION_CONSTRAINT` at this index, on the step to
    /// [`Failure::state`].
    ActionConstraint(usize),
    /// In the invariant at this index, on [`Failure::state`].
    Invariant(usize),
    /// In the implied initial predicate at this index, on the initial
    /// [`Failure::state`].
    ImpliedInit(usize),
    /// In the implied action at this index, on the step to
    /// [`Failure::state`].
    ImpliedAction(usize),
    /// [`Failure::state`] leaves variables unassigned (not an evaluation
    /// error, but an ending with the same shape).
    Incomplete,
    /// [`Failure::state`], a new state inside the model, holds a set TLC
    /// cannot fingerprint, such as `Nat` (TLC's `fingerPrint` error).
    Fingerprint,
    /// In the `VIEW`, on [`Failure::state`], a new state inside the model.
    View,
}

/// An evaluation error that ended the search, or the search's replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Where.
    pub site: Site,
    /// The core's message (not TLC's text; the caller classifies).
    pub message: String,
    /// Whether it was raised on, or while generating, the initial states.
    pub initial: bool,
    /// The state being expanded (persistent handles), unless initial.
    pub parent: Option<Vec<Handle>>,
    /// The state checked (persistent handles; empty handles are unassigned
    /// variables), for a check or an incomplete state.
    pub state: Option<Vec<Handle>>,
    /// The action that emitted [`Failure::state`] (`None`: initial).
    pub action: Option<usize>,
    /// Actual compiled Init action index; distinct from a next-state action.
    pub init_action: Option<usize>,
    /// Whether [`Failure::state`] passed the constraints (for an invariant).
    pub in_model: bool,
    /// For [`Site::Init`] and [`Site::Next`]: the states the failing
    /// predicate or action emitted before the error.
    pub emitted: u64,
}

impl Failure {
    /// Whether TLC's call-stack replay goes on past the state: an invariant
    /// that failed to evaluate on a new successor inside the model.
    #[must_use]
    pub fn site_resumes(&self) -> bool {
        matches!(self.site, Site::Invariant(_)) && !self.initial && self.in_model
    }
}

/// What TLC's call-stack replay changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Replayed {
    /// States it inserted.
    pub distinct: u64,
    /// States it queued.
    pub queued: u64,
    /// A further error it met, which decides TLC's class.
    pub error: Option<Failure>,
}

fn persist(m: &mut Machine, hs: &[Handle]) -> Result<Vec<Handle>, Error> {
    let mut out = Vec::with_capacity(hs.len());
    for &h in hs {
        out.push(if h.is_empty() { h } else { m.heap.intern(h)? });
    }
    Ok(out)
}

impl Explorer<'_> {
    /// Records an evaluation error raised by a check of the state in `m.s1`
    /// and returns the stop; any other error passes through.
    pub(crate) fn fail_check(&mut self, m: &mut Machine, site: Site, e: Error) -> Error {
        if e.kind() != ErrorKind::Eval {
            return e;
        }
        match self.failure_at(m, site, e.message().to_owned()) {
            Ok(f) => {
                self.failure = Some(f);
                Error::stop()
            }
            Err(e) => e,
        }
    }

    /// The failure at `site` for the state in `m.s1`.
    pub(crate) fn failure_at(
        &mut self,
        m: &mut Machine,
        site: Site,
        message: String,
    ) -> Result<Failure, Error> {
        let state = Some(persist(m, &m.s1.clone())?);
        let parent = if self.initial {
            None
        } else {
            Some(m.s0.clone())
        };
        Ok(Failure {
            site,
            message,
            initial: self.initial,
            parent,
            state,
            action: self.action,
            init_action: self.init_action,
            in_model: self.in_model,
            emitted: 0,
        })
    }

    /// The failure of a generator error at the current action.
    pub(crate) fn generator_failure(&self, m: &Machine, message: String) -> Failure {
        Failure {
            site: self.action.map_or(Site::Init, Site::Next),
            message,
            initial: self.initial,
            parent: (!self.initial).then(|| m.s0.clone()),
            state: None,
            action: self.action,
            init_action: self.init_action,
            in_model: false,
            emitted: self.emitted - self.action_start,
        }
    }
}

/// Collects every emission of one action, as persistent handles.
struct Collect {
    states: Vec<(Vec<Handle>, bool)>,
}

impl Sink for Collect {
    fn emit(&mut self, m: &mut Machine, _code: &Code) -> Result<(), Error> {
        let complete = m.assigned == m.s1.len();
        let values = persist(m, &m.s1.clone())?;
        self.states.push((values, complete));
        Ok(())
    }
}

/// TLC's call-stack replay after the invariant failure at emission
/// `ordinal` of the expansion of `m.s0` (module documentation).
///
/// # Errors
/// An error the replay cannot attribute to the specification (a refusal or
/// a core bug).
pub(crate) fn replay_after(
    m: &mut Machine,
    ex: &mut Explorer<'_>,
    ordinal: u64,
) -> Result<Replayed, Error> {
    // This path reuses the primary machine, unlike fresh trace replay. Its
    // entire action/check traversal is diagnostic, including every early exit.
    m.without_debugger(|m| replay_after_unobserved(m, ex, ordinal))
}

fn replay_after_unobserved(
    m: &mut Machine,
    ex: &mut Explorer<'_>,
    ordinal: u64,
) -> Result<Replayed, Error> {
    let plan = ex.plan;
    let code = &plan.code;
    let s0 = m.s0.clone();
    let mut out = Replayed::default();
    let mut added = ex.seen.empty_like();
    let mut emission = 0u64;
    for (index, a) in plan.next.iter().enumerate() {
        ex.action = Some(index);
        let mut sink = Collect { states: Vec::new() };
        let old = m.enter(a.frame);
        for &(slot, v) in &a.preset {
            if let Some(x) = m.frame.get_mut(m.base + slot as usize) {
                *x = v;
            }
        }
        let ran = m.run_action(code, a.g, &mut sink);
        m.leave(old);
        if let Err(e) = ran {
            if e.kind() != ErrorKind::Eval {
                return Err(e);
            }
            // The search reached the failing state without this error, so
            // it lies in that state's action or a later one.
            out.error = Some(Failure {
                site: Site::Next(index),
                message: e.message().to_owned(),
                initial: false,
                parent: Some(s0),
                state: None,
                action: Some(index),
                init_action: None,
                in_model: false,
                emitted: sink.states.len() as u64,
            });
            return Ok(out);
        }
        for (values, complete) in sink.states {
            let o = emission;
            emission += 1;
            if o < ordinal {
                continue;
            }
            if !complete {
                // `doNextSetErr`: an incomplete state ends it quietly.
                return Ok(out);
            }
            m.s0.copy_from_slice(&s0);
            m.s1.copy_from_slice(&values);
            if o == ordinal {
                // The failing state's own step: only its implied actions.
                if !replay_implied(m, ex, &mut out)? {
                    return Ok(out);
                }
                continue;
            }
            ex.changed = crate::changed_vars(m);
            let inside = match ex.in_model_of(m) {
                Ok(b) => b,
                Err((site, e)) => {
                    if e.kind() != ErrorKind::Eval {
                        return Err(e);
                    }
                    ex.in_model = true;
                    out.error = Some(ex.failure_at(m, site, e.message().to_owned())?);
                    return Ok(out);
                }
            };
            let mut new = true;
            if inside {
                let mut identity = Vec::new();
                let key =
                    match state_identity(m, plan, ex.seen.exact.as_ref().map(|_| &mut identity)) {
                        Ok(k) => k,
                        Err(e) if e.kind() == ErrorKind::Eval && plan.view.is_some() => {
                            out.error =
                                Some(ex.failure_at(m, Site::View, e.message().to_owned())?);
                            return Ok(out);
                        }
                        Err(e) => return Err(e),
                    };
                new = !ex.seen_contains(key, &identity, &m.heap, ("ReplayMembership", Some(o)))?
                    && (ex.disk.is_some()
                        || !added.contains_row(key, &identity, &m.heap, ex.poll)?);
                if new {
                    if ex.disk.is_some() {
                        ex.seen_insert(key, &identity, &m.heap, ("ReplayInsertion", Some(o)))?;
                    } else {
                        added.insert_row(key, &identity, &m.heap, ex.poll)?;
                    }
                    out.distinct += 1;
                }
            }
            if new && !replay_invariants(m, ex, inside, &mut out)? {
                return Ok(out);
            }
            if !replay_implied(m, ex, &mut out)? {
                return Ok(out);
            }
            if inside && new {
                out.queued += 1;
            }
        }
    }
    Ok(out)
}

/// The invariants on the replayed state `m.s1`: whether the replay goes
/// on. A violation ends it quietly; an error ends it with the error
/// recorded.
fn replay_invariants(
    m: &mut Machine,
    ex: &mut Explorer<'_>,
    inside: bool,
    out: &mut Replayed,
) -> Result<bool, Error> {
    let plan = ex.plan;
    for (i, c) in plan.invariants.iter().enumerate() {
        match ex.check(m, &plan.code, c) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(e) => {
                if e.kind() != ErrorKind::Eval {
                    return Err(e);
                }
                ex.in_model = inside;
                out.error = Some(ex.failure_at(m, Site::Invariant(i), e.message().to_owned())?);
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// The implied actions on the replayed step from `m.s0` to `m.s1`: whether
/// the replay goes on. A violation ends it quietly; an error ends it with
/// the error recorded (the reference engine's `record_error`).
fn replay_implied(
    m: &mut Machine,
    ex: &mut Explorer<'_>,
    out: &mut Replayed,
) -> Result<bool, Error> {
    let plan = ex.plan;
    for (i, c) in plan.implied_actions.iter().enumerate() {
        match crate::check_step(m, &plan.code, c) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(e) => {
                if e.kind() != ErrorKind::Eval {
                    return Err(e);
                }
                out.error =
                    Some(ex.failure_at(m, Site::ImpliedAction(i), e.message().to_owned())?);
                return Ok(false);
            }
        }
    }
    Ok(true)
}
