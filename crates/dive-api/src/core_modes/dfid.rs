//! Native DFID visitation/status algorithm adapted from the reference driver.
//! TLC provenance is the same pinned DFIDModelChecker/DFIDWorker contract;
//! the execution substrate is the existing compiled core, not the evaluator.

use std::collections::BTreeMap;

use dive_check::dfid::{Options, Outcome, Report};
use dive_core::{Error, ErrorKind, Handle};
use dive_stdlib::random::{JavaRandom, RandomSource, fresh_seed};

use super::{Native, allowed, assumptions, generate, roles, values};

type State = Vec<Handle>;
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Exact(State),
    Fingerprint(u64),
}
struct Entry {
    status: u32,
    exact: State,
}
struct Frame {
    key: Key,
    successors: Vec<(State, Key)>,
}
struct Search<'a> {
    native: &'a mut Native,
    model: &'a dive_model::Model,
    options: &'a Options,
    report: Report,
    seen: BTreeMap<Key, Entry>,
    exact_index: BTreeMap<u64, Vec<State>>,
    roots: Vec<(State, Key)>,
    level: u32,
    phase: u32,
    more: bool,
}

pub(crate) fn empty(outcome: Outcome) -> Report {
    Report {
        stage: "preflight",
        outcome,
        generated: 0,
        initial_candidates: 0,
        distinct: 0,
        level: 0,
        worker_seeds: Vec::new(),
        trace: Vec::new(),
        tlc_completion_message: false,
        deciding: None,
    }
}
fn outcome(value: dive_sim::Outcome) -> Outcome {
    match value {
        dive_sim::Outcome::Violation {
            kind, name, code, ..
        } => Outcome::Violation { kind, name, code },
        dive_sim::Outcome::Error { code, message } => Outcome::Error { code, message },
        dive_sim::Outcome::Unsupported(message) => Outcome::Unsupported(message),
        dive_sim::Outcome::Internal(message) => Outcome::Internal(message),
        dive_sim::Outcome::Limit(message) => Outcome::Limit(message),
        dive_sim::Outcome::Cancelled => Outcome::Cancelled,
        dive_sim::Outcome::Sampled => Outcome::Internal("sampled result in DFID".into()),
    }
}
fn classify(error: &Error, wrapper: Option<u32>) -> Outcome {
    outcome(super::classify(error, wrapper))
}
// The same closed-state admission used by the BFS core: symbolic infinite
// values cannot acquire a completed finite-state certificate merely by hashing.
fn fingerprintable(heap: &dive_core::Heap, value: Handle) -> bool {
    if !value.is_compound() {
        return true;
    }
    let node = heap.view(value);
    match node.kind {
        dive_core::node::Kind::Set | dive_core::node::Kind::IntFcn | dive_core::node::Kind::Fcn => {
            node.data.iter().all(|&value| fingerprintable(heap, value))
        }
        dive_core::node::Kind::BigInt | dive_core::node::Kind::Interval => true,
        _ => false,
    }
}

// A candidate batch stays live through its actual constraint/key demand.
// Only rows retained by the DFS are closed afterwards; all other transients
// are released when that batch ends.
fn live_row(machine: &dive_core::Machine) -> Result<State, Error> {
    fn closed(heap: &dive_core::Heap, value: Handle) -> bool {
        if !value.is_compound() {
            return true;
        }
        let node = heap.view(value);
        !node.is_lazy() && node.data.iter().all(|&value| closed(heap, value))
    }
    if machine
        .s1
        .iter()
        .any(|&value| !closed(&machine.heap, value))
    {
        return Err(Error::unsupported(
            "native DFID retains an unclosed lazy value",
        ));
    }
    let mut row = Vec::new();
    row.try_reserve_exact(machine.s1.len())
        .map_err(|_| Error::unsupported("native DFID candidate allocation"))?;
    row.extend_from_slice(&machine.s1);
    Ok(row)
}

fn equal(heap: &dive_core::Heap, left: &[Handle], right: &[Handle]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(&left, &right)| heap.eq(left, right))
}
fn complete(state: &[Handle], initial: bool) -> Result<(), Outcome> {
    if state.iter().any(|value| value.is_empty()) {
        Err(Outcome::Error {
            code: if initial { 2106 } else { 2109 },
            message: "incomplete DFID state".into(),
        })
    } else {
        Ok(())
    }
}

impl Native {
    pub(crate) fn dfid(&mut self, model: &dive_model::Model, options: &Options) -> Report {
        if let Some(result) = dive_check::dfid::preflight(model, options) {
            return empty(result);
        }
        let mut search = Search {
            native: self,
            model,
            options,
            report: empty(Outcome::Incomplete),
            seen: BTreeMap::new(),
            exact_index: BTreeMap::new(),
            roots: Vec::new(),
            level: 1,
            phase: 0,
            more: false,
        };
        let result = search.run().unwrap_or_else(|result| result);
        search.report.distinct = search.seen.len();
        search.report.outcome = if search.native.machine.heap.poisoned() {
            Outcome::Internal("native DFID read a stale value".into())
        } else {
            result
        };
        search.report
    }
}

impl Search<'_> {
    fn checkpoint(&self) -> Result<(), Outcome> {
        match self.options.check.budget.poll() {
            Ok(None) => Ok(()),
            Ok(Some(_)) => Err(Outcome::Cancelled),
            Err(error) => Err(Outcome::Internal(error.to_string())),
        }
    }
    fn run(&mut self) -> Result<Outcome, Outcome> {
        self.checkpoint()?;
        self.report.stage = "assumptions";
        if let Some(failure) = assumptions(self.model) {
            return Err(outcome(failure));
        }
        let actual = Native::prepare_with_options(
            self.model,
            &self.options.check.budget,
            self.native.machine.opts,
        )
        .map_err(super::SetupError::dfid)?;
        *self.native = actual;
        self.report.stage = "initialization";
        self.initialize()?;
        if self.native.plan.next.is_empty() {
            self.report.tlc_completion_message = true;
            return Ok(Outcome::Complete);
        }
        self.report.stage = "exploration";
        let mut seeds = JavaRandom::new(self.options.check.seed);
        for level in 2..=self.options.max_level {
            self.checkpoint()?;
            self.level = level;
            self.phase = 2 - self.phase;
            self.report.level = level;
            let seed = if let Some(explicit) = &self.options.worker_seeds {
                *explicit
                    .get(self.report.worker_seeds.len())
                    .ok_or_else(|| {
                        Outcome::Limit("DFID worker seed replay list exhausted".into())
                    })?
            } else if self.options.check.profile == dive_eval::Profile::Tlc {
                fresh_seed()
            } else {
                seeds.next_long()
            };
            self.report.worker_seeds.push(seed);
            let more = self.iteration(&mut JavaRandom::new(seed))?;
            if let Some(failure) = self.report.deciding.clone() {
                return Err(failure);
            }
            if !more {
                self.report.tlc_completion_message = level < self.options.max_level;
                self.report.stage = "complete";
                return Ok(Outcome::Complete);
            }
        }
        self.report.stage = "depth-bound";
        Ok(Outcome::Incomplete)
    }

    fn initialize(&mut self) -> Result<(), Outcome> {
        let mark = self.native.machine.heap.mark();
        let mut initial = Vec::new();
        let native = &mut *self.native;
        for action in &native.plan.init {
            let mut limit = false;
            let result = native.machine.with_deferred_reclamation(|machine| {
                generate(machine, &native.plan.code, action, &mut |machine| {
                    if self.options.check.budget.poll() != Ok(None) {
                        return Err(Error::stop());
                    }
                    if initial.len() >= self.options.max_states
                        || self.report.initial_candidates >= self.options.max_generated
                    {
                        limit = true;
                        return Err(Error::stop());
                    }
                    self.report.initial_candidates += 1;
                    initial.push(live_row(machine)?);
                    Ok(())
                })
            });
            if let Err(error) = result
                && error.kind() != ErrorKind::Stop
            {
                return Err(classify(&error, Some(2103)));
            }
            if limit {
                return Err(Outcome::Limit("DFID initial-state bound".into()));
            }
        }
        self.checkpoint()?;
        self.report.generated = self.report.initial_candidates;
        for mut state in initial {
            self.checkpoint()?;
            complete(&state, true)?;
            self.native.machine.s1.clone_from(&state);
            let in_model = allowed(
                &mut self.native.machine,
                &self.native.plan,
                &self.native.plan.constraints,
                false,
            )
            .map_err(|error| classify(&error, None))?;
            let old = if in_model {
                let (key, exact) = self.key(&mut state)?;
                let old = self.insert(&key, exact)?;
                if old == 0 {
                    state = super::retain(&mut self.native.machine)
                        .map_err(|error| classify(&error, None))?;
                    self.native.machine.s1.clone_from(&state);
                    self.roots.push((state.clone(), key));
                }
                old
            } else {
                0
            };
            if old == 0
                && let Some(failure) = self.properties(true, false)
            {
                return self.witness(std::slice::from_ref(&state), failure);
            }
        }
        self.finish_batch(mark)?;
        Ok(())
    }

    fn finish_batch(&mut self, mark: dive_core::arena::Mark) -> Result<(), Outcome> {
        super::finish_batch(&mut self.native.machine, mark).map_err(|error| classify(&error, None))
    }

    fn key(&mut self, state: &mut State) -> Result<(Key, State), Outcome> {
        let mut exact = Vec::new();
        exact
            .try_reserve_exact(state.len())
            .map_err(|_| Outcome::Limit("native DFID exact-key allocation".into()))?;
        let mut hash = 0u64;
        for (index, value) in state.iter_mut().enumerate() {
            let identity = self
                .native
                .machine
                .heap
                .intern(*value)
                .map_err(|error| classify(&error, None))?;
            if self.native.machine.heap.has_construction_history(*value) {
                let normalized = self
                    .native
                    .machine
                    .heap
                    .normalize_construction(*value)
                    .map_err(|error| classify(&error, None))?;
                *value = normalized;
            }
            if !fingerprintable(&self.native.machine.heap, identity) {
                return Err(Outcome::Unsupported(
                    "native DFID state value cannot be fingerprinted".into(),
                ));
            }
            hash = hash
                .wrapping_add(
                    self.native
                        .machine
                        .heap
                        .deep_hash(identity)
                        .rotate_left(u32::try_from(index % 64).unwrap_or(0)),
                )
                .wrapping_mul(0x9e37_79b9_7f4a_7c15);
            exact.push(identity);
        }
        self.native.machine.s1.clone_from(state);
        let key = if self.options.check.store == dive_store::StoreMode::Exact {
            let bucket = self.exact_index.entry(hash).or_default();
            if let Some(existing) = bucket
                .iter()
                .find(|existing| equal(&self.native.machine.heap, existing, &exact))
            {
                Key::Exact(existing.clone())
            } else {
                bucket.push(exact.clone());
                Key::Exact(exact.clone())
            }
        } else {
            Key::Fingerprint(hash & (u64::MAX >> (64 - self.options.fingerprint_bits)))
        };
        Ok((key, exact))
    }
    fn insert(&mut self, key: &Key, exact: State) -> Result<u32, Outcome> {
        if let Some(entry) = self.seen.get(key) {
            if !equal(&self.native.machine.heap, &entry.exact, &exact) {
                return Err(Outcome::Limit(
                    "DFID fingerprint collision detected by exact guard".into(),
                ));
            }
            return Ok(entry.status);
        }
        if self.seen.len() >= self.options.max_states {
            return Err(Outcome::Limit("DFID distinct-state bound".into()));
        }
        self.seen.insert(
            key.clone(),
            Entry {
                status: (self.level << 2) | self.phase,
                exact,
            },
        );
        Ok(0)
    }
    fn status(&self, key: &Key) -> Result<u32, Outcome> {
        self.seen.get(key).map(|entry| entry.status).ok_or_else(|| {
            Outcome::Internal("native DFID successor missing visitation status".into())
        })
    }
    fn completed(&self, status: u32) -> bool {
        status & 1 != 0 || status & 2 == self.phase
    }
    fn leaf(&self, status: u32) -> bool {
        status == 0 || status >> 2 == self.level
    }
    fn mark(&mut self, key: &Key, done: bool) -> Result<(), Outcome> {
        let entry = self.seen.get_mut(key).ok_or_else(|| {
            Outcome::Internal("native DFID missing state while marking completion".into())
        })?;
        if done {
            entry.status |= 1;
        } else {
            entry.status = (entry.status & !2) | self.phase;
        }
        Ok(())
    }
    fn properties(&mut self, initial: bool, actions_only: bool) -> Option<Outcome> {
        let Native { machine, plan, .. } = &mut *self.native;
        let invariant = if actions_only {
            None
        } else {
            roles(
                machine,
                plan,
                &plan.invariants,
                false,
                "invariant",
                if initial { 2107 } else { 2110 },
                if initial { 2102 } else { 2111 },
            )
        };
        invariant
            .or_else(|| {
                if initial {
                    roles(
                        machine,
                        plan,
                        &plan.implied_inits,
                        false,
                        "property",
                        2108,
                        2102,
                    )
                } else {
                    roles(
                        machine,
                        plan,
                        &plan.implied_actions,
                        true,
                        "property",
                        2112,
                        2113,
                    )
                }
            })
            .map(outcome)
    }
    fn keep_witness(&mut self, path: &[State]) -> Result<(), Outcome> {
        if path.len() > self.options.max_trace {
            return Err(Outcome::Limit(
                "DFID violation observed but complete witness exceeds bound".into(),
            ));
        }
        let mut trace = Vec::new();
        for state in path {
            self.checkpoint()?;
            trace.push(
                values(&mut self.native.machine, self.model, state)
                    .map_err(Outcome::Unsupported)?,
            );
        }
        self.report.trace = trace;
        Ok(())
    }
    fn witness(&mut self, path: &[State], failure: Outcome) -> Result<(), Outcome> {
        self.report.deciding = Some(failure.clone());
        if matches!(failure, Outcome::Violation { .. }) {
            self.keep_witness(path)?;
        }
        Err(failure)
    }
    fn record(&mut self, path: &[State], failure: Outcome) -> Result<(), Outcome> {
        if !matches!(failure, Outcome::Violation { .. } | Outcome::Error { .. }) {
            return Err(failure);
        }
        if self.report.deciding.is_none() {
            self.report.deciding = Some(failure.clone());
            if matches!(failure, Outcome::Violation { .. }) {
                self.keep_witness(path)?;
            }
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "retain DFID per-action eager generation and first deciding-event order"
    )]
    fn expand(
        &mut self,
        state: &State,
        key: &Key,
        leaf: bool,
        path: &[State],
        candidate_budget: usize,
    ) -> Result<Vec<(State, Key)>, Outcome> {
        let mut successors = Vec::new();
        let mut all_done = true;
        let mut all_nonleaf = true;
        let mut deadlock = true;
        for index in 0..self.native.plan.next.len() {
            self.checkpoint()?;
            self.native.machine.s0.clone_from(state);
            let mark = self.native.machine.heap.mark();
            let mut vector = Vec::new();
            let mut limit = false;
            let native = &mut *self.native;
            let result = native.machine.with_deferred_reclamation(|machine| {
                generate(
                    machine,
                    &native.plan.code,
                    &native.plan.next[index],
                    &mut |machine| {
                        if self.options.check.budget.poll() != Ok(None) {
                            return Err(Error::stop());
                        }
                        if vector.len() >= self.options.max_states
                            || vector.len() >= candidate_budget.saturating_sub(successors.len())
                            || self.report.generated.saturating_add(vector.len() as u64)
                                >= self.options.max_generated
                        {
                            limit = true;
                            return Err(Error::stop());
                        }
                        vector.push(live_row(machine)?);
                        Ok(())
                    },
                )
            });
            if let Err(error) = result
                && error.kind() != ErrorKind::Stop
            {
                return Err(classify(&error, None));
            }
            self.checkpoint()?;
            if limit {
                return Err(Outcome::Limit("DFID successor vector bound".into()));
            }
            self.report.generated += vector.len() as u64;
            deadlock &= vector.is_empty();
            for mut next in vector {
                if let Err(failure) = complete(&next, false) {
                    self.record(path, failure)?;
                    self.more |= !all_nonleaf;
                    self.finish_batch(mark)?;
                    return Ok(successors);
                }
                self.native.machine.s1.clone_from(&next);
                let native = &mut *self.native;
                let in_model = allowed(
                    &mut native.machine,
                    &native.plan,
                    &native.plan.constraints,
                    false,
                )
                .and_then(|yes| {
                    if yes {
                        allowed(
                            &mut native.machine,
                            &native.plan,
                            &native.plan.action_constraints,
                            true,
                        )
                    } else {
                        Ok(false)
                    }
                })
                .map_err(|error| classify(&error, None))?;
                let old = if in_model {
                    let (next_key, exact) = self.key(&mut next)?;
                    let old = self.insert(&next_key, exact)?;
                    all_done &= old & 1 != 0;
                    all_nonleaf &= !self.leaf(old);
                    if !self.completed(old) {
                        next = super::retain(&mut self.native.machine)
                            .map_err(|error| classify(&error, None))?;
                        self.native.machine.s1.clone_from(&next);
                        successors.push((next.clone(), next_key));
                    }
                    old
                } else {
                    0
                };
                if let Some(failure) = self.properties(false, old != 0) {
                    let mut trace = path.to_vec();
                    trace.push(next);
                    self.record(&trace, failure)?;
                    self.more |= !all_nonleaf;
                    self.finish_batch(mark)?;
                    return Ok(successors);
                }
            }
            self.finish_batch(mark)?;
        }
        if deadlock && self.native.plan.check_deadlock && !self.options.check.no_deadlock {
            self.record(
                path,
                Outcome::Violation {
                    kind: "deadlock",
                    name: None,
                    code: 2114,
                },
            )?;
            self.more |= !all_nonleaf;
            return Ok(successors);
        }
        if all_done || (leaf && all_nonleaf) {
            self.mark(key, true)?;
        }
        if !all_nonleaf {
            self.more = true;
        }
        Ok(successors)
    }

    fn iteration(&mut self, rng: &mut JavaRandom) -> Result<bool, Outcome> {
        let mut roots = self.roots.clone();
        self.more = false;
        while !roots.is_empty() && self.report.deciding.is_none() {
            self.checkpoint()?;
            let index = usize::try_from(rng.next_index(roots.len() as u64)).unwrap_or(0);
            let (state, key) = &roots[index];
            if self.completed(self.status(key)?) {
                roots.swap_remove(index);
                continue;
            }
            let state = state.clone();
            let key = key.clone();
            if self.options.max_trace == 0 {
                return Err(Outcome::Limit("DFID traversal witness bound".into()));
            }
            let mut path = vec![state.clone()];
            let candidates = self.expand(
                &state,
                &key,
                false,
                &path,
                self.options.max_retained_candidates,
            )?;
            let mut stack = vec![Frame {
                key,
                successors: candidates,
            }];
            loop {
                self.checkpoint()?;
                let depth = stack.len();
                let depth_level = u32::try_from(depth).map_err(|_| {
                    Outcome::Internal("DFID depth exceeds status representation".into())
                })?;
                let frame = &mut stack[depth - 1];
                let mut selected = None;
                while !frame.successors.is_empty() {
                    let index =
                        usize::try_from(rng.next_index(frame.successors.len() as u64)).unwrap_or(0);
                    let (state, key) = &frame.successors[index];
                    let status = self.status(key)?;
                    if !self.completed(status) && depth_level < status >> 2 {
                        selected = Some((state.clone(), key.clone()));
                        break;
                    }
                    frame.successors.swap_remove(index);
                }
                if let Some((state, key)) = selected {
                    if path.len() >= self.options.max_trace {
                        return Err(Outcome::Limit("DFID traversal witness bound".into()));
                    }
                    path.push(state.clone());
                    let leaf = depth_level >= self.level - 1;
                    let retained = stack
                        .iter()
                        .try_fold(0usize, |count, frame| {
                            count.checked_add(frame.successors.len())
                        })
                        .ok_or_else(|| {
                            Outcome::Limit("DFID retained candidate size overflow".into())
                        })?;
                    let candidates = self.expand(
                        &state,
                        &key,
                        leaf,
                        &path,
                        self.options
                            .max_retained_candidates
                            .saturating_sub(retained),
                    )?;
                    if leaf {
                        break;
                    }
                    stack.push(Frame {
                        key,
                        successors: candidates,
                    });
                } else {
                    self.mark(&frame.key, false)?;
                    if depth == 1 {
                        break;
                    }
                    stack.pop();
                    path.pop();
                }
            }
        }
        Ok(self.more)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{SpecModel, TraceOptions};
    use dive_core::Opts;
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    #[test]
    fn exact_and_guarded_fingerprints_preserve_equal_aliases_without_hashcons() -> Result {
        let model = SpecModel::from_source(
            "Aliases.tla",
            "---- MODULE Aliases ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = <<1>>\nNext == x' = [i \\in {1} |-> 1]\n====\n",
            "INIT Init\nNEXT Next\n",
            &TraceOptions::default(),
        )?;
        for store in [
            dive_store::StoreMode::Exact,
            dive_store::StoreMode::Fingerprint,
        ] {
            let mut options = Options::new(dive_check::CheckOptions::dive(7), 6);
            options.check.store = store;
            options.worker_seeds = Some(vec![0; 6]);
            let mut native = Native::prepare_with_options(
                model.model(),
                &options.check.budget,
                Opts {
                    hashcons: false,
                    ..Opts::default()
                },
            )
            .map_err(|_| "native preparation")?;
            let actual = native.dfid(model.model(), &options);
            let reference = dive_check::dfid::check(model.model(), &options);
            assert_eq!(actual.outcome, reference.outcome);
            assert_eq!(actual.distinct, reference.distinct);
            assert_eq!(actual.generated, reference.generated);
            assert_eq!(actual.level, reference.level);
            assert_eq!(actual.outcome, Outcome::Complete);
        }
        Ok(())
    }

    #[test]
    fn forced_native_fingerprint_collision_refuses_instead_of_certifying_completion() -> Result {
        let model = SpecModel::from_source(
            "Collision.tla",
            "---- MODULE Collision ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x < 8 /\\ x' = x + 1\n====\n",
            "INIT Init\nNEXT Next\n",
            &TraceOptions::default(),
        )?;
        let mut options = Options::new(dive_check::CheckOptions::dive(7), 10);
        options.fingerprint_bits = 1;
        options.worker_seeds = Some(vec![0; 10]);
        let mut native = Native::prepare(model.model(), &options.check.budget)
            .map_err(|_| "native preparation")?;
        let actual = native.dfid(model.model(), &options);
        assert!(
            matches!(actual.outcome, Outcome::Limit(message) if message.contains("fingerprint collision"))
        );
        Ok(())
    }

    #[test]
    fn enumerated_compound_rows_survive_continuations_without_hashcons() -> Result {
        for (name, generated, level) in [("NextEnumerated", 13, 3), ("InitEnumerated", 6, 2)] {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/core-finite-dfid");
            for compat_tlc in [false, true] {
                let model = SpecModel::load(
                    &root.join(format!("{name}.tla")),
                    &root.join(format!("{name}.cfg")),
                    &TraceOptions {
                        compat_tlc,
                        ..TraceOptions::default()
                    },
                )?;
                for store in [
                    dive_store::StoreMode::Exact,
                    dive_store::StoreMode::Fingerprint,
                ] {
                    let check = if compat_tlc {
                        dive_check::CheckOptions::tlc(0, 7)
                    } else {
                        dive_check::CheckOptions::dive(7)
                    };
                    let mut options = Options::new(check, 6);
                    options.check.store = store;
                    options.worker_seeds = Some(vec![0; 6]);
                    let mut native = Native::prepare_with_options(
                        model.model(),
                        &options.check.budget,
                        Opts {
                            hashcons: false,
                            ..Opts::default()
                        },
                    )
                    .map_err(|_| "native preparation")?;
                    let actual = native.dfid(model.model(), &options);
                    assert_eq!(actual.outcome, Outcome::Complete, "{name}: {actual:?}");
                    assert_eq!(
                        (actual.distinct, actual.generated, actual.level),
                        (3, generated, level)
                    );
                    assert_eq!(native.machine.heap.arena().len(), 0);
                    assert!(!native.machine.heap.poisoned());
                }
            }
        }
        Ok(())
    }
    #[test]
    fn repeated_compound_iterations_release_every_completed_candidate_batch() -> Result {
        let model = SpecModel::from_source(
            "Repeated.tla",
            "---- MODULE Repeated ----\nEXTENDS Naturals\nVARIABLES x,s\nInit == x = 0 /\\ s = {0}\nNext == x' \\in {x + 1, x + 2} /\\ s' = {x'}\n====\n",
            "INIT Init\nNEXT Next\n",
            &TraceOptions::default(),
        )?;
        let mut observations = Vec::new();
        for level in [4, 24] {
            let mut options = Options::new(dive_check::CheckOptions::dive(7), level);
            options.worker_seeds = Some(vec![0; level as usize]);
            let mut native = Native::prepare(model.model(), &options.check.budget)
                .map_err(|_| "native preparation")?;
            let report = native.dfid(model.model(), &options);
            assert_eq!(report.outcome, Outcome::Incomplete);
            assert_eq!(native.machine.heap.arena().len(), 0);
            assert!(!native.machine.heap.poisoned());
            observations.push((report.generated, native.machine.heap.arena().peak()));
        }
        assert!(observations[1].0 > observations[0].0 * 10);
        assert!(observations[1].1.0 <= observations[0].1.0 + 8);
        assert!(observations[1].1.1 <= observations[0].1.1 + 64);
        Ok(())
    }

    #[test]
    fn infinite_state_values_cannot_gain_a_native_completion_certificate() -> Result {
        let model = SpecModel::from_source(
            "InfiniteValue.tla",
            "---- MODULE InfiniteValue ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = Nat\nNext == UNCHANGED x\n====\n",
            "INIT Init\nNEXT Next\n",
            &TraceOptions::default(),
        )?;
        for store in [
            dive_store::StoreMode::Exact,
            dive_store::StoreMode::Fingerprint,
        ] {
            let mut options = Options::new(dive_check::CheckOptions::dive(7), 8);
            options.check.store = store;
            options.worker_seeds = Some(vec![0; 8]);
            let mut native = Native::prepare(model.model(), &options.check.budget)
                .map_err(|_| "native preparation")?;
            let report = native.dfid(model.model(), &options);
            assert!(matches!(report.outcome, Outcome::Unsupported(_)));
        }
        Ok(())
    }
}
