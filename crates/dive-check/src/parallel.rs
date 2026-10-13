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
const PH_NATIVE: u8 = 2;
const PH_CAPTURE: u8 = 3;
const PH_INV: u8 = 4;
const PH_IMPLIED: u8 = 5;
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
    progress: Option<Arc<crate::progress::Lane>>,
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
    /// Capture waves have one block per concrete source. Missing output stops
    /// this prefix; a claim alone never counts as a returned expansion.
    fn completed_prefix(&self, cursor: usize, end: usize) -> usize {
        contiguous_returned(
            self.blocks.iter().zip(&self.outs).map(|(block, out)| {
                (
                    block.clone(),
                    out.lock().unwrap_or_else(PoisonError::into_inner).is_some(),
                )
            }),
            cursor,
            end,
        )
    }

    /// Moves only the selected completed source records and candidate positions.
    /// Later buffers remain intact and are never resolved prematurely.
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
        let mut buffers = self.buffers.lock().unwrap_or_else(PoisonError::into_inner);
        if self
            .blocks
            .last()
            .is_some_and(|block| block.end == units.end)
        {
            // All retained candidates belong to this wave's uncommitted suffix.
            // Preserve the original whole-wave move without repartition buffers.
            input.buffers = std::mem::take(&mut *buffers);
            return input;
        }
        for buffer in buffers.iter_mut() {
            let selected = buffer.extract_if(|pos| {
                usize::try_from(pos.0)
                    .ok()
                    .and_then(|source| {
                        if self.narrow {
                            source
                                .checked_mul(self.actions())?
                                .checked_add(pos.1 as usize)
                        } else {
                            Some(source)
                        }
                    })
                    .is_some_and(|unit| units.contains(&unit))
            });
            if !selected.is_empty() {
                input.buffers.push(selected);
            }
        }
        buffers.retain(|buffer| !buffer.is_empty());
        input
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
            let k = if let Some(progress) = &self.progress {
                let Some(claim) = progress.claim(&self.claim, self.order.len()) else {
                    break;
                };
                claim
            } else {
                self.claim.fetch_add(1, Ordering::Relaxed)
            };
            let Some(&b) = self.order.get(k) else { break };
            let range = self.blocks[b].clone();
            if range.start as u64 > self.cut.load(Ordering::Relaxed) {
                continue;
            }
            let out = self.block(&set, range, &mut buf, &mut rng);
            *self.outs[b].lock().unwrap_or_else(PoisonError::into_inner) = Some(out);
            #[cfg(feature = "test-rendezvous")]
            if let Some(progress) = &self.progress {
                progress.returned_source_rendezvous();
            }
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
                        verdict: error_verdict(&e.error, ErrorSite::Next),
                        then: None,
                        resume: false,
                        stutter: false,
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
    /// The actual resolved store key, retained without re-keying a concrete state.
    checkpoint_key: Option<Vec<u8>>,
}

fn ordinal_of(winner: &Winner, gens: &[u64], narrow: bool, actions: usize) -> Option<u32> {
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

fn stopped_fault<S: Keyed>(
    search: &mut Search<'_, '_, S>,
    fault: crate::capture::StoppedDiagnosticFault,
) {
    search.stopped_faults[fault as usize] = true;
    if let Some(observer) = search.observer.as_mut()
        && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observer.stopped_diagnostic_failed(fault);
        }))
        .is_err()
    {
        search.stopped_faults[crate::capture::StoppedDiagnosticFault::Unwind as usize] = true;
    }
}

fn stopped_origin(stop: &Stop) -> Option<(crate::capture::WorkerStop, u64)> {
    if stop.in_init {
        return None;
    }
    let (source, _) = stop.path.as_ref()?;
    Some((
        crate::capture::WorkerStop::classify(&stop.verdict)?,
        *source,
    ))
}

fn stopped_service<S: Keyed>(
    search: &mut Search<'_, '_, S>,
    stop: &Stop,
    kind: crate::capture::WorkerStop,
    stats: Stats,
    pool: &Pool<'_>,
    progress: Option<&Arc<crate::progress::Lane>>,
) {
    use crate::capture::StoppedDiagnosticFault as Fault;
    let Ok(pending) = progress.map_or(Ok(None), |lane| lane.finish()) else {
        stopped_fault(search, Fault::LaneClose);
        return;
    };
    let callback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pool.with_pause(|pause| {
            let Some((source, _)) = stop.path.as_ref() else {
                return Ok(());
            };
            let end = crate::capture::StoppedQueue::new(
                &stop.verdict,
                kind,
                stats,
                pause,
                pending,
                *source,
            );
            search.stopped_transaction = Some(crate::capture::StoppedTransaction {
                verdict: stop.verdict.clone(),
                source: *source,
                stats,
                workers: pause.workers(),
                generation: pause.generation(),
            });
            search
                .observer
                .as_mut()
                .map_or(Ok(()), |observer| observer.stopped_queue_finished(&end))
        })
    }));
    let fault = match callback {
        Ok(Ok(Ok(()))) => None,
        Ok(Ok(Err(_))) => Some(Fault::Callback),
        Ok(Err(_)) => Some(Fault::Dispatch),
        Err(_) => Some(Fault::Unwind),
    };
    if let Some(fault) = fault {
        stopped_fault(search, fault);
    }
}

/// Runs every level from the frontier `next` (the initial states) with up
/// to `workers` threads. Returns the stop, the statistics and the number of
/// threads used.
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
    let initial_stats = search.stats(next.len() as u64);
    let set = RwLock::new(std::mem::replace(&mut search.set, S::fresh(0)));
    let (stop, stats, used) = with_pool(workers, dive_eval::RECOMMENDED_STACK, |pool| {
        let request_options = search.observer.as_ref().and_then(|o| o.progress_requests());
        let mut returned_stats = None;
        let mut stopped_enabled = false;
        match crate::progress::with_lane_retained(request_options, |progress| {
            let (stop, stats) =
                run_levels(search, &set, next, opts, pool, level, progress.as_ref());
            returned_stats = Some(stats);
            if let Some((winner, kind)) = stop
                .as_ref()
                .and_then(|winner| stopped_origin(winner).map(|(kind, _)| (winner, kind)))
            {
                let enabled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    search
                        .observer
                        .as_ref()
                        .is_some_and(|observer| observer.requires_stopped_service())
                }));
                match enabled {
                    Ok(true) => {
                        stopped_enabled = true;
                        stopped_service(search, winner, kind, stats, pool, progress.as_ref());
                    }
                    Ok(false) => {}
                    Err(_) => {
                        stopped_enabled = true;
                        stopped_fault(search, crate::capture::StoppedDiagnosticFault::Unwind);
                    }
                }
            }
            // Only the real exhausted-frontier return may enter this additive
            // transaction. A failed source/Init/cut never creates EOF authority.
            if stop.is_none()
                && search
                    .observer
                    .as_ref()
                    .is_some_and(|o| o.requires_terminal_service())
            {
                let result = progress
                    .as_ref()
                    .map_or(Ok(None), |lane| lane.finish())
                    .and_then(|pending| {
                        pool.with_pause(|pause| {
                            let end = crate::capture::ExhaustedQueue::new(stats, pause, pending);
                            search
                                .observer
                                .as_mut()
                                .map_or(Ok(()), |o| o.terminal_queue_finished(&end))
                        })
                    })
                    .and_then(std::convert::identity);
                if let Err(feature) = result {
                    return (
                        Some(Stop {
                            verdict: Verdict::Unsupported { feature },
                            in_init: false,
                            path: None,
                            then: None,
                        }),
                        stats,
                    );
                }
            }
            (stop, stats)
        }) {
            Ok(((stop, stats), None)) => (stop, stats, pool.threads()),
            Ok(((stop, stats), Some(_))) if stopped_enabled => {
                stopped_fault(search, crate::capture::StoppedDiagnosticFault::LaneJoin);
                (stop, stats, pool.threads())
            }
            Ok((_, Some(feature))) | Err(feature) => (
                Some(Stop {
                    verdict: Verdict::Unsupported { feature },
                    in_init: false,
                    path: None,
                    then: None,
                }),
                returned_stats.unwrap_or(initial_stats),
                pool.threads(),
            ),
        }
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
    search: &mut Search<'_, '_, S>,
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
    search: &mut Search<'a, '_, S>,
    set: &'env RwLock<S>,
    next: crate::frontier::Queue,
    opts: &CheckOptions,
    pool: &Pool<'env>,
    mut level: u64,
    progress: Option<&Arc<crate::progress::Lane>>,
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
        let mut observed_pool_generated = 0u64;
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
                progress: progress.cloned(),
                chaos: opts.chaos,
            });
            let job: Job<'env> = {
                let e = expand.clone();
                Arc::new(move |w| e.work(w))
            };
            let mut cursor = wave_start;
            loop {
                if let Err(message) = run(pool, &job, inline) {
                    return internal(
                        message,
                        stats_now(search, (len - cursor + next_frontier.len()) as u64),
                    );
                }
                if let Some(progress) = &progress {
                    let request = if expand.cut.load(Ordering::Relaxed) == u64::MAX {
                        match progress.take() {
                            Ok(request) => request,
                            Err(message) => {
                                return internal(
                                    message,
                                    stats_now(search, (len - cursor + next_frontier.len()) as u64),
                                );
                            }
                        }
                    } else {
                        let (fresh, pending) = progress.cancel_requests();
                        if fresh && let Some(observer) = &mut search.observer {
                            observer.expansion_requests_cancelled(pending);
                        }
                        None
                    };
                    if let Some(request) = request {
                        let mut completed: Vec<_> = expand
                            .outs
                            .iter()
                            .enumerate()
                            .filter_map(|(b, out)| {
                                out.lock()
                                    .unwrap_or_else(PoisonError::into_inner)
                                    .as_ref()
                                    .map(|_| first_id + expand.blocks[b].start as u64)
                            })
                            .collect();
                        completed.extend((wave_start..cursor).map(|unit| first_id + unit as u64));
                        completed.sort_unstable();
                        let claimed = expand
                            .order
                            .iter()
                            .take(expand.claim.load(Ordering::Relaxed))
                            .map(|b| first_id + expand.blocks[*b].start as u64)
                            .collect();
                        let mut failed = Vec::new();
                        let mut expanded_generated = 0u64;
                        for (b, output) in expand.outs.iter().enumerate() {
                            let guard = output.lock().unwrap_or_else(PoisonError::into_inner);
                            if let Some(output) = guard.as_ref() {
                                if output.event.is_some() {
                                    failed.push(first_id + expand.blocks[b].start as u64);
                                }
                                for generated in &output.gens {
                                    let Some(total) = expanded_generated.checked_add(*generated)
                                    else {
                                        return internal(
                                            "request scratch generation counter overflow".into(),
                                            stats_now(
                                                search,
                                                (len - cursor + next_frontier.len()) as u64,
                                            ),
                                        );
                                    };
                                    expanded_generated = total;
                                }
                            }
                        }
                        let logical_before =
                            stats_now(search, (len - cursor + next_frontier.len()) as u64);
                        let prefix_end = if search.capture.is_some() && !narrow {
                            expand.completed_prefix(cursor, wave_end)
                        } else {
                            cursor
                        };
                        let prefix_start = cursor;
                        let mut stopped = None;
                        let result = pool
                            .with_pause(|pause| {
                                if prefix_end > cursor {
                                    let input = expand.take_sources(cursor..prefix_end, n_units);
                                    let committed_generated = input.gens.iter().sum::<u64>();
                                    let result = commit_sources(
                                        search,
                                        set,
                                        pool,
                                        Some(pause),
                                        threads,
                                        opts,
                                        level,
                                        first_id,
                                        &frontier,
                                        &mut next_frontier,
                                        narrow,
                                        concrete_pool,
                                        wave_start..wave_end,
                                        cursor..prefix_end,
                                        &input.gens,
                                        &input.done,
                                        input.lines,
                                        input.first,
                                        input.accepted,
                                        input.buffers,
                                        &mut rng,
                                        observed_pool_generated + expanded_generated,
                                    );
                                    if result.0.is_some() {
                                        stopped = Some(result);
                                        return Ok(());
                                    }
                                    observed_pool_generated += committed_generated;
                                    cursor = prefix_end;
                                }
                                let receipt = crate::capture::ExpansionQuiescence {
                                    request,
                                    submitted: first_id + wave_start as u64
                                        ..first_id + wave_end as u64,
                                    completed,
                                    claimed,
                                    failed,
                                    expanded_generated,
                                    logical: logical_before,
                                    committed: first_id + prefix_start as u64
                                        ..first_id + cursor as u64,
                                    logical_after: stats_now(
                                        search,
                                        (len - cursor + next_frontier.len()) as u64,
                                    ),
                                    predicate_workers: usize::from(
                                        cursor > prefix_start
                                            && !m.invariants.is_empty()
                                            && set
                                                .read()
                                                .unwrap_or_else(PoisonError::into_inner)
                                                .len()
                                                > logical_before.distinct,
                                    ),
                                };
                                search.observer.as_mut().map_or(Ok(()), |observer| {
                                    observer.expansion_quiescent(receipt, pause)
                                })
                            })
                            .and_then(std::convert::identity);
                        if let Some(result) = stopped {
                            return result;
                        }
                        if let Err(feature) = result {
                            return (
                                Some(Stop {
                                    verdict: Verdict::Unsupported { feature },
                                    in_init: false,
                                    path: None,
                                    then: None,
                                }),
                                stats_now(search, (len - cursor + next_frontier.len()) as u64),
                            );
                        }
                    }
                }
                if expand.claim.load(Ordering::Relaxed) >= expand.order.len() {
                    break;
                }
            }
            drop(job);
            let CommitInput {
                gens,
                done,
                lines,
                mut first,
                accepted,
                buffers,
            } = expand.take_sources(cursor..wave_end, n_units);
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
            observed_pool_generated += gens.iter().sum::<u64>();
            let result = commit_sources(
                search,
                set,
                pool,
                None,
                threads,
                opts,
                level,
                first_id,
                &frontier,
                &mut next_frontier,
                narrow,
                concrete_pool,
                wave_start..wave_end,
                cursor..wave_end,
                &gens,
                &done,
                lines,
                first,
                accepted,
                buffers,
                &mut rng,
                observed_pool_generated,
            );
            if result.0.is_some() {
                return result;
            }
        }
        frontier = Arc::new(next_frontier);
        first_id = new_first;
        level += 1;
    }
    (None, stats_now(search, 0))
}

/// Shared publication path for a whole wave or an actually completed contiguous
/// prefix. A borrowed pause keeps expansion dispatch closed; predicate work is
/// inline and resolve uses one thread rather than re-entering that pool.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one ordered publication transaction shared by waves and prefixes"
)]
fn commit_sources<'a: 'env, 'env, S: Keyed>(
    search: &mut Search<'a, '_, S>,
    set: &'env RwLock<S>,
    pool: &Pool<'env>,
    pause: Option<&crate::capture::DispatchPause<'_>>,
    threads: usize,
    opts: &CheckOptions,
    level: u64,
    first_id: u64,
    frontier: &crate::frontier::Queue,
    next_frontier: &mut crate::frontier::Queue,
    narrow: bool,
    concrete_pool: bool,
    submitted: Range<usize>,
    units: Range<usize>,
    gens: &[u64],
    done: &[bool],
    mut lines: Lines,
    mut first: Option<Event>,
    accepted: Vec<Accepted<S::Key>>,
    buffers: Vec<Candidates<S::Key, Pos, Payload>>,
    rng: &mut Option<Rng>,
    speculative_generated: u64,
) -> (Option<Stop>, Stats) {
    let m: &'env Model = search.m;
    let keyer: &'env S::Keyer = search.keyer;
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
        threads: if pause.is_some() {
            1
        } else {
            match &mut *rng {
                Some(r) => 1 + r.below(threads),
                None if candidates < PARALLEL_RESOLVE => 1,
                None => threads,
            }
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
                checkpoint_key: search.has_checkpoint().then(|| S::checkpoint_key(&c.key)),
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
                    }),
                    first,
                );
                break;
            }
            generated += count;
        }
    }
    let mut observed = if search.observer.is_some() {
        accepted
            .iter()
            .map(|a| (a.pos, a.vals.clone(), a.action))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
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
        if let Err(message) = if pause.is_some() {
            Pool::run_inline(&job)
        } else {
            run(pool, &job, inline)
        } {
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
    // Consume complete sources only after their safety checks, before
    // a later source's deciding event. Counts include previous committed
    // waves but exclude speculative work beyond this source.
    if search.observer.is_some() {
        observed.sort_by_key(|a| a.0);
        let pool_generated = speculative_generated;
        let source_limit = first.as_ref().map_or(units.end, |e| {
            usize::try_from(e.key.0.0)
                .unwrap_or(units.end)
                .min(units.end)
        });
        let mut from = 0;
        let mut to = 0;
        let mut upto = 0;
        let mut generated = search.generated;
        for (source, emitted) in gens.iter().enumerate().take(source_limit).skip(units.start) {
            while to < observed.len() && observed[to].0.0 <= source as u64 {
                to += 1;
            }
            while upto < winners.len() && winners[upto].pos.0 <= source as u64 {
                upto += 1;
            }
            generated += emitted;
            let successors: Vec<_> = observed[from..to]
                .iter()
                .map(|a| crate::capture::ConcreteSuccessor {
                    state: a.1.clone(),
                    ordinal: u64::from(a.0.2),
                    label: m.actions[a.2 as usize].label().to_owned(),
                })
                .collect();
            from = to;
            let counters = Stats {
                generated,
                distinct: distinct_before + upto as u64,
                left_on_queue: ((len - source - 1) + next_frontier.len() + upto) as u64,
                depth: if upto > 0 {
                    search.depth.max(level + 1)
                } else {
                    search.depth
                },
                initial_generated: search.initial_generated,
            };
            let (state, _) = match frontier.read(source, &m.names) {
                Ok(state) => state,
                Err(verdict) => {
                    return (
                        Some(Stop {
                            verdict,
                            in_init: false,
                            path: None,
                            then: None,
                        }),
                        counters,
                    );
                }
            };
            let observation = crate::capture::CommittedSource {
                id: first_id + source as u64,
                level,
                state: &state,
                successors: &successors,
                stats: counters,
                pool_generated,
                barrier: if pause.is_some() {
                    crate::capture::SourceBarrier::joined_prefix(
                        threads,
                        first_id + submitted.start as u64,
                        first_id + submitted.end as u64,
                        first_id + units.start as u64,
                        first_id + units.end as u64,
                        usize::from(!m.invariants.is_empty() && !winners.is_empty()),
                    )
                } else {
                    crate::capture::SourceBarrier::joined(
                        threads,
                        first_id + submitted.start as u64,
                        first_id + submitted.end as u64,
                    )
                },
            };
            let observation_result = if let Some(observer) = &mut search.observer {
                if observer.requires_dispatch_pause() {
                    if let Some(pause) = pause {
                        observer.paused_source(observation, pause)
                    } else {
                        pool.with_pause(|pause| observer.paused_source(observation, pause))
                            .and_then(std::convert::identity)
                    }
                } else {
                    observer.source(observation)
                }
            } else {
                Ok(())
            };
            if let Err(message) = observation_result {
                first = earlier(
                    first,
                    Some(Event {
                        key: ((source as u64, u32::MAX, u32::MAX), PH_DEADLOCK + 1),
                        unit: source,
                        c: *emitted,
                        verdict: Verdict::Unsupported {
                            feature: format!("ordered source observer: {message}"),
                        },
                        then: None,
                        resume: false,
                        stutter: false,
                    }),
                );
                break;
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
                ((len - i - 1) + next_frontier.len() + before) as u64
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
            let r = search.replay_after(&set, &vals, ordinal, level, &unseen);
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

/// Contiguity depends on returned outputs, never on claim position or later work.
fn contiguous_returned(
    outputs: impl IntoIterator<Item = (Range<usize>, bool)>,
    cursor: usize,
    end: usize,
) -> usize {
    let mut upto = cursor;
    for (block, returned) in outputs {
        if block.start < cursor {
            continue;
        }
        if block.start != upto || block.end > end || !returned {
            break;
        }
        upto = block.end;
    }
    upto
}
#[cfg(test)]
mod prefix_tests {
    use super::contiguous_returned;
    #[test]
    fn unclaimed_gap_never_publishes_later_returned_sources() {
        for mask in 0u16..256 {
            for cursor in 0..=8 {
                let expected = (cursor..8).find(|i| mask & (1 << i) == 0).unwrap_or(8);
                let outputs = (0..8).map(|i| (i..i + 1, mask & (1 << i) != 0));
                assert_eq!(contiguous_returned(outputs, cursor, 8), expected);
            }
        }
    }
}

#[cfg(test)]
mod stopped_origin_tests {
    use super::*;
    #[test]
    fn eligible_classification_without_actual_source_path_is_not_authority() {
        let stop = Stop {
            verdict: Verdict::Deadlock,
            in_init: false,
            path: None,
            then: None,
        };
        assert!(stopped_origin(&stop).is_none());
    }
}
