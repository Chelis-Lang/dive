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

mod candidate;
pub mod checkpoint;
pub mod distributed;
pub mod ending;
pub mod graph;
mod parallel;
mod reclaim;
pub mod sample;
pub mod storage;
pub mod trace;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_core::handle::{K_STATE, fin, state_term};
use dive_core::memo::{Memo, SiteStats};
use dive_core::plan::{Check, Code};
use dive_core::store::{StoreMemory, StoreStats};
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
    /// A requested completed-level pause; never a model verdict.
    CheckpointPaused {
        /// Next unexpanded source level.
        next_level: u64,
    },
    /// A checkpoint transaction failed without proving a model result.
    CheckpointError {
        /// The explicit structural, ownership or admission failure.
        message: String,
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
    /// Authenticated remote work facts when distributed safety was requested.
    pub distributed: Option<distributed::Usage>,
    /// Actual opt-in native safety storage facts, not whole-process memory.
    pub storage: storage::Usage,
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
    /// Known store storage before explorer workers are dropped; this is a
    /// lower-bound component snapshot, not process RSS or peak memory.
    pub store_memory: StoreMemory,
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
    exact: Option<storage::Exact>,
}

impl Default for Seen {
    fn default() -> Self {
        Self {
            table: vec![0; 1 << 16],
            len: 0,
            exact: None,
        }
    }
}

impl Seen {
    fn with_detached_exact(exact: bool, detached: bool) -> Self {
        Self {
            exact: exact.then(|| {
                if detached {
                    storage::Exact::detached()
                } else {
                    storage::Exact::default()
                }
            }),
            ..Self::default()
        }
    }

    fn empty_like(&self) -> Self {
        Self::with_detached_exact(
            self.exact.is_some(),
            self.exact.as_ref().is_some_and(storage::Exact::is_detached),
        )
    }

    fn contains_row(
        &self,
        key: u64,
        row: &[Handle],
        heap: &dive_core::Heap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        self.exact.as_ref().map_or_else(
            || Ok(self.contains(key)),
            |exact| exact.contains_controlled(key, row, heap, poll),
        )
    }

    fn insert_row(
        &mut self,
        key: u64,
        row: &[Handle],
        heap: &dive_core::Heap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        if let Some(exact) = &mut self.exact {
            exact.insert_controlled(key, row, heap, poll)
        } else {
            Ok(self.insert(key))
        }
    }

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
fn state_identity(
    m: &mut Machine,
    plan: &Plan,
    identity: Option<&mut Vec<Handle>>,
) -> Result<u64, Error> {
    if let Some(view) = &plan.view {
        return view_key(m, plan, view, identity);
    }
    let image;
    let state: &[Handle] = match &plan.symmetry {
        Some(sym) => {
            let mut out = Vec::with_capacity(m.s1.len());
            let s1 = m.s1.clone();
            // The current state is usually the successor's predecessor.
            sym.canonical_from(&mut m.heap, &s1, &m.s0, &mut out)?;
            image = out;
            &image
        }
        None => &m.s1,
    };
    let mut sum = 0u64;
    for (i, &h) in state.iter().enumerate() {
        sum = sum.wrapping_add(state_term(i, m.heap.hash(h)));
    }
    if let Some(identity) = identity {
        capture_reserve(identity, state.len())?;
        if m.heap.is_speculative() {
            identity.extend(m.heap.close_identity_row_local(state)?);
        } else {
            for &handle in state {
                identity.push(m.heap.intern(handle)?);
            }
        }
    }
    Ok(fin(K_STATE, state.len() as u64, sum))
}

/// The key of the `VIEW`'s value on `m.s1`. TLC normalizes the state's
/// variables before it evaluates the VIEW (`TLCStateMut.fingerPrint`); the
/// core interns them, and refuses a state holding an infinite set, whose
/// normalization it does not reproduce. The value is interned too and must
/// hold no infinite set, which TLC fails to fingerprint (refused here
/// rather than reported).
fn view_key(
    m: &mut Machine,
    plan: &Plan,
    view: &Check,
    identity: Option<&mut Vec<Handle>>,
) -> Result<u64, Error> {
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
            // The current state is usually the successor's predecessor.
            sym.canonical_from(&mut m.heap, &s1, &m.s0, &mut out)?;
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
    let v = m.heap.state_identity(r?)?;
    if !closed_fingerprintable(&m.heap, v) {
        return Err(Error::unsupported(
            "a VIEW value holding an infinite or lazily represented set (core)",
        ));
    }
    if let Some(identity) = identity {
        capture_reserve(identity, 1)?;
        identity.push(v);
    }
    Ok(fin(K_STATE, 1, state_term(0, m.heap.hash(v))))
}

// Capture-only provisional/frontier buffers use checked, fallible growth.
// The ordinary safety route does not call this helper or retain extra rows.
pub(crate) fn capture_reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), Error> {
    values
        .len()
        .checked_add(additional)
        .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
        .ok_or_else(|| Error::unsupported("capture buffer address space"))?;
    values
        .try_reserve(additional)
        .map_err(|_| Error::unsupported("capture buffer allocation"))
}

pub(crate) fn capture_copy(values: &[Handle]) -> Result<Vec<Handle>, Error> {
    let mut copy = Vec::new();
    capture_reserve(&mut copy, values.len())?;
    copy.extend_from_slice(values);
    Ok(copy)
}

struct Explorer<'p> {
    plan: &'p Plan,
    /// The behavior graph's sink, when a temporal check needs it.
    graph: Option<&'p mut dyn graph::GraphSink>,
    stats: Stats,
    metrics: Metrics,
    seen: Seen,
    disk: Option<storage::Disk>,
    checkpoint: Option<&'p mut checkpoint::Session>,
    distributed: Option<&'p distributed::Coordinator>,
    next: Vec<Handle>,
    /// Opt-in encoded current/next queues tied to this live Store.
    frontier: Option<storage::Frontiers>,
    candidates: Option<candidate::Account>,
    candidate_storage: storage::Storage,
    /// Capture-only first-discovered representations, parallel to `next`.
    /// Canonical frontier identities remain separate for logs and Step.new.
    next_originals: Vec<Handle>,
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
    /// Actual Init action currently generating a candidate.
    init_action: Option<usize>,
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
    /// The canonical frontier identities of `upto`, not the original views
    /// used to evaluate a captured source; replay must reproduce them.
    expect: Vec<Handle>,
}

enum Probe {
    Seen,
    New,
}

impl Explorer<'_> {
    fn seen_contains(
        &mut self,
        key: u64,
        row: &[Handle],
        heap: &dive_core::Heap,
        phase: (&'static str, Option<u64>),
    ) -> Result<bool, Error> {
        let exact = self.seen.exact.is_some();
        let position = (
            phase.0,
            self.stats.generated,
            self.cur_id,
            phase.1.unwrap_or_else(|| self.emitted.saturating_sub(1)),
        );
        self.disk.as_mut().map_or_else(
            || self.seen.contains_row(key, row, heap, self.poll),
            |disk| disk.operation(exact, key, row, heap, false, position, self.poll),
        )
    }

    fn seen_insert(
        &mut self,
        key: u64,
        row: &[Handle],
        heap: &dive_core::Heap,
        phase: (&'static str, Option<u64>),
    ) -> Result<bool, Error> {
        let exact = self.seen.exact.is_some();
        let position = (
            phase.0,
            self.stats.generated,
            self.cur_id,
            phase.1.unwrap_or_else(|| self.emitted.saturating_sub(1)),
        );
        if let Some(disk) = &mut self.disk {
            disk.operation(exact, key, row, heap, true, position, self.poll)
        } else {
            self.seen.insert_row(key, row, heap, self.poll)
        }
    }

    fn queued_states(&self) -> usize {
        if self.plan.vars.is_empty() {
            return 0;
        }
        self.frontier.as_ref().map_or_else(
            || self.next.len() / self.plan.vars.len(),
            storage::Frontiers::next_len,
        )
    }

    fn advance_frontier(&mut self) -> Result<(), Error> {
        let queued = self.queued_states();
        if let Some(frontier) = &mut self.frontier {
            match frontier.advance(self.poll) {
                Ok(true) => {}
                Ok(false) => {
                    self.stats.left_on_queue = queued as u64;
                    self.stop = Some(Verdict::Budget);
                    return Err(Error::stop());
                }
                Err(error) => {
                    self.stats.left_on_queue = queued as u64;
                    return Err(error);
                }
            }
        }
        Ok(())
    }

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
        let mut identity = Vec::new();
        let key = match state_identity(
            m,
            self.plan,
            self.seen.exact.as_ref().map(|_| &mut identity),
        ) {
            Ok(k) => k,
            Err(e) if e.kind() == ErrorKind::Eval && self.plan.view.is_some() => {
                self.failure = Some(self.failure_at(m, Site::View, e.message().to_owned())?);
                self.end_here(m)?;
                return Err(Error::stop());
            }
            Err(e) => return Err(e),
        };
        self.metrics.probed += 1;
        if in_model
            && (self.max_states.is_some() || self.max_depth.is_some())
            && !self.seen_contains(key, &identity, &m.heap, ("AdmissionMembership", None))?
        {
            self.admit_new()?;
        }
        let new = if in_model {
            self.seen_insert(key, &identity, &m.heap, ("Insertion", None))?
        } else {
            !self.seen_contains(key, &identity, &m.heap, ("AdmissionMembership", None))?
        };
        if !new {
            self.metrics.discarded_before_intern +=
                u64::from(self.opts.early_probe && (!in_model || self.graph.is_none()));
            return Ok((Probe::Seen, in_model, key));
        }
        Ok((Probe::New, in_model, key))
    }

    fn accept(&mut self, m: &mut Machine) -> Result<(), Error> {
        let plan = self.plan;
        let code = &plan.code;
        // The -early-probe control can replace m.s1 with canonical identity
        // handles. Keep the emitted representation available to capture too.
        let original = (self.graph.is_some() || self.checkpoint.is_some())
            .then(|| capture_copy(&m.s1))
            .transpose()?;
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
                self.record(m, key, false, original.as_deref())
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
            self.enqueue(m, original.as_deref())?;
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
            self.enqueue(m, original.as_deref())?;
        }
        if in_model {
            self.record(m, key, true, original.as_deref())?;
        }
        Ok(())
    }

    /// Reports the accepted state inside the model with key `key` to the
    /// graph sink; a `new` state is the last one enqueued.
    fn record(
        &mut self,
        m: &mut Machine,
        key: u64,
        new: bool,
        original: Option<&[Handle]>,
    ) -> Result<(), Error> {
        if new && let Some(session) = &mut self.checkpoint {
            let values = m.s1.clone();
            let original = original.unwrap_or(&values);
            let ordinal = u32::try_from(self.emitted.saturating_sub(1))
                .map_err(|_| Error::unsupported("checkpoint successor ordinal bound"))?;
            if let Err(error) =
                session.record(&mut m.heap, &values, original, key, self.cur_id, ordinal)
            {
                self.stop = Some(Verdict::CheckpointError {
                    message: error.message().to_owned(),
                });
                return Err(Error::stop());
            }
        }
        let Some(sink) = self.graph.as_deref_mut() else {
            return Ok(());
        };
        let nvars = m.s1.len();
        // Capture preserves the original representation separately from the
        // canonical identity queued for BFS. No graph means no extra interning
        // or retained row, including on the duplicate fast path.
        let kept = if new {
            Vec::new()
        } else {
            let values = original.unwrap_or(&m.s1);
            let mut kept = Vec::new();
            capture_reserve(&mut kept, values.len())?;
            for &handle in values {
                kept.push(m.heap.intern_exact(handle)?);
            }
            kept
        };
        let original = if new {
            self.next_originals
                .get(self.next_originals.len().saturating_sub(nvars)..)
                .ok_or_else(|| Error::internal("a new state's original values were not retained"))?
        } else {
            &kept
        };
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
            original,
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
        if self.checkpoint.is_some() && self.stats.generated > i64::MAX as u64 {
            self.stop = Some(Verdict::CheckpointError {
                message: "native recovery counter bound exceeded".into(),
            });
            return Err(Error::stop());
        }
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
    fn admit_new(&mut self) -> Result<(), Error> {
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
    fn enqueue(&mut self, m: &mut Machine, original: Option<&[Handle]>) -> Result<(), Error> {
        if let Some(frontier) = &mut self.frontier {
            let mut identity = Vec::new();
            let mut retained = Vec::new();
            capture_reserve(&mut identity, m.s1.len())?;
            capture_reserve(&mut retained, m.s1.len())?;
            for &value in &m.s1 {
                identity.push(m.heap.intern(value)?);
            }
            for &value in original.unwrap_or(&m.s1) {
                retained.push(m.heap.intern_exact(value)?);
            }
            let ordinal = u32::try_from(self.emitted.saturating_sub(1))
                .map_err(|_| Error::unsupported("more than 2^32 successors of one state"))?;
            let id = match self.cur_id {
                None => self.log.push_initial(ordinal),
                Some(parent) => self.log.push(parent, ordinal),
            }
            .map_err(|e| Error::internal(format!("predecessor log: {e}")))?;
            return frontier.push(id, &identity, &retained);
        }
        if self.graph.is_some() || self.checkpoint.is_some() {
            capture_reserve(&mut self.next, m.s1.len())?;
            capture_reserve(&mut self.next_originals, m.s1.len())?;
            capture_reserve(&mut self.next_ids, 1)?;
        }
        for i in 0..m.s1.len() {
            let h = m.heap.intern(m.s1[i])?;
            self.next.push(h);
        }
        if self.graph.is_some() || self.checkpoint.is_some() {
            for &h in original.unwrap_or(&m.s1) {
                self.next_originals.push(m.heap.intern_exact(h)?);
            }
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
            if self.graph.is_some() || self.checkpoint.is_some() || self.distributed.is_some() {
                m.s0.iter()
                    .map(|&h| m.heap.intern(h))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                m.s0.clone()
            }
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
    fingerprintable_with_identity(m, initial, None)
}

/// Reuses only this emission's successful exact identity closure, when it is
/// the same unsymmetrized row. No cache survives an emission or Arena rollback.
pub(crate) fn fingerprintable_with_identity(
    m: &mut Machine,
    initial: bool,
    identity: Option<&[Handle]>,
) -> Result<bool, Error> {
    for i in 0..m.s1.len() {
        let h = m.s1[i];
        if h.is_empty() || !h.is_compound() || (!initial && m.s0.get(i) == Some(&h)) {
            continue;
        }
        let p = if let Some(identity) = identity {
            *identity
                .get(i)
                .ok_or_else(|| Error::internal("candidate identity row width"))?
        } else {
            m.heap.state_identity(h)?
        };
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
        if self.graph.is_some() {
            self.stats.generated = self
                .stats
                .generated
                .checked_add(1)
                .ok_or_else(|| Error::unsupported("capture generated counter overflow"))?;
            self.emitted = self
                .emitted
                .checked_add(1)
                .ok_or_else(|| Error::unsupported("capture successor ordinal overflow"))?;
            if self.initial {
                self.stats.initial_generated = self
                    .stats
                    .initial_generated
                    .checked_add(1)
                    .ok_or_else(|| Error::unsupported("capture initial counter overflow"))?;
            }
        } else {
            self.stats.generated += 1;
            self.emitted += 1;
            if self.initial {
                self.stats.initial_generated += 1;
            }
        }
        self.report_emitted()?;
        if !self.initial && self.graph.is_some() && (self.poll)() {
            self.stop = Some(Verdict::Budget);
            return Err(Error::stop());
        }
        // The checks' transients are freed once the state is accepted (R10):
        // an `Init` that enumerates many states, or a state with many
        // successors, would otherwise hold them all until the expansion
        // ends (#551). Only when no thunk was forced to a transient
        // meanwhile, whose slot may hold a value built after the mark
        // (`03-value-model.md` §11.5 rule 2). The state's own values predate
        // the mark, and the frontier holds them interned.
        let before = m.heap.mark();
        let forced = m.thunk_mark();
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
        if r.is_ok() && m.thunks_released(forced) {
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
        ex.init_action = ex.initial.then_some(i);
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
#[allow(
    clippy::too_many_lines,
    reason = "ordered search setup and result assembly stay in one entrypoint"
)]
pub fn explore_with<'p>(
    plan: &'p Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    graph: Option<&'p mut dyn graph::GraphSink>,
) -> Outcome {
    explore_with_control(plan, store, config, poll, graph, None)
}

/// Explores with the caller's existing owned work checkpoint in addition to
/// state-boundary polls. Mathematical identity walks use this same owner.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "ordered search setup/result assembly"
)]
pub fn explore_with_control<'p>(
    plan: &'p Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    graph: Option<&'p mut dyn graph::GraphSink>,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
) -> Outcome {
    explore_with_storage(
        plan,
        store,
        config,
        poll,
        graph,
        control,
        &storage::Storage::default(),
    )
}

/// Explores with explicit native safety storage policies. Persistent values stay
/// owned by `store`; transient files are not restartable checkpoints.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "search setup and storage policy"
)]
pub fn explore_with_storage<'p>(
    plan: &'p Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    graph: Option<&'p mut dyn graph::GraphSink>,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
    storage: &storage::Storage,
) -> Outcome {
    let mut owner = Arc::clone(store);
    explore_inner(
        plan, &mut owner, config, poll, graph, control, storage, None, None, false, None,
    )
}

/// Runs a quiescent completed-level safety transaction over the existing kernel.
/// Ordinary exploration does not retain checkpoint discovery rows.
#[must_use]
pub fn explore_checkpoint<'p>(
    plan: &'p Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
    storage: &storage::Storage,
    session: &'p mut checkpoint::Session,
) -> Outcome {
    let mut owner = Arc::clone(store);
    explore_inner(
        plan,
        &mut owner,
        config,
        poll,
        None,
        control,
        storage,
        Some(session),
        None,
        false,
        None,
    )
}

/// Explores pure safety while allowing completed-level replacement of its Store.
///
/// `store` always names the owner of the returned trace/sample handles. A supplied
/// `permanent` must be captured from the same compiled startup before recovery;
/// omission captures the current startup. Caller-held clones of replaced Stores
/// naturally retain their contents, so callers should transfer their sole owner.
/// Checkpoint recovery may continue here; checkpoint pause, graph capture, effects
/// and sampling explicitly refuse reclamation. The byte policy is a trigger,
/// not a live-root or process-memory limit.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "existing owned search, control and recovery owners"
)]
pub fn explore_owned<'p>(
    plan: &'p Plan,
    store: &mut Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
    storage: &storage::Storage,
    session: Option<&'p mut checkpoint::Session>,
    permanent: Option<&dive_core::store::compact::Permanent>,
) -> Outcome {
    explore_inner(
        plan, store, config, poll, None, control, storage, session, permanent, true, None,
    )
}

/// Explores pure safety with authenticated remote phase-one evaluation.
/// The coordinator retains ordered commit, seen keys and concrete trace replay.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "existing search owners and explicit authenticated coordinator"
)]
pub fn explore_distributed<'p>(
    plan: &'p Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
    storage: &storage::Storage,
    coordinator: &'p distributed::Coordinator,
) -> Outcome {
    let mut owner = Arc::clone(store);
    explore_inner(
        plan,
        &mut owner,
        config,
        poll,
        None,
        control,
        storage,
        None,
        None,
        false,
        Some(coordinator),
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one existing search owner with an optional completed-level transaction"
)]
fn explore_inner<'p>(
    plan: &'p Plan,
    store: &mut Arc<Store>,
    config: Config,
    poll: &'p (dyn Fn() -> bool + Sync),
    graph: Option<&'p mut dyn graph::GraphSink>,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
    storage: &storage::Storage,
    checkpoint: Option<&'p mut checkpoint::Session>,
    permanent: Option<&dive_core::store::compact::Permanent>,
    owned: bool,
    distributed: Option<&'p distributed::Coordinator>,
) -> Outcome {
    let start = Instant::now();
    let trigger_bytes = storage.store_compact_bytes;
    let mut compact = trigger_bytes.is_some();
    let mut reclaim_refusal = if compact && distributed.is_some() {
        Some("Store reclamation with distributed workers is not implemented")
    } else if compact && !owned {
        Some("Store reclamation requires the owned exploration entrypoint")
    } else if compact && (graph.is_some() || plan.effects.any || config.sample.is_some()) {
        Some("Store reclamation with temporal capture, runtime effects or sampling")
    } else if compact && checkpoint.as_ref().is_some_and(|s| s.pause_after.is_some()) {
        Some("Store reclamation with checkpoint pause (discovery residency is unsupported)")
    } else {
        None
    };
    // A default trigger never turns an admitted search into a refusal: when
    // reclamation cannot run, the search keeps its resident Store and says why.
    let mut default_skip = None;
    if storage.store_compact_default
        && let Some(feature) = reclaim_refusal.take()
    {
        compact = false;
        default_skip = Some(feature.to_owned());
    }
    let captured_permanent = if compact && permanent.is_none() && reclaim_refusal.is_none() {
        Some(dive_core::store::compact::Permanent::capture(
            store,
            &|| !poll(),
        ))
    } else {
        None
    };
    let (captured_permanent, mut permanent_error) = match captured_permanent.transpose() {
        Ok(permanent) => (permanent, None),
        Err(error) => (None, Some(error)),
    };
    if storage.store_compact_default
        && let Some(error) = permanent_error.take_if(|error| reclaim::skippable(error))
    {
        compact = false;
        default_skip = Some(error.message().to_owned());
    }
    let storage_owned;
    let storage = if storage.store_compact_default && !compact {
        storage_owned = storage::Storage {
            store_compact_bytes: None,
            ..storage.clone()
        };
        &storage_owned
    } else {
        storage
    };
    let permanent = permanent.or(captured_permanent.as_ref());
    let config = if storage.seen_disk.is_some() {
        Config {
            workers: 1,
            ..config
        }
    } else {
        config
    };
    let unsupported_storage = graph.is_some()
        && (storage.seen_disk.is_some()
            || storage.exact
            || storage.predecessor_memory_bytes.is_some()
            || storage.predecessor_directory.is_some()
            || storage.frontier_memory_bytes.is_some()
            || storage.frontier_directory.is_some()
            || storage.candidate_memory_bytes.is_some()
            || storage.candidate_directory.is_some());
    let nvars = plan.vars.len();
    let (frontier, storage_setup_error) = match storage
        .frontier_memory_bytes
        .map(|_| storage::Frontiers::new(store, nvars, storage))
        .transpose()
    {
        Ok(frontier) => (frontier, None),
        Err(error) => (None, Some(error)),
    };
    let mut initial_metrics = Metrics::default();
    initial_metrics.storage.reclamation.trigger_bytes = trigger_bytes;
    initial_metrics.storage.reclamation.default_trigger = storage.store_compact_default;
    initial_metrics.storage.reclamation.skipped = default_skip;
    let (disk, disk_setup_error) = match storage
        .seen_disk
        .as_ref()
        .map(|options| storage::Disk::new(storage.exact, options))
        .transpose()
    {
        Ok(disk) => (disk, None),
        Err(error) => {
            initial_metrics.storage.seen = Some(storage::DiskUsage {
                limits: storage
                    .seen_disk
                    .as_ref()
                    .map_or_else(dive_store::seen::Limits::default, |options| options.limits),
                admitted_keys: 0,
                before_cleanup: None,
                after_cleanup: None,
                first_failure: Some(storage::DiskFailure {
                    phase: "Creation",
                    error: error.clone(),
                    generated: 0,
                    predecessor: None,
                    successor_ordinal: 0,
                }),
                deciding_event_before_failure: None,
                audit_completed: false,
            });
            (None, Some(Error::internal(error.to_string())))
        }
    };
    let mut seen = Seen::with_detached_exact(storage.exact, compact);
    if storage.seen_disk.is_some() {
        seen.table = Vec::new();
    }
    let keys: Vec<usize> = plan
        .code
        .sites
        .iter()
        .map(dive_core::plan::MemoSite::key_len)
        .collect();
    let local_workers = if distributed.is_some() {
        1
    } else {
        workers(plan, &config)
    };
    let mut machines: Vec<Machine> = (0..local_workers)
        .map(|_| {
            let mut m = Machine::new(Arc::clone(store), nvars, config.opts, Memo::new(&keys));
            m.warn = config.warn;
            m.set_work_control(control.cloned());
            m.set_debugger(plan.code.debugger.clone());
            m
        })
        .collect();
    let mut ex = Explorer {
        plan,
        graph,
        stats: Stats::default(),
        metrics: initial_metrics,
        seen,
        disk,
        checkpoint,
        distributed,
        next: Vec::new(),
        frontier,
        candidates: storage.candidate_memory_bytes.map(|bytes| {
            Arc::new(std::sync::Mutex::new(storage::CandidateUsage {
                memory_bytes_per_worker: Some(bytes),
                ..storage::CandidateUsage::default()
            }))
        }),
        candidate_storage: storage.clone(),
        next_originals: Vec::new(),
        emitted: 0,
        initial: true,
        changed: u64::MAX,
        level: 1,
        opts: config.opts,
        stop: None,
        action: None,
        init_action: None,
        log: PredecessorLog::new(
            storage.predecessor_memory_bytes,
            storage.predecessor_directory.as_deref(),
        ),
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
    let result = if let Some(feature) = reclaim_refusal {
        Err(Error::unsupported(feature))
    } else if let Some(error) = permanent_error.or(disk_setup_error).or(storage_setup_error) {
        Err(error)
    } else if distributed.is_some()
        && (ex.graph.is_some()
            || config.sample.is_some()
            || plan.effects.any
            || plan.view.is_some()
            || plan.symmetry.is_some()
            || storage.seen_disk.is_some()
            || storage.candidate_memory_bytes.is_some())
    {
        Err(Error::unsupported(
            "distributed effects, VIEW, symmetry, sampling, disk seen or candidate spill",
        ))
    } else if unsupported_storage {
        Err(Error::unsupported(
            "exact or spilled predecessor storage with native graph capture",
        ))
    } else if storage.candidate_directory.is_some() && storage.candidate_memory_bytes.is_none() {
        Err(Error::unsupported(
            "a candidate directory without a candidate memory budget",
        ))
    } else if storage.candidate_memory_bytes.is_some() && plan.effects.any {
        Err(Error::unsupported("candidate spill with runtime effects"))
    } else if storage.predecessor_directory.is_some() && storage.predecessor_memory_bytes.is_none()
    {
        Err(Error::unsupported(
            "a predecessor directory without a predecessor memory budget",
        ))
    } else if storage.frontier_directory.is_some() && storage.frontier_memory_bytes.is_none() {
        Err(Error::unsupported(
            "a frontier directory without a frontier memory budget",
        ))
    } else {
        run(
            &mut machines,
            &mut ex,
            plan,
            &mut replayed,
            reservoir.as_mut(),
            store,
            permanent,
            control,
        )
    };
    let parallel = machines.len() > 1 || ex.candidates.is_some() || ex.distributed.is_some();
    if let (Some(coordinator), Err(error)) = (distributed, &result) {
        coordinator.record_failure(error);
    }
    let mut m = machines.swap_remove(0);
    if let Err(error) = &result
        && error.message().starts_with("DIVE-S0203:")
    {
        ex.metrics
            .storage
            .reclamation
            .failures
            .push(error.message().to_owned());
    }
    let mut trace = ex
        .end
        .take()
        .map(|end| reconstruct(plan, store, &config, &ex.log, end));
    let mut verdict = match result {
        Ok(()) => Verdict::Success,
        Err(e) => match e.into_parts() {
            (ErrorKind::Stop, _) if ex.failure.is_some() => Verdict::Failed,
            (ErrorKind::Stop, _) => match ex.stop.take() {
                Some(stop) => stop,
                None if (poll)() || m.heap.work_interrupted() => Verdict::Budget,
                None => Verdict::Success,
            },
            (ErrorKind::Unsupported, message) => Verdict::Unsupported { feature: message },
            (ErrorKind::Eval, message) => Verdict::Unsupported {
                feature: format!("evaluation error classification (M0 core): {message}"),
            },
            (ErrorKind::Internal, message) => Verdict::Internal { message },
        },
    };
    if let Some(account) = &ex.candidates {
        ex.metrics.storage.candidates = account
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if !ex.metrics.storage.candidates.failures.is_empty() {
            ex.metrics.storage.candidates.deciding_event_before_failure =
                Some(format!("{verdict:?}"));
            verdict = Verdict::Internal {
                message: ex.metrics.storage.candidates.failures.join("; "),
            };
            trace = None;
        }
    }
    ex.metrics.storage.exact_states = ex.seen.exact.as_ref().map_or(0, storage::Exact::len);
    if let Some(disk) = &mut ex.disk {
        let validate = !matches!(
            verdict,
            Verdict::Budget
                | Verdict::Admission { .. }
                | Verdict::Unsupported { .. }
                | Verdict::Internal { .. }
        );
        let complete = disk.finish(validate, poll, verdict.clone());
        if storage.exact {
            ex.metrics.storage.exact_states =
                usize::try_from(disk.usage.admitted_keys).unwrap_or(usize::MAX);
        }
        ex.metrics.storage.seen = Some(disk.usage.clone());
        if let Some(failure) = &disk.usage.first_failure {
            verdict = Verdict::Internal {
                message: failure.error.to_string(),
            };
            trace = None;
        } else if !complete {
            verdict = Verdict::Budget;
            trace = None;
        }
    }
    ex.metrics.storage.predecessor = ex.log.usage();
    if let Some(error) = ex.log.failure() {
        ex.metrics
            .storage
            .predecessor_failures
            .push(error.to_string());
    }
    match ex.log.close() {
        Ok(()) => ex.metrics.storage.predecessor_cleanup_completed = true,
        Err(error) => ex
            .metrics
            .storage
            .predecessor_failures
            .push(error.to_string()),
    }
    if !ex.metrics.storage.predecessor_failures.is_empty() {
        ex.metrics.storage.predecessor_deciding_event = Some(format!("{verdict:?}"));
        verdict = Verdict::Internal {
            message: ex.metrics.storage.predecessor_failures.join("; "),
        };
        trace = None;
    }
    if let Some(frontier) = &mut ex.frontier {
        let validate = !matches!(
            verdict,
            Verdict::Budget
                | Verdict::Admission { .. }
                | Verdict::Unsupported { .. }
                | Verdict::Internal { .. }
        );
        let completed = frontier.finish(validate, poll);
        ex.metrics.storage.frontier = frontier.usage.clone();
        if !ex.metrics.storage.frontier.failures.is_empty() {
            ex.metrics.storage.frontier.deciding_event_before_failure =
                Some(format!("{verdict:?}"));
            verdict = Verdict::Internal {
                message: ex.metrics.storage.frontier.failures.join("; "),
            };
            trace = None;
        } else if !completed {
            verdict = Verdict::Budget;
            trace = None;
        }
    }
    let mut metrics = ex.metrics;
    metrics.distributed = distributed.map(distributed::Coordinator::usage);
    metrics.store = store.stats();
    metrics.store_memory = store.memory_usage();
    metrics.memo = plan
        .code
        .sites
        .iter()
        .map(|s| s.name.clone())
        .zip(m.memo.stats())
        .collect();
    let (nodes, handles) = m.heap.arena().peak();
    metrics.arena_peak.0 = metrics.arena_peak.0.max(nodes);
    metrics.arena_peak.1 = metrics.arena_peak.1.max(handles);
    for worker in &machines {
        let (nodes, handles) = worker.heap.arena().peak();
        metrics.arena_peak.0 = metrics.arena_peak.0.max(nodes);
        metrics.arena_peak.1 = metrics.arena_peak.1.max(handles);
    }
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
/// `identity` is the canonical frontier row, distinct from capture's original
/// source in `m.s0`; the initial generator has no logged parent.
fn generator_error(m: &Machine, ex: &mut Explorer<'_>, identity: &[Handle], e: Error) -> Error {
    if e.kind() != ErrorKind::Eval || ex.failure.is_some() {
        return e;
    }
    ex.failure = Some(ex.generator_failure(m, e.message().to_owned()));
    if let Some(id) = ex.cur_id {
        ex.end = Some(End {
            upto: Some(id),
            last: None,
            expect: identity.to_vec(),
        });
    }
    Error::stop()
}

fn source_row(
    machine: &mut Machine,
    explorer: &mut Explorer<'_>,
    identity: &[Handle],
    left: u64,
    terminal: bool,
) -> Result<(), Error> {
    let Some(source) = explorer.cur_id else {
        return Ok(());
    };
    let error = terminal.then_some(explorer.failure.as_ref()).flatten();
    let Some(sink) = explorer.graph.as_deref_mut() else {
        return Ok(());
    };
    let row = graph::SourceRow {
        source,
        original: &machine.s0,
        emitted: explorer.emitted,
        next_error: error,
        stats: Stats {
            left_on_queue: left,
            ..explorer.stats
        },
    };
    if sink.source_row(&mut machine.heap, row).is_err() {
        explorer.stats.left_on_queue = left;
        explorer.stop = Some(Verdict::GraphStop);
        explorer.end = Some(End {
            upto: Some(source),
            last: None,
            expect: identity.to_vec(),
        });
        return Err(Error::stop());
    }
    Ok(())
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
fn level_done(ex: &mut Explorer<'_>, _nvars: usize) -> Result<(), Error> {
    let left_on_queue = ex.queued_states() as u64;
    let Some(sink) = ex.graph.as_deref_mut() else {
        return Ok(());
    };
    let counts = Stats {
        left_on_queue,
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
/// The optional encoded path decodes at most this many state pairs at once.
const FRONTIER_CHUNK_STATES: usize = 1024;

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "TLC's ordered level loop: expansion, stops, replay and deadlock"
)]
fn run(
    machines: &mut [Machine],
    ex: &mut Explorer<'_>,
    plan: &Plan,
    replayed: &mut Option<Replayed>,
    mut reservoir: Option<&mut sample::Reservoir>,
    store: &mut Arc<Store>,
    permanent: Option<&dive_core::store::compact::Permanent>,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
) -> Result<(), Error> {
    let code = &plan.code;
    let nvars = plan.vars.len();
    let parallel = machines.len() > 1 || ex.candidates.is_some() || ex.distributed.is_some();
    let Some(m) = machines.first_mut() else {
        return Err(Error::internal("no core machine"));
    };
    let recovered = checkpoint::restore(ex, m)?;
    if !recovered {
        start_stream(m, plan, None)?;
        // `TLCGet("level")` is 0 in `Init` (TLC's `INIT_LEVEL - 1`).
        m.level = 0;
        if let Err(e) = run_actions(m, ex, &plan.init, code) {
            ex.stats.left_on_queue = ex.queued_states() as u64;
            return Err(generator_error(m, ex, &[], e));
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
        if plan.next.is_empty() && ex.queued_states() > 0 {
            ex.stats.left_on_queue = ex.queued_states() as u64;
            ex.stop = Some(Verdict::NoNextAction);
            return Err(Error::stop());
        }
        m.heap.reset();
        ex.initial = false;
        ex.warnings.clone_from(&m.warnings);
    }
    checkpoint::boundary(ex)?;
    reclaim::boundary(machines, ex, store, permanent, control)?;
    let mut cur = std::mem::take(&mut ex.next);
    let mut cur_originals = std::mem::take(&mut ex.next_originals);
    let mut cur_ids = std::mem::take(&mut ex.next_ids);
    ex.advance_frontier()?;
    let mut generated_before = 0;
    loop {
        let states = cur.len().checked_div(nvars).map_or(0, |in_memory| {
            ex.frontier
                .as_ref()
                .map_or(in_memory, storage::Frontiers::current_len)
        });
        if states == 0 {
            break;
        }
        if (ex.graph.is_some() || ex.checkpoint.is_some())
            && ex.frontier.is_none()
            && cur_originals.len() != cur.len()
        {
            return Err(Error::internal(
                "capture frontier original/identity row mismatch",
            ));
        }
        ex.level += 1;
        // Claims are sized from the previous level's successors.
        let estimate = usize::try_from(ex.stats.generated - generated_before).unwrap_or(1 << 20);
        generated_before = ex.stats.generated;
        let level_started = Instant::now();
        let mut expand_time = std::time::Duration::ZERO;
        // The parallel level is expanded and committed a chunk of states at
        // a time, so that phase 1 buffers one chunk's successors, not the
        // level's (#551). A chunk's commit fills the seen set before the
        // next chunk is expanded, which reads it as an earlier level's.
        let chunk = ex.graph.as_deref().map_or_else(
            || (machines.len() * CHUNK_PER_WORKER).max(1),
            |sink| sink.max_expansion_units().max(1),
        );
        let chunk = if ex.candidates.is_some() {
            machines.len().max(1)
        } else {
            chunk
        };
        let chunk = ex
            .distributed
            .map_or(chunk, |coordinator| chunk.min(coordinator.width()));
        let chunk = if ex.frontier.is_some() {
            chunk.min(FRONTIER_CHUNK_STATES)
        } else {
            chunk
        };
        let mut expanded: Option<Vec<Option<parallel::Expanded>>> = None;
        let mut received: Option<Vec<Option<distributed::Received>>> = None;
        let mut decoded = storage::Chunk::default();
        let mut base = 0;
        for k in 0..states {
            if k % chunk == 0 {
                let queued = ex.queued_states();
                if let Some(frontier) = &mut ex.frontier {
                    decoded = match frontier.chunk(k, (k + chunk).min(states), ex.poll) {
                        Ok(Some(decoded)) => decoded,
                        Ok(None) => {
                            ex.stats.left_on_queue = (states - k + queued) as u64;
                            ex.stop = Some(Verdict::Budget);
                            return Err(Error::stop());
                        }
                        Err(error) => {
                            ex.stats.left_on_queue = (states - k + queued) as u64;
                            return Err(error);
                        }
                    };
                }
            }
            let local = k % chunk;
            let state = if ex.frontier.is_some() {
                &decoded.identity[local * nvars..(local + 1) * nvars]
            } else {
                &cur[k * nvars..(k + 1) * nvars]
            };
            if parallel && k % chunk == 0 {
                let started = Instant::now();
                let end = (k + chunk).min(states);
                let mut metrics = Metrics::default();
                let r = if let Some(coordinator) = ex.distributed {
                    coordinator
                        .expand(
                            plan,
                            machines[0].heap.store(),
                            if ex.frontier.is_some() {
                                &decoded.original
                            } else {
                                &cur[k * nvars..end * nvars]
                            },
                            &ex.seen,
                            ex.level - 1,
                            ex.poll,
                        )
                        .map(|work| {
                            received = Some(work);
                            Vec::new()
                        })
                } else {
                    parallel::expand_level(
                        plan,
                        machines,
                        if ex.frontier.is_some() {
                            &decoded.original
                        } else if ex.graph.is_some() || ex.checkpoint.is_some() {
                            &cur_originals[k * nvars..end * nvars]
                        } else {
                            &cur[k * nvars..end * nvars]
                        },
                        nvars,
                        &ex.seen,
                        ex.opts,
                        ex.checkpoint.is_some(),
                        ex.graph
                            .as_deref()
                            .map(graph::GraphSink::max_successors_per_unit),
                        ex.candidates
                            .as_ref()
                            .map(|account| (&ex.candidate_storage, account)),
                        ex.poll,
                        estimate.saturating_mul(end - k) / states.max(1),
                        &mut metrics,
                    )
                };
                let r = match r {
                    Ok(r) => r,
                    Err(error) => {
                        ex.stats.left_on_queue = (states - k + ex.queued_states()) as u64;
                        return Err(error);
                    }
                };
                ex.metrics.conjuncts_total += metrics.conjuncts_total;
                ex.metrics.conjuncts_skipped += metrics.conjuncts_skipped;
                ex.metrics.discarded_before_intern += metrics.discarded_before_intern;
                expand_time += started.elapsed();
                expanded = if received.is_some() { None } else { Some(r) };
                base = k;
            }
            let Some(m) = machines.first_mut() else {
                return Err(Error::internal("no core machine"));
            };
            let have_expansion = expanded.is_some() || received.is_some();
            let mut remote_expansion = if let Some(work) = received
                .as_mut()
                .and_then(|work| work.get_mut(k - base))
                .and_then(Option::take)
            {
                let Some(coordinator) = ex.distributed else {
                    return Err(Error::internal("remote result without its coordinator"));
                };
                let started = Instant::now();
                let mut metrics = Metrics::default();
                let imported = coordinator.import(work, plan, &mut m.heap, ex.poll, &mut metrics);
                let imported = match imported {
                    Ok(imported) => imported,
                    Err(error) => {
                        ex.stats.left_on_queue = (states - k + ex.queued_states()) as u64;
                        return Err(error);
                    }
                };
                ex.metrics.conjuncts_total += metrics.conjuncts_total;
                ex.metrics.conjuncts_skipped += metrics.conjuncts_skipped;
                ex.metrics.discarded_before_intern += metrics.discarded_before_intern;
                expand_time += started.elapsed();
                Some(imported)
            } else {
                None
            };
            let res = remote_expansion.as_mut().or_else(|| {
                expanded
                    .as_mut()
                    .and_then(|all| all.get_mut(k - base).and_then(Option::as_mut))
            });
            // Phase 1 polled the budget before each state it expanded, and
            // stopped before this one if it ran out.
            let exhausted = if have_expansion {
                res.is_none()
            } else {
                (ex.poll)()
            };
            if exhausted {
                ex.stats.left_on_queue = (states - k + ex.queued_states()) as u64;
                ex.stop = Some(Verdict::Budget);
                return Err(Error::stop());
            }
            let source = if ex.frontier.is_some() {
                &decoded.original[local * nvars..(local + 1) * nvars]
            } else if ex.graph.is_some() || ex.checkpoint.is_some() {
                cur_originals
                    .get(k * nvars..(k + 1) * nvars)
                    .ok_or_else(|| Error::internal("capture source original row missing"))?
            } else {
                state
            };
            m.s0.copy_from_slice(source);
            let level = i64::try_from(ex.level - 1).unwrap_or(i64::MAX);
            if res.is_none() {
                start_stream(m, plan, Some(source))?;
                m.level = level;
            }
            if let Some(r) = reservoir.as_deref_mut() {
                r.offer(source, level);
            }
            ex.cur_id = if ex.frontier.is_some() {
                decoded.ids.get(local).copied()
            } else {
                cur_ids.get(k).copied()
            };
            ex.emitted = 0;
            let r = match res {
                Some(res) => ex.commit(m, res),
                None => run_actions(m, ex, &plan.next, code),
            }
            .map_err(|e| generator_error(m, ex, state, e));
            if r.is_err()
                && ex
                    .failure
                    .as_ref()
                    .is_some_and(|failure| matches!(failure.site, Site::Next(_)))
            {
                let row_left = (states - k - 1 + ex.queued_states()) as u64;
                source_row(m, ex, state, row_left, true)?;
            }
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
            if let Some(all) = &mut expanded {
                all[k - base] = None;
            }
            m.heap.reset();
            let left = (states - k - 1 + ex.queued_states()) as u64;
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
            source_row(m, ex, state, left, false)?;
        }
        level_done(ex, nvars)?;
        if !parallel {
            ex.warnings.clone_from(&machines[0].warnings);
        }
        checkpoint::boundary(ex)?;
        if parallel {
            ex.metrics.expand_time += expand_time;
            ex.metrics.commit_time += level_started.elapsed().saturating_sub(expand_time);
        }
        cur.clear();
        cur_originals.clear();
        cur_ids.clear();
        // Every source-level owner is now dead. Only the next frontier and
        // permanent compiled roots may cross the Store replacement boundary.
        drop(expanded);
        drop(decoded);
        reclaim::boundary(machines, ex, store, permanent, control)?;
        std::mem::swap(&mut cur, &mut ex.next);
        std::mem::swap(&mut cur_originals, &mut ex.next_originals);
        std::mem::swap(&mut cur_ids, &mut ex.next_ids);
        ex.advance_frontier()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn frontier_plan(store: &Arc<Store>, false_invariant: bool) -> Result<Plan, Error> {
        use dive_core::plan::{Action, CompileStats, Conjunct, Effects, GNode, Node};

        let mut heap = dive_core::Heap::new(Arc::clone(store), true, false);
        let domain = heap.interval(0, 127);
        let domain = heap.intern_const(domain)?;
        let action = |name: &str, g| Action {
            name: name.into(),
            g,
            frame: 0,
            preset: Vec::new(),
        };
        Ok(Plan {
            code: Code {
                nodes: vec![
                    Node::Const(domain),
                    Node::Var(0),
                    Node::Const(Handle::FALSE),
                ],
                gnodes: vec![
                    GNode::AssignIn { var: 0, set: 0 },
                    GNode::Assign { var: 0, rhs: 1 },
                ],
                ..Code::default()
            },
            vars: vec!["x".into()],
            init: vec![action("Init", 0)],
            next: vec![action("Next", 1)],
            invariants: if false_invariant {
                vec![Check {
                    name: "Inv".into(),
                    frame: 0,
                    conjuncts: vec![Conjunct {
                        p: 2,
                        reads: 1,
                        skippable: false,
                        site: None,
                    }],
                }]
            } else {
                Vec::new()
            },
            constraints: Vec::new(),
            action_constraints: Vec::new(),
            implied_inits: Vec::new(),
            implied_actions: Vec::new(),
            check_deadlock: true,
            symmetry: None,
            effects: Effects::default(),
            view: None,
            stats: CompileStats::default(),
            bound_preset: Vec::new(),
            startup_assumptions: Vec::new(),
            startup_assumption_false: None,
        })
    }

    #[derive(Clone, Copy, Debug)]
    enum FrontierFault {
        Corrupt,
        Missing,
        Cleanup,
    }

    #[allow(
        clippy::unwrap_used,
        clippy::too_many_lines,
        reason = "fault mutation and real explorer outcome remain in one deterministic control"
    )]
    fn frontier_fault(
        tlc: bool,
        exact: bool,
        workers: usize,
        initial_violation: bool,
        fault: FrontierFault,
    ) {
        use std::fs;
        use std::io::{Seek, SeekFrom, Write};
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "dive-frontier-explorer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let store = Arc::new(Store::with_semantics(
            true,
            if tlc {
                dive_core::Semantics::tlc()
            } else {
                dive_core::Semantics::default()
            },
        ));
        let plan = frontier_plan(&store, initial_violation).unwrap();
        let calls = AtomicUsize::new(0);
        let injected = AtomicBool::new(false);
        let trigger = if initial_violation { 1 } else { 129 };
        let poll = || {
            if calls.fetch_add(1, Ordering::Relaxed) == trigger {
                let owned = fs::read_dir(&directory)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                match fault {
                    FrontierFault::Corrupt => {
                        let mut file = fs::OpenOptions::new()
                            .write(true)
                            .open(owned.join("states"))
                            .unwrap();
                        file.seek(SeekFrom::End(-1)).unwrap();
                        file.write_all(&[255]).unwrap();
                    }
                    FrontierFault::Missing => fs::remove_file(owned.join("states")).unwrap(),
                    FrontierFault::Cleanup => {
                        fs::rename(&owned, directory.join("moved")).unwrap();
                        fs::write(owned, b"blocks directory cleanup").unwrap();
                    }
                }
                injected.store(true, Ordering::Relaxed);
            }
            false
        };
        let out = explore_with_storage(
            &plan,
            &store,
            Config {
                opts: Opts::default(),
                max_states: None,
                max_depth: None,
                workers,
                warn: true,
                sample: None,
            },
            &poll,
            None,
            None,
            &storage::Storage {
                exact,
                frontier_memory_bytes: Some(0),
                frontier_directory: Some(directory.clone()),
                ..storage::Storage::default()
            },
        );
        assert!(injected.load(Ordering::Relaxed), "{fault:?}");
        assert!(
            matches!(out.verdict, Verdict::Internal { .. }),
            "{fault:?}: {:?}",
            out.verdict
        );
        assert!(
            out.trace.is_none(),
            "corrupt storage must clear a deciding word"
        );
        let failures = &out.metrics.storage.frontier.failures;
        assert_ne!(failures.as_slice(), [] as [String; 0]);
        let expected = match fault {
            FrontierFault::Corrupt => "checksum",
            FrontierFault::Missing => "open record",
            FrontierFault::Cleanup => "cleanup",
        };
        assert!(
            failures.iter().any(|message| message.contains(expected)),
            "{failures:?}"
        );
        if initial_violation {
            assert!(
                out.metrics
                    .storage
                    .frontier
                    .deciding_event_before_failure
                    .as_ref()
                    .unwrap()
                    .contains("InvariantViolated")
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn corrupt_read_and_cleanup_failures_prevent_success_and_counterexample_certification() {
        for tlc in [false, true] {
            for exact in [false, true] {
                for workers in [1, 4, 8] {
                    for initial in [false, true] {
                        for fault in [
                            FrontierFault::Corrupt,
                            FrontierFault::Missing,
                            FrontierFault::Cleanup,
                        ] {
                            frontier_fault(tlc, exact, workers, initial, fault);
                        }
                    }
                }
            }
        }
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "owned cancellation during final frontier validation"
    )]
    fn validation_cancellation_is_a_budget_stop_without_storage_failure_or_word() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        for initial_violation in [false, true] {
            for workers in [1, 4, 8] {
                let store = Arc::new(Store::new(true));
                let plan = frontier_plan(&store, initial_violation).unwrap();
                // One Init plus final validation for a deciding initial word;
                // otherwise 128 Init + one decoded chunk + 128 expansions.
                let trigger = if initial_violation { 1 } else { 257 };
                let calls = AtomicUsize::new(0);
                let poll = || calls.fetch_add(1, Ordering::Relaxed) >= trigger;
                let out = explore_with_storage(
                    &plan,
                    &store,
                    Config {
                        opts: Opts::default(),
                        max_states: None,
                        max_depth: None,
                        workers,
                        warn: true,
                        sample: None,
                    },
                    &poll,
                    None,
                    None,
                    &storage::Storage {
                        frontier_memory_bytes: Some(0),
                        ..storage::Storage::default()
                    },
                );
                assert_eq!(out.verdict, Verdict::Budget);
                assert!(out.trace.is_none());
                assert_eq!(out.metrics.storage.frontier.failures, [] as [String; 0]);
                assert_eq!(calls.load(Ordering::Relaxed), trigger + 1);
                assert_eq!(out.metrics.storage.frontier.retained_bytes, 0);
            }
        }
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "deterministic owned budget and spilled-frontier control"
    )]
    fn budget_stop_closes_an_unread_spilled_frontier_without_validation_polls() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        for tlc in [false, true] {
            for exact in [false, true] {
                for workers in [1, 4, 8] {
                    let store = Arc::new(Store::with_semantics(
                        true,
                        if tlc {
                            dive_core::Semantics::tlc()
                        } else {
                            dive_core::Semantics::default()
                        },
                    ));
                    let plan = frontier_plan(&store, false).unwrap();
                    let calls = AtomicUsize::new(0);
                    let poll = || calls.fetch_add(1, Ordering::Relaxed) >= 128;
                    let out = explore_with_storage(
                        &plan,
                        &store,
                        Config {
                            opts: Opts::default(),
                            max_states: None,
                            max_depth: None,
                            workers,
                            warn: true,
                            sample: None,
                        },
                        &poll,
                        None,
                        None,
                        &storage::Storage {
                            exact,
                            frontier_memory_bytes: Some(0),
                            ..storage::Storage::default()
                        },
                    );
                    assert_eq!(out.verdict, Verdict::Budget);
                    assert_eq!(
                        out.stats,
                        Stats {
                            generated: 128,
                            distinct: 128,
                            depth: 1,
                            left_on_queue: 128,
                            initial_generated: 128
                        }
                    );
                    assert!(out.trace.is_none());
                    assert_eq!(
                        calls.load(Ordering::Relaxed),
                        129,
                        "cleanup must not scan/poll the unread queue"
                    );
                    assert!(out.metrics.storage.frontier.spilled_bytes > 0);
                    assert_eq!(out.metrics.storage.frontier.retained_bytes, 0);
                    assert_eq!(out.metrics.storage.frontier.failures, [] as [String; 0]);
                }
            }
        }
    }

    #[test]
    fn impossible_capture_growth_is_fallible_without_modifying_retained_payload() {
        let mut values = vec![Handle::EMPTY];
        let capacity = values.capacity();
        assert!(capture_reserve(&mut values, usize::MAX).is_err());
        assert_eq!(values, [Handle::EMPTY]);
        assert_eq!(values.capacity(), capacity);
    }

    /// An invariant whose own frame forces a thunk to a transient (a
    /// definition built per checked state, as `Corpus` in
    /// `LeastCircularSubstring`) still frees the check's transients once each
    /// initial state is accepted: the thunk's slot leaves the frame with the
    /// check. Before, every initial state's copy was held until `Init` ended.
    #[test]
    #[allow(clippy::unwrap_used, reason = "deterministic explorer control")]
    fn invariant_frame_thunks_do_not_hold_initial_transients() {
        use dive_core::plan::{Action, CompileStats, Conjunct, Effects, GNode, Node};

        const WIDTH: usize = 64;
        const STATES: u64 = 128;
        let store = Arc::new(Store::new(false));
        let mut heap = dive_core::Heap::new(Arc::clone(&store), false, false);
        let domain = heap.interval(0, 127);
        let domain = heap.intern_const(domain).unwrap();
        let action = |name: &str, g| Action {
            name: name.into(),
            g,
            // A live action frame below the check's.
            frame: 1,
            preset: Vec::new(),
        };
        let plan = Plan {
            code: Code {
                nodes: vec![
                    Node::Const(domain),
                    Node::Var(0),
                    // A tuple built from the checked state: always transient.
                    Node::Tuple(vec![1; WIDTH].into_boxed_slice()),
                    Node::Lazy { slot: 0, body: 2 },
                    Node::Eq(3, 3),
                ],
                gnodes: vec![
                    GNode::AssignIn { var: 0, set: 0 },
                    GNode::Assign { var: 0, rhs: 1 },
                ],
                ..Code::default()
            },
            vars: vec!["x".into()],
            init: vec![action("Init", 0)],
            next: vec![action("Next", 1)],
            invariants: vec![Check {
                name: "Inv".into(),
                frame: 1,
                conjuncts: vec![Conjunct {
                    p: 4,
                    reads: 1,
                    skippable: false,
                    site: None,
                }],
            }],
            constraints: Vec::new(),
            action_constraints: Vec::new(),
            implied_inits: Vec::new(),
            implied_actions: Vec::new(),
            check_deadlock: true,
            symmetry: None,
            effects: Effects::default(),
            view: None,
            stats: CompileStats::default(),
            bound_preset: Vec::new(),
            startup_assumptions: Vec::new(),
            startup_assumption_false: None,
        };
        let out = explore(
            &plan,
            &store,
            Config {
                opts: Opts::default(),
                max_states: None,
                max_depth: None,
                workers: 1,
                warn: true,
                sample: None,
            },
            &|| false,
        );
        assert_eq!(out.verdict, Verdict::Success);
        assert_eq!(out.stats.distinct, STATES);
        assert_eq!(out.metrics.thunks, (STATES, STATES));
        // One state's tuple at a time, not one per initial state.
        let (_, handles) = out.metrics.arena_peak;
        assert!(
            handles < 4 * WIDTH,
            "arena peak of {handles} handles holds the initial states' check transients"
        );
    }

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
