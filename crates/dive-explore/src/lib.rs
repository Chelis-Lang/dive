//! `dive-explore`: single-threaded breadth-first exploration over a compiled
//! [`dive_core::Plan`] (the M0 spike of `docs/developer/core/07-engine.md`).
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

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_core::handle::{K_STATE, fin, state_term};
use dive_core::memo::{Memo, SiteStats};
use dive_core::plan::{Check, Code};
use dive_core::store::StoreStats;
use dive_core::{Error, ErrorKind, Handle, Machine, Opts, Plan, Sink, Store};

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
    /// A construct, size or error the M0 core does not handle.
    Unsupported {
        /// What it was.
        feature: String,
    },
    /// The time limit was reached.
    TimeLimit,
    /// A bug in the core.
    Internal {
        /// What happened.
        message: String,
    },
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
}

/// Run settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Config {
    /// Strategy switches.
    pub opts: Opts,
    /// When to stop.
    pub deadline: Option<Instant>,
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
}

enum Probe {
    Seen,
    New,
}

impl Explorer<'_> {
    /// Evaluates a check on the state in `m.s1`.
    fn check(&mut self, m: &mut Machine, code: &Code, c: &Check) -> Result<bool, Error> {
        std::mem::swap(&mut m.s0, &mut m.s1);
        let old = m.enter(c.frame);
        let mut r = Ok(true);
        for conj in &c.conjuncts {
            self.metrics.conjuncts_total += 1;
            if self.opts.parent_skip
                && !self.initial
                && conj.skippable
                && conj.reads & self.changed == 0
            {
                self.metrics.conjuncts_skipped += 1;
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

    fn probe(&mut self, m: &mut Machine) -> Result<(Probe, bool), Error> {
        let plan = self.plan;
        let code = &plan.code;
        // Constraints first.
        let mut in_model = true;
        for c in &plan.constraints {
            if !self.check(m, code, c)? {
                in_model = false;
                break;
            }
        }
        if !self.opts.early_probe {
            for i in 0..m.s1.len() {
                m.s1[i] = m.heap.intern(m.s1[i])?;
            }
        }
        let key = state_key(m);
        self.metrics.probed += 1;
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
            let missing: Vec<&str> =
                m.s1.iter()
                    .zip(&plan.vars)
                    .filter(|(h, _)| h.is_empty())
                    .map(|(_, n)| n.as_str())
                    .collect();
            return Err(Error::eval(format!(
                "{} is not completely specified (variables {})",
                if self.initial {
                    "an initial state"
                } else {
                    "a successor state"
                },
                missing.join(", ")
            )));
        }
        if !self.initial {
            self.changed = 0;
            for (i, (a, b)) in m.s0.iter().zip(&m.s1).enumerate() {
                if a != b {
                    self.changed |= 1u64 << i;
                }
            }
        }
        let (probe, in_model) = self.probe(m)?;
        if matches!(probe, Probe::Seen) {
            return Ok(());
        }
        // TLC inserts a new state, counting it and its level, before it
        // checks the invariants.
        if in_model {
            self.stats.distinct += 1;
            self.stats.depth = self.stats.depth.max(self.level);
        }
        for c in &plan.invariants {
            if !self.check(m, code, c)? {
                self.stop = Some(Verdict::InvariantViolated {
                    property: c.name.clone(),
                    initial: self.initial,
                });
                return Err(Error::stop());
            }
        }
        if in_model {
            for i in 0..m.s1.len() {
                let h = m.heap.intern(m.s1[i])?;
                self.next.push(h);
            }
        }
        Ok(())
    }
}

impl Sink for Explorer<'_> {
    fn emit(&mut self, m: &mut Machine, _code: &Code) -> Result<(), Error> {
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
    for a in actions {
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
pub fn explore(plan: &Plan, store: &Arc<Store>, config: Config) -> Outcome {
    let start = Instant::now();
    let nvars = plan.vars.len();
    let keys: Vec<usize> = plan
        .code
        .sites
        .iter()
        .map(|s| s.vars.len() + s.slots.len())
        .collect();
    let mut m = Machine::new(Arc::clone(store), nvars, config.opts, Memo::new(&keys));
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
    };
    let result = run(&mut m, &mut ex, plan, config);
    let verdict = match result {
        Ok(()) => Verdict::Success,
        Err(e) => match e.into_parts() {
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
    Outcome {
        verdict,
        stats: ex.stats,
        metrics,
    }
}

fn run(m: &mut Machine, ex: &mut Explorer<'_>, plan: &Plan, config: Config) -> Result<(), Error> {
    let code = &plan.code;
    let nvars = plan.vars.len();
    run_actions(m, ex, &plan.init, code)?;
    m.heap.reset();
    ex.initial = false;
    let mut cur = std::mem::take(&mut ex.next);
    let mut count = 0u64;
    while !cur.is_empty() {
        ex.level += 1;
        for (k, state) in cur.chunks_exact(nvars.max(1)).enumerate() {
            if nvars == 0 {
                break;
            }
            m.s0.copy_from_slice(state);
            ex.emitted = 0;
            let r = run_actions(m, ex, &plan.next, code);
            m.heap.reset();
            if let Err(e) = r {
                ex.stats.left_on_queue = (cur.len() / nvars - k - 1 + ex.next.len() / nvars) as u64;
                return Err(e);
            }
            if ex.emitted == 0 && plan.check_deadlock {
                ex.stop = Some(Verdict::Deadlock);
                return Err(Error::stop());
            }
            count += 1;
            if count.is_multiple_of(64)
                && let Some(d) = config.deadline
                && Instant::now() >= d
            {
                ex.stop = Some(Verdict::TimeLimit);
                return Err(Error::stop());
            }
        }
        cur.clear();
        std::mem::swap(&mut cur, &mut ex.next);
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
