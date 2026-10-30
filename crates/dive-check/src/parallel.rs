//! The parallel level engine (ADR 0015 §2–§3): every level in three phases
//! separated by barriers, on a pool of workers, with results that are a
//! function of the reference order 𝒪 alone.
//!
//! **Units and positions.** A level's work is a sequence of *units*: one
//! per state, or, on a narrow level (fewer than two states per worker, more
//! than one action), one per (state, action), so that the first levels of a
//! search stay parallel. A successor's **position** is `(i, a, j)`: its
//! state's index `i` in the level, the unit's action `a` (0 when the unit
//! covers every action), and its ordinal `j` within the unit. Positions
//! order successors as 𝒪 does. An **event key** adds the phase of the
//! check within the successor (0 completeness and enumeration, 1
//! constraints and `VIEW`, 2 invariants, 3 implied actions, 9 deadlock).
//!
//! **Phase 1, expand.** Workers claim blocks of consecutive units through
//! an atomic counter. For each successor they count it, check it is fully
//! assigned, evaluate the constraints, key it (fingerprint or exact) and
//! test the key against the seen-state set, which is read-only in this
//! phase: a key not yet seen becomes a candidate `(key, position, state,
//! action)` in the worker's buffer. Successors outside the model are
//! checked against the invariants here, on every occurrence, as in TLC.
//! Implied actions are evaluated on every step. A block stops at its first
//! event.
//!
//! **Phase 2, resolve.** `StateSet::resolve` keeps, for each new key, the
//! candidate with the smallest position: exactly TLC's first occurrence.
//!
//! **Phase 3, check.** The winners, in position order, are checked against
//! the invariants (TLC checks a state the first time it inserts it), in
//! parallel chunks. They form the next level's frontier, in order.
//!
//! **The stop.** E\* is the smallest event key of all phases. The counts at
//! E\* are ADR 0015 §3's closed forms over positions, so no part of the
//! result depends on which worker ran which block, on the block sizes, or
//! on the order of claims.
//!
//! **`Print`.** Each worker captures `Print` output and tags it with the
//! event key of the evaluation that printed it (enumeration lines with the
//! position of the successor that follows them). Output is released at the
//! end of the level in key order, up to and including E\*: TLC's order.
//!
//! **Early cut.** The smallest unit with a phase-1 event is published in an
//! atomic; workers skip units after it, which can only hold events after
//! E\*. Correctness never depends on the cut.
//!
//! **Chaos** (`CheckOptions::chaos`, test-only) randomizes block and chunk
//! sizes, claim orders, the narrow-level split, which phases run inline,
//! the resolve's shard order and thread count, and inserts yields.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use dive_eval::St;
use dive_eval::capture::{Captured, PrintCapture};
use dive_eval::next::{Flow, StateVals, Successor};
use dive_model::Model;
use dive_store::trace::StateId;
use dive_store::{Candidate, Candidates, ResolveOptions};
use dive_value::Ctx;

use crate::class;
use crate::engine::{
    Keyed, Levels, Search, Stop, action_name, check_implied_actions, check_invariants,
    error_verdict, in_model, incomplete, level_of, resumes, state_key_of, store_verdict,
    trace_state, values,
};
use crate::host::{ThreadLevel, set_thread_level};
use crate::options::CheckOptions;
use crate::pool::{Job, Pool, with_pool};
use crate::result::{ErrorSite, Stats, TraceState, Verdict};

/// A successor's position in its level: state, unit action, ordinal in the
/// unit.
type Pos = (u64, u32, u32);
/// An event key: a position and the phase of the check within it.
type Key = (Pos, u8);

const PH_ENUM: u8 = 0;
const PH_CHECK: u8 = 1;
const PH_NATIVE: u8 = 2;
const PH_CAPTURE: u8 = 3;
const PH_INV: u8 = 4;
const PH_IMPLIED: u8 = 5;
const PH_DEADLOCK: u8 = 9;

/// Below this many candidates a level's resolve runs on one thread.
const PARALLEL_RESOLVE: usize = 4096;

/// At most this many emitted successors run between budget polls within a unit.
const EMISSION_BUDGET_INTERVAL: u64 = 1024;

/// What travels with a candidate to the next frontier.
struct Payload {
    vals: StateVals,
    /// The action that produced it (index into `Model::actions`).
    action: u32,
}

/// An event: something that stops the search if nothing precedes it.
struct Event {
    key: Key,
    /// The unit it happened in.
    unit: usize,
    /// Successors of the unit that TLC counts at this event (ADR 0015 §3's
    /// `c`, within the unit).
    c: u64,
    verdict: Verdict,
    /// The counterexample's last state, for events on a successor.
    then: Option<TraceState>,
    /// Whether TLC's replay goes on past this event (an invariant that
    /// failed to evaluate on a new state, `Search::replay_after`).
    resume: bool,
    /// An implicit-stutter bound uses the inline engine's no-path/zero-queue stop.
    stutter: bool,
    /// This unit stopped before its next successor; its source remains queued.
    partial: bool,
}

fn earlier(a: Option<Event>, b: Option<Event>) -> Option<Event> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if y.key < x.key { y } else { x }),
        (x, None) => x,
        (None, y) => y,
    }
}

type Lines = Vec<(Key, Captured)>;

fn flush(cap: &PrintCapture, lines: &mut Lines, key: Key) {
    for l in cap.take() {
        lines.push((key, l));
    }
}

/// A deterministic generator for chaos (`splitmix64`).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n > 0`).
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n.max(1) as u64)).unwrap_or(0)
    }

    fn coin(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn maybe_yield(rng: &mut Option<Self>) {
        if let Some(r) = rng
            && r.next() % 4 == 0
        {
            std::thread::yield_now();
        }
    }

    fn shuffle<X>(&mut self, v: &mut [X]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

/// `n` items split into consecutive ranges of `size`.
fn ranges(n: usize, size: usize) -> Vec<Range<usize>> {
    let size = size.max(1);
    (0..n).step_by(size).map(|s| s..(s + size).min(n)).collect()
}

/// Whether an event at unit `cut` (`u64::MAX`: none) is decided by expanded
/// blocks alone: every block starting at or before it has output (`done`).
fn covers(blocks: &[Range<usize>], cut: u64, done: impl Fn(usize) -> bool) -> bool {
    cut != u64::MAX
        && blocks
            .iter()
            .enumerate()
            .all(|(b, range)| range.start as u64 > cut || done(b))
}

/// ADR 0015 §2's block size: about 16 blocks per worker, 1 to 256 units.
fn block_size(n: usize, threads: usize) -> usize {
    (n / (16 * threads.max(1))).clamp(1, 256)
}

/// One level's phase-1 output of a block.
struct Accepted<K> {
    vals: StateVals,
    pos: Pos,
    key: K,
    action: u32,
}

struct BlockOut<K> {
    /// Successors generated by each unit processed, from the block's first.
    gens: Vec<u64>,
    event: Option<Event>,
    lines: Lines,
    /// Every accepted occurrence, including seen states and same-wave duplicates.
    accepted: Vec<Accepted<K>>,
}

/// Returned outputs selected for one actual ordered publication transaction.
struct CommitInput<K> {
    gens: Vec<u64>,
    done: Vec<bool>,
    lines: Lines,
    first: Option<Event>,
    accepted: Vec<Accepted<K>>,
    buffers: Vec<Candidates<K, Pos, Payload>>,
}

/// One level's phase-1 job.
struct Expand<'env, S: Keyed> {
    m: &'env Model,
    keyer: &'env S::Keyer,
    set: &'env RwLock<S>,
    ctx: Ctx,
    level: u64,
    frontier: Arc<crate::frontier::Queue>,
    narrow: bool,
    check_deadlock: bool,
    blocks: Vec<Range<usize>>,
    /// The order in which blocks are claimed (identity without chaos).
    order: Vec<usize>,
    claim: AtomicUsize,
    /// The smallest unit with an event so far.
    cut: AtomicU64,
    outs: Vec<Mutex<Option<BlockOut<S::Key>>>>,
    /// Per-source scratch cap, absent for ordinary safety exploration.
    capture_limit: Option<u64>,
    /// Per-unit ceiling; the aggregate native counter is resolved in reference order.
    native_counter_remaining: Option<u64>,
    capture_quotas: Option<(usize, u64)>,
    buffers: Mutex<Vec<Candidates<S::Key, Pos, Payload>>>,
    chaos: Option<u64>,
    /// The run's armed budget, polled per block and amortized during emission.
    budget: crate::budget::Budget,
    control: Arc<crate::budget::EvaluationControl>,
    /// Whether a block was left unexpanded because the budget ran out; the
    /// wave is then not published.
    exhausted: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    pause: Option<Arc<budget_cut_tests::Pause>>,
}

/// The result of a successor's phase-1 checks that stops its block.
struct Stopped {
    verdict: Verdict,
    phase: u8,
    /// Whether the successor ends the counterexample.
    with: bool,
}

impl<S: Keyed> Expand<'_, S> {
    /// Capture waves have one block per concrete source. Missing output stops
    /// this prefix; a claim alone never counts as a returned expansion.
    /// Moves the joined whole wave outputs and their candidate buffers.
    fn take_sources(&self, units: Range<usize>, n_units: usize) -> CommitInput<S::Key> {
        let mut input = CommitInput {
            gens: vec![0; n_units],
            done: vec![false; n_units],
            lines: Vec::new(),
            first: None,
            accepted: Vec::new(),
            buffers: Vec::new(),
        };
        for (block, out) in self.blocks.iter().zip(&self.outs) {
            if block.start < units.start || block.end > units.end {
                continue;
            }
            let Some(out) = out.lock().unwrap_or_else(PoisonError::into_inner).take() else {
                continue;
            };
            for (k, generated) in out.gens.iter().enumerate() {
                input.gens[block.start + k] = *generated;
                input.done[block.start + k] = true;
            }
            input.lines.extend(out.lines);
            input.accepted.extend(out.accepted);
            input.first = earlier(input.first, out.event);
        }
        input.buffers =
            std::mem::take(&mut *self.buffers.lock().unwrap_or_else(PoisonError::into_inner));
        input
    }

    /// Whether some block found an event and every block that starts at or
    /// before the earliest one was expanded, so that the event is the wave's
    /// first in reference order even though the budget left later blocks
    /// unexpanded. Coverage is checked per block, whatever the claim order.
    fn covers_first_event(&self) -> bool {
        covers(&self.blocks, self.cut.load(Ordering::Relaxed), |b| {
            self.outs[b]
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some()
        })
    }

    fn actions(&self) -> usize {
        self.m.actions.len()
    }

    /// The state index and action range of unit `u`, and the unit's action
    /// number in positions.
    fn unit(&self, u: usize) -> (usize, Range<usize>, u32) {
        if self.narrow {
            let a = u % self.actions();
            (
                u / self.actions(),
                a..a + 1,
                u32::try_from(a).unwrap_or(u32::MAX),
            )
        } else {
            (u, 0..self.actions(), 0)
        }
    }

    fn work(&self, w: usize) {
        let _level = ThreadLevel::enter(level_of(self.level));
        let set = self.set.read().unwrap_or_else(PoisonError::into_inner);
        let mut buf: Candidates<S::Key, Pos, Payload> = set.candidates();
        let mut rng = self
            .chaos
            .map(|c| Rng(c ^ (w as u64).wrapping_mul(0x2545_f491_4f6c_dd1d) ^ self.level));
        loop {
            Rng::maybe_yield(&mut rng);
            let k = self.claim.fetch_add(1, Ordering::Relaxed);
            let Some(&b) = self.order.get(k) else { break };
            let range = self.blocks[b].clone();
            #[cfg(test)]
            if let Some(pause) = &self.pause {
                pause.before_poll(range.start);
            }
            if range.start as u64 > self.cut.load(Ordering::Relaxed) {
                continue;
            }
            if !matches!(self.budget.poll(), Ok(None)) {
                self.exhausted.store(true, Ordering::Relaxed);
                #[cfg(test)]
                if let Some(pause) = &self.pause {
                    pause.stopped();
                }
                break;
            }
            #[cfg(test)]
            if let Some(pause) = &self.pause {
                pause.after_poll(range.start);
            }
            let out = self.block(&set, range, &mut buf, &mut rng);
            *self.outs[b].lock().unwrap_or_else(PoisonError::into_inner) = Some(out);
        }
        drop(set);
        self.buffers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(buf);
    }

    fn block(
        &self,
        set: &S,
        range: Range<usize>,
        buf: &mut Candidates<S::Key, Pos, Payload>,
        rng: &mut Option<Rng>,
    ) -> BlockOut<S::Key> {
        let cap = PrintCapture::start();
        let mut out = BlockOut {
            gens: Vec::with_capacity(range.len()),
            event: None,
            lines: Vec::new(),
            accepted: Vec::new(),
        };
        for u in range {
            if u as u64 > self.cut.load(Ordering::Relaxed) {
                break;
            }
            Rng::maybe_yield(rng);
            let (generated, event) =
                self.expand(set, u, buf, &cap, &mut out.lines, &mut out.accepted);
            out.gens.push(generated);
            if let Some(e) = event {
                #[cfg(test)]
                if let Some(pause) = &self.pause {
                    pause.event(e.unit);
                }
                self.cut.fetch_min(u as u64, Ordering::Relaxed);
                out.event = Some(e);
                break;
            }
        }
        out
    }

    /// Expands unit `unit`. Returns the successors generated and the event that
    /// stopped it.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one unit ordered expansion and fallible frontier reads"
    )]
    fn expand(
        &self,
        set: &S,
        unit: usize,
        buf: &mut Candidates<S::Key, Pos, Payload>,
        cap: &PrintCapture,
        lines: &mut Lines,
        accepted: &mut Vec<Accepted<S::Key>>,
    ) -> (u64, Option<Event>) {
        let m = self.m;
        let (index, actions, ua) = self.unit(unit);
        let (state_values, _) = match self.frontier.read(index, &m.names) {
            Ok(state) => state,
            Err(verdict) => {
                return (
                    0,
                    Some(Event {
                        key: ((index as u64, ua, 0), PH_ENUM),
                        unit,
                        c: 0,
                        verdict,
                        then: None,
                        resume: false,
                        stutter: false,
                        partial: false,
                    }),
                );
            }
        };
        let s0 = &state_values;
        let state = index as u64;
        let mut generated = 0u64;
        // TLC's intermediate states of `\cdot`, counted as generated but
        // never successors (`--action-composition=tlc`, D-022).
        let mut intermediate = 0u64;
        let accepted_start = accepted.len();
        let mut event: Option<Event> = None;
        for a in actions {
            set_thread_level(level_of(self.level));
            let first = if self.narrow { 0 } else { generated };
            let action = u32::try_from(a).unwrap_or(u32::MAX);
            let r = m
                .evaluator
                .next_states(&m.actions[a], s0, first, &mut |succ: &Successor<'_>| {
                    if self
                        .native_counter_remaining
                        .is_some_and(|limit| generated >= limit)
                    {
                        event = Some(Event {
                            key: (
                                (state, ua, u32::try_from(succ.ordinal).unwrap_or(u32::MAX)),
                                PH_ENUM,
                            ),
                            unit,
                            c: generated,
                            verdict: crate::checkpoint::failure(
                                "generated counter exceeds native positive TLC-long bound",
                            ),
                            then: None,
                            resume: false,
                            stutter: false,
                            partial: false,
                        });
                        return Flow::Stop;
                    }
                    #[cfg(test)]
                    if let Some(pause) = &self.pause {
                        pause.emitted(unit, generated, &self.budget);
                    }
                    // The first chunk was admitted by the block-start poll.
                    // Keep already-in-flight events authoritative, then sample
                    // the same armed signal at amortized emission boundaries.
                    if generated != 0
                        && generated.is_multiple_of(EMISSION_BUDGET_INTERVAL)
                        && !matches!(self.budget.poll(), Ok(None))
                    {
                        let pos = (state, ua, u32::try_from(succ.ordinal).unwrap_or(u32::MAX));
                        flush(cap, lines, (pos, PH_ENUM));
                        self.exhausted.store(true, Ordering::Relaxed);
                        let phase = if self.capture_limit.is_some() {
                            crate::budget::Phase::Capture
                        } else {
                            crate::budget::Phase::Search
                        };
                        event = Some(Event {
                            key: (pos, PH_ENUM),
                            unit,
                            c: generated,
                            verdict: match self.budget.poll() {
                                Ok(Some(exhausted)) => {
                                    Verdict::Stopped(Box::new(exhausted.during(phase)))
                                }
                                Err(error) => Verdict::BudgetMeasurement { error },
                                Ok(None) => return Flow::Stop,
                            },
                            then: None,
                            resume: false,
                            stutter: false,
                            partial: true,
                        });
                        return Flow::Stop;
                    }
                    generated += 1;
                    let Ok(ord) = u32::try_from(succ.ordinal) else {
                        event = Some(Event {
                            key: ((state, ua, u32::MAX), PH_ENUM),
                            unit,
                            c: generated,
                            verdict: Verdict::InternalError {
                                message: "more than 2^32 successors of one state".to_owned(),
                            },
                            then: None,
                            resume: false,
                            stutter: false,
                            partial: false,
                        });
                        return Flow::Stop;
                    };
                    let pos = (state, ua, ord);
                    flush(cap, lines, (pos, PH_ENUM));
                    if self.capture_limit.is_some_and(|limit| generated > limit) {
                        event = Some(Event {
                            key: (pos, PH_ENUM),
                            unit,
                            c: generated,
                            verdict: crate::capture::limit(
                                "successors per source (scratch admission)",
                            ),
                            then: None,
                            resume: false,
                            stutter: false,
                            partial: false,
                        });
                        return Flow::Stop;
                    }
                    match self.successor(
                        set,
                        s0,
                        succ,
                        action,
                        pos,
                        buf,
                        cap,
                        lines,
                        accepted,
                        accepted_start,
                    ) {
                        None => Flow::Continue,
                        Some(s) => {
                            event = Some(Event {
                                key: (pos, s.phase),
                                unit,
                                c: u64::from(ord) + 1,
                                then: if s.with {
                                    Some(trace_state(m, succ, false))
                                } else {
                                    None
                                },
                                verdict: s.verdict,
                                resume: false,
                                stutter: false,
                                partial: false,
                            });
                            Flow::Stop
                        }
                    }
                });
            intermediate = intermediate.saturating_add(match &r {
                Ok(o) => o.intermediate,
                Err(e) => e.intermediate,
            });
            match r {
                Ok(o) if o.stopped => break,
                Ok(_) => {}
                Err(e) => {
                    // After the successors already emitted (probe
                    // `EvalErrorMidLevel`); the counterexample ends at the
                    // state being expanded.
                    let j = u32::try_from(generated).unwrap_or(u32::MAX);
                    let key = ((state, ua, j), PH_ENUM);
                    flush(cap, lines, key);
                    event = Some(Event {
                        key,
                        unit,
                        c: generated,
                        verdict: error_verdict(&e.error, ErrorSite::Next, &self.control),
                        then: None,
                        resume: false,
                        stutter: false,
                        partial: e.error.class() == dive_eval::ErrorClass::Interrupted,
                    });
                    break;
                }
            }
        }
        if intermediate > 0 {
            // The pool's per-unit counts are successor ordinals as well as
            // generated counts; TLC's uncounted-successor intermediates would
            // split them. Refused by name, never miscounted.
            let j = u32::try_from(generated).unwrap_or(u32::MAX);
            let key = ((state, ua, j), PH_ENUM);
            flush(cap, lines, key);
            event = Some(Event {
                key,
                unit,
                c: generated,
                verdict: Verdict::Unsupported {
                    feature: "TLC's counting of action-composition intermediate states (`--action-composition=tlc`) with more than one worker".to_owned(),
                },
                then: None,
                resume: false,
                stutter: false,
                        partial: false,
            });
        }
        if event.is_none() {
            let j = u32::try_from(generated).unwrap_or(u32::MAX);
            flush(cap, lines, ((state, ua, j), PH_ENUM));
            if !self.narrow && self.check_deadlock && generated == 0 {
                event = Some(deadlock(index, unit));
            }
        }
        (generated, event)
    }

    /// Phase 1 for one successor (TLC's `Worker.addElement`, without the
    /// insertion and the invariants of in-model states).
    #[allow(clippy::too_many_arguments, reason = "one successor's context")]
    fn successor(
        &self,
        set: &S,
        s0: &StateVals,
        succ: &Successor<'_>,
        action: u32,
        pos: Pos,
        buf: &mut Candidates<S::Key, Pos, Payload>,
        cap: &PrintCapture,
        lines: &mut Lines,
        accepted: &mut Vec<Accepted<S::Key>>,
        accepted_start: usize,
    ) -> Option<Stopped> {
        let m = self.m;
        if !succ.complete() {
            return Some(Stopped {
                verdict: incomplete(m, succ, false, self.ctx),
                phase: PH_ENUM,
                with: true,
            });
        }
        let t: StateVals = succ.state.into();
        let st1 = St::state(t.clone());
        let step = St::action(s0.clone(), t.clone());
        let levels = Levels {
            set: &set_thread_level,
            here: level_of(self.level + 1),
            from: level_of(self.level),
        };
        // A constraint's error ends the counterexample with the successor; a
        // state TLC cannot fingerprint (or whose `VIEW` fails) is not
        // printed (fixtures `ErConstr*`, `ErView*`).
        let checked = match in_model(m, &st1, Some(&step), &levels, &self.control) {
            Err(v) => Err((v, true)),
            Ok(true) => state_key_of::<S>(m, self.keyer, self.ctx, &t, &st1, &self.control)
                .map(Some)
                .map_err(|v| (v, false)),
            Ok(false) => Ok(None),
        };
        flush(cap, lines, (pos, PH_CHECK));
        match checked {
            Err((verdict, with)) => {
                return Some(Stopped {
                    verdict,
                    phase: PH_CHECK,
                    with,
                });
            }
            Ok(Some(key)) => {
                if self.capture_limit.is_some() {
                    if self
                        .capture_quotas
                        .is_some_and(|(quota, _)| accepted.len() - accepted_start >= quota)
                    {
                        return Some(Stopped {
                            verdict: crate::capture::limit("concrete per-source retained quota"),
                            phase: PH_CHECK,
                            with: false,
                        });
                    }
                    accepted.push(Accepted {
                        vals: t.clone(),
                        pos,
                        key: S::duplicate_key(&key),
                        action,
                    });
                }
                if !set.contains(&key) {
                    buf.push(set, key, pos, Payload { vals: t, action });
                }
            }
            Ok(None) => {
                // Outside the model: checked on every occurrence, never
                // inserted (probe `ConstrainedViolation`); TLC's replay checks
                // it again.
                let v = check_invariants(m, &st1, false, &self.control);
                flush(cap, lines, (pos, PH_INV));
                if let Some(verdict) = v {
                    return Some(Stopped {
                        verdict: class::recurring(verdict),
                        phase: PH_INV,
                        with: true,
                    });
                }
            }
        }
        if !m.implied_actions.is_empty() {
            levels.from();
            let v = check_implied_actions(m, &step, &self.control);
            flush(cap, lines, (pos, PH_IMPLIED));
            if let Some(verdict) = v {
                return Some(Stopped {
                    verdict,
                    phase: PH_IMPLIED,
                    with: true,
                });
            }
        }
        None
    }
}

fn deadlock(i: usize, unit: usize) -> Event {
    Event {
        key: ((i as u64, u32::MAX, u32::MAX), PH_DEADLOCK),
        unit,
        c: 0,
        verdict: Verdict::Deadlock,
        then: None,
        resume: false,
        stutter: false,
        partial: false,
    }
}

/// A new state of the level, in position order.
struct Winner<K> {
    key: K,
    pos: Pos,
    vals: StateVals,
    action: u32,
    /// The actual resolved store key, retained without re-keying a concrete state.
    checkpoint_key: Option<Vec<u8>>,
}

/// Replay membership uses resolved identities without evaluating VIEW again.
fn replay_unseen_keys<S: Keyed>(winners: &[Winner<S::Key>]) -> BTreeSet<S::Key> {
    winners
        .iter()
        .map(|winner| S::duplicate_key(&winner.key))
        .collect()
}

fn ordinal_of<K>(winner: &Winner<K>, gens: &[u64], narrow: bool, actions: usize) -> Option<u32> {
    let (i, action, ordinal) = winner.pos;
    let prior = if narrow {
        let i = usize::try_from(i).ok()?;
        gens[i * actions..i * actions + action as usize]
            .iter()
            .sum::<u64>()
    } else {
        0
    };
    u32::try_from(prior.checked_add(u64::from(ordinal))?).ok()
}

fn unit_position(unit: usize, narrow: bool, actions: usize, ordinal: u64) -> Pos {
    if narrow {
        (
            unit as u64 / actions as u64,
            u32::try_from(unit % actions).unwrap_or(u32::MAX),
            u32::try_from(ordinal).unwrap_or(u32::MAX),
        )
    } else {
        (unit as u64, 0, u32::try_from(ordinal).unwrap_or(u32::MAX))
    }
}

/// Native integer limits are prefix decisions, never worker-local cutoffs.
fn native_counter_event(
    generated: u64,
    gens: &[u64],
    units: Range<usize>,
    narrow: bool,
    actions: usize,
) -> Option<Event> {
    let mut remaining = crate::checkpoint::MAX_COUNTER.saturating_sub(generated);
    for unit in units {
        let count = gens[unit];
        if count > remaining {
            return Some(Event {
                key: (unit_position(unit, narrow, actions, remaining), PH_ENUM),
                unit,
                c: remaining,
                verdict: crate::checkpoint::failure(
                    "generated counter exceeds native positive TLC-long bound",
                ),
                then: None,
                resume: false,
                stutter: false,
                partial: false,
            });
        }
        remaining -= count;
    }
    None
}

struct ChunkOut {
    event: Option<Event>,
    lines: Lines,
}

/// One level's phase-3 job: the invariants of the new states.
struct Check<'env, K> {
    m: &'env Model,
    control: Arc<crate::budget::EvaluationControl>,
    level: u64,
    winners: Arc<Vec<Winner<K>>>,
    /// Winners at or after this key are not checked (phase 1 stopped
    /// before them).
    limit: Option<Key>,
    narrow: bool,
    chunks: Vec<Range<usize>>,
    order: Vec<usize>,
    claim: AtomicUsize,
    /// The smallest winner index with a violation so far.
    cut: AtomicUsize,
    outs: Vec<Mutex<Option<ChunkOut>>>,
    chaos: Option<u64>,
}

impl<K> Check<'_, K> {
    fn unit_of(&self, pos: Pos) -> usize {
        let i = usize::try_from(pos.0).unwrap_or(usize::MAX);
        if self.narrow {
            i * self.m.actions.len() + pos.1 as usize
        } else {
            i
        }
    }

    fn work(&self, w: usize) {
        let _level = ThreadLevel::enter(level_of(self.level + 1));
        let mut rng = self
            .chaos
            .map(|c| Rng(c ^ (w as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ !self.level));
        loop {
            Rng::maybe_yield(&mut rng);
            let k = self.claim.fetch_add(1, Ordering::Relaxed);
            let Some(&c) = self.order.get(k) else { break };
            let range = self.chunks[c].clone();
            if range.start > self.cut.load(Ordering::Relaxed) {
                continue;
            }
            let out = self.chunk(range, &mut rng);
            *self.outs[c].lock().unwrap_or_else(PoisonError::into_inner) = Some(out);
        }
    }

    fn chunk(&self, range: Range<usize>, rng: &mut Option<Rng>) -> ChunkOut {
        let m = self.m;
        let cap = PrintCapture::start();
        let mut out = ChunkOut {
            event: None,
            lines: Vec::new(),
        };
        set_thread_level(level_of(self.level + 1));
        for k in range {
            let w = &self.winners[k];
            let key = (w.pos, PH_INV);
            if k > self.cut.load(Ordering::Relaxed) || self.limit.is_some_and(|l| key >= l) {
                break;
            }
            Rng::maybe_yield(rng);
            let v = check_invariants(m, &St::state(w.vals.clone()), false, &self.control);
            flush(&cap, &mut out.lines, key);
            if let Some(verdict) = v {
                self.cut.fetch_min(k, Ordering::Relaxed);
                let a = &m.actions[w.action as usize];
                out.event = Some(Event {
                    key,
                    unit: self.unit_of(w.pos),
                    c: u64::from(w.pos.2) + 1,
                    resume: resumes(&verdict),
                    stutter: false,
                    partial: false,
                    verdict,
                    then: values(&w.vals).map(|values| TraceState {
                        label: a.label().to_owned(),
                        action: action_name(&m.evaluator, a),
                        values,
                        text: None,
                        partial: None,
                    }),
                });
                break;
            }
        }
        out
    }
}

/// Runs `job` on the pool, or inline on the calling thread.
fn run<'env>(pool: &Pool<'env>, job: &Job<'env>, inline: bool) -> Result<(), String> {
    if inline || pool.threads() == 1 {
        Pool::run_inline(job)
    } else {
        pool.run(job)
    }
}

/// A stop the engine itself causes (a bug or a store failure).
fn internal(message: String, stats: Stats) -> (Option<Stop>, Stats) {
    (
        Some(Stop {
            verdict: Verdict::InternalError { message },
            in_init: false,
            path: None,
            then: None,
        }),
        stats,
    )
}

pub(crate) fn levels<S: Keyed>(
    search: &mut Search<'_, '_, S>,
    next: crate::frontier::Queue,
    opts: &CheckOptions,
    workers: usize,
) -> (Option<Stop>, Stats, usize) {
    levels_from(search, next, opts, workers, 1)
}

/// Resume from the archive's actual completed-level boundary.
pub(crate) fn levels_from<S: Keyed>(
    search: &mut Search<'_, '_, S>,
    next: crate::frontier::Queue,
    opts: &CheckOptions,
    workers: usize,
    level: u64,
) -> (Option<Stop>, Stats, usize) {
    let set = RwLock::new(std::mem::replace(&mut search.set, S::fresh(0)));
    let (stop, stats, used) = with_pool(workers, dive_eval::RECOMMENDED_STACK, |pool| {
        let (stop, stats) = run_levels(search, &set, next, opts, pool, level);
        (stop, stats, pool.threads())
    });
    search.set = set.into_inner().unwrap_or_else(PoisonError::into_inner);
    // Statistics computed with the set outside the search report its size.
    let stats = if stop.is_none() && search.prefix_level.is_none() {
        search.stats(0)
    } else {
        stats
    };
    (stop, stats, used)
}

/// Commits accepted occurrences in source/successor order, including keys
/// already seen before this wave and duplicate candidates within it. No model
/// evaluation or reconstruction occurs here. New IDs are the future trace-log
/// IDs in winner order; only a successful wave publishes its frontier/log.
#[allow(
    clippy::too_many_arguments,
    reason = "ordered wave evidence and buffers"
)]
fn capture_wave<S: Keyed>(
    search: &mut Search<'_, '_, S>,
    first_id: u64,
    winners: &[Winner<S::Key>],
    mut accepted: Vec<Accepted<S::Key>>,
    gens: &[u64],
    done: &[bool],
    units: Range<usize>,
    first: Option<Event>,
) -> Option<Event> {
    let base = search.log.len();
    let capture = search.capture.as_mut()?;
    accepted.sort_by_key(|a| a.pos);
    let mut records = accepted.into_iter().peekable();
    for unit in units {
        while records.peek().is_some_and(|a| a.pos.0 == unit as u64) {
            let Some(record) = records.next() else { break };
            let key = (record.pos, PH_CAPTURE);
            if first.as_ref().is_some_and(|e| e.key < key) {
                return first;
            }
            let at = winners.partition_point(|w| w.pos < record.pos);
            let fresh = winners.get(at).filter(|w| w.pos == record.pos);
            let id = fresh.map(|_| base + at as u64);
            let result = capture.record_fields(
                record.key,
                record.vals,
                id,
                Some(first_id + unit as u64),
                u64::from(record.pos.2),
                search.m.actions[record.action as usize].label(),
            );
            if let Err(verdict) = result {
                return earlier(
                    first,
                    Some(Event {
                        key,
                        unit,
                        c: u64::from(record.pos.2) + 1,
                        verdict,
                        then: None,
                        resume: false,
                        stutter: false,
                        partial: false,
                    }),
                );
            }
        }
        let key = ((unit as u64, u32::MAX, u32::MAX), PH_DEADLOCK);
        if done[unit] && first.as_ref().is_none_or(|e| e.key > key) {
            if let Err(verdict) = capture.stutter(first_id + unit as u64) {
                return earlier(
                    first,
                    Some(Event {
                        key,
                        unit,
                        c: gens[unit],
                        verdict,
                        then: None,
                        resume: false,
                        stutter: true,
                        partial: false,
                    }),
                );
            }
        } else {
            break;
        }
    }
    first
}

#[allow(clippy::too_many_lines, reason = "the three phases of a level")]
fn run_levels<'a: 'env, 'env, S: Keyed>(
    search: &mut Search<'a, '_, S>,
    set: &'env RwLock<S>,
    next: crate::frontier::Queue,
    opts: &CheckOptions,
    pool: &Pool<'env>,
    mut level: u64,
) -> (Option<Stop>, Stats) {
    let m: &'env Model = search.m;
    let keyer: &'env S::Keyer = search.keyer;
    let threads = pool.threads();
    let check_deadlock = m.settings.check_deadlock;
    let n_actions = m.actions.len();
    let mut first_id: StateId = if next.is_empty() {
        0
    } else {
        match next.read(0, &m.names) {
            Ok((_, id)) => id,

            Err(verdict) => {
                return (
                    Some(Stop {
                        verdict,
                        in_init: false,
                        path: None,
                        then: None,
                    }),
                    search.stats(next.len() as u64),
                );
            }
        }
    };
    let mut frontier = Arc::new(next);
    let stats_now = |search: &Search<'_, '_, S>, left: u64| Stats {
        generated: search.generated,
        distinct: set.read().unwrap_or_else(PoisonError::into_inner).len(),
        left_on_queue: left,
        depth: search.depth,
        initial_generated: search.initial_generated,
    };
    while !frontier.is_empty() {
        if let Some(stop) = search.budget_stop() {
            return (Some(stop), stats_now(search, frontier.len() as u64));
        }
        if let Some(verdict) = search.checkpoint_boundary(&frontier, level) {
            return (
                Some(Stop {
                    verdict,
                    in_init: false,
                    path: None,
                    then: None,
                }),
                stats_now(search, frontier.len() as u64),
            );
        }
        let mut rng = opts
            .chaos
            .map(|c| Rng(c ^ level.wrapping_mul(0xa076_1d64_78bd_642f)));
        let len = frontier.len();
        let narrow = search.capture.is_none()
            && n_actions > 1
            && match &mut rng {
                Some(r) => r.coin(),
                None => len < 2 * threads,
            };
        let n_units = if narrow { len * n_actions } else { len };
        let mut next_frontier = frontier.empty_like();
        let new_first = search.log.len();
        let concrete_pool = search
            .capture
            .as_ref()
            .is_some_and(crate::capture::Collector::concrete_pool);
        let capture_quotas = if concrete_pool {
            match search
                .capture
                .as_ref()
                .map(|c| c.quotas(len, search.generated))
            {
                Some(Ok(q)) => Some(q),
                Some(Err(verdict)) => {
                    return (
                        Some(Stop {
                            verdict,
                            in_init: false,
                            path: None,
                            then: None,
                        }),
                        stats_now(search, 0),
                    );
                }
                None => None,
            }
        } else {
            None
        };
        let wave_size = if search.capture.is_some() {
            crate::capture::MAX_EXPANSION_UNITS
        } else {
            n_units.max(1)
        };
        for wave_start in (0..n_units).step_by(wave_size) {
            let wave_end = (wave_start + wave_size).min(n_units);
            let size = if search.capture.is_some() {
                1
            } else {
                match &mut rng {
                    Some(r) => 1 + r.below(2 * block_size(n_units, threads)),
                    None => block_size(n_units, threads),
                }
            };
            let blocks = ranges(wave_end - wave_start, size)
                .into_iter()
                .map(|r| r.start + wave_start..r.end + wave_start)
                .collect::<Vec<_>>();
            let mut order: Vec<usize> = (0..blocks.len()).collect();
            if let Some(r) = &mut rng {
                r.shuffle(&mut order);
            }
            let inline = !concrete_pool
                && match &mut rng {
                    Some(r) => r.coin(),
                    None => blocks.len() == 1,
                };
            // Phase 1: expand.
            let expand = Arc::new(Expand::<S> {
                m,
                keyer,
                set,
                ctx: search.ctx,
                level,
                frontier: frontier.clone(),
                narrow,
                check_deadlock,
                capture_quotas,
                capture_limit: search.capture.as_ref().map(|c| {
                    c.limits()
                        .successors_per_unit
                        .min(c.limits().generated.saturating_sub(search.generated))
                        .min(capture_quotas.map_or(u64::MAX, |(_, generated)| generated))
                }),
                native_counter_remaining: search
                    .has_checkpoint()
                    .then(|| crate::checkpoint::MAX_COUNTER.saturating_sub(search.generated)),
                outs: blocks.iter().map(|_| Mutex::new(None)).collect(),
                blocks,
                order,
                claim: AtomicUsize::new(0),
                cut: AtomicU64::new(u64::MAX),
                buffers: Mutex::new(Vec::new()),
                chaos: opts.chaos,
                budget: search.budget.clone(),
                control: search.control.clone(),
                exhausted: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                pause: opts.parallel_budget_test.clone(),
            });
            let job: Job<'env> = {
                let e = expand.clone();
                Arc::new(move |w| e.work(w))
            };
            if let Err(message) = run(pool, &job, inline) {
                return internal(
                    message,
                    stats_now(search, (len - wave_start + next_frontier.len()) as u64),
                );
            }
            drop(job);
            if expand.exhausted.load(Ordering::Relaxed)
                && !expand.covers_first_event()
                && let Some(stop) = search.budget_stop()
            {
                // The wave is incomplete before any event: report the
                // published prefix only. An event every earlier block was
                // expanded for is the first in reference order, and is
                // published below as it would be without a budget.
                return (
                    Some(stop),
                    stats_now(search, (len - wave_start + next_frontier.len()) as u64),
                );
            }
            let CommitInput {
                gens,
                done,
                lines,
                mut first,
                accepted,
                buffers,
            } = expand.take_sources(wave_start..wave_end, n_units);
            if narrow && check_deadlock {
                // A state's deadlock needs all its actions' counts.
                for i in 0..len {
                    let us = i * n_actions..(i + 1) * n_actions;
                    if done[us.clone()].iter().all(|&d| d)
                        && gens[us.clone()].iter().all(|&g| g == 0)
                    {
                        first = earlier(first, Some(deadlock(i, us.end - 1)));
                        break;
                    }
                }
            }
            drop(expand);
            let result = commit_sources(
                search,
                set,
                pool,
                threads,
                opts,
                level,
                first_id,
                &frontier,
                &mut next_frontier,
                narrow,
                concrete_pool,
                wave_start..wave_end,
                &gens,
                &done,
                lines,
                first,
                accepted,
                buffers,
                &mut rng,
            );
            if result.0.is_some() {
                return result;
            }
        }
        let stats = stats_now(search, next_frontier.len() as u64);
        if let Err(verdict) = search.observe_level(level, first_id, len, stats) {
            return (
                Some(Stop {
                    verdict,
                    in_init: false,
                    path: None,
                    then: None,
                }),
                stats,
            );
        }
        if search.prefix_level.is_some() {
            return (None, stats);
        }
        frontier = Arc::new(next_frontier);
        first_id = new_first;
        level += 1;
    }
    (None, stats_now(search, 0))
}

/// Publishes one joined whole wave in reference order.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "ordered whole-wave publication preserves deciding events and counters"
)]
fn commit_sources<'a: 'env, 'env, S: Keyed>(
    search: &mut Search<'a, '_, S>,
    set: &'env RwLock<S>,
    pool: &Pool<'env>,
    threads: usize,
    opts: &CheckOptions,
    level: u64,
    first_id: u64,
    frontier: &crate::frontier::Queue,
    next_frontier: &mut crate::frontier::Queue,
    narrow: bool,
    concrete_pool: bool,
    units: Range<usize>,
    gens: &[u64],
    done: &[bool],
    mut lines: Lines,
    mut first: Option<Event>,
    accepted: Vec<Accepted<S::Key>>,
    buffers: Vec<Candidates<S::Key, Pos, Payload>>,
    rng: &mut Option<Rng>,
) -> (Option<Stop>, Stats) {
    let m: &'env Model = search.m;
    let n_actions = m.actions.len();
    let len = frontier.len();
    let stats_now = |search: &Search<'_, '_, S>, left: u64| Stats {
        generated: search.generated,
        distinct: set.read().unwrap_or_else(PoisonError::into_inner).len(),
        left_on_queue: left,
        depth: search.depth,
        initial_generated: search.initial_generated,
    };
    let candidates: usize = buffers.iter().map(Candidates::len).sum();
    let distinct_before = set.read().unwrap_or_else(PoisonError::into_inner).len();
    let resolve = ResolveOptions {
        threads: match &mut *rng {
            Some(r) => 1 + r.below(threads),
            None if candidates < PARALLEL_RESOLVE => 1,
            None => threads,
        },
        chaos: rng.as_mut().map(Rng::next),
    };
    let resolved = if search.admission.is_some() {
        match resolve_admitted(
            search,
            set,
            buffers,
            level,
            narrow,
            n_actions,
            first.as_ref(),
        ) {
            Ok((winners, stop)) => {
                first = earlier(first, stop);
                winners
            }
            Err(error) => return internal(error, stats_now(search, 0)),
        }
    } else {
        let resolved = set
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .resolve(buffers, &resolve);
        match resolved {
            Ok(r) => r.into_position_order(),
            Err(e) => return internal(e.to_string(), stats_now(search, 0)),
        }
    };
    let winners: Arc<Vec<Winner<S::Key>>> = Arc::new(
        resolved
            .into_iter()
            .map(|c| Winner {
                pos: c.pos,
                vals: c.payload.vals,
                action: c.payload.action,
                checkpoint_key: search.has_checkpoint().then(|| S::checkpoint_key(&c.key)),
                key: c.key,
            })
            .collect(),
    );
    if search.has_checkpoint() {
        first = earlier(
            native_counter_event(
                search.generated,
                gens,
                units.start..units.end,
                narrow,
                n_actions,
            ),
            first,
        );
        // Record winners in discovery order before the invariant phase,
        // matching inline admission's key/record/invariant ordering.
        // The resolved key is retained; the model is never re-keyed here.
        for winner in winners.iter() {
            if first
                .as_ref()
                .is_some_and(|event| (winner.pos, PH_NATIVE) > event.key)
            {
                break;
            }
            let Some(ordinal) = ordinal_of(winner, gens, narrow, n_actions) else {
                return internal(
                    "more than 2^32 successors of one state".into(),
                    stats_now(search, 0),
                );
            };
            let Some(key) = &winner.checkpoint_key else {
                return internal(
                    "native winner key was not retained".into(),
                    stats_now(search, 0),
                );
            };
            if let Err(verdict) = search.checkpoint_discovery(
                key.clone(),
                &winner.vals,
                first_id + winner.pos.0,
                ordinal,
            ) {
                let unit = if narrow {
                    usize::try_from(winner.pos.0).unwrap_or(usize::MAX) * n_actions
                        + winner.pos.1 as usize
                } else {
                    usize::try_from(winner.pos.0).unwrap_or(usize::MAX)
                };
                first = earlier(
                    Some(Event {
                        key: (winner.pos, PH_NATIVE),
                        unit,
                        c: u64::from(winner.pos.2) + 1,
                        verdict,
                        then: None,
                        resume: false,
                        stutter: false,
                        partial: false,
                    }),
                    first,
                );
                break;
            }
        }
    }
    // Resolve logical generated limits in reference order, independently of
    // which workers consumed the bounded per-source scratch budget.
    if let Some(capture) = &search.capture {
        let mut generated = search.generated;
        for (unit, &count) in gens.iter().enumerate().take(units.end).skip(units.start) {
            if generated.saturating_add(count) > capture.limits().generated {
                let j = capture.limits().generated.saturating_sub(generated);
                first = earlier(
                    Some(Event {
                        key: (
                            (unit as u64, 0, u32::try_from(j).unwrap_or(u32::MAX)),
                            PH_ENUM,
                        ),
                        unit,
                        c: j + 1,
                        verdict: crate::capture::limit("generated states"),
                        then: None,
                        resume: false,
                        stutter: false,
                        partial: false,
                    }),
                    first,
                );
                break;
            }
            generated += count;
        }
    }
    if search.capture.is_some() {
        first = capture_wave(
            search,
            first_id,
            &winners,
            accepted,
            gens,
            done,
            units.start..units.end,
            first,
        );
    }
    // Phase 3: the invariants of the new states.
    let limit = first.as_ref().map(|e| e.key);
    if !m.invariants.is_empty() && !winners.is_empty() {
        let n = winners.len();
        let size = match &mut *rng {
            Some(r) => 1 + r.below(2 * block_size(n, threads)),
            None => block_size(n, threads),
        };
        let chunks = ranges(n, size);
        let mut order: Vec<usize> = (0..chunks.len()).collect();
        if let Some(r) = &mut *rng {
            r.shuffle(&mut order);
        }
        let inline = !concrete_pool
            && match &mut *rng {
                Some(r) => r.coin(),
                None => chunks.len() == 1,
            };
        let check = Arc::new(Check {
            m,
            control: search.control.clone(),
            level,
            winners: winners.clone(),
            limit,
            narrow,
            outs: chunks.iter().map(|_| Mutex::new(None)).collect(),
            chunks,
            order,
            claim: AtomicUsize::new(0),
            cut: AtomicUsize::new(usize::MAX),
            chaos: opts.chaos,
        });
        let job: Job<'env> = {
            let c = check.clone();
            Arc::new(move |w| c.work(w))
        };
        if let Err(message) = run(pool, &job, inline) {
            return internal(message, stats_now(search, 0));
        }
        drop(job);
        for out in &check.outs {
            if let Some(out) = out.lock().unwrap_or_else(PoisonError::into_inner).take() {
                lines.extend(out.lines);
                first = earlier(first, out.event);
            }
        }
    }
    // `Print` output in TLC's order, up to the stop.
    if let Some(e) = &first {
        lines.retain(|(k, _)| *k <= e.key);
    }
    lines.sort_by_key(|(k, _)| *k);
    m.evaluator
        .release(lines.into_iter().map(|(_, l)| l).collect());
    if let Some(e) = first {
        // ADR 0015 §3's closed forms at E*.
        let (pos, _) = e.key;
        let i = usize::try_from(pos.0).unwrap_or(usize::MAX);
        let upto =
            winners.partition_point(|w| w.pos < pos || (w.pos == pos && e.key.1 >= PH_NATIVE));
        let before = winners.partition_point(|w| w.pos < pos);
        let generated = search.generated + gens[..e.unit].iter().sum::<u64>() + e.c;
        let depth_prefix = if e.key.1 == PH_NATIVE { before } else { upto };
        let depth = if depth_prefix > 0 {
            search.depth.max(level + 1)
        } else {
            search.depth
        };
        search.generated = generated;
        search.depth = depth;
        let mut stats = Stats {
            generated,
            distinct: distinct_before + upto as u64,
            left_on_queue: if e.stutter {
                0
            } else {
                ((len - i - 1) + next_frontier.len() + before + usize::from(e.partial)) as u64
            },
            depth,
            initial_generated: search.initial_generated,
        };
        let mut verdict = e.verdict;
        let vals = match frontier.read(i, &m.names) {
            Ok((vals, _)) => vals,
            Err(verdict) => {
                return (
                    Some(Stop {
                        verdict,
                        in_init: false,
                        path: None,
                        then: None,
                    }),
                    stats,
                );
            }
        };
        if e.resume {
            // TLC's replay goes on past the state: as of E*, the states
            // this level found after it are not in the set yet.
            let unseen = replay_unseen_keys::<S>(&winners[upto..]);
            let first_unit = if narrow { i * n_actions } else { i };
            let ordinal = gens[first_unit..e.unit].iter().sum::<u64>() + e.c - 1;
            let set = set.read().unwrap_or_else(PoisonError::into_inner);
            let r = search.replay_after(&set, &vals, ordinal, level, &unseen);
            stats.distinct += r.distinct;
            stats.left_on_queue += r.queued;
            verdict = r.apply(verdict);
        }
        let no_path = e.stutter
            || matches!(
                &verdict,
                Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. }
            );
        let stop = Stop {
            verdict,
            in_init: false,
            path: if no_path {
                None
            } else {
                Some((first_id + pos.0, vals.clone()))
            },
            then: e.then,
        };
        return (Some(stop), stats);
    }
    search.generated += gens.iter().sum::<u64>();
    // The trace log and the next frontier, in position order.
    for w in winners.iter() {
        let i = w.pos.0;
        let ordinal = ordinal_of(w, gens, narrow, n_actions);
        let Some(ordinal) = ordinal else {
            return internal(
                "more than 2^32 successors of one state".to_owned(),
                stats_now(search, 0),
            );
        };
        if let Err(e) = search.log.push(first_id + i, ordinal) {
            return (
                Some(Stop {
                    verdict: store_verdict(&e, &search.control),
                    in_init: false,
                    path: None,
                    then: None,
                }),
                stats_now(search, 0),
            );
        }
    }
    if !winners.is_empty() {
        search.depth = search.depth.max(level + 1);
    }
    let winners = Arc::try_unwrap(winners).unwrap_or_else(|w| {
        w.iter()
            .map(|x| Winner {
                key: S::duplicate_key(&x.key),
                pos: x.pos,
                vals: x.vals.clone(),
                action: x.action,
                checkpoint_key: x.checkpoint_key.clone(),
            })
            .collect()
    });
    let wave_first = search.log.len() - winners.len() as u64;
    for (j, w) in winners.into_iter().enumerate() {
        if let Err(verdict) = next_frontier.push(w.vals, wave_first + j as u64) {
            return (
                Some(Stop {
                    verdict,
                    in_init: false,
                    path: None,
                    then: None,
                }),
                stats_now(search, (frontier.len() + next_frontier.len()) as u64),
            );
        }
    }
    (None, stats_now(search, next_frontier.len() as u64))
}

type Admitted<K> = (Vec<Candidate<K, Pos, Payload>>, Option<Event>);

/// Explicit search budgets use ordered admission while generation still uses
/// the requested worker pool. No key beyond the stopping prefix is inserted.
#[allow(
    clippy::too_many_arguments,
    reason = "reference positions retain source and action units"
)]
#[cold]
#[inline(never)]
fn resolve_admitted<S: Keyed>(
    search: &Search<'_, '_, S>,
    set: &RwLock<S>,
    buffers: Vec<Candidates<S::Key, Pos, Payload>>,
    level: u64,
    narrow: bool,
    actions: usize,
    first: Option<&Event>,
) -> Result<Admitted<S::Key>, String> {
    let mut candidates: Vec<_> = buffers
        .into_iter()
        .flat_map(Candidates::into_position_order)
        .collect();
    candidates.sort_unstable_by_key(|candidate| candidate.pos);
    if candidates
        .windows(2)
        .any(|pair| pair[0].pos == pair[1].pos && pair[0].key == pair[1].key)
    {
        return Err("duplicate candidate key and reference position".into());
    }
    let mut set = set.write().unwrap_or_else(PoisonError::into_inner);
    let mut winners = Vec::new();
    for candidate in candidates {
        if first.is_some_and(|event| event.key < (candidate.pos, PH_NATIVE)) {
            break;
        }
        if set.contains(&candidate.key) {
            continue;
        }
        if let Some(verdict) = search.admission_stop(level + 1, set.len()) {
            let state = usize::try_from(candidate.pos.0)
                .map_err(|_| "candidate source outside native range")?;
            let unit = if narrow {
                state
                    .checked_mul(actions)
                    .and_then(|u| u.checked_add(candidate.pos.1 as usize))
                    .ok_or("candidate action outside native range")?
            } else {
                state
            };
            let event = Event {
                key: (candidate.pos, PH_NATIVE),
                unit,
                c: u64::from(candidate.pos.2) + 1,
                verdict,
                then: None,
                resume: false,
                stutter: false,
                partial: false,
            };
            return Ok((winners, Some(event)));
        }
        if !set.insert(S::duplicate_key(&candidate.key)) {
            return Err("candidate changed membership during exclusive admission".into());
        }
        winners.push(candidate);
    }
    Ok((winners, None))
}

#[cfg(test)]
pub(crate) mod budget_cut_tests {
    use super::covers;
    use std::sync::{Arc, Condvar, Mutex, PoisonError};
    use std::time::Duration;

    #[derive(Debug, Default)]
    struct State {
        entered: bool,
        tripped: bool,
        skipped: bool,
        later_event: Option<usize>,
    }

    /// Holds the first expansion in flight until another worker observes a stop.
    #[derive(Debug, Default)]
    pub(crate) struct Pause {
        state: Mutex<State>,
        wake: Condvar,
        /// Emitted-prefix length and whether another worker must decide later.
        emission: Option<(u64, bool)>,
    }

    impl Pause {
        fn wait_for(&self, predicate: impl Fn(&State) -> bool) {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let (state, _) = self
                .wake
                .wait_timeout_while(state, Duration::from_secs(10), |s| !predicate(s))
                .unwrap_or_else(PoisonError::into_inner);
            assert!(
                predicate(&state),
                "parallel budget synchronization timed out"
            );
        }

        pub(super) fn before_poll(&self, unit: usize) {
            if self.emission.is_some() {
                return;
            }
            if unit != 0 {
                self.wait_for(|s| s.tripped);
            }
        }

        pub(super) fn after_poll(&self, unit: usize) {
            if self.emission.is_some() {
                return;
            }
            if unit == 0 {
                self.state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .entered = true;
                self.wake.notify_all();
                self.wait_for(|s| s.skipped);
            }
        }

        pub(super) fn stopped(&self) {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .skipped = true;
            self.wake.notify_all();
        }

        pub(super) fn emitted(&self, unit: usize, generated: u64, budget: &crate::budget::Budget) {
            let Some((at, later)) = self.emission else {
                return;
            };
            if unit != 0 || generated != at {
                return;
            }
            if later {
                self.wait_for(|s| s.later_event.is_some());
            }
            budget.trip_for_test();
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .tripped = true;
            self.wake.notify_all();
        }

        pub(super) fn event(&self, unit: usize) {
            if self.emission.is_some() && unit > 0 {
                self.state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .later_event = Some(unit);
                self.wake.notify_all();
            }
        }

        fn trip(&self, budget: &crate::budget::Budget) {
            self.wait_for(|s| s.entered);
            budget.trip_for_test();
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .tripped = true;
            self.wake.notify_all();
        }
    }

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "fixed model and synchronized worker controls"
    )]
    fn an_in_flight_violation_is_published_when_another_worker_stops() {
        let source = "---- MODULE BudgetRace ----\nEXTENDS Naturals\nVARIABLE x\n\
            Init == x \\in 0..127\nNext == x' = 128\n\
            Safe == [][x' < 128]_x\n====\n";
        let analysis =
            dive_sema::analyze_source("BudgetRace.tla", source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("valid IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Safe\n")
            .into_config()
            .expect("valid config");
        for mut opts in [crate::CheckOptions::tlc(0, 0), crate::CheckOptions::dive(0)] {
            opts.workers = 4;
            let reference = crate::check(&ir, &cfg, &opts);
            assert!(
                matches!(
                    reference.verdict,
                    crate::Verdict::ActionPropertyViolated { .. }
                ),
                "{reference:?}"
            );
            assert_eq!(reference.workers.used, 4);
            let watchdog = crate::budget::Budget::new(Some(Duration::from_secs(3600)), None)
                .start()
                .expect("watchdog thread starts")
                .expect("watchdog");
            let pause = Arc::new(Pause::default());
            opts.budget = watchdog.budget().clone();
            opts.parallel_budget_test = Some(pause.clone());
            let result = std::thread::scope(|scope| {
                let check = scope.spawn(|| crate::check(&ir, &cfg, &opts));
                pause.trip(&opts.budget);
                check.join().expect("check returns")
            });
            assert!(opts.budget.poll().expect("measurement").is_some());
            assert!(
                pause
                    .state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .skipped
            );
            assert_eq!(result.verdict, reference.verdict);
            assert_eq!(result.stats, reference.stats);
            assert_eq!(
                format!("{:?}", result.counterexample),
                format!("{:?}", reference.counterexample)
            );
        }
    }

    #[test]
    fn an_event_survives_a_budget_stop_only_when_every_earlier_block_was_expanded() {
        let blocks = [0..4, 4..8, 8..12, 12..16];
        // Blocks 0 and 1 expanded, event at unit 5: decided.
        assert!(covers(&blocks, 5, |b| b < 2));
        // Chaos claimed block 1 first and block 0 was left unexpanded: an
        // earlier event may hide in block 0, so the wave is not decided.
        assert!(!covers(&blocks, 5, |b| b == 1));
        // An event in block 2 with block 1 unexpanded is not decided either.
        assert!(!covers(&blocks, 9, |b| b != 1));
        // Later blocks never matter.
        assert!(covers(&blocks, 3, |b| b == 0));
        // No event: nothing to publish.
        assert!(!covers(&blocks, u64::MAX, |_| true));
    }

    #[allow(
        clippy::expect_used,
        clippy::panic,
        reason = "fixed models and deterministic emission-budget controls"
    )]
    mod emission_tests {
        use super::*;
        use crate::{CheckOptions, CheckResult, Stats, Verdict};

        fn input(source: &str, config: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
            let analysis =
                dive_sema::analyze_source("Emission.tla", source, &dive_sema::Options::default());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("valid IR"));
            let cfg = dive_config::parse(config)
                .into_config()
                .expect("valid config");
            (ir, cfg)
        }

        fn trip_after_chunk(
            options: &mut CheckOptions,
            wait_for_later: bool,
        ) -> (crate::budget::Watchdog, Arc<Pause>) {
            let watchdog = crate::budget::Budget::new(Some(Duration::from_secs(3600)), None)
                .start()
                .expect("watchdog starts")
                .expect("watchdog");
            let pause = Arc::new(Pause {
                emission: Some((super::super::EMISSION_BUDGET_INTERVAL, wait_for_later)),
                ..Pause::default()
            });
            options.workers = 4;
            options.budget = watchdog.budget().clone();
            options.parallel_budget_test = Some(pause.clone());
            (watchdog, pause)
        }

        fn stopped(result: &CheckResult, generated: u64, phase: crate::budget::Phase) {
            let Verdict::Stopped(stop) = &result.verdict else {
                panic!("an incomplete source cannot decide a later property: {result:?}");
            };
            assert_eq!(stop.resource, crate::budget::Resource::Time);
            assert_eq!(stop.phase, phase);
            assert_eq!(result.workers.used, 4);
            assert!(result.counterexample.is_none());
            assert_eq!(
                result.stats,
                Some(Stats {
                    generated,
                    distinct: 1024,
                    left_on_queue: 1024,
                    depth: 2,
                    initial_generated: generated - 1024,
                })
            );
        }

        #[test]
        fn an_earlier_invariant_violation_or_error_survives_the_emission_stop() {
            for (invariant, evaluation_error) in [
                ("x < 512", false),
                ("IF x < 512 THEN TRUE ELSE (x + TRUE = x)", true),
            ] {
                let source = format!(
                    "---- MODULE Emission ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' \\in 0..4095\nSafe == {invariant}\n====\n"
                );
                let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\nINVARIANT Safe\n");
                for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
                    options.workers = 4;
                    let reference = crate::check(&ir, &cfg, &options);
                    if evaluation_error {
                        assert!(matches!(
                            reference.verdict,
                            Verdict::EvaluationError {
                                site: crate::ErrorSite::Invariant,
                                ..
                            }
                        ));
                    } else {
                        assert!(matches!(
                            &reference.verdict,
                            Verdict::InvariantViolated { property, initial: false }
                                if property == "Safe"
                        ));
                    }
                    let (_watchdog, pause) = trip_after_chunk(&mut options, false);
                    let result = crate::check(&ir, &cfg, &options);
                    assert!(pause.state.lock().expect("state").tripped);
                    assert_eq!(result.verdict, reference.verdict);
                    assert_eq!(result.stats, reference.stats);
                    assert_eq!(
                        format!("{:?}", result.counterexample),
                        format!("{:?}", reference.counterexample)
                    );
                }
            }
        }

        #[test]
        fn an_unprocessed_later_invariant_cannot_replace_the_emission_stop() {
            let source = "---- MODULE Emission ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' \\in 0..4095\nSafe == x < 2048\n====\n";
            let (ir, cfg) = input(source, "INIT Init\nNEXT Next\nINVARIANT Safe\n");
            for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
                let (_watchdog, pause) = trip_after_chunk(&mut options, false);
                let result = crate::check(&ir, &cfg, &options);
                assert!(pause.state.lock().expect("state").tripped);
                stopped(&result, 1025, crate::budget::Phase::Search);
            }
        }

        #[test]
        fn a_later_worker_violation_cannot_certify_an_earlier_partial_source() {
            let source = "---- MODULE Emission ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..127\nNext == IF x = 0 THEN x' \\in 0..4095 ELSE x' = 9999\nSafe == [][x' < 9000]_x\n====\n";
            let (ir, cfg) = input(source, "INIT Init\nNEXT Next\nPROPERTY Safe\n");
            for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
                let (_watchdog, pause) = trip_after_chunk(&mut options, true);
                let result = crate::check(&ir, &cfg, &options);
                let state = pause.state.lock().expect("state");
                assert!(state.tripped && state.later_event.is_some());
                stopped(&result, 1152, crate::budget::Phase::Search);
            }
        }

        #[test]
        fn concrete_pool_partial_sources_never_receive_completion_evidence() {
            let source = "---- MODULE Emission ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' \\in 0..4095\n====\n";
            let (ir, cfg) = input(source, "INIT Init\nNEXT Next\n");
            for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
                let model = dive_model::bind(
                    ir.clone(),
                    &cfg,
                    &dive_model::BindOptions {
                        eval: options.eval,
                        symmetry_liveness: options.symmetry_liveness,
                        no_deadlock: options.no_deadlock,
                        host: None,
                    },
                )
                .expect("model");
                let (_watchdog, pause) = trip_after_chunk(&mut options, false);
                let result = crate::capture::explore_concrete_parallel_with_evidence(
                    &model,
                    &options,
                    crate::capture::Limits::default(),
                )
                .expect("actual capture pool");
                assert!(pause.state.lock().expect("state").tripped);
                let crate::capture::Exploration::Stopped(result) = result else {
                    panic!("an incomplete source cannot receive GraphCompletion");
                };
                stopped(&result, 1025, crate::budget::Phase::Capture);
            }
        }
    }
}

#[cfg(test)]
mod replay_key_tests {
    use super::{Winner, replay_unseen_keys};
    use dive_store::ExactSet;

    #[test]
    fn replay_retains_resolved_keys_even_when_concrete_values_cannot_be_rekeyed() {
        // Internal control, not a claim of a reachable valid-spec disagreement.
        // The helper has no model/evaluator and must use the stored identity.
        let winner = Winner {
            key: vec![1, 2, 3].into_boxed_slice(),
            pos: (0, 0, 0),
            vals: vec![None].into(),
            action: 0,
            checkpoint_key: None,
        };
        let unseen = replay_unseen_keys::<ExactSet>(&[winner]);
        assert_eq!(
            unseen.into_iter().collect::<Vec<_>>(),
            [vec![1, 2, 3].into_boxed_slice()]
        );
    }
}
