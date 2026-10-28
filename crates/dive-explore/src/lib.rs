//! `dive-explore`: breadth-first exploration over a compiled
//! [`dive_core::Plan`] (`docs/developer/core/07-engine.md`). With several
//! workers, each level is expanded in parallel and committed in order
//! (`parallel.rs`), so every result is the one-worker result.
//!
//! The loop follows TLC's `ModelChecker` with one worker:
//!
//! - every emitted state counts as generated, before any check;
//! - an initial state, or a successor, that is not completely specified is
//!   an error;
//! - constraints are evaluated before the seen check; a state outside the
//!   model is not inserted, but its invariants are still checked if it was
//!   not seen;
//! - invariants are checked only for new states;
//! - a state with no successors at all is a deadlock when deadlock is
//!   checked.
//!
//! Successors are probed against the seen set with an incremental hash
//! computed from the transient buffer, and only new states are interned (S3).
//! Invariant and constraint conjuncts whose read set does not meet the
//! variables a step changed are skipped: the parent satisfied them (S6).
//!
//! # Example
//!
//! The counts of a run are reported in [`Outcome::stats`]; see
//! `tests/golden.rs` for complete runs from TLA+ sources.
#![forbid(unsafe_code)]

pub mod ending;
pub mod graph;
mod parallel;
pub mod sample;
pub mod trace;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_core::handle::{K_STATE, fin, state_term};
use dive_core::memo::{Memo, SiteStats};
use dive_core::plan::{Check, Code};
use dive_core::store::StoreStats;
use dive_core::{Error, ErrorKind, Handle, Machine, Opts, Plan, Sink, Store};
use dive_store::predecessor::PredecessorLog;
use dive_store::trace::StateId;

pub use ending::{Failure, Replayed, Site};
pub use trace::TraceState;

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The state space was exhausted with no violation.
    Success,
    /// An invariant was violated.
    InvariantViolated {
        /// The invariant's name.
        property: String,
        /// Whether the violating state is initial.
        initial: bool,
    },
    /// A reachable state has no successors.
    Deadlock,
    /// Initial states were retained, but no next-state action was defined
    /// (TLC's `TLC_STATES_AND_NO_NEXT_ACTION`, rather than a deadlock).
    NoNextAction,
    /// An implied initial predicate of a `PROPERTY` is false on an initial
    /// state.
    PropertyViolatedInitially {
        /// The property's name.
        property: String,
    },
    /// An implied action `[A]_v` of a `PROPERTY` is false on a step.
    ActionPropertyViolated {
        /// The property's name.
        property: String,
    },
    /// An evaluation error, or a state that is not completely specified,
    /// ended the search: [`Outcome::failure`] says where.
    Failed,
    /// A construct, size or error the M0 core does not handle.
    Unsupported {
        /// What it was.
        feature: String,
    },
    /// The run's budget (time or memory) ran out: the caller's poll said so.
    Budget,
    /// The next new state would exceed a state or depth limit.
    Admission {
        /// Which limit.
        limit: Limit,
        /// Its value.
        max: u64,
        /// The admission that was prevented: the distinct count or the depth
        /// the state would have had.
        observed: u64,
    },
    /// The graph sink ended the search ([`graph::GraphSink`]); it keeps the
    /// reason.
    GraphStop,
    /// The graph sink stopped the search after a completed level, whose
    /// frontier is not exhausted.
    PrefixStop {
        /// The completed level (the initial states are level 1).
        level: u64,
    },
    /// A bug in the core.
    Internal {
        /// What happened.
        message: String,
    },
}

/// An admission limit (`--max-states`, `--max-depth`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// Distinct states inside the model.
    States,
    /// Breadth-first depth (initial states have depth one).
    Depth,
}

/// TLC's statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// States generated (emitted), initial ones included.
    pub generated: u64,
    /// Distinct states found.
    pub distinct: u64,
    /// The number of BFS levels reached (initial states are level 1).
    pub depth: u64,
    /// States left on the queue when the run ended.
    pub left_on_queue: u64,
    /// Initial states generated.
    pub initial_generated: u64,
}

/// Strategy metrics (`09-performance-method.md` §4).
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// Successors discarded as seen before any interning (S3).
    pub discarded_before_intern: u64,
    /// Successors probed against the seen set.
    pub probed: u64,
    /// Invariant and constraint conjunct evaluations needed.
    pub conjuncts_total: u64,
    /// Of those, skipped by the parent-delta rule.
    pub conjuncts_skipped: u64,
    /// Store intern statistics at the end.
    pub store: StoreStats,
    /// Memo statistics per site, with the site names.
    pub memo: Vec<(String, SiteStats)>,
    /// The largest arena epoch: nodes, pool handles.
    pub arena_peak: (usize, usize),
    /// Thunks forced and thunk reads answered from cache.
    pub thunks: (u64, u64),
    /// Wall time spent exploring (after compilation).
    pub explore_time: Duration,
    /// With several workers: wall time in the parallel expand phase, and in
    /// the sequential commit phase (`parallel.rs`).
    pub expand_time: Duration,
    /// See [`Metrics::expand_time`].
    pub commit_time: Duration,
}

/// The result of a run.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// How it ended.
    pub verdict: Verdict,
    /// TLC's statistics.
    pub stats: Stats,
    /// Strategy metrics.
    pub metrics: Metrics,
    /// Warnings 2143 (variable, `UNCHANGED` site as an `ExprId` index), once each, in the
    /// order raised.
    pub warnings: Vec<(u16, u32)>,
    /// The counterexample of a violation, deadlock or failure, initial
    /// state first, rebuilt by replay; `Err` when the replay failed (a core
    /// bug). After a failure in a check, the last state is the one checked.
    pub trace: Option<Result<Vec<TraceState>, String>>,
    /// Where the search failed ([`Verdict::Failed`]).
    pub failure: Option<Failure>,
    /// Whether the run stopped while generating the initial states.
    pub in_init: bool,
    /// What TLC's call-stack replay changed after an invariant failed to
    /// evaluate on a new successor.
    pub replayed: Option<Replayed>,
    /// The sampled expanded states ([`Config::sample`]), as persistent
    /// handles and their actual source evaluation levels.
    pub sampled: Vec<sample::SampledState>,
    /// What `Print` and `PrintT` printed during the search, in TLC's order
    /// (a plan that prints runs one worker: [`Plan::effects`]).
    pub printed: Vec<String>,
}

/// Run settings.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Strategy switches.
    pub opts: Opts,
    /// At most this many distinct states (`--max-states`).
    pub max_states: Option<u64>,
    /// At most this depth (`--max-depth`).
    pub max_depth: Option<u64>,
    /// Worker threads. Every observable result is the one-worker result.
    pub workers: usize,
    /// TLC's warning switch (`-nowarning` turns it off): it also selects
    /// the all-assigned shortcut for successors (`05` §5).
    pub warn: bool,
    /// Keep a sample of the expanded states ([`Outcome::sampled`]).
    pub sample: Option<sample::Sample>,
}

/// A set of 64-bit state keys (open addressing, linear probing).
#[derive(Debug)]
pub struct Seen {
    table: Vec<u64>,
    len: usize,
}

impl Default for Seen {
    fn default() -> Self {
        Self {
            table: vec![0; 1 << 16],
            len: 0,
        }
    }
}

impl Seen {
    fn fix(k: u64) -> u64 {
        if k == 0 { 1 } else { k }
    }

    fn mask(&self) -> usize {
        self.table.len() - 1
    }

    #[allow(clippy::cast_possible_truncation, reason = "masked to the table size")]
    fn start(&self, k: u64) -> usize {
        (k as usize) & self.mask()
    }

    /// Whether `k` is present.
    #[must_use]
    pub fn contains(&self, k: u64) -> bool {
        let k = Self::fix(k);
        let mut i = self.start(k);
        loop {
            match self.table[i] {
                0 => return false,
                x if x == k => return true,
                _ => i = (i + 1) & self.mask(),
            }
        }
    }

    /// Inserts `k`; returns whether it was new.
    pub fn insert(&mut self, k: u64) -> bool {
        if (self.len + 1) * 4 > self.table.len() * 3 {
            self.grow();
        }
        let k = Self::fix(k);
        let mut i = self.start(k);
        loop {
            match self.table[i] {
                0 => {
                    self.table[i] = k;
                    self.len += 1;
                    return true;
                }
                x if x == k => return false,
                _ => i = (i + 1) & self.mask(),
            }
        }
    }

    fn grow(&mut self) {
        let size = self.table.len() * 2;
        let old = std::mem::replace(&mut self.table, vec![0; size]);
        for k in old.into_iter().filter(|&k| k != 0) {
            let mut i = self.start(k);
            while self.table[i] != 0 {
                i = (i + 1) & self.mask();
            }
            self.table[i] = k;
        }
    }

    /// The number of keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no key is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The state key of `m.s1`: with a `VIEW`, the key of its value; under
/// `SYMMETRY`, the key of its canonical image (`dive_core::symmetry`).
///
/// # Errors
/// The VIEW's evaluation error (kind `Eval`), or a VIEW value the core
/// cannot key (unsupported).
fn state_key(m: &mut Machine, plan: &Plan) -> Result<u64, Error> {
    if let Some(view) = &plan.view {
        return view_key(m, plan, view);
    }
    let image;
    let state: &[Handle] = match &plan.symmetry {
        Some(sym) => {
            let mut out = Vec::with_capacity(m.s1.len());
            let s1 = m.s1.clone();
            sym.canonical(&mut m.heap, &s1, &mut out)?;
            image = out;
            &image
        }
        None => &m.s1,
    };
    let mut sum = 0u64;
    for (i, &h) in state.iter().enumerate() {
        sum = sum.wrapping_add(state_term(i, m.heap.hash(h)));
    }
    Ok(fin(K_STATE, state.len() as u64, sum))
}

/// The key of the `VIEW`'s value on `m.s1`. TLC normalizes the state's
/// variables before it evaluates the VIEW (`TLCStateMut.fingerPrint`); the
/// core interns them, and refuses a state holding an infinite set, whose
/// normalization it does not reproduce. The value is interned too and must
/// hold no infinite set, which TLC fails to fingerprint (refused here
/// rather than reported).
fn view_key(m: &mut Machine, plan: &Plan, view: &Check) -> Result<u64, Error> {
    let Some(conj) = view.conjuncts.first() else {
        return Err(Error::internal("a VIEW without its expression"));
    };
    if !fingerprintable(m, true)? {
        return Err(Error::unsupported(
            "a VIEW over a state holding an infinite or lazily represented set (core)",
        ));
    }
    // Under SYMMETRY, TLC evaluates the VIEW on the canonical image
    // (`TLCStateMut.fingerPrint`, the reference engine's `state_key_of`).
    let image = match &plan.symmetry {
        Some(sym) => {
            let mut out = Vec::with_capacity(m.s1.len());
            let s1 = m.s1.clone();
            sym.canonical(&mut m.heap, &s1, &mut out)?;
            Some(out)
        }
        None => None,
    };
    // The VIEW reads the image, or the state itself, as its current state.
    let saved = if let Some(image) = image {
        Some(std::mem::replace(&mut m.s0, image))
    } else {
        std::mem::swap(&mut m.s0, &mut m.s1);
        None
    };
    let old = m.enter(view.frame);
    let r = m.eval_norm(&plan.code, conj.p);
    m.leave(old);
    if let Some(s0) = saved {
        m.s0 = s0;
    } else {
        std::mem::swap(&mut m.s0, &mut m.s1);
    }
    let v = m.heap.intern(r?)?;
    if !closed_fingerprintable(&m.heap, v) {
        return Err(Error::unsupported(
            "a VIEW value holding an infinite or lazily represented set (core)",
        ));
    }
    Ok(fin(K_STATE, 1, state_term(0, m.heap.hash(v))))
}

struct Explorer<'p> {
    plan: &'p Plan,
    /// The behavior graph's sink, when a temporal check needs it.
    graph: Option<&'p mut dyn graph::GraphSink>,
    stats: Stats,
    metrics: Metrics,
    seen: Seen,
    next: Vec<Handle>,
    emitted: u64,
    initial: bool,
    /// Variables the current step changed (bit per variable); all ones for
    /// initial states.
    changed: u64,
    /// The BFS level of the states being emitted (initial states are 1).
    level: u64,
    opts: Opts,
    stop: Option<Verdict>,
    /// The index of the action being run (`None` for initial predicates).
    action: Option<usize>,
    /// Parent pointers and ordinals of every enqueued state.
    log: PredecessorLog,
    /// The log ids of the states in `next`, in order.
    next_ids: Vec<StateId>,
    /// The log id of the state being expanded.
    cur_id: Option<StateId>,
    /// How the trace of a stop ends: the state to replay up to, and the
    /// last state when it is not a logged one.
    end: Option<End>,
    /// Whether the state being checked passed the constraints.
    in_model: bool,
    /// `emitted` when the current action started.
    action_start: u64,
    /// The failure that stopped the search.
    failure: Option<Failure>,
    /// Whether the budget has run out (polled once per expanded state and
    /// per initial state).
    poll: &'p (dyn Fn() -> bool + Sync),
    max_states: Option<u64>,
    max_depth: Option<u64>,
    /// Warnings in the order raised, when levels run in parallel (each
    /// worker's machine keeps its own).
    warnings: Vec<(u16, u32)>,
}

/// The end of a counterexample, recorded at the stop.
struct End {
    /// The last logged state of the behavior (`None`: an initial state).
    upto: Option<StateId>,
    /// The final state, when it is not logged: the violating successor,
    /// with the action that emitted it.
    last: Option<(Option<usize>, Vec<Handle>)>,
    /// The values of `upto` as the search held them; the replay must
    /// reproduce them.
    expect: Vec<Handle>,
}

enum Probe {
    Seen,
    New,
}

impl Explorer<'_> {
    /// Evaluates a check on the state in `m.s1`.
    fn check(&mut self, m: &mut Machine, code: &Code, c: &Check) -> Result<bool, Error> {
        let skip = self.opts.parent_skip && !self.initial;
        eval_check(m, code, c, skip.then_some(self.changed), &mut self.metrics)
    }

    /// Whether the state in `m.s1` satisfies the constraints, and the
    /// action constraints on the step to it; on an error, its site.
    fn in_model_of(&mut self, m: &mut Machine) -> Result<bool, (Site, Error)> {
        let skip = self.opts.parent_skip && !self.initial;
        in_model(
            m,
            self.plan,
            !self.initial,
            skip.then_some(self.changed),
            &mut self.metrics,
        )
    }

    fn probe(&mut self, m: &mut Machine) -> Result<(Probe, bool, u64), Error> {
        let in_model = match self.in_model_of(m) {
            Ok(b) => b,
            Err((site, e)) => {
                let e = self.fail_check(m, site, e);
                if self.failure.is_some() {
                    self.end_here(m)?;
                }
                return Err(e);
            }
        };
        self.in_model = in_model;
        if !self.opts.early_probe {
            for i in 0..m.s1.len() {
                m.s1[i] = m.heap.intern(m.s1[i])?;
            }
        }
        if !in_model && (self.plan.symmetry.is_some() || self.plan.view.is_some()) {
            // TLC checks a state outside the model without looking it up
            // (`isSeenState` is for states inside it). Without symmetry or a
            // VIEW a lookup only skips a state already checked; with them,
            // a seen image or view value may hide a concrete state that is
            // not.
            return Ok((Probe::New, false, 0));
        }
        let key = match state_key(m, self.plan) {
            Ok(k) => k,
            Err(e) if e.kind() == ErrorKind::Eval && self.plan.view.is_some() => {
                self.failure = Some(self.failure_at(m, Site::View, e.message().to_owned())?);
                self.end_here(m)?;
                return Err(Error::stop());
            }
            Err(e) => return Err(e),
        };
        self.metrics.probed += 1;
        if in_model && (self.max_states.is_some() || self.max_depth.is_some()) {
            self.admit(key)?;
        }
        let new = if in_model {
            self.seen.insert(key)
        } else {
            !self.seen.contains(key)
        };
        if !new {
            self.metrics.discarded_before_intern += u64::from(self.opts.early_probe);
            return Ok((Probe::Seen, in_model, key));
        }
        Ok((Probe::New, in_model, key))
    }

    fn accept(&mut self, m: &mut Machine) -> Result<(), Error> {
        let plan = self.plan;
        let code = &plan.code;
        if m.assigned != m.s1.len() {
            let message = incomplete_message(plan, m);
            self.failure = Some(self.failure_at(m, Site::Incomplete, message)?);
            self.end_here(m)?;
            return Err(Error::stop());
        }
        if !self.initial {
            self.changed = changed_vars(m);
        }
        let (probe, in_model, key) = self.probe(m)?;
        if matches!(probe, Probe::Seen) {
            // TLC checks the implied actions on every step, to a seen state
            // too (`Search::successor`).
            if !self.initial {
                self.implied_actions(m)?;
            }
            return if in_model {
                self.record(m, key, false)
            } else {
                Ok(())
            };
        }
        // With a VIEW, TLC fingerprints the view's value instead
        // (`view_key`).
        if in_model && self.plan.view.is_none() && !fingerprintable(m, self.initial)? {
            self.failure = Some(self.failure_at(m, Site::Fingerprint, FP_MESSAGE.into())?);
            self.end_here(m)?;
            return Err(Error::stop());
        }
        // TLC inserts a new state, counting it and its level, before it
        // checks the invariants.
        if in_model {
            self.stats.distinct += 1;
            self.stats.depth = self.stats.depth.max(self.level);
        }
        // TLC enqueues a new initial state before checking its invariants
        // (`DoInitFunctor.addElement`), and a successor only after
        // (`Worker.addElement`).
        let enqueue_first = self.initial && in_model;
        if enqueue_first {
            self.enqueue(m)?;
        }
        for (i, c) in plan.invariants.iter().enumerate() {
            let holds = match self.check(m, code, c) {
                Ok(b) => b,
                Err(e) => {
                    let e = self.fail_check(m, Site::Invariant(i), e);
                    if self.failure.is_some() {
                        self.end_here(m)?;
                    }
                    return Err(e);
                }
            };
            if !holds {
                self.stop = Some(Verdict::InvariantViolated {
                    property: c.name.clone(),
                    initial: self.initial,
                });
                self.end_here(m)?;
                return Err(Error::stop());
            }
        }
        if self.initial {
            for (i, c) in plan.implied_inits.iter().enumerate() {
                let holds = match self.check(m, code, c) {
                    Ok(b) => b,
                    Err(e) => {
                        let e = self.fail_check(m, Site::ImpliedInit(i), e);
                        if self.failure.is_some() {
                            self.end_here(m)?;
                        }
                        return Err(e);
                    }
                };
                if !holds {
                    self.stop = Some(Verdict::PropertyViolatedInitially {
                        property: c.name.clone(),
                    });
                    self.end_here(m)?;
                    return Err(Error::stop());
                }
            }
        } else {
            self.implied_actions(m)?;
        }
        if in_model && !enqueue_first {
            self.enqueue(m)?;
        }
        if in_model {
            self.record(m, key, true)?;
        }
        Ok(())
    }

    /// Reports the accepted state inside the model with key `key` to the
    /// graph sink; a `new` state is the last one enqueued.
    fn record(&mut self, m: &mut Machine, key: u64, new: bool) -> Result<(), Error> {
        let Some(sink) = self.graph.as_deref_mut() else {
            return Ok(());
        };
        let nvars = m.s1.len();
        let new = if new {
            let id = self
                .next_ids
                .last()
                .copied()
                .ok_or_else(|| Error::internal("a new state was not enqueued"))?;
            let values = self
                .next
                .get(self.next.len().saturating_sub(nvars)..)
                .ok_or_else(|| Error::internal("a new state's values were not enqueued"))?;
            Some((id, values))
        } else {
            None
        };
        let step = graph::Step {
            key,
            new,
            parent: self.cur_id,
            ordinal: self.emitted.saturating_sub(1),
            action: self.action,
        };
        if sink.state(&mut m.heap, step).is_err() {
            self.stop = Some(Verdict::GraphStop);
            return Err(Error::stop());
        }
        Ok(())
    }

    /// Reports the `emitted`-th state of its source to the graph sink,
    /// before its checks.
    fn report_emitted(&mut self) -> Result<(), Error> {
        let ordinal = (!self.initial).then(|| self.emitted.saturating_sub(1));
        let generated = self.stats.generated;
        if let Some(sink) = self.graph.as_deref_mut()
            && sink.emitted(generated, ordinal).is_err()
        {
            self.stop = Some(Verdict::GraphStop);
            return Err(Error::stop());
        }
        Ok(())
    }

    /// The implied actions on the step from `m.s0` to `m.s1`: the first
    /// false one stops the search, its trace ending with the successor.
    fn implied_actions(&mut self, m: &mut Machine) -> Result<(), Error> {
        // An action check reads the expanded state's `TLCGet("level")`.
        m.level -= 1;
        let r = self.implied_actions_at(m);
        m.level += 1;
        r
    }

    fn implied_actions_at(&mut self, m: &mut Machine) -> Result<(), Error> {
        let plan = self.plan;
        for (i, c) in plan.implied_actions.iter().enumerate() {
            let holds = match check_step(m, &plan.code, c) {
                Ok(b) => b,
                Err(e) => {
                    let e = self.fail_check(m, Site::ImpliedAction(i), e);
                    if self.failure.is_some() {
                        self.end_here(m)?;
                    }
                    return Err(e);
                }
            };
            if !holds {
                self.stop = Some(Verdict::ActionPropertyViolated {
                    property: c.name.clone(),
                });
                self.end_here(m)?;
                return Err(Error::stop());
            }
        }
        Ok(())
    }

    /// Stops before a new state inside the model that would exceed
    /// `--max-states` or `--max-depth` (the reference engine's
    /// `admission_stop`); a state already seen is never refused.
    fn admit(&mut self, key: u64) -> Result<(), Error> {
        if self.seen.contains(key) {
            return Ok(());
        }
        let states = self.stats.distinct + 1;
        let stop = if let Some(max) = self.max_states
            && states > max
        {
            Some((Limit::States, max, states))
        } else if let Some(max) = self.max_depth
            && self.level > max
        {
            Some((Limit::Depth, max, self.level))
        } else {
            None
        };
        if let Some((limit, max, observed)) = stop {
            self.stop = Some(Verdict::Admission {
                limit,
                max,
                observed,
            });
            return Err(Error::stop());
        }
        Ok(())
    }

    /// Interns the successor in `m.s1`, queues it and logs its parent and
    /// ordinal.
    fn enqueue(&mut self, m: &mut Machine) -> Result<(), Error> {
        for i in 0..m.s1.len() {
            let h = m.heap.intern(m.s1[i])?;
            self.next.push(h);
        }
        let ordinal = u32::try_from(self.emitted.saturating_sub(1))
            .map_err(|_| Error::unsupported("more than 2^32 successors of one state"))?;
        let id = match self.cur_id {
            None => self.log.push_initial(ordinal),
            Some(parent) => self.log.push(parent, ordinal),
        }
        .map_err(|e| Error::internal(format!("predecessor log: {e}")))?;
        self.next_ids.push(id);
        Ok(())
    }

    /// Records the successor in `m.s1` as the last state of the trace.
    fn end_here(&mut self, m: &mut Machine) -> Result<(), Error> {
        let mut values = Vec::with_capacity(m.s1.len());
        for &h in &m.s1 {
            values.push(if h.is_empty() { h } else { m.heap.intern(h)? });
        }
        let expect = if self.cur_id.is_some() {
            m.s0.clone()
        } else {
            Vec::new()
        };
        self.end = Some(End {
            upto: self.cur_id,
            last: Some((self.action, values)),
            expect,
        });
        Ok(())
    }
}

/// Evaluates a check on the state in `m.s1`. With `skip`, the variables the
/// step changed: a conjunct that reads none of them is skipped, since the
/// parent satisfied it (S6).
pub(crate) fn eval_check(
    m: &mut Machine,
    code: &Code,
    c: &Check,
    skip: Option<u64>,
    metrics: &mut Metrics,
) -> Result<bool, Error> {
    std::mem::swap(&mut m.s0, &mut m.s1);
    let old = m.enter(c.frame);
    let mut r = Ok(true);
    for conj in &c.conjuncts {
        metrics.conjuncts_total += 1;
        if let Some(changed) = skip
            && conj.skippable
            && conj.reads & changed == 0
        {
            metrics.conjuncts_skipped += 1;
            continue;
        }
        match m.eval_bool(code, conj.p) {
            Ok(true) => {}
            Ok(false) => {
                r = Ok(false);
                break;
            }
            Err(e) => {
                r = Err(e);
                break;
            }
        }
    }
    m.leave(old);
    std::mem::swap(&mut m.s0, &mut m.s1);
    r
}

/// Whether the state in `m.s1` satisfies the constraints, and (`step`) the
/// action constraints on the step from `m.s0`; on an error, its site.
pub(crate) fn in_model(
    m: &mut Machine,
    plan: &Plan,
    step: bool,
    skip: Option<u64>,
    metrics: &mut Metrics,
) -> Result<bool, (Site, Error)> {
    let code = &plan.code;
    for (i, c) in plan.constraints.iter().enumerate() {
        if !eval_check(m, code, c, skip, metrics).map_err(|e| (Site::Constraint(i), e))? {
            return Ok(false);
        }
    }
    // `ACTION_CONSTRAINT`s, on the step from the parent (TLC's
    // `isInActions`, after `isInModel`); never on initial states.
    if step {
        // An action check reads the expanded state's `TLCGet("level")`.
        m.level -= 1;
        let r = (|| {
            for (i, c) in plan.action_constraints.iter().enumerate() {
                if !check_step(m, code, c).map_err(|e| (Site::ActionConstraint(i), e))? {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        m.level += 1;
        return r;
    }
    Ok(true)
}

/// The core's message for a state in `m.s1` that leaves variables
/// unassigned.
pub(crate) fn incomplete_message(plan: &Plan, m: &Machine) -> String {
    let missing: Vec<&str> =
        m.s1.iter()
            .zip(&plan.vars)
            .filter(|(h, _)| h.is_empty())
            .map(|(_, n)| n.as_str())
            .collect();
    format!("not completely specified: {}", missing.join(", "))
}

/// The variables that differ between `m.s0` and `m.s1` (bit per variable).
pub(crate) fn changed_vars(m: &Machine) -> u64 {
    let mut changed = 0;
    for (i, (a, b)) in m.s0.iter().zip(&m.s1).enumerate() {
        if a != b {
            changed |= 1u64 << i;
        }
    }
    changed
}

/// The core's message for [`Site::Fingerprint`]; the caller reports TLC's.
const FP_MESSAGE: &str = "a state holds a set that cannot be fingerprinted";

/// Whether TLC can fingerprint the new state in `m.s1`, which it does before
/// inserting a state inside the model (`ErFpInfinite`). The variables are
/// interned in place first, which closes every finite set, so any set kind
/// left is an infinite one (`Nat`, `Int`, an infinite `Seq(S)` and the
/// like). Only the variables that differ from the parent's are read: the
/// parent was fingerprinted. A flagged state the reference fingerprinter
/// accepts is refused when the ending is reported, never reported as an
/// error.
///
/// # Errors
/// A value too large to intern.
pub(crate) fn fingerprintable(m: &mut Machine, initial: bool) -> Result<bool, Error> {
    for i in 0..m.s1.len() {
        let h = m.s1[i];
        if h.is_empty() || !h.is_compound() || (!initial && m.s0.get(i) == Some(&h)) {
            continue;
        }
        let p = m.heap.intern(h)?;
        m.s1[i] = p;
        if !closed_fingerprintable(&m.heap, p) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether an interned value holds no infinite set.
fn closed_fingerprintable(heap: &dive_core::Heap, h: Handle) -> bool {
    use dive_core::node::Kind;
    if !h.is_compound() {
        return true;
    }
    let v = heap.view(h);
    match v.kind {
        Kind::Set | Kind::IntFcn | Kind::Fcn => {
            v.data.iter().all(|&d| closed_fingerprintable(heap, d))
        }
        Kind::BigInt | Kind::Interval => true,
        _ => false,
    }
}

/// Evaluates a step predicate on `(m.s0, m.s1)`.
fn check_step(m: &mut Machine, code: &Code, c: &Check) -> Result<bool, Error> {
    let old = m.enter(c.frame);
    let mut r = Ok(true);
    for conj in &c.conjuncts {
        match m.eval_bool(code, conj.p) {
            Ok(true) => {}
            other => {
                r = other;
                break;
            }
        }
    }
    m.leave(old);
    r
}

impl Sink for Explorer<'_> {
    fn emit(&mut self, m: &mut Machine, _code: &Code) -> Result<(), Error> {
        // One poll per initial state: the budget can stop a long `Init`
        // enumeration between its states (ADR 0107).
        if self.initial && (self.poll)() {
            self.stop = Some(Verdict::Budget);
            return Err(Error::stop());
        }
        self.stats.generated += 1;
        self.emitted += 1;
        if self.initial {
            self.stats.initial_generated += 1;
        }
        self.report_emitted()?;
        // The checks' transients are freed once the state is accepted (R10):
        // an `Init` that enumerates many states, or a state with many
        // successors, would otherwise hold them all until the expansion
        // ends (#551). Only when no thunk was forced to a transient
        // meanwhile, whose slot may hold a value built after the mark
        // (`03-value-model.md` §11.5 rule 2). The state's own values predate
        // the mark, and the frontier holds them interned.
        let before = m.heap.mark();
        let forced = m.stats.transient_thunks;
        let draws = m.draws;
        // State checks read the checked state's `TLCGet("level")`.
        let level = m.level;
        m.level = i64::try_from(self.level).unwrap_or(i64::MAX);
        let r = self.accept(m);
        m.level = level;
        if m.draws != draws {
            // TLC's checks draw from the expansion's stream, which the
            // trace rebuild replays without them (`03-value-model.md` §6.1).
            return Err(Error::unsupported(
                "RandomElement in an invariant, constraint or implied action (M1 core)",
            ));
        }
        if r.is_ok() && m.stats.transient_thunks == forced {
            m.heap.rollback(before);
        }
        if m.heap.poisoned() {
            return Err(Error::internal("a stale value handle was read"));
        }
        r
    }
}

fn run_actions(
    m: &mut Machine,
    ex: &mut Explorer<'_>,
    actions: &[dive_core::plan::Action],
    code: &Code,
) -> Result<(), Error> {
    for (i, a) in actions.iter().enumerate() {
        ex.action = (!ex.initial).then_some(i);
        ex.action_start = ex.emitted;
        let old = m.enter(a.frame);
        for &(slot, v) in &a.preset {
            if let Some(x) = m.frame.get_mut(m.base + slot as usize) {
                *x = v;
            }
        }
        let r = m.run_action(code, a.g, ex);
        m.leave(old);
        r?;
    }
    Ok(())
}

/// The workers a run uses: one for a plan with an effect, so that its
/// output and draws are in TLC's order ([`Plan::effects`]).
fn workers(plan: &Plan, config: &Config) -> usize {
    if plan.effects.any {
        1
    } else {
        config.workers.max(1)
    }
}

/// Explores the plan's state space.
#[must_use]
pub fn explore(
    plan: &Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &(dyn Fn() -> bool + Sync),
) -> Outcome {
    explore_with(plan, store, config, poll, None)
}

/// Explores the plan's state space, reporting its behavior graph to `graph`
/// ([`graph::GraphSink`]) in the one-worker order.
#[must_use]
pub fn explore_with<'p>(
    plan: &'p Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    graph: Option<&'p mut dyn graph::GraphSink>,
) -> Outcome {
    let start = Instant::now();
    let nvars = plan.vars.len();
    let keys: Vec<usize> = plan
        .code
        .sites
        .iter()
        .map(dive_core::plan::MemoSite::key_len)
        .collect();
    let mut machines: Vec<Machine> = (0..workers(plan, &config))
        .map(|_| {
            let mut m = Machine::new(Arc::clone(store), nvars, config.opts, Memo::new(&keys));
            m.warn = config.warn;
            m
        })
        .collect();
    let mut ex = Explorer {
        plan,
        graph,
        stats: Stats::default(),
        metrics: Metrics::default(),
        seen: Seen::default(),
        next: Vec::new(),
        emitted: 0,
        initial: true,
        changed: u64::MAX,
        level: 1,
        opts: config.opts,
        stop: None,
        action: None,
        log: PredecessorLog::new(None, None),
        next_ids: Vec::new(),
        cur_id: None,
        end: None,
        in_model: true,
        action_start: 0,
        failure: None,
        poll,
        max_states: config.max_states,
        max_depth: config.max_depth,
        warnings: Vec::new(),
    };
    let mut replayed = None;
    let mut reservoir = config.sample.map(sample::Reservoir::new);
    let result = run(
        &mut machines,
        &mut ex,
        plan,
        &mut replayed,
        reservoir.as_mut(),
    );
    let parallel = machines.len() > 1;
    let mut m = machines.swap_remove(0);
    let trace = ex
        .end
        .take()
        .map(|end| reconstruct(plan, store, &config, &ex.log, end));
    let verdict = match result {
        Ok(()) => Verdict::Success,
        Err(e) => match e.into_parts() {
            (ErrorKind::Stop, _) if ex.failure.is_some() => Verdict::Failed,
            (ErrorKind::Stop, _) => ex.stop.take().unwrap_or(Verdict::Success),
            (ErrorKind::Unsupported, message) => Verdict::Unsupported { feature: message },
            (ErrorKind::Eval, message) => Verdict::Unsupported {
                feature: format!("evaluation error classification (M0 core): {message}"),
            },
            (ErrorKind::Internal, message) => Verdict::Internal { message },
        },
    };
    let mut metrics = ex.metrics;
    metrics.store = store.stats();
    metrics.memo = plan
        .code
        .sites
        .iter()
        .map(|s| s.name.clone())
        .zip(m.memo.stats())
        .collect();
    metrics.arena_peak = m.heap.arena().peak();
    metrics.thunks = (m.stats.forced, m.stats.cached);
    metrics.explore_time = start.elapsed();
    let warnings = if !config.warn {
        Vec::new()
    } else if parallel {
        std::mem::take(&mut ex.warnings)
    } else {
        std::mem::take(&mut m.warnings)
    };
    Outcome {
        verdict,
        stats: ex.stats,
        metrics,
        warnings,
        trace,
        failure: ex.failure,
        in_init: ex.initial,
        replayed,
        sampled: reservoir.map(|r| r.states).unwrap_or_default(),
        printed: std::mem::take(&mut m.printed),
    }
}

/// Rebuilds the behavior that ends at `end` (`07-engine.md` §4).
fn reconstruct(
    plan: &Plan,
    store: &Arc<Store>,
    config: &Config,
    log: &PredecessorLog,
    end: End,
) -> Result<Vec<TraceState>, String> {
    let mut states = match end.upto {
        None => Vec::new(),
        Some(id) => {
            let path = log.path(id).map_err(|e| format!("predecessor log: {e}"))?;
            let states = trace::replay(plan, store, config.opts, config.warn, &path)
                .map_err(|e| e.message().to_owned())?;
            if !states
                .last()
                .is_some_and(|s| replay_values_match(store, &s.values, &end.expect))
            {
                return Err("trace replay produced a different state than the search".into());
            }
            states
        }
    };
    if let Some((action, values)) = end.last {
        states.push(TraceState { action, values });
    }
    Ok(states)
}

/// Replay allocates fresh persistent compounds when hash-consing is disabled.
/// Their values must still match; unequal handles alone are not a mismatch.
fn replay_values_match(store: &Arc<Store>, actual: &[Handle], expected: &[Handle]) -> bool {
    if actual == expected {
        return true;
    }
    if store.hashcons() || actual.len() != expected.len() {
        return false;
    }
    let heap = dive_core::Heap::new(Arc::clone(store), false, false);
    actual.iter().zip(expected).all(|(&a, &b)| heap.eq(a, b))
}

/// Turns an evaluation error raised by the generator itself (not by a
/// check in the sink, which records its own failure) into a failure.
fn generator_error(m: &Machine, ex: &mut Explorer<'_>, e: Error) -> Error {
    if e.kind() != ErrorKind::Eval || ex.failure.is_some() {
        return e;
    }
    ex.failure = Some(ex.generator_failure(m, e.message().to_owned()));
    if let Some(id) = ex.cur_id {
        ex.end = Some(End {
            upto: Some(id),
            last: None,
            expect: m.s0.clone(),
        });
    }
    Error::stop()
}

/// Starts the random stream draws come from: the startup stream for
/// `Init`, or the stream of expanding `state` (`03-value-model.md` §6.1).
pub(crate) fn start_stream(
    m: &mut Machine,
    plan: &Plan,
    state: Option<&[Handle]>,
) -> Result<(), Error> {
    if !plan.effects.draws {
        return Ok(());
    }
    let Some(draws) = m.heap.store().draws().cloned() else {
        return Err(Error::unsupported(
            "RandomElement without the run's random streams (M1 core)",
        ));
    };
    m.draw = Some(match state {
        None => draws.startup(),
        Some(vars) => draws.state(&mut m.heap, vars)?,
    });
    Ok(())
}

/// Reports the completed level to the graph sink, which may stop the
/// search after it.
fn level_done(ex: &mut Explorer<'_>, nvars: usize) -> Result<(), Error> {
    let Some(sink) = ex.graph.as_deref_mut() else {
        return Ok(());
    };
    let counts = Stats {
        left_on_queue: (ex.next.len() / nvars.max(1)) as u64,
        ..ex.stats
    };
    let level = ex.level - 1;
    let stop = sink.level_done(level, counts);
    if stop == Ok(false) {
        return Ok(());
    }
    ex.stats.left_on_queue = counts.left_on_queue;
    ex.stop = Some(match stop {
        Ok(_) if counts.left_on_queue > 0 => Verdict::PrefixStop { level },
        Ok(_) => Verdict::Internal {
            message: "a prefix stop with an exhausted frontier".into(),
        },
        Err(graph::Stopped) => Verdict::GraphStop,
    });
    Err(Error::stop())
}

/// The states of a parallel level each worker expands per chunk (#551).
const CHUNK_PER_WORKER: usize = 4096;

#[allow(
    clippy::too_many_lines,
    reason = "TLC's ordered level loop: expansion, stops, replay and deadlock"
)]
fn run(
    machines: &mut [Machine],
    ex: &mut Explorer<'_>,
    plan: &Plan,
    replayed: &mut Option<Replayed>,
    mut reservoir: Option<&mut sample::Reservoir>,
) -> Result<(), Error> {
    let code = &plan.code;
    let nvars = plan.vars.len();
    let parallel = machines.len() > 1;
    let Some(m) = machines.first_mut() else {
        return Err(Error::internal("no core machine"));
    };
    start_stream(m, plan, None)?;
    // `TLCGet("level")` is 0 in `Init` (TLC's `INIT_LEVEL - 1`).
    m.level = 0;
    if let Err(e) = run_actions(m, ex, &plan.init, code) {
        ex.stats.left_on_queue = (ex.next.len() / nvars.max(1)) as u64;
        return Err(generator_error(m, ex, e));
    }
    // TLC reports depth one after Init emits a state, even when the
    // constraints exclude every candidate from the retained graph.
    ex.stats.depth = ex
        .stats
        .depth
        .max(u64::from(ex.stats.initial_generated > 0));
    // ModelChecker checks this after Init, before starting any workers.
    // An empty initial frontier succeeds; defining a disabled action instead
    // reaches the ordinary deadlock check, if enabled.
    if plan.next.is_empty() && !ex.next.is_empty() {
        ex.stats.left_on_queue = (ex.next.len() / nvars.max(1)) as u64;
        ex.stop = Some(Verdict::NoNextAction);
        return Err(Error::stop());
    }
    m.heap.reset();
    ex.initial = false;
    ex.warnings.clone_from(&m.warnings);
    let mut cur = std::mem::take(&mut ex.next);
    let mut cur_ids = std::mem::take(&mut ex.next_ids);
    let mut generated_before = 0;
    while !cur.is_empty() {
        ex.level += 1;
        // Claims are sized from the previous level's successors.
        let estimate = usize::try_from(ex.stats.generated - generated_before).unwrap_or(1 << 20);
        generated_before = ex.stats.generated;
        let level_started = Instant::now();
        let mut expand_time = std::time::Duration::ZERO;
        let states = cur.len() / nvars.max(1);
        // The parallel level is expanded and committed a chunk of states at
        // a time, so that phase 1 buffers one chunk's successors, not the
        // level's (#551). A chunk's commit fills the seen set before the
        // next chunk is expanded, which reads it as an earlier level's.
        let chunk = (machines.len() * CHUNK_PER_WORKER).max(1);
        let mut expanded: Option<Vec<Option<parallel::Expanded>>> = None;
        let mut base = 0;
        for (k, state) in cur.chunks_exact(nvars.max(1)).enumerate() {
            if nvars == 0 {
                break;
            }
            if parallel && k % chunk == 0 {
                let started = Instant::now();
                let end = (k + chunk).min(states);
                let mut metrics = Metrics::default();
                let r = parallel::expand_level(
                    plan,
                    machines,
                    &cur[k * nvars..end * nvars],
                    nvars,
                    &ex.seen,
                    ex.opts,
                    ex.poll,
                    estimate.saturating_mul(end - k) / states.max(1),
                    &mut metrics,
                )?;
                ex.metrics.conjuncts_total += metrics.conjuncts_total;
                ex.metrics.conjuncts_skipped += metrics.conjuncts_skipped;
                ex.metrics.discarded_before_intern += metrics.discarded_before_intern;
                expand_time += started.elapsed();
                expanded = Some(r);
                base = k;
            }
            let Some(m) = machines.first_mut() else {
                return Err(Error::internal("no core machine"));
            };
            let res = expanded
                .as_ref()
                .and_then(|all| all.get(k - base).and_then(Option::as_ref));
            // Phase 1 polled the budget before each state it expanded, and
            // stopped before this one if it ran out.
            let exhausted = if expanded.is_some() {
                res.is_none()
            } else {
                (ex.poll)()
            };
            if exhausted {
                ex.stats.left_on_queue = (cur.len() / nvars - k + ex.next.len() / nvars) as u64;
                ex.stop = Some(Verdict::Budget);
                return Err(Error::stop());
            }
            m.s0.copy_from_slice(state);
            let level = i64::try_from(ex.level - 1).unwrap_or(i64::MAX);
            if res.is_none() {
                start_stream(m, plan, Some(state))?;
                m.level = level;
            }
            if let Some(r) = reservoir.as_deref_mut() {
                r.offer(state, level);
            }
            ex.cur_id = cur_ids.get(k).copied();
            ex.emitted = 0;
            let r = match res {
                Some(res) => ex.commit(m, res),
                None => run_actions(m, ex, &plan.next, code),
            }
            .map_err(|e| generator_error(m, ex, e));
            if r.is_err()
                && let Some(f) = &ex.failure
                && f.site_resumes()
            {
                // TLC's call-stack replay (`ending.rs`).
                if plan.effects.draws {
                    return Err(Error::unsupported(
                        "TLC's replay after a failing check, in a specification that draws (RandomElement) (M1 core)",
                    ));
                }
                if plan.effects.registers || plan.effects.level {
                    return Err(Error::unsupported(
                        "TLC's replay after a failing check, in a specification that uses TLCSet or TLCGet (M1 core)",
                    ));
                }
                // The search ends here; the graph has no part in the replay.
                ex.graph = None;
                let ordinal = ex.emitted - 1;
                let saved = std::mem::take(&mut m.warnings);
                *replayed = Some(ending::replay_after(m, ex, ordinal)?);
                for w in std::mem::replace(&mut m.warnings, saved) {
                    if parallel && !ex.warnings.contains(&w) {
                        ex.warnings.push(w);
                    } else if !parallel && !m.warnings.contains(&w) {
                        m.warnings.push(w);
                    }
                }
            }
            m.heap.reset();
            let left = (cur.len() / nvars - k - 1 + ex.next.len() / nvars) as u64;
            if let Err(e) = r {
                ex.stats.left_on_queue = left;
                return Err(e);
            }
            if ex.emitted == 0 && plan.check_deadlock {
                ex.stats.left_on_queue = left;
                ex.stop = Some(Verdict::Deadlock);
                ex.end = Some(End {
                    upto: ex.cur_id,
                    last: None,
                    expect: state.to_vec(),
                });
                return Err(Error::stop());
            }
            if let Some(id) = ex.cur_id
                && let Some(sink) = ex.graph.as_deref_mut()
                && sink.stutter(id).is_err()
            {
                ex.stats.left_on_queue = left;
                ex.stop = Some(Verdict::GraphStop);
                return Err(Error::stop());
            }
        }
        level_done(ex, nvars)?;
        if parallel {
            ex.metrics.expand_time += expand_time;
            ex.metrics.commit_time += level_started.elapsed().saturating_sub(expand_time);
        }
        cur.clear();
        cur_ids.clear();
        std::mem::swap(&mut cur, &mut ex.next);
        std::mem::swap(&mut cur_ids, &mut ex.next_ids);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_checks_compound_values_without_hashconsing() -> Result<(), Error> {
        let store = Arc::new(Store::new(false));
        let mut heap = dive_core::Heap::new(Arc::clone(&store), false, false);
        let one = heap.int_from_i64(1);
        let two = heap.int_from_i64(2);
        let a = heap.tuple(&[one]);
        let a = heap.intern(a)?;
        let equal = heap.tuple(&[one]);
        let equal = heap.intern(equal)?;
        let different = heap.tuple(&[two]);
        let different = heap.intern(different)?;
        assert_ne!(a, equal, "the control must allocate distinct handles");
        assert!(replay_values_match(&store, &[a], &[equal]));
        assert!(!replay_values_match(&store, &[a], &[different]));
        assert!(!replay_values_match(&store, &[a], &[equal, equal]));
        assert!(!replay_values_match(&store, &[], &[equal]));
        let nested_a = heap.tuple(&[a]);
        let nested_a = heap.intern(nested_a)?;
        let nested_equal = heap.tuple(&[equal]);
        let nested_equal = heap.intern(nested_equal)?;
        let nested_different = heap.tuple(&[different]);
        let nested_different = heap.intern(nested_different)?;
        assert!(replay_values_match(&store, &[nested_a], &[nested_equal]));
        assert!(!replay_values_match(
            &store,
            &[nested_a],
            &[nested_different]
        ));
        Ok(())
    }

    #[test]
    fn seen_set_inserts_and_grows() {
        let mut s = Seen::default();
        for k in 0..200_000u64 {
            assert!(s.insert(k.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        }
        assert_eq!(s.len(), 200_000);
        assert!(!s.insert(0));
        assert!(s.contains(5u64.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        assert!(!s.contains(7));
    }
}
