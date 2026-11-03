//! The parallel level (`docs/developer/core/07-engine.md` §3).
//!
//! A level is expanded in two phases, so that every observable result is
//! the one-worker result, whatever the worker count:
//!
//! 1. **Expand** (parallel). Workers claim batches of the frontier's states
//!    in position order from an atomic counter. Each worker has its own
//!    [`Machine`] (arena, memo tables) and shares the persistent store. For
//!    each successor it evaluates everything that depends only on the
//!    successor and its parent: completeness, the constraints, the state
//!    key, and, unless the state was seen in an earlier level, the
//!    invariants. A successor that may be new is interned. Nothing shared is
//!    written apart from the store, which is safe to intern into from every
//!    worker, and the seen set of earlier levels is only read.
//! 2. **Commit** (sequential, in position order). The explorer walks the
//!    recorded successors as the one-worker search would emit them: it
//!    counts them, inserts new ones into the seen set (the first position
//!    wins), decides the first event (a violation, an error, a deadlock, a
//!    limit), queues new states and logs their parents. The commit does no
//!    evaluation, so it is a small part of the level's work.
//!
//! An invariant evaluated in phase 1 for a successor that turns out to be a
//! duplicate within the level is wasted work but cannot change a result: it
//! is a function of the state, and the state's first occurrence had it
//! evaluated too. Warnings are merged in the order the one-worker search
//! raises them: each worker records, with each warning it raises first, how
//! many successors of the state it had emitted.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use dive_core::plan::Code;
use dive_core::{Error, ErrorKind, Handle, Machine, Opts, Plan, Sink};

use crate::ending::Site;
use crate::{Metrics, Seen, changed_vars, eval_check, in_model, state_identity};

/// Lock-free minimum-position claims on the state keys of one level
/// (`07-engine.md` §2): a successor inside the model, not seen in an
/// earlier level, claims its key with its position. A successor whose key
/// is held by an earlier position is a duplicate within the level: the
/// one-worker search inserts the earlier one first, so this one is not new,
/// and it is neither interned nor checked.
///
/// A claim only saves work. When the table is full, or a position does not
/// fit, the successor is treated as possibly new and checked; the commit
/// decides either way.
struct Claims {
    keys: Box<[AtomicU64]>,
    pos: Box<[AtomicU64]>,
}

/// Successors per state that a position can name.
const POS_BITS: u32 = 24;

/// The most slots a claim probes. A key it does not place within them is
/// treated as possibly new, so a crowded table costs checks, never time.
const PROBES: usize = 32;

/// The largest claim table: 2^23 slots, 128 MiB with both columns.
const MAX_SLOTS: usize = 1 << 23;

impl Claims {
    fn new(capacity: usize) -> Self {
        let size = capacity.next_power_of_two().clamp(1 << 12, MAX_SLOTS);
        Self {
            keys: (0..size).map(|_| AtomicU64::new(0)).collect(),
            pos: (0..size).map(|_| AtomicU64::new(u64::MAX)).collect(),
        }
    }

    /// The slot of `key` (as the seen set fixes it), claiming an empty one.
    fn slot(&self, key: u64) -> Option<usize> {
        let k = Seen::fix(key);
        let mask = self.keys.len() - 1;
        #[allow(clippy::cast_possible_truncation, reason = "masked to the table size")]
        let mut i = (k as usize) & mask;
        for _ in 0..PROBES {
            match self.keys[i].compare_exchange(0, k, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return Some(i),
                Err(x) if x == k => return Some(i),
                Err(_) => i = (i + 1) & mask,
            }
        }
        None
    }

    /// Lowers the claim on `key` to `pos`; whether `pos` now holds it (no
    /// earlier position does).
    fn claim(&self, key: u64, pos: u64) -> bool {
        let Some(i) = self.slot(key) else {
            return true;
        };
        let held = self.pos[i].fetch_min(pos, Ordering::AcqRel);
        held >= pos
    }

    /// Whether an earlier position than `pos` holds `key`.
    fn held_before(&self, key: u64, pos: u64) -> bool {
        let k = Seen::fix(key);
        let mask = self.keys.len() - 1;
        #[allow(clippy::cast_possible_truncation, reason = "masked to the table size")]
        let mut i = (k as usize) & mask;
        for _ in 0..PROBES {
            match self.keys[i].load(Ordering::Acquire) {
                0 => return false,
                x if x == k => return self.pos[i].load(Ordering::Acquire) < pos,
                _ => i = (i + 1) & mask,
            }
        }
        false
    }
}

/// What the invariants say about a successor.
#[derive(Clone, Debug)]
pub(crate) enum Inv {
    Holds,
    Violated(usize),
    Error(usize, Error),
}

/// One emitted successor, as phase 1 recorded it.
#[derive(Clone, Debug)]
pub(crate) enum Occ {
    /// Inside the model and seen in an earlier level, or a duplicate of an
    /// earlier position in this level: only counted (`key` for the
    /// admission check), and reported to the graph when `inside` the model.
    Old {
        key: u64,
        inside: bool,
        /// Original values only when graph capture needs this accepted target.
        at: Option<usize>,
    },
    /// The first emission beyond the collector's per-source admission bound.
    /// It has no payload; commit delivers its ordinary emitted event and stop.
    CaptureLimit,
    /// The armed run budget expired before retaining a capture emission.
    CaptureBudget,
    /// Possibly new: its key, whether it is inside the model, the
    /// invariants' verdict (applied if it is new), the implied actions'
    /// verdict (applied in any case), and its values at `at` in the
    /// result's values. A seen successor whose implied actions do not hold
    /// is recorded here too, for its values.
    Cand {
        key: u64,
        identity: Vec<Handle>,
        in_model: bool,
        inv: Inv,
        imp: Inv,
        at: usize,
    },
    /// Leaves variables unassigned (values at `at`).
    Incomplete { at: usize },
    /// A constraint failed to evaluate on it (values at `at`).
    CheckErr { site: Site, error: Error, at: usize },
}

/// How the expansion of one state ended.
#[derive(Clone, Debug)]
pub(crate) enum Ending {
    /// Every action ran to the end, or the last successor ends the search.
    Done,
    /// Actual caller-owned work stopped before another emitted successor.
    Budget,
    /// A candidate could not be retained; it is not a source evaluation error.
    RetentionError(Error),
    /// The generator itself raised `error` in `action`, after that action
    /// had emitted `emitted` successors.
    GenError {
        action: usize,
        emitted: u64,
        error: Error,
    },
}

/// The expansion of one state.
#[derive(Debug)]
pub(crate) struct Expanded {
    /// The successors in emission order, each with its action.
    pub occs: Vec<(usize, Occ)>,
    /// Their values (persistent handles), `nvars` per recorded state.
    pub values: Vec<Handle>,
    pub payloads: Option<crate::candidate::Spool>,
    pub end: Ending,
    /// Warnings 2143 this worker raised first while expanding the state,
    /// each with the number of successors emitted before it.
    pub warnings: Vec<(u64, (u16, u32))>,
}

impl Expanded {
    fn push(&mut self, action: usize, occ: Occ) -> Result<(), Error> {
        if let Some(payloads) = &mut self.payloads {
            payloads.push(action, &occ)
        } else {
            self.occs.push((action, occ));
            Ok(())
        }
    }

    fn len(&self) -> usize {
        self.payloads
            .as_ref()
            .map_or(self.occs.len(), crate::candidate::Spool::len)
    }
}

/// A worker's expansions, by position, and its metrics.
type WorkerResult = (Vec<(usize, Expanded)>, Metrics);

struct WorkerSink<'a> {
    plan: &'a Plan,
    seen: &'a Seen,
    claims: &'a Claims,
    /// The position of the state being expanded, shifted by [`POS_BITS`].
    base: u64,
    opts: Opts,
    /// Capture's existing per-source bound; `None` retains the safety fast path.
    preserve_originals: bool,
    capture_limit: Option<u64>,
    poll: &'a (dyn Fn() -> bool + Sync),
    metrics: Metrics,
    out: Expanded,
    action: usize,
    emitted: u64,
    warned: usize,
}

impl WorkerSink<'_> {
    fn take_warnings(&mut self, m: &Machine) {
        while self.warned < m.warnings.len() {
            self.out
                .warnings
                .push((self.emitted, m.warnings[self.warned]));
            self.warned += 1;
        }
    }

    fn keep(&mut self, m: &mut Machine) -> Result<usize, Error> {
        self.keep_values(m, None, None)
    }

    fn keep_values(
        &mut self,
        m: &mut Machine,
        original: Option<&[Handle]>,
        identity: Option<&mut Vec<Handle>>,
    ) -> Result<usize, Error> {
        if let Some(payloads) = &mut self.out.payloads {
            let row = original.unwrap_or(&m.s1);
            let mut roots = crate::capture_copy(row)?;
            if self.capture_limit.is_none() && !self.preserve_originals {
                for h in &mut roots {
                    if !h.is_empty() && !m.heap.has_construction_history(*h) {
                        *h = m.heap.close_identity_local(*h)?;
                    }
                }
            }
            if let Some(identity) = identity {
                crate::capture_reserve(&mut roots, identity.len())?;
                roots.append(identity);
            }
            return payloads.keep(m, &roots);
        }
        let at = self.out.values.len();
        if self.capture_limit.is_some() || self.preserve_originals {
            crate::capture_reserve(&mut self.out.values, original.unwrap_or(&m.s1).len())?;
        }
        for &h in original.unwrap_or(&m.s1) {
            let h = if h.is_empty() {
                h
            } else if self.capture_limit.is_some()
                || self.preserve_originals
                || m.heap.has_construction_history(h)
            {
                m.heap.intern_exact(h)?
            } else {
                m.heap.intern(h)?
            };
            self.out.values.push(h);
        }
        Ok(at)
    }

    /// Records an evaluation error of a check at `site` on the current
    /// candidate, and returns the stop that ends its expansion.
    fn check_err(&mut self, m: &mut Machine, site: Site, error: Error) -> Result<Error, Error> {
        let at = self.keep(m)?;
        self.out
            .push(self.action, Occ::CheckErr { site, error, at })?;
        Ok(Error::stop())
    }

    fn fingerprintable(&self, m: &mut Machine, identity: &[Handle]) -> Result<bool, Error> {
        let closed = (self.out.payloads.is_some()
            && self.seen.exact.is_some()
            && self.plan.symmetry.is_none()
            && self.plan.view.is_none())
        .then_some(identity);
        crate::fingerprintable_with_identity(m, false, closed)
    }

    fn record(&mut self, m: &mut Machine) -> Result<(), Error> {
        let plan = self.plan;
        // Fingerprinting/VIEW can normalize m.s1 in place. Only capture needs
        // the original emitted representation as well as its identity.
        let original = (self.capture_limit.is_some() || self.preserve_originals)
            .then(|| crate::capture_copy(&m.s1))
            .transpose()?;
        if m.assigned != m.s1.len() {
            let at = self.keep(m)?;
            self.out.push(self.action, Occ::Incomplete { at })?;
            return Err(Error::stop());
        }
        let skip = self.opts.parent_skip.then(|| changed_vars(m));
        let inside = match in_model(m, plan, true, skip, &mut self.metrics) {
            Ok(b) => b,
            Err((site, error)) => return Err(self.check_err(m, site, error)?),
        };
        // Under symmetry or a VIEW a state outside the model is checked
        // without a lookup, as TLC checks it (see `Explorer::probe`).
        if inside {
            crate::normalize_state_history(m)?;
        }
        let unkeyed = !inside && (plan.symmetry.is_some() || plan.view.is_some());
        let mut identity = Vec::new();
        let key = if unkeyed {
            0
        } else {
            match state_identity(m, plan, self.seen.exact.as_ref().map(|_| &mut identity)) {
                Ok(k) => k,
                Err(error) if error.kind() == ErrorKind::Eval && plan.view.is_some() => {
                    return Err(self.check_err(m, Site::View, error)?);
                }
                Err(e) => return Err(e),
            }
        };
        self.metrics.probed += 1;
        // TLC checks the implied actions on every step (`Search::successor`).
        let mut imp = Inv::Holds;
        for (i, c) in plan.implied_actions.iter().enumerate() {
            match crate::check_step(m, &plan.code, c) {
                Ok(true) => {}
                Ok(false) => {
                    imp = Inv::Violated(i);
                    break;
                }
                Err(e) => {
                    imp = Inv::Error(i, e);
                    break;
                }
            }
        }
        let pos = (self.emitted - 1 < 1 << POS_BITS).then_some(self.base | (self.emitted - 1));
        let duplicate = if self.out.payloads.is_some() || self.seen.exact.is_some() {
            // A fingerprint claim supplies no equality evidence. Exact mode
            // retains all possibly distinct collisions for ordered commit.
            false
        } else if inside {
            self.seen.contains(key) || pos.is_some_and(|p| !self.claims.claim(key, p))
        } else if unkeyed {
            false
        } else {
            // Not inserted; new unless an earlier position inside the model
            // inserts it first.
            pos.is_some_and(|p| self.claims.held_before(key, p))
        };
        if duplicate && matches!(imp, Inv::Holds) {
            let at = if inside && self.capture_limit.is_some() {
                Some(self.keep_values(m, original.as_deref(), None)?)
            } else {
                self.metrics.discarded_before_intern += 1;
                None
            };
            self.out.push(self.action, Occ::Old { key, inside, at })?;
            return Ok(());
        }
        if inside && plan.view.is_none() && !self.fingerprintable(m, &identity)? {
            let error = Error::eval(crate::FP_MESSAGE);
            return Err(self.check_err(m, Site::Fingerprint, error)?);
        }
        let mut inv = Inv::Holds;
        for (i, c) in plan.invariants.iter().enumerate() {
            match eval_check(m, &plan.code, c, skip, &mut self.metrics) {
                Ok(true) => {}
                Ok(false) => {
                    inv = Inv::Violated(i);
                    break;
                }
                Err(e) => {
                    inv = Inv::Error(i, e);
                    break;
                }
            }
        }
        let at = self.keep_values(m, original.as_deref(), Some(&mut identity))?;
        self.out.push(
            self.action,
            Occ::Cand {
                key,
                identity,
                in_model: inside,
                inv,
                imp,
                at,
            },
        )?;
        Ok(())
    }
}

impl Sink for WorkerSink<'_> {
    fn emit(&mut self, m: &mut Machine, _code: &Code) -> Result<(), Error> {
        self.take_warnings(m);
        self.emitted = self
            .emitted
            .checked_add(1)
            .ok_or_else(|| Error::unsupported("capture successor ordinal overflow"))?;
        if self.capture_limit.is_some() || self.preserve_originals {
            crate::capture_reserve(&mut self.out.occs, 1)?;
        }
        if self.capture_limit.is_some_and(|limit| self.emitted > limit) {
            self.out.push(self.action, Occ::CaptureLimit)?;
            return Err(Error::stop());
        }
        if self.capture_limit.is_some() && (self.poll)() {
            self.out.push(self.action, Occ::CaptureBudget)?;
            return Err(Error::stop());
        }
        let r = self.record(m);
        if m.heap.poisoned() {
            return Err(Error::internal("a stale value handle was read"));
        }
        r
    }
}

/// Expands the state `s0` in phase 1.
#[allow(clippy::too_many_arguments, reason = "one level's shared inputs")]
fn expand_one(
    m: &mut Machine,
    plan: &Plan,
    seen: &Seen,
    claims: &Claims,
    k: usize,
    opts: Opts,
    preserve_originals: bool,
    capture_limit: Option<u64>,
    candidates: Option<(&crate::storage::Storage, &crate::candidate::Account)>,
    poll: &(dyn Fn() -> bool + Sync),
    s0: &[Handle],
    metrics: &mut Metrics,
) -> Expanded {
    m.s0.copy_from_slice(s0);
    let payloads = candidates
        .map(|(storage, account)| {
            let directory = storage
                .candidate_directory
                .clone()
                .unwrap_or_else(std::env::temp_dir);
            crate::candidate::Spool::new(
                m,
                storage.candidate_memory_bytes.unwrap_or(0),
                &directory,
                std::sync::Arc::clone(account),
                if seen.exact.is_some() {
                    if plan.view.is_some() {
                        1
                    } else {
                        plan.vars.len()
                    }
                } else {
                    0
                },
                plan.symmetry.is_some() || plan.view.is_some(),
            )
        })
        .transpose();
    let old_speculative = m.heap.set_speculative(candidates.is_some());
    let (payloads, setup_error) = match payloads {
        Ok(p) => (p, None),
        Err(e) => (None, Some(e)),
    };
    let mut sink = WorkerSink {
        plan,
        seen,
        claims,
        base: (k as u64) << POS_BITS,
        opts,
        preserve_originals,
        capture_limit,
        poll,
        metrics: Metrics::default(),
        out: Expanded {
            occs: Vec::new(),
            values: Vec::new(),
            payloads,
            end: Ending::Done,
            warnings: Vec::new(),
        },
        action: 0,
        emitted: 0,
        warned: m.warnings.len(),
    };
    let code = &plan.code;
    for (i, a) in plan.next.iter().enumerate() {
        if let Some(error) = &setup_error {
            sink.out.end = Ending::RetentionError(error.clone());
            break;
        }
        sink.action = i;
        let start = sink.emitted;
        let old = m.enter(a.frame);
        for &(slot, v) in &a.preset {
            if let Some(x) = m.frame.get_mut(m.base + slot as usize) {
                *x = v;
            }
        }
        let r = m.run_action(code, a.g, &mut sink);
        m.leave(old);
        match r {
            Ok(()) => {}
            // The last successor recorded ends the search.
            Err(e) if e.kind() == ErrorKind::Stop => {
                if m.heap.work_interrupted() {
                    sink.out.end = Ending::Budget;
                }
                break;
            }
            Err(error) if sink.out.payloads.is_some() && sink.out.len() as u64 != sink.emitted => {
                sink.out.end = Ending::RetentionError(error);
                break;
            }
            Err(error) => {
                sink.out.end = Ending::GenError {
                    action: i,
                    emitted: sink.emitted - start,
                    error,
                };
                break;
            }
        }
    }
    sink.take_warnings(m);
    m.heap.set_speculative(old_speculative);
    m.heap.reset();
    metrics.conjuncts_total += sink.metrics.conjuncts_total;
    metrics.conjuncts_skipped += sink.metrics.conjuncts_skipped;
    metrics.probed += sink.metrics.probed;
    metrics.discarded_before_intern += sink.metrics.discarded_before_intern;
    sink.out
}

/// Phase 1 over the frontier `cur` (`nvars` handles per state): the
/// expansion of each state in position order, or `None` for a state no
/// worker expanded because the budget ran out first.
///
/// # Errors
/// An internal error if a worker thread failed.
#[allow(clippy::too_many_arguments, reason = "one level's shared inputs")]
pub(crate) fn expand_level(
    plan: &Plan,
    machines: &mut [Machine],
    cur: &[Handle],
    nvars: usize,
    seen: &Seen,
    opts: Opts,
    preserve_originals: bool,
    capture_limit: Option<u64>,
    candidates: Option<(&crate::storage::Storage, &crate::candidate::Account)>,
    poll: &(dyn Fn() -> bool + Sync),
    estimate: usize,
    metrics: &mut Metrics,
) -> Result<Vec<Option<Expanded>>, Error> {
    let n = cur.len() / nvars.max(1);
    let workers = machines.len().max(1);
    if candidates.is_some() && n > workers {
        return Err(Error::internal(
            "candidate expansion exceeds one source per worker",
        ));
    }
    let batch = (n / (workers * 16)).clamp(1, 64);
    let next = AtomicUsize::new(0);
    let halt = AtomicBool::new(false);
    let claims = Claims::new(estimate.saturating_mul(2));
    let results: Vec<Option<WorkerResult>> = std::thread::scope(|scope| {
        let handles: Vec<_> = machines
            .iter_mut()
            .map(|m| {
                let (next, halt, claims) = (&next, &halt, &claims);
                scope.spawn(move || {
                    let mut done = Vec::new();
                    let mut local = Metrics::default();
                    loop {
                        let start = next.fetch_add(batch, Ordering::Relaxed);
                        if start >= n || halt.load(Ordering::Relaxed) {
                            break;
                        }
                        for k in start..(start + batch).min(n) {
                            if poll() {
                                halt.store(true, Ordering::Relaxed);
                                break;
                            }
                            let s0 = &cur[k * nvars..(k + 1) * nvars];
                            done.push((
                                k,
                                expand_one(
                                    m,
                                    plan,
                                    seen,
                                    claims,
                                    k,
                                    opts,
                                    preserve_originals,
                                    capture_limit,
                                    candidates,
                                    poll,
                                    s0,
                                    &mut local,
                                ),
                            ));
                        }
                        if candidates.is_some() {
                            break;
                        }
                    }
                    (done, local)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().ok()).collect()
    });
    let mut out: Vec<Option<Expanded>> = (0..n).map(|_| None).collect();
    for r in results {
        let Some((done, local)) = r else {
            return Err(Error::internal("a core worker thread failed"));
        };
        metrics.conjuncts_total += local.conjuncts_total;
        metrics.conjuncts_skipped += local.conjuncts_skipped;
        metrics.probed += local.probed;
        metrics.discarded_before_intern += local.discarded_before_intern;
        for (k, e) in done {
            if let Some(slot) = out.get_mut(k) {
                *slot = Some(e);
            }
        }
    }
    if !halt.load(Ordering::Relaxed) && out.iter().any(Option::is_none) {
        return Err(Error::internal("a core worker skipped a state"));
    }
    Ok(out)
}

impl crate::Explorer<'_> {
    /// Applies a successor's implied-action verdict, with its values in
    /// `m.s1`: a violation or error ends the search there.
    fn apply_implied(&mut self, m: &mut Machine, imp: &Inv) -> Result<(), Error> {
        match imp {
            Inv::Holds => Ok(()),
            Inv::Violated(i) => {
                let property = self
                    .plan
                    .implied_actions
                    .get(*i)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                self.stop = Some(crate::Verdict::ActionPropertyViolated { property });
                self.end_here(m)?;
                Err(Error::stop())
            }
            Inv::Error(i, error) => {
                let e = self.fail_check(m, Site::ImpliedAction(*i), error.clone());
                if self.failure.is_some() {
                    self.end_here(m)?;
                }
                Err(e)
            }
        }
    }

    /// Records a warning in the one-worker order (first raise wins).
    fn warn(&mut self, w: (u16, u32)) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    /// Phase 2 for one state, already in `m.s0`: the one-worker search's
    /// decisions over the successors phase 1 recorded (`Explorer::accept`).
    ///
    /// # Errors
    /// The stop, or the error that ends the search, as `accept` returns it.
    #[allow(
        clippy::too_many_lines,
        reason = "mirrors accept's ordered decisions, one arm per recorded kind"
    )]
    pub(crate) fn commit(&mut self, m: &mut Machine, res: &mut Expanded) -> Result<(), Error> {
        let plan = self.plan;
        let mut w = 0;
        let mut action_start = 0;
        let mut last_action = None;
        for k in 0..res.len() {
            let decoded = res
                .payloads
                .as_mut()
                .map(|payloads| payloads.read(k, m))
                .transpose()?;
            let (action, occ) = decoded
                .as_ref()
                .or_else(|| res.occs.get(k))
                .ok_or_else(|| Error::internal("missing candidate occurrence"))?;
            if res.payloads.is_none() {
                let at = match occ {
                    Occ::Old { at, .. } => *at,
                    Occ::Cand { at, .. } | Occ::Incomplete { at } | Occ::CheckErr { at, .. } => {
                        Some(*at)
                    }
                    _ => None,
                };
                if let Some(at) = at {
                    let n = m.s1.len();
                    let values = res
                        .values
                        .get(at..at + n)
                        .ok_or_else(|| Error::internal("resident occurrence state width"))?;
                    m.s1.copy_from_slice(values);
                }
            }
            while let Some(&(before, warning)) = res.warnings.get(w)
                && before <= k as u64
            {
                self.warn(warning);
                w += 1;
            }
            if last_action != Some(*action) {
                last_action = Some(*action);
                action_start = self.emitted;
            }
            self.action = Some(*action);
            self.action_start = action_start;
            self.stats.generated += 1;
            self.emitted += 1;
            self.report_emitted()?;
            match occ {
                Occ::Old { key, inside, .. } => {
                    self.metrics.probed += 1;
                    // Already inserted: never refused by a limit.
                    if !self.seen.contains(*key) {
                        return Err(Error::internal(
                            "a duplicate successor whose first occurrence was not inserted",
                        ));
                    }
                    if *inside {
                        self.record(m, *key, false, None)?;
                    }
                }
                Occ::CaptureLimit => {
                    return Err(Error::internal(
                        "graph sink accepted an emission beyond its scratch admission bound",
                    ));
                }
                Occ::CaptureBudget => {
                    self.stop = Some(crate::Verdict::Budget);
                    return Err(Error::stop());
                }
                Occ::Incomplete { .. } => {
                    let message = crate::incomplete_message(plan, m);
                    self.failure = Some(self.failure_at(m, Site::Incomplete, message)?);
                    self.end_here(m)?;
                    return Err(Error::stop());
                }
                Occ::CheckErr { site, error, .. } => {
                    self.in_model = true;
                    let e = self.fail_check(m, *site, error.clone());
                    if self.failure.is_some() {
                        self.end_here(m)?;
                    }
                    return Err(e);
                }
                Occ::Cand {
                    key,
                    identity,
                    in_model,
                    inv,
                    imp,
                    ..
                } => {
                    self.in_model = *in_model;
                    self.changed = changed_vars(m);
                    self.metrics.probed += 1;
                    if *in_model
                        && (self.max_states.is_some() || self.max_depth.is_some())
                        && !self.seen_contains(
                            *key,
                            identity,
                            &m.heap,
                            ("AdmissionMembership", None),
                        )?
                    {
                        self.admit_new()?;
                    }
                    let new = if *in_model {
                        self.seen_insert(*key, identity, &m.heap, ("Insertion", None))?
                    } else {
                        self.plan.symmetry.is_some()
                            || self.plan.view.is_some()
                            || !self.seen_contains(
                                *key,
                                identity,
                                &m.heap,
                                ("AdmissionMembership", None),
                            )?
                    };
                    if !new {
                        self.apply_implied(m, imp)?;
                        if *in_model {
                            self.record(m, *key, false, None)?;
                        }
                        continue;
                    }
                    if *in_model {
                        self.stats.distinct += 1;
                        self.stats.depth = self.stats.depth.max(self.level);
                    }
                    match inv {
                        Inv::Holds => {}
                        Inv::Violated(i) => {
                            let property = plan
                                .invariants
                                .get(*i)
                                .map(|c| c.name.clone())
                                .unwrap_or_default();
                            self.stop = Some(crate::Verdict::InvariantViolated {
                                property,
                                initial: false,
                            });
                            self.end_here(m)?;
                            return Err(Error::stop());
                        }
                        Inv::Error(i, error) => {
                            let e = self.fail_check(m, Site::Invariant(*i), error.clone());
                            if self.failure.is_some() {
                                self.end_here(m)?;
                            }
                            return Err(e);
                        }
                    }
                    self.apply_implied(m, imp)?;
                    if *in_model {
                        self.enqueue(m, None)?;
                        self.record(m, *key, true, None)?;
                    }
                }
            }
        }
        for &(_, warning) in res.warnings.get(w..).unwrap_or_default() {
            self.warn(warning);
        }
        match &res.end {
            Ending::Done => Ok(()),
            Ending::RetentionError(error) => Err(error.clone()),
            Ending::Budget => {
                self.stop = Some(crate::Verdict::Budget);
                Err(Error::stop())
            }
            Ending::GenError {
                action,
                emitted,
                error,
            } => {
                self.action = Some(*action);
                self.action_start = self.emitted - emitted;
                Err(error.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Claims;

    /// A claim may report a duplicate only when an earlier position
    /// claimed the key, also when the table is too crowded to place keys.
    #[test]
    fn claims_never_invent_a_duplicate() {
        let claims = Claims::new(0);
        let mut first: HashMap<u64, u64> = HashMap::new();
        let mut placed_out = 0;
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for pos in 0..60_000u64 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            // Few distinct keys repeat, so most probes meet a crowded table.
            let key = if pos % 3 == 0 { x % 512 } else { x };
            let holds = claims.claim(key, pos);
            // The seen set's key: 0 and 1 are one key there too.
            let earlier = first.get(&key.max(1)).copied();
            if !holds {
                assert!(earlier.is_some_and(|q| q < pos), "key {key} at {pos}");
            }
            if claims.held_before(key, pos) {
                assert!(earlier.is_some_and(|q| q < pos), "held {key} at {pos}");
            }
            if holds && earlier.is_some() {
                placed_out += 1;
            }
            first.entry(key.max(1)).or_insert(pos);
        }
        // The table (4096 slots) cannot place most of the 40,000 distinct
        // keys, so some repeated keys fall back to "possibly new".
        assert!(first.len() > 4096 && placed_out > 0, "{placed_out}");
    }
    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "actual high-fanout expansion and ordered consumption control"
    )]
    fn candidate_high_fanout_spills_owned_payloads_before_any_store_promotion() {
        use crate::storage::{CandidateUsage, Storage};
        use dive_core::memo::Memo;
        use dive_core::plan::{Bind, GNode, Node};
        use dive_core::{Handle, Machine, Opts, Store};
        use std::sync::{Arc, Mutex};
        for count in [256, 2048] {
            let store = Arc::new(Store::new(true));
            let mut plan = crate::tests::frontier_plan(&store, false).unwrap();
            let mut heap = dive_core::Heap::new(Arc::clone(&store), false, false);
            let domain = heap.interval(0, count - 1);
            let domain = heap.intern_const(domain).unwrap();
            plan.code.nodes = vec![
                Node::Const(domain),
                Node::Slot(0),
                Node::Tuple(vec![1; 128].into()),
                Node::Const(Handle::TRUE),
            ];
            plan.code.gnodes = vec![
                GNode::Assign { var: 0, rhs: 2 },
                GNode::Exists {
                    binds: vec![Bind {
                        slots: vec![0].into(),
                        tuple: false,
                        domain: 0,
                    }]
                    .into(),
                    body: 0,
                    body_p: 3,
                },
            ];
            plan.next[0].g = 1;
            plan.next[0].frame = 1;
            let opts = Opts::default();
            let mut machine = Machine::new(Arc::clone(&store), 1, opts, Memo::new(&[]));
            let storage = Storage {
                candidate_memory_bytes: Some(8192),
                ..Storage::default()
            };
            let account = Arc::new(Mutex::new(CandidateUsage {
                memory_bytes_per_worker: Some(8192),
                ..CandidateUsage::default()
            }));
            let seen = crate::Seen::default();
            let claims = super::Claims::new(0);
            let before = store.stats();
            let mut metrics = crate::Metrics::default();
            let mut out = super::expand_one(
                &mut machine,
                &plan,
                &seen,
                &claims,
                0,
                opts,
                false,
                None,
                Some((&storage, &account)),
                &|| false,
                &[Handle::FALSE],
                &mut metrics,
            );
            assert!(matches!(out.end, super::Ending::Done), "{out:?}");
            assert_eq!(out.len(), usize::try_from(count).unwrap());
            assert!(
                out.occs.is_empty() && out.values.is_empty(),
                "metadata and payload both belong to encoded queue"
            );
            assert_eq!(
                store.stats().nodes,
                before.nodes,
                "emission cannot promote candidate compounds"
            );
            assert_eq!(store.stats().handles, before.handles);
            let usage = account.lock().unwrap().clone();
            assert!(usage.peak_retained_bytes <= 8192);
            assert!(usage.spilled_bytes > 100_000);
            let payloads = out.payloads.as_mut().unwrap();
            for index in 0..4 {
                let (_, occ) = payloads.read(index, &mut machine).unwrap();
                assert!(matches!(occ, super::Occ::Cand { .. }));
                let expected = Handle::int(i64::try_from(index).unwrap()).unwrap();
                assert_eq!(machine.heap.view(machine.s1[0]).data, [expected; 128]);
                assert_eq!(
                    store.stats().nodes - before.nodes,
                    index as u64 + 1,
                    "Store follows ordered consumption only"
                );
            }
            drop(out);
            assert_eq!(account.lock().unwrap().retained_bytes, 0);
            assert_eq!(account.lock().unwrap().failures, [] as [String; 0]);
        }
    }
}
