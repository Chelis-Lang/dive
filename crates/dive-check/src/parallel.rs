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
use dive_store::{Candidates, ResolveOptions};
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
const PH_CAPTURE: u8 = 2;
const PH_INV: u8 = 3;
const PH_IMPLIED: u8 = 4;
const PH_DEADLOCK: u8 = 9;

/// Below this many candidates a level's resolve runs on one thread.
const PARALLEL_RESOLVE: usize = 4096;

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

/// ADR 0015 §2's block size: about 16 blocks per worker, 1 to 256 units.
fn block_size(n: usize, threads: usize) -> usize {
    (n / (16 * threads.max(1))).clamp(1, 256)
}

/// One level's phase-1 output of a block.
struct Accepted<K> {
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

/// One level's phase-1 job.
struct Expand<'env, S: Keyed> {
    m: &'env Model,
    keyer: &'env S::Keyer,
    set: &'env RwLock<S>,
    ctx: Ctx,
    level: u64,
    frontier: Arc<Vec<StateVals>>,
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
    buffers: Mutex<Vec<Candidates<S::Key, Pos, Payload>>>,
    chaos: Option<u64>,
}

/// The result of a successor's phase-1 checks that stops its block.
struct Stopped {
    verdict: Verdict,
    phase: u8,
    /// Whether the successor ends the counterexample.
    with: bool,
}

impl<S: Keyed> Expand<'_, S> {
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
            if range.start as u64 > self.cut.load(Ordering::Relaxed) {
                continue;
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
                self.cut.fetch_min(u as u64, Ordering::Relaxed);
                out.event = Some(e);
                break;
            }
        }
        out
    }

    /// Expands unit `unit`. Returns the successors generated and the event that
    /// stopped it.
    #[allow(clippy::too_many_arguments, reason = "one unit's buffers")]
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
        let s0 = &self.frontier[index];
        let state = index as u64;
        let mut generated = 0u64;
        let mut event: Option<Event> = None;
        for a in actions {
            set_thread_level(level_of(self.level));
            let first = if self.narrow { 0 } else { generated };
            let action = u32::try_from(a).unwrap_or(u32::MAX);
            let r = m
                .evaluator
                .next_states(&m.actions[a], s0, first, &mut |succ: &Successor<'_>| {
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
                        });
                        return Flow::Stop;
                    }
                    match self.successor(set, s0, succ, action, pos, buf, cap, lines, accepted) {
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
                            });
                            Flow::Stop
                        }
                    }
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
                        verdict: error_verdict(&e.error, ErrorSite::Next),
                        then: None,
                        resume: false,
                        stutter: false,
                    });
                    break;
                }
            }
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
        let checked = match in_model(m, &st1, Some(&step), &levels) {
            Err(v) => Err((v, true)),
            Ok(true) => state_key_of::<S>(m, self.keyer, self.ctx, &t, &st1)
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
                    accepted.push(Accepted {
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
                let v = check_invariants(m, &st1, false);
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
            let v = check_implied_actions(m, &step);
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
    }
}

/// A new state of the level, in position order.
struct Winner {
    pos: Pos,
    vals: StateVals,
    action: u32,
}

struct ChunkOut {
    event: Option<Event>,
    lines: Lines,
}

/// One level's phase-3 job: the invariants of the new states.
struct Check<'env> {
    m: &'env Model,
    level: u64,
    winners: Arc<Vec<Winner>>,
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

impl Check<'_> {
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
            let v = check_invariants(m, &St::state(w.vals.clone()), false);
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
                    verdict,
                    then: values(&w.vals).map(|values| TraceState {
                        label: a.label().to_owned(),
                        action: action_name(&m.evaluator, a),
                        values,
                        text: None,
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

/// Runs every level from the frontier `next` (the initial states) with up
/// to `workers` threads. Returns the stop, the statistics and the number of
/// threads used.
pub(crate) fn levels<S: Keyed>(
    search: &mut Search<'_, S>,
    next: Vec<(StateVals, StateId)>,
    opts: &CheckOptions,
    workers: usize,
) -> (Option<Stop>, Stats, usize) {
    let set = RwLock::new(std::mem::replace(&mut search.set, S::fresh(0)));
    let (stop, stats, used) = with_pool(workers, dive_eval::RECOMMENDED_STACK, |pool| {
        let (stop, stats) = run_levels(search, &set, next, opts, pool);
        (stop, stats, pool.threads())
    });
    search.set = set.into_inner().unwrap_or_else(PoisonError::into_inner);
    // Statistics computed with the set outside the search report its size.
    let stats = if stop.is_none() {
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
    search: &mut Search<'_, S>,
    frontier: &[StateVals],
    first_id: u64,
    winners: &[Winner],
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
            let vals = fresh.map_or_else(|| frontier[unit].clone(), |w| w.vals.clone());
            let id = fresh.map(|_| base + at as u64);
            let target = capture.state(record.key, vals, id);
            let result = target.and_then(|to| {
                capture.edge(
                    first_id + unit as u64,
                    crate::capture::Edge {
                        to,
                        ordinal: Some(u64::from(record.pos.2)),
                        label: Some(search.m.actions[record.action as usize].label().to_owned()),
                    },
                )
            });
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
    search: &mut Search<'a, S>,
    set: &'env RwLock<S>,
    next: Vec<(StateVals, StateId)>,
    opts: &CheckOptions,
    pool: &Pool<'env>,
) -> (Option<Stop>, Stats) {
    let m: &'env Model = search.m;
    let keyer: &'env S::Keyer = search.keyer;
    let threads = pool.threads();
    let check_deadlock = m.settings.check_deadlock;
    let n_actions = m.actions.len();
    let mut first_id: StateId = next.first().map_or(0, |(_, id)| *id);
    let mut frontier: Arc<Vec<StateVals>> = Arc::new(next.into_iter().map(|(s, _)| s).collect());
    let mut level = 1u64;
    let stats_now = |search: &Search<'_, S>, left: u64| Stats {
        generated: search.generated,
        distinct: set.read().unwrap_or_else(PoisonError::into_inner).len(),
        left_on_queue: left,
        depth: search.depth,
        initial_generated: search.initial_generated,
    };
    while !frontier.is_empty() {
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
        let mut next_frontier = Vec::new();
        let new_first = search.log.len();
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
            let inline = match &mut rng {
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
                capture_limit: search.capture.as_ref().map(|c| {
                    c.limits()
                        .successors_per_unit
                        .min(c.limits().generated.saturating_sub(search.generated))
                }),
                outs: blocks.iter().map(|_| Mutex::new(None)).collect(),
                blocks,
                order,
                claim: AtomicUsize::new(0),
                cut: AtomicU64::new(u64::MAX),
                buffers: Mutex::new(Vec::new()),
                chaos: opts.chaos,
            });
            let job: Job<'env> = {
                let e = expand.clone();
                Arc::new(move |w| e.work(w))
            };
            if let Err(message) = run(pool, &job, inline) {
                return internal(message, stats_now(search, 0));
            }
            drop(job);
            let mut gens = vec![0u64; n_units];
            let mut done = vec![false; n_units];
            let mut lines: Lines = Vec::new();
            let mut first: Option<Event> = None;
            let mut accepted = Vec::new();
            for (b, out) in expand.outs.iter().enumerate() {
                let Some(out) = out.lock().unwrap_or_else(PoisonError::into_inner).take() else {
                    continue;
                };
                let start = expand.blocks[b].start;
                for (k, g) in out.gens.iter().enumerate() {
                    gens[start + k] = *g;
                    done[start + k] = true;
                }
                lines.extend(out.lines);
                accepted.extend(out.accepted);
                first = earlier(first, out.event);
            }
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
            // Phase 2: resolve.
            let buffers = std::mem::take(
                &mut *expand
                    .buffers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            );
            drop(expand);
            let candidates: usize = buffers.iter().map(Candidates::len).sum();
            let distinct_before = set.read().unwrap_or_else(PoisonError::into_inner).len();
            let resolve = ResolveOptions {
                threads: match &mut rng {
                    Some(r) => 1 + r.below(threads),
                    None if candidates < PARALLEL_RESOLVE => 1,
                    None => threads,
                },
                chaos: rng.as_mut().map(Rng::next),
            };
            let resolved = set
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .resolve(buffers, &resolve);
            let resolved = match resolved {
                Ok(r) => r,
                Err(e) => return internal(e.to_string(), stats_now(search, 0)),
            };
            let winners: Arc<Vec<Winner>> = Arc::new(
                resolved
                    .into_position_order()
                    .into_iter()
                    .map(|c| Winner {
                        pos: c.pos,
                        vals: c.payload.vals,
                        action: c.payload.action,
                    })
                    .collect(),
            );
            // Resolve logical generated limits in reference order, independently of
            // which workers consumed the bounded per-source scratch budget.
            if let Some(capture) = &search.capture {
                let mut generated = search.generated;
                for (unit, &count) in gens.iter().enumerate().take(wave_end).skip(wave_start) {
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
                    &frontier,
                    first_id,
                    &winners,
                    accepted,
                    &gens,
                    &done,
                    wave_start..wave_end,
                    first,
                );
            }
            // Phase 3: the invariants of the new states.
            let limit = first.as_ref().map(|e| e.key);
            if !m.invariants.is_empty() && !winners.is_empty() {
                let n = winners.len();
                let size = match &mut rng {
                    Some(r) => 1 + r.below(2 * block_size(n, threads)),
                    None => block_size(n, threads),
                };
                let chunks = ranges(n, size);
                let mut order: Vec<usize> = (0..chunks.len()).collect();
                if let Some(r) = &mut rng {
                    r.shuffle(&mut order);
                }
                let inline = match &mut rng {
                    Some(r) => r.coin(),
                    None => chunks.len() == 1,
                };
                let check = Arc::new(Check {
                    m,
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
                let upto = winners
                    .partition_point(|w| w.pos < pos || (w.pos == pos && e.key.1 >= PH_CAPTURE));
                let before = winners.partition_point(|w| w.pos < pos);
                let generated = search.generated + gens[..e.unit].iter().sum::<u64>() + e.c;
                let depth = if upto > 0 {
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
                        ((len - i - 1) + next_frontier.len() + before) as u64
                    },
                    depth,
                    initial_generated: search.initial_generated,
                };
                let mut verdict = e.verdict;
                if e.resume {
                    // TLC's replay goes on past the state: as of E*, the states
                    // this level found after it are not in the set yet.
                    let unseen: BTreeSet<S::Key> = winners[upto..]
                        .iter()
                        .filter_map(|w| {
                            let st = St::state(w.vals.clone());
                            state_key_of::<S>(m, keyer, search.ctx, &w.vals, &st).ok()
                        })
                        .collect();
                    let first_unit = if narrow { i * n_actions } else { i };
                    let ordinal = gens[first_unit..e.unit].iter().sum::<u64>() + e.c - 1;
                    let set = set.read().unwrap_or_else(PoisonError::into_inner);
                    let r = search.replay_after(&set, &frontier[i], ordinal, level, &unseen);
                    stats.distinct += r.distinct;
                    stats.left_on_queue += r.queued;
                    verdict = r.apply(verdict);
                }
                let stop = Stop {
                    verdict,
                    in_init: false,
                    path: if e.stutter {
                        None
                    } else {
                        Some((first_id + pos.0, frontier[i].clone()))
                    },
                    then: e.then,
                };
                return (Some(stop), stats);
            }
            search.generated += gens.iter().sum::<u64>();
            // The trace log and the next frontier, in position order.
            for w in winners.iter() {
                let (i, ua, j) = w.pos;
                let ordinal = if narrow {
                    let i = usize::try_from(i).unwrap_or(usize::MAX);
                    let prior: u64 = gens[i * n_actions..i * n_actions + ua as usize]
                        .iter()
                        .sum();
                    u32::try_from(prior + u64::from(j)).ok()
                } else {
                    Some(j)
                };
                let Some(ordinal) = ordinal else {
                    return internal(
                        "more than 2^32 successors of one state".to_owned(),
                        stats_now(search, 0),
                    );
                };
                if let Err(e) = search.log.push(first_id + i, ordinal) {
                    return (
                        Some(Stop {
                            verdict: store_verdict(&e),
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
                        pos: x.pos,
                        vals: x.vals.clone(),
                        action: x.action,
                    })
                    .collect()
            });
            next_frontier.extend(winners.into_iter().map(|w| w.vals));
        }
        frontier = Arc::new(next_frontier);
        first_id = new_first;
        level += 1;
    }
    (None, stats_now(search, 0))
}
