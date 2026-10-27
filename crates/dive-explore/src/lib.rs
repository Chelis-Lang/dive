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
mod parallel;
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

/// The state key of `m.s1`.
fn state_key(m: &Machine) -> u64 {
    let mut sum = 0u64;
    for (i, &h) in m.s1.iter().enumerate() {
        sum = sum.wrapping_add(state_term(i, m.heap.hash(h)));
    }
    fin(K_STATE, m.s1.len() as u64, sum)
}

struct Explorer<'p> {
    plan: &'p Plan,
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

    fn probe(&mut self, m: &mut Machine) -> Result<(Probe, bool), Error> {
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
        let key = state_key(m);
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
            return Ok((Probe::Seen, in_model));
        }
        Ok((Probe::New, in_model))
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
        let (probe, in_model) = self.probe(m)?;
        if matches!(probe, Probe::Seen) {
            return Ok(());
        }
        if in_model && !fingerprintable(m, self.initial)? {
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
        if in_model && !enqueue_first {
            self.enqueue(m)?;
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
        for (i, c) in plan.action_constraints.iter().enumerate() {
            if !check_step(m, code, c).map_err(|e| (Site::ActionConstraint(i), e))? {
                return Ok(false);
            }
        }
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
        let r = self.accept(m);
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

/// Explores the plan's state space.
#[must_use]
pub fn explore(
    plan: &Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &(dyn Fn() -> bool + Sync),
) -> Outcome {
    let start = Instant::now();
    let nvars = plan.vars.len();
    let keys: Vec<usize> = plan
        .code
        .sites
        .iter()
        .map(|s| s.vars.len() + s.slots.len())
        .collect();
    let mut machines: Vec<Machine> = (0..config.workers.max(1))
        .map(|_| {
            let mut m = Machine::new(Arc::clone(store), nvars, config.opts, Memo::new(&keys));
            m.warn = config.warn;
            m
        })
        .collect();
    let mut ex = Explorer {
        plan,
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
    let result = run(&mut machines, &mut ex, plan, &mut replayed);
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
            if states.last().map(|s| s.values.as_slice()) != Some(end.expect.as_slice()) {
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

#[allow(
    clippy::too_many_lines,
    reason = "TLC's ordered level loop: expansion, stops, replay and deadlock"
)]
fn run(
    machines: &mut [Machine],
    ex: &mut Explorer<'_>,
    plan: &Plan,
    replayed: &mut Option<Replayed>,
) -> Result<(), Error> {
    let code = &plan.code;
    let nvars = plan.vars.len();
    let parallel = machines.len() > 1;
    let Some(m) = machines.first_mut() else {
        return Err(Error::internal("no core machine"));
    };
    if let Err(e) = run_actions(m, ex, &plan.init, code) {
        ex.stats.left_on_queue = (ex.next.len() / nvars.max(1)) as u64;
        return Err(generator_error(m, ex, e));
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
        let expanded = if parallel && nvars > 0 {
            let mut metrics = Metrics::default();
            let r = parallel::expand_level(
                plan,
                machines,
                &cur,
                nvars,
                &ex.seen,
                ex.opts,
                ex.poll,
                estimate,
                &mut metrics,
            )?;
            ex.metrics.conjuncts_total += metrics.conjuncts_total;
            ex.metrics.conjuncts_skipped += metrics.conjuncts_skipped;
            ex.metrics.discarded_before_intern += metrics.discarded_before_intern;
            ex.metrics.expand_time += level_started.elapsed();
            Some(r)
        } else {
            None
        };
        let Some(m) = machines.first_mut() else {
            return Err(Error::internal("no core machine"));
        };
        for (k, state) in cur.chunks_exact(nvars.max(1)).enumerate() {
            if nvars == 0 {
                break;
            }
            let res = expanded
                .as_ref()
                .and_then(|all| all.get(k).and_then(Option::as_ref));
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
        }
        if expanded.is_some() {
            ex.metrics.commit_time += level_started.elapsed();
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
