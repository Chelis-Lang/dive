//! The sequential search: ADR 0015's level engine run inline, with one
//! logical worker.
//!
//! It reproduces TLC's single-worker breadth-first search
//! (`ModelChecker.DoInitFunctor`, `Worker.run`/`addElement`, tlaplus
//! `94d0c50`) event for event, in the reference order 𝒪 of ADR 0015 §1:
//! levels in order, states in frontier order, successors in enumeration
//! order, and within a successor the phases completeness, constraints,
//! invariants (on new or out-of-model states), implied actions (on every
//! step). The search stops at the first event (E\*): a violation, a
//! deadlock or an error.
//!
//! **Counts at E\*** (ADR 0015 §3). The level is held as a vector, the
//! state being expanded is `current[i]`, and the next level so far is
//! `next`: generated is the running total (every successor before E\*, and
//! E\*'s own), distinct is the set's size (TLC inserts a new state before
//! checking it), and left on the queue is `(m − i − 1) + |next|` (E\*'s own
//! state is never enqueued). These are exactly the closed forms of §3, so a
//! parallel engine (WP-D3) computing them from per-block positions gives
//! the same numbers.
//!
//! **Counterexamples** (§4) are rebuilt by replay from the trace log:
//! regenerate the initial states, or a parent's successors, and take the
//! recorded ordinal. The replayed end state is checked against the state
//! the search held; any difference is an internal error, never a verdict.

use dive_eval::next::{Action, Flow, StateVals, Successor};
use dive_eval::{ErrorClass, EvalError, Evaluator, St};
use dive_ir::actions::ActionName;
use dive_store::state::{StateFingerprinter, state_key};
use dive_store::trace::{StateId, TraceLog};
use dive_store::{ExactSet, FpSet, StateSet, StoreError, StoreMode, collision};
use dive_value::{Ctx, Value};

use dive_eval::{Counter, RunConfig};
use dive_stdlib::host::RunHost;
use dive_stdlib::random::{RandomMode, Streams};

use crate::options::CheckOptions;
use crate::result::{Collision, Counterexample, ErrorSite, Stats, TraceState, Verdict};
use dive_model::{Model, Named};

/// The seen-state set: fingerprints or full states.
enum Store {
    Fp {
        set: FpSet,
        /// The state fingerprint.
        states: Box<StateFingerprinter>,
        /// The fingerprint of a `VIEW` value (`val.fingerPrint(FP64.New())`).
        view: Box<StateFingerprinter>,
    },
    Exact {
        set: ExactSet,
        buf: Vec<u8>,
    },
}

impl Store {
    fn new(opts: &CheckOptions, tlc_order: &[usize]) -> Result<Self, String> {
        let bits = dive_store::shard_bits_for(1);
        Ok(match opts.store {
            StoreMode::Fingerprint => {
                let bad = || {
                    format!(
                        "fingerprint polynomial index {} out of range",
                        opts.fp_index
                    )
                };
                let (states, view) = if opts.tlc_fingerprints {
                    (
                        StateFingerprinter::tlc(opts.fp_index, tlc_order).ok_or_else(bad)?,
                        StateFingerprinter::tlc(opts.fp_index, &[0]).ok_or_else(bad)?,
                    )
                } else {
                    (
                        StateFingerprinter::new(opts.fp_index).ok_or_else(bad)?,
                        StateFingerprinter::new(opts.fp_index).ok_or_else(bad)?,
                    )
                };
                Self::Fp {
                    set: FpSet::new(bits),
                    states: Box::new(states),
                    view: Box::new(view),
                }
            }
            StoreMode::Exact => Self::Exact {
                set: ExactSet::new(bits),
                buf: Vec::new(),
            },
        })
    }

    /// Inserts the state with values `vals` (or, with `view`, the state whose
    /// view value is `vals[0]`), returning whether it was new.
    fn insert(&mut self, vals: &[Value], view: bool, ctx: Ctx) -> Result<bool, StoreError> {
        match self {
            Self::Fp {
                set,
                states,
                view: v,
            } => {
                let fp = if view {
                    v.fingerprint(vals, ctx)?
                } else {
                    states.fingerprint(vals, ctx)?
                };
                Ok(set.insert(fp))
            }
            Self::Exact { set, buf } => {
                buf.clear();
                state_key(vals, ctx, buf)?;
                Ok(set.insert(buf.as_slice().into()))
            }
        }
    }

    fn len(&self) -> u64 {
        match self {
            Self::Fp { set, .. } => set.len(),
            Self::Exact { set, .. } => set.len(),
        }
    }
}

/// The run's host (ADR 0014 §7): the random streams of `--seed`, TLC's
/// under `--compat=tlc` and dive's replayable ones otherwise (D-002), the
/// level, and the counters.
pub(crate) fn run_host(opts: &CheckOptions) -> RunHost {
    let mode = match opts.profile {
        dive_eval::Profile::Tlc => RandomMode::Tlc,
        dive_eval::Profile::Dive => RandomMode::Dive,
    };
    RunHost::new(Streams::new(opts.seed, mode), None)
}

/// `TLCGet("config")` (`AbstractChecker.createConfig`). The worker count is
/// the logical one, 1, by default and the requested one under
/// `--compat=tlc`, as TLC reports `-workers` (D-011); the install location
/// is dive's executable.
pub(crate) fn run_config(opts: &CheckOptions, deadlock: bool) -> RunConfig {
    #[allow(clippy::cast_possible_wrap, reason = "Java's long: same bits")]
    let fingerprint = dive_value::fp64::POLYS
        .get(opts.fp_index)
        .copied()
        .unwrap_or(0) as i64;
    RunConfig {
        deadlock,
        workers: match opts.profile {
            dive_eval::Profile::Tlc => i64::try_from(opts.workers).unwrap_or(i64::MAX),
            dive_eval::Profile::Dive => 1,
        },
        seed: opts.seed,
        fingerprint,
        install: std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}

/// The counters while TLC checks assumptions: no state yet, diameter 1
/// (measured on the pinned TLC, fixture `TgAssume`).
pub(crate) fn startup_counters(host: &RunHost) {
    for c in [
        Counter::Generated,
        Counter::Distinct,
        Counter::Queue,
        Counter::Initial,
    ] {
        host.set_counter(c, Some(0));
    }
    host.set_counter(Counter::Diameter, Some(1));
}

/// During the search the live counters are not provided: reading one is
/// refused rather than answered with a count TLC may not report at that
/// point.
fn search_counters(host: &RunHost) {
    for c in [
        Counter::Generated,
        Counter::Distinct,
        Counter::Queue,
        Counter::Initial,
        Counter::Diameter,
        Counter::DurationSecs,
    ] {
        host.set_counter(c, None);
    }
}

/// Whether the specification refers to a random operator, so that expanded
/// states need their own random stream.
fn uses_random(ir: &dive_ir::Ir) -> bool {
    use dive_ir::{OpRef, StdOp};
    let random = |op: StdOp| {
        matches!(
            op,
            StdOp::TlcRandomElement
                | StdOp::RandomizationRandomSubset
                | StdOp::RandomizationRandomSetOfSubsets
                | StdOp::RandomizationRandomSubsetSet
        )
    };
    ir.nodes().iter().any(|n| {
        n.expr.op_refs().into_iter().any(|r| match r {
            OpRef::Std(op) => random(*op),
            OpRef::Def(d) => ir.def(*d).std.is_some_and(random),
            _ => false,
        })
    })
}

/// TLC's level of a state at BFS level `n` (initial states are level 1).
fn level_of(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// What stopped the search, and how to print its counterexample.
struct Stop {
    verdict: Verdict,
    /// Whether the stop happened while generating the initial states.
    in_init: bool,
    /// The counterexample: the path to this state (id and values)...
    path: Option<(StateId, StateVals)>,
    /// ...followed by this state.
    then: Option<TraceState>,
}

/// The name an action is reported under.
pub(crate) fn action_name(ev: &Evaluator, a: &Action) -> Option<String> {
    match a.name() {
        ActionName::Def(d) => Some(ev.ir().def(d).name.clone()),
        ActionName::Std(op) => Some(op.name().to_owned()),
        ActionName::Builtin(name) => Some(name.to_owned()),
        ActionName::Unnamed => None,
    }
}

/// The verdict for an error raised at `site`.
pub(crate) fn error_verdict(e: &EvalError, site: ErrorSite) -> Verdict {
    match e.class() {
        ErrorClass::Eval => Verdict::EvaluationError {
            site,
            code: e.ec().code(),
            name: e.ec().name(),
            message: e.message().to_owned(),
        },
        ErrorClass::Unsupported => Verdict::Unsupported {
            feature: e.message().to_owned(),
        },
        ErrorClass::ResourceLimit => Verdict::ResourceLimit {
            message: e.message().to_owned(),
        },
        ErrorClass::Bug => Verdict::InternalError {
            message: e.message().to_owned(),
        },
    }
}

fn store_verdict(e: &StoreError) -> Verdict {
    match e {
        StoreError::Value(v) => error_verdict(&EvalError::from(v.clone()), ErrorSite::Fingerprint),
        other => Verdict::InternalError {
            message: other.to_string(),
        },
    }
}

fn values(s: &[Option<Value>]) -> Option<Vec<Value>> {
    s.iter().cloned().collect()
}

struct Search<'a> {
    m: &'a Model,
    host: &'a RunHost,
    /// TLC's state fingerprint, which seeds each expanded state's random
    /// stream.
    seeder: StateFingerprinter,
    /// Whether the specification uses a random operator at all.
    random: bool,
    ctx: Ctx,
    store: Store,
    log: TraceLog,
    generated: u64,
    depth: u64,
    initial_generated: u64,
}

impl<'a> Search<'a> {
    /// Evaluates a check as a boolean.
    fn holds(&self, c: &Named, st: &St, site: ErrorSite) -> Result<bool, Verdict> {
        match self.m.eval(&c.pred, st) {
            Ok(Value::Bool(v)) => Ok(v),
            Ok(_) => Err(Verdict::Unsupported {
                feature: format!(
                    "{} ({}) is not a boolean (TLC's error for this is not reproduced yet)",
                    c.name,
                    site.name()
                ),
            }),
            Err(e) => Err(error_verdict(&e, site)),
        }
    }

    fn trace_state(&self, succ: &Successor<'_>, initial: bool) -> Option<TraceState> {
        let ev = &self.m.evaluator;
        Some(TraceState {
            label: if initial {
                "<Initial predicate>".to_owned()
            } else {
                succ.action.label().to_owned()
            },
            action: if initial {
                None
            } else {
                action_name(ev, succ.action)
            },
            values: values(succ.state)?,
        })
    }

    /// One generated state: TLC's `DoInitFunctor.addElement` (no parent) or
    /// `Worker.addElement`. Returns the verdict that stops the search, and
    /// whether the successor ends the counterexample.
    #[allow(
        clippy::too_many_lines,
        reason = "the phases of one successor, in TLC's order"
    )]
    fn on_state(
        &mut self,
        parent: Option<(&StateVals, StateId)>,
        succ: &Successor<'_>,
        new_level: u64,
        next: &mut Vec<(StateVals, StateId)>,
    ) -> Option<(Verdict, bool)> {
        let r = self.on_state_checks(parent, succ, new_level, next);
        // Back to the level of the state being expanded.
        self.host.set_level(level_of(new_level - 1));
        r
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the phases of one successor, in TLC's order"
    )]
    fn on_state_checks(
        &mut self,
        parent: Option<(&StateVals, StateId)>,
        succ: &Successor<'_>,
        new_level: u64,
        next: &mut Vec<(StateVals, StateId)>,
    ) -> Option<(Verdict, bool)> {
        self.generated += 1;
        // `TLCGet("level")`: the successor's level in state checks, the
        // expanded state's in action checks.
        let (here, from) = (level_of(new_level), level_of(new_level - 1));
        self.host.set_level(here);
        let initial = parent.is_none();
        let m = self.m;
        if !succ.complete() {
            let variables = succ
                .unassigned()
                .iter()
                .map(|&i| m.ir.decl(m.ir.vars()[i]).name.clone())
                .collect();
            return Some((
                Verdict::NotCompletelySpecified { initial, variables },
                false,
            ));
        }
        let t: StateVals = succ.state.into();
        let st1 = St::state(t.clone());
        let st_step = parent.map(|(s0, _)| St::action(s0.clone(), t.clone()));
        let mut in_model = true;
        for c in &m.constraints {
            match self.holds(c, &st1, ErrorSite::Constraint) {
                Ok(true) => {}
                Ok(false) => {
                    in_model = false;
                    break;
                }
                Err(v) => return Some((v, true)),
            }
        }
        if in_model && let Some(st) = &st_step {
            self.host.set_level(from);
            for c in &m.action_constraints {
                match self.holds(c, st, ErrorSite::ActionConstraint) {
                    Ok(true) => {}
                    Ok(false) => {
                        in_model = false;
                        break;
                    }
                    Err(v) => return Some((v, true)),
                }
            }
        }
        self.host.set_level(here);
        let mut fresh = true;
        let mut new_id = None;
        if in_model {
            let vals = values(&t).unwrap_or_default();
            let (key, view) = match &m.view {
                None => (vals, false),
                Some(v) => match m.eval(&v.pred, &st1) {
                    Ok(x) => (vec![x], true),
                    Err(e) => return Some((error_verdict(&e, ErrorSite::View), true)),
                },
            };
            match self.store.insert(&key, view, self.ctx) {
                Ok(true) => {
                    let Ok(ordinal) = u32::try_from(succ.ordinal) else {
                        return Some((
                            Verdict::InternalError {
                                message: "more than 2^32 successors of one state".to_owned(),
                            },
                            false,
                        ));
                    };
                    let id = match parent {
                        None => Ok(self.log.push_initial(ordinal)),
                        Some((_, p)) => self.log.push(p, ordinal),
                    };
                    match id {
                        Ok(id) => new_id = Some(id),
                        Err(e) => return Some((store_verdict(&e), false)),
                    }
                    self.depth = self.depth.max(new_level);
                }
                Ok(false) => fresh = false,
                Err(e) => return Some((store_verdict(&e), true)),
            }
        }
        if fresh {
            for c in &m.invariants {
                match self.holds(c, &st1, ErrorSite::Invariant) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Some((
                            Verdict::InvariantViolated {
                                property: c.name.clone(),
                                initial,
                            },
                            true,
                        ));
                    }
                    Err(v) => return Some((v, true)),
                }
            }
            if initial {
                for c in &m.implied_inits {
                    match self.holds(c, &st1, ErrorSite::ImpliedInit) {
                        Ok(true) => {}
                        Ok(false) => {
                            return Some((
                                Verdict::PropertyViolatedInitially {
                                    property: c.name.clone(),
                                },
                                true,
                            ));
                        }
                        Err(v) => return Some((v, true)),
                    }
                }
            }
        }
        if let Some(st) = &st_step {
            self.host.set_level(from);
            for c in &m.implied_actions {
                match self.holds(c, st, ErrorSite::ImpliedAction) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Some((
                            Verdict::ActionPropertyViolated {
                                property: c.name.clone(),
                            },
                            true,
                        ));
                    }
                    Err(v) => return Some((v, true)),
                }
            }
        }
        if let Some(id) = new_id {
            next.push((t, id));
        }
        None
    }

    /// Runs the search. Returns the stop (if any) and the statistics.
    #[allow(clippy::too_many_lines, reason = "the search loop reads best whole")]
    fn run(&mut self, check_deadlock: bool) -> (Option<Stop>, Stats) {
        let ev: &'a Evaluator = &self.m.evaluator;
        let m = self.m;
        let mut next: Vec<(StateVals, StateId)> = Vec::new();
        let mut stop: Option<Stop> = None;
        search_counters(self.host);
        // Initial states: `DoInitFunctor`, on the start-up stream that the
        // constant definitions and assumptions drew from.
        let r = ev.init_states(&m.init_actions, &mut |succ: &Successor<'_>| match self
            .on_state(None, succ, 1, &mut next)
        {
            None => Flow::Continue,
            Some((verdict, with)) => {
                stop = Some(Stop {
                    verdict,
                    in_init: true,
                    path: None,
                    then: if with {
                        self.trace_state(succ, true)
                    } else {
                        None
                    },
                });
                Flow::Stop
            }
        });
        self.initial_generated = self.generated;
        if let Err(e) = r {
            stop = Some(Stop {
                verdict: error_verdict(&e.error, ErrorSite::Init),
                in_init: true,
                path: None,
                then: None,
            });
        }
        if let Some(s) = stop {
            let stats = self.stats(0);
            return (Some(s), stats);
        }
        // No action at all: success if no state was queued, otherwise TLC's
        // `TLC_STATES_AND_NO_NEXT_ACTION`, without statistics
        // (`ModelChecker.java:233-245`).
        if m.actions.is_empty() && !next.is_empty() {
            let stop = Stop {
                verdict: Verdict::ModelError {
                    code: 2115,
                    name: "TLC_STATES_AND_NO_NEXT_ACTION".to_owned(),
                    message: "No next state actions defined to generate successor states from."
                        .to_owned(),
                },
                in_init: true,
                path: None,
                then: None,
            };
            return (Some(stop), self.stats(next.len() as u64));
        }
        // The levels: `Worker.run`.
        let mut level = 1u64;
        while !next.is_empty() {
            let current = std::mem::take(&mut next);
            let count = current.len();
            for (i, (s0, id)) in current.iter().enumerate() {
                if self.host.exit_requested() {
                    // `TLCSet("exit", TRUE)`: TLC's `stop()` ends the search
                    // as a success once the state that asked is done.
                    let left = (count - i) + next.len();
                    return (None, self.stats(left as u64));
                }
                let before = self.generated;
                if let Err(v) = self.seed_state(s0) {
                    stop = Some(Stop {
                        verdict: v,
                        in_init: false,
                        path: Some((*id, s0.clone())),
                        then: None,
                    });
                    let left = (count - i - 1) + next.len();
                    let stats = self.stats(left as u64);
                    return (stop, stats);
                }
                self.host.set_level(level_of(level));
                let r = ev.successors(&m.actions, s0, &mut |succ: &Successor<'_>| match self
                    .on_state(Some((s0, *id)), succ, level + 1, &mut next)
                {
                    None => Flow::Continue,
                    Some((verdict, with)) => {
                        stop = Some(Stop {
                            verdict,
                            in_init: false,
                            path: Some((*id, s0.clone())),
                            then: if with {
                                self.trace_state(succ, false)
                            } else {
                                None
                            },
                        });
                        Flow::Stop
                    }
                });
                if let Err(e) = r {
                    // The counterexample ends at the state being expanded
                    // (probe `EvalErrorMidLevel`).
                    stop = Some(Stop {
                        verdict: error_verdict(&e.error, ErrorSite::Next),
                        in_init: false,
                        path: Some((*id, s0.clone())),
                        then: None,
                    });
                }
                if stop.is_none() && check_deadlock && self.generated == before {
                    // No successor at all, counting those outside the model
                    // (probes `ConstraintNoDeadlock`, `DeadlockMidLevel`).
                    stop = Some(Stop {
                        verdict: Verdict::Deadlock,
                        in_init: false,
                        path: Some((*id, s0.clone())),
                        then: None,
                    });
                }
                if let Some(s) = stop {
                    let left = (count - i - 1) + next.len();
                    let stats = self.stats(left as u64);
                    return (Some(s), stats);
                }
            }
            level += 1;
        }
        (None, self.stats(0))
    }

    /// Switches the host to the random stream of expanding `s0`: TLC reseeds
    /// with `fingerprint(s0) ^ seed` (`RandomEnumerableValues`).
    fn seed_state(&self, s0: &StateVals) -> Result<(), Verdict> {
        if !self.random {
            return Ok(());
        }
        let vals = values(s0).unwrap_or_default();
        let fp = self
            .seeder
            .fingerprint(&vals, self.ctx)
            .map_err(|e| store_verdict(&e))?;
        self.host.expand(fp);
        Ok(())
    }

    fn stats(&self, left_on_queue: u64) -> Stats {
        Stats {
            generated: self.generated,
            distinct: self.store.len(),
            left_on_queue,
            depth: self.depth,
            initial_generated: self.initial_generated,
        }
    }

    /// Rebuilds the path to state `id` by replay (ADR 0015 §4).
    fn replay(&self, id: StateId) -> Result<Vec<TraceState>, String> {
        let ev = &self.m.evaluator;
        let path = self.log.path(id).map_err(|e| e.to_string())?;
        let mut out: Vec<TraceState> = Vec::new();
        let mut current: Option<StateVals> = None;
        // Step k expands the state at level k (initial states are level 1).
        for (level, step) in (0u64..).zip(path) {
            let want = u64::from(step.ordinal);
            let mut found: Option<(StateVals, TraceState)> = None;
            let mut sink = |succ: &Successor<'_>| {
                if succ.ordinal == want {
                    let initial = current.is_none();
                    found = self
                        .trace_state(succ, initial)
                        .map(|t| (succ.state.into(), t));
                    Flow::Stop
                } else {
                    Flow::Continue
                }
            };
            // TLC replays with the same seeding: a fresh start-up stream for
            // the initial states, then each predecessor's own (D-002).
            let r = match &current {
                None => {
                    self.host.restart();
                    ev.init_states(&self.m.init_actions, &mut sink)
                }
                Some(s) => {
                    self.seed_state(s)
                        .map_err(|v| format!("replay failed: {v:?}"))?;
                    self.host.set_level(level_of(level));
                    ev.successors(&self.m.actions, s, &mut sink)
                }
            };
            if let Err(e) = r
                && found.is_none()
            {
                return Err(format!("replay failed: {}", e.error));
            }
            let Some((s, t)) = found else {
                return Err(format!(
                    "replay did not regenerate successor {} of state {:?}",
                    step.ordinal, step.parent
                ));
            };
            current = Some(s);
            out.push(t);
        }
        Ok(out)
    }

    /// The counterexample of a stop.
    fn counterexample(&self, stop: &Stop) -> Result<Option<Counterexample>, String> {
        let mut states = Vec::new();
        if let Some((id, held)) = &stop.path {
            states = self.replay(*id)?;
            let last = states.last().map(|t| t.values.as_slice());
            let held = values(held);
            if last != held.as_deref() {
                return Err(format!(
                    "the replayed trace does not end in the state the search expanded (state {id})"
                ));
            }
        }
        if let Some(t) = &stop.then {
            states.push(t.clone());
        }
        Ok((!states.is_empty()).then_some(Counterexample { states }))
    }
}

/// What the engine hands back.
pub(crate) struct Outcome {
    pub verdict: Verdict,
    pub stats: Option<Stats>,
    pub tlc_prints_stats: bool,
    pub counterexample: Option<Counterexample>,
    pub collision: Option<Collision>,
    /// `Print` output of the search, in order (not of the replay).
    pub output: Vec<String>,
    /// Notes qualifying the result.
    pub notes: Vec<String>,
}

/// TLC's `checkPostCondition` after a complete search (`ModelChecker.java:283-288`,
/// `Tool.checkPostConditionWithContext`): each postcondition in order, in the
/// empty context; the first that is not `TRUE` decides.
fn postconditions(m: &Model) -> Option<Verdict> {
    let list = match &m.postconditions {
        Ok(list) => list,
        Err(e) => return Some(crate::model_error(e)),
    };
    for pc in list {
        match m.eval(&pc.pred, &St::constant()) {
            Ok(Value::Bool(true)) => {}
            Ok(_) => {
                return Some(Verdict::PostconditionFalse {
                    property: pc.name.clone(),
                });
            }
            Err(e) => {
                return Some(match error_verdict(&e, ErrorSite::Postcondition) {
                    Verdict::EvaluationError { site, message, .. } => Verdict::EvaluationError {
                        site,
                        code: 2405,
                        name: "TLC_POSTCONDITION_EVALUATION_ERROR",
                        message,
                    },
                    other => other,
                });
            }
        }
    }
    None
}

/// Runs the search over `m` (assumptions already checked).
#[allow(
    clippy::too_many_lines,
    reason = "the search and its outcome, in order"
)]
pub(crate) fn search(m: &Model, opts: &CheckOptions, host: &RunHost) -> Outcome {
    let mut notes = Vec::new();
    let tlc_order: Vec<usize> = m.ir.tlc_var_order().iter().map(|v| v.index()).collect();
    let store = match Store::new(opts, &tlc_order) {
        Ok(s) => s,
        Err(message) => {
            return Outcome {
                verdict: Verdict::InternalError { message },
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: Vec::new(),
                notes,
            };
        }
    };
    let Some(seeder) = StateFingerprinter::tlc(opts.fp_index, &tlc_order) else {
        return Outcome {
            verdict: Verdict::InternalError {
                message: format!(
                    "fingerprint polynomial index {} out of range",
                    opts.fp_index
                ),
            },
            stats: None,
            tlc_prints_stats: false,
            counterexample: None,
            collision: None,
            output: Vec::new(),
            notes,
        };
    };
    let mut s = Search {
        m,
        host,
        seeder,
        random: uses_random(&m.ir),
        ctx: opts.eval.ctx,
        store,
        log: TraceLog::new(),
        generated: 0,
        depth: 0,
        initial_generated: 0,
    };
    let (stop, stats) = s.run(m.settings.check_deadlock);
    let mut output = m.take_output();
    let Some(stop) = stop else {
        let collision = match &s.store {
            Store::Fp { set, .. } => {
                let f = collision::tlc_success_figures(stats.distinct, stats.generated, || {
                    set.tlc_min_gap(collision::TLC_DEFAULT_FP_BITS)
                });
                Some(Collision {
                    optimistic: f.optimistic,
                    actual: f.actual,
                })
            }
            Store::Exact { .. } => None,
        };
        let verdict = postconditions(m).unwrap_or(Verdict::Success);
        output.extend(m.take_output());
        return Outcome {
            collision: if verdict == Verdict::Success {
                collision
            } else {
                None
            },
            verdict,
            stats: Some(stats),
            tlc_prints_stats: true,
            counterexample: None,
            output,
            notes,
        };
    };
    // The replay re-evaluates `Init` and `Next`; their output is not repeated.
    let cx = s.counterexample(&stop);
    let _ = m.take_output();
    let (verdict, counterexample) = match cx {
        Ok(c) => (stop.verdict, c),
        Err(message) if s.random => {
            // TLC rebuilds the trace the same way, from a fresh start-up
            // stream, and loses it too when draws made before `Init`
            // (constant definitions, `ASSUME`s, constant-level invariants)
            // shift the initial states (D-002, fixture `TmRandom`).
            notes.push(format!(
                "The counterexample cannot be rebuilt: the specification draws random values before Init, and the initial states regenerated from a fresh start-up stream differ, so TLC prints no trace either (D-002). ({message})"
            ));
            (stop.verdict, None)
        }
        Err(message) => (Verdict::InternalError { message }, None),
    };
    if matches!(&m.postconditions, Ok(l) if !l.is_empty()) {
        notes.push(
            "POSTCONDITION is not evaluated after a violation (TLC evaluates it with the counterexample; its result does not change the verdict).".to_owned(),
        );
    }
    Outcome {
        tlc_prints_stats: !stop.in_init,
        verdict,
        stats: Some(stats),
        counterexample,
        collision: None,
        output,
        notes,
    }
}
