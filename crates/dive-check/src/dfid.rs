//! Native bounded depth-first iterative deepening (ADR 0021).
//!
//! Ports `DFIDModelChecker`, `DFIDWorker`, and `FPIntSet` at the pinned
//! tlaplus revision. Completion is an iteration certificate; a depth-bound
//! stop is explicitly incomplete even when TLC returns zero.

mod pure;

use std::collections::BTreeMap;

use dive_eval::next::{Flow, StateVals};
use dive_eval::{AssumptionOutcome, ErrorClass, EvalError, St};
use dive_model::{Model, Named};
use dive_stdlib::random::{JavaRandom, RandomSource, fresh_seed};
use dive_store::StoreMode;
use dive_store::state::{StateFingerprinter, state_key};
use dive_value::Value;

use crate::CheckOptions;

/// A bounded DFID request. Resource limits never imply a model verdict.
#[derive(Clone, Debug)]
pub struct Options {
    /// Shared evaluation, fingerprint, deadlock, and profile settings.
    pub check: CheckOptions,
    /// Maximum iterative level, matching TLC's `-dfid N`.
    pub max_level: u32,
    /// Explicit worker seeds, one per iteration. This replay override is
    /// labelled mixed in TLC mode because TLC does not seed workers from CLI.
    pub worker_seeds: Option<Vec<i64>>,
    /// Maximum retained distinct states and candidate states per action.
    pub max_states: usize,
    /// Maximum eager/candidate states across the retained DFS frame stack.
    pub max_retained_candidates: usize,
    /// Maximum total generated states across all iterations.
    pub max_generated: u64,
    /// Maximum retained witness states.
    pub max_trace: usize,
    /// Restricts fingerprint bits for collision controls. Normal runs use 64.
    pub fingerprint_bits: u32,
}

impl Options {
    /// Shared checker settings with explicit iterative depth.
    #[must_use]
    pub const fn new(check: CheckOptions, max_level: u32) -> Self {
        Self {
            check,
            max_level,
            worker_seeds: None,
            max_states: 1_000_000,
            max_retained_candidates: 1_000_000,
            max_generated: 10_000_000,
            max_trace: 1_000_000,
            fingerprint_bits: 64,
        }
    }
}

/// A deciding result. Only `Complete` certifies full pure safety exploration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Visitation completion certificate, with all configured checks passing.
    Complete,
    /// Maximum iterative depth reached before a completion certificate.
    Incomplete,
    /// A reachable safety violation, pending independent certification.
    Violation {
        /// Invariant, property, or deadlock.
        kind: &'static str,
        /// Configured property name.
        name: Option<String>,
        /// Pinned TLC deciding code.
        code: u32,
    },
    /// A measured evaluation/configuration failure.
    Error {
        /// Pinned TLC deciding code.
        code: u32,
        /// Diagnostic detail.
        message: String,
    },
    /// Unimplemented mode interaction, explicitly named.
    Unsupported(String),
    /// Resource or collision bound reached, without a conclusion.
    Limit(String),
    /// Internal invariant failure, without a conclusion.
    Internal(String),
}

/// Run evidence, including partial initialization and actual worker streams.
#[derive(Clone, Debug)]
pub struct Report {
    /// Last reached phase.
    pub stage: &'static str,
    /// Result; a violation still requires independent validation.
    pub outcome: Outcome,
    /// TLC's generated count (eager vectors included).
    pub generated: u64,
    /// Initial enumeration prefix, including failed initialization.
    pub initial_candidates: u64,
    /// Distinct constrained states retained by the visitation map.
    pub distinct: usize,
    /// Last iterative level started, or zero before any iteration.
    pub level: u32,
    /// Actual selection seed used in every started iteration.
    pub worker_seeds: Vec<i64>,
    /// Concrete witness; never reproduces TLC's stale cross-root stack bug.
    pub trace: Vec<StateVals>,
    /// Whether TLC would emit its completion message at this depth bound.
    pub tlc_completion_message: bool,
    /// First deciding failure, retained if a later resource bound interrupts draining.
    pub deciding: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Fingerprint(u64),
    Exact(Vec<u8>),
}

struct Entry {
    status: u32,
    encoded: Vec<u8>,
}
struct Frame {
    key: Key,
    successors: Vec<(StateVals, Key)>,
}
struct Search<'a> {
    model: &'a Model,
    opts: &'a Options,
    report: Report,
    seen: BTreeMap<Key, Entry>,
    roots: Vec<(StateVals, Key)>,
    level: u32,
    phase: u32,
    more_level: bool,
    fp: StateFingerprinter,
}

/// Runs DFID inline on a bound model. The caller must provide a thread with
/// the evaluator's recommended stack. Independent witness validation belongs to `dive-trace`, not here.
#[must_use]
pub fn check(model: &Model, opts: &Options) -> Report {
    let mut report = Report {
        stage: "preflight",
        outcome: Outcome::Incomplete,
        generated: 0,
        initial_candidates: 0,
        distinct: 0,
        level: 0,
        worker_seeds: Vec::new(),
        trace: Vec::new(),
        tlc_completion_message: false,
        deciding: None,
    };
    if let Some(outcome) = preflight(model, opts) {
        report.outcome = outcome;
        return report;
    }
    // AbstractChecker creates its eager config before checking assumptions;
    // dynamic ToString/CHOOSE can observe these tokens without TLCGet.
    model.evaluator.intern_checker_names();
    let config = crate::engine::run_config(&opts.check, model.settings.check_deadlock);
    model.evaluator.intern_bfs_config(&config);
    let order: Vec<_> = model.ir.tlc_var_order().iter().map(|v| v.index()).collect();
    let fp = if opts.check.tlc_fingerprints {
        StateFingerprinter::tlc(opts.check.fp_index, &order)
    } else {
        StateFingerprinter::new(opts.check.fp_index)
    };
    let Some(fp) = fp else {
        report.outcome = Outcome::Unsupported("DFID fingerprint polynomial index".to_owned());
        return report;
    };
    let mut search = Search {
        model,
        opts,
        report,
        seen: BTreeMap::new(),
        roots: Vec::new(),
        level: 1,
        phase: 0,
        more_level: false,
        fp,
    };
    let outcome = search.run().unwrap_or_else(|o| o);
    search.report.distinct = search.seen.len();
    search.report.outcome = outcome;
    search.report
}

impl Search<'_> {
    fn run(&mut self) -> Result<Outcome, Outcome> {
        self.report.stage = "assumptions";
        match self.model.check_assumptions() {
            AssumptionOutcome::AllTrue => {}
            AssumptionOutcome::False { message, .. } => {
                return Err(Outcome::Error {
                    code: 2104,
                    message,
                });
            }
            AssumptionOutcome::Error { error, message, .. } => {
                return Err(classify(&error, Some((2105, message))));
            }
        }
        self.report.stage = "initialization";
        self.initialize()?;
        if self.model.actions.is_empty() {
            self.report.tlc_completion_message = true;
            return Ok(Outcome::Complete);
        }
        self.report.stage = "exploration";
        let mut seeds = JavaRandom::new(self.opts.check.seed);
        for level in 2..=self.opts.max_level {
            self.level = level;
            self.phase = 2 - self.phase;
            self.report.level = level;
            let seed = if let Some(explicit) = &self.opts.worker_seeds {
                *explicit
                    .get(self.report.worker_seeds.len())
                    .ok_or_else(|| {
                        Outcome::Limit("DFID worker seed replay list exhausted".to_owned())
                    })?
            } else if self.opts.check.profile == dive_eval::Profile::Tlc {
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
                self.report.tlc_completion_message = level < self.opts.max_level;
                self.report.stage = "complete";
                return Ok(Outcome::Complete);
            }
        }
        self.report.stage = "depth-bound";
        Ok(Outcome::Incomplete)
    }

    fn initialize(&mut self) -> Result<(), Outcome> {
        let mut inits = Vec::new();
        let mut limit = false;
        let enumeration = self
            .model
            .evaluator
            .init_states(&self.model.init_actions, &mut |s| {
                if inits.len() >= self.opts.max_states
                    || self.report.initial_candidates >= self.opts.max_generated
                {
                    limit = true;
                    return Flow::Stop;
                }
                self.report.initial_candidates += 1;
                inits.push(StateVals::from(s.state));
                Flow::Continue
            });
        if let Err(e) = enumeration {
            // modelCheckImpl resets official counts and diagnostic-replays init.
            return Err(classify(&e.error, Some((2103, e.error.tool_text()))));
        }
        if limit {
            return Err(Outcome::Limit("DFID initial-state bound".to_owned()));
        }
        self.report.generated = self.report.initial_candidates;
        for state in inits {
            complete(&state, true)?;
            let st = St::state(state.clone());
            let in_model = allowed(self.model, &self.model.constraints, &st)?;
            let old = if in_model {
                let key = self.key(&state)?;
                let old = self.insert(&key, &state)?;
                if old == 0 {
                    self.roots.push((state.clone(), key));
                }
                old
            } else {
                0
            };
            if old == 0 {
                if let Some(failure) = properties(
                    self.model,
                    &self.model.invariants,
                    &st,
                    "invariant",
                    2107,
                    2102,
                ) {
                    self.witness(std::slice::from_ref(&state), failure)?;
                }
                if let Some(failure) = properties(
                    self.model,
                    &self.model.implied_inits,
                    &st,
                    "property",
                    2108,
                    2102,
                ) {
                    self.witness(std::slice::from_ref(&state), failure)?;
                }
            }
        }
        Ok(())
    }

    fn key(&self, state: &StateVals) -> Result<Key, Outcome> {
        let values: Vec<_> = state
            .iter()
            .cloned()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Outcome::Internal("DFID key for incomplete state".to_owned()))?;
        let ctx = self.model.evaluator.options().ctx;
        match self.opts.check.store {
            StoreMode::Exact => {
                let mut bytes = Vec::new();
                state_key(&values, ctx, &mut bytes)
                    .map_err(|e| Outcome::Unsupported(format!("DFID state encoding: {e}")))?;
                Ok(Key::Exact(bytes))
            }
            StoreMode::Fingerprint => {
                let fp = self
                    .fp
                    .fingerprint(&values, ctx)
                    .map_err(|e| Outcome::Unsupported(format!("DFID state fingerprint: {e}")))?;
                let mask = u64::MAX >> (64 - self.opts.fingerprint_bits);
                Ok(Key::Fingerprint(fp & mask))
            }
        }
    }

    fn insert(&mut self, key: &Key, state: &StateVals) -> Result<u32, Outcome> {
        let mut encoded = Vec::new();
        let values: Vec<_> = state
            .iter()
            .cloned()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Outcome::Internal("DFID insertion of incomplete state".to_owned()))?;
        state_key(&values, self.model.evaluator.options().ctx, &mut encoded)
            .map_err(|e| Outcome::Unsupported(format!("DFID collision guard encoding: {e}")))?;
        if let Some(entry) = self.seen.get(key) {
            if entry.encoded != encoded {
                return Err(Outcome::Limit(
                    "DFID fingerprint collision detected by exact guard".to_owned(),
                ));
            }
            return Ok(entry.status);
        }
        if self.seen.len() >= self.opts.max_states {
            return Err(Outcome::Limit("DFID distinct-state bound".to_owned()));
        }
        self.seen.insert(
            key.clone(),
            Entry {
                status: (self.level << 2) | self.phase,
                encoded,
            },
        );
        Ok(0)
    }

    fn status(&self, key: &Key) -> Result<u32, Outcome> {
        self.seen
            .get(key)
            .map(|e| e.status)
            .ok_or_else(|| Outcome::Internal("DFID successor missing visitation status".to_owned()))
    }

    fn completed(&self, status: u32) -> bool {
        status & 1 != 0 || status & 2 == self.phase
    }
    fn leaf(&self, status: u32) -> bool {
        status == 0 || status >> 2 == self.level
    }

    fn mark(&mut self, key: &Key, done: bool) -> Result<(), Outcome> {
        let phase = self.phase;
        let entry = self.seen.get_mut(key).ok_or_else(|| {
            Outcome::Internal("DFID missing state while marking completion".to_owned())
        })?;
        if done {
            entry.status |= 1;
        } else {
            entry.status = (entry.status & !2) | phase;
        }
        Ok(())
    }

    fn witness(&mut self, path: &[StateVals], failure: Outcome) -> Result<(), Outcome> {
        self.report.deciding = Some(failure.clone());
        if matches!(failure, Outcome::Violation { .. }) {
            if path.len() > self.opts.max_trace {
                return Err(Outcome::Limit(
                    "DFID violation observed but complete witness exceeds bound".to_owned(),
                ));
            }
            self.report.trace = path.to_vec();
        }
        Err(failure)
    }

    // DFIDWorker tests stopCode only around its outer root loop. Its inner
    // DFS keeps draining after the first deciding failure; those expansions
    // contribute counts but cannot replace the first error or witness.
    fn record(&mut self, path: &[StateVals], failure: Outcome) -> Result<(), Outcome> {
        if !matches!(failure, Outcome::Violation { .. } | Outcome::Error { .. }) {
            return Err(failure);
        }
        if self.report.deciding.is_none() {
            self.report.deciding = Some(failure.clone());
            if matches!(failure, Outcome::Violation { .. }) {
                if path.len() > self.opts.max_trace {
                    return Err(Outcome::Limit(
                        "DFID violation observed but complete witness exceeds bound".to_owned(),
                    ));
                }
                self.report.trace = path.to_vec();
            }
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "preserve per-action eager generation and check ordering"
    )]
    fn expand(
        &mut self,
        state: &StateVals,
        key: &Key,
        leaf: bool,
        path: &[StateVals],
        candidate_budget: usize,
    ) -> Result<Vec<(StateVals, Key)>, Outcome> {
        let mut successors = Vec::new();
        let mut all_done = true;
        let mut all_nonleaf = true;
        let mut deadlock = true;
        for action in &self.model.actions {
            let mut vector = Vec::new();
            let mut limit = false;
            let enumeration = self
                .model
                .evaluator
                .next_states(action, state, 0, &mut |s| {
                    if vector.len() >= self.opts.max_states
                        || vector.len() >= candidate_budget.saturating_sub(successors.len())
                        || self.report.generated.saturating_add(vector.len() as u64)
                            >= self.opts.max_generated
                    {
                        limit = true;
                        return Flow::Stop;
                    }
                    vector.push(StateVals::from(s.state));
                    Flow::Continue
                });
            if let Err(e) = enumeration {
                return Err(classify(&e.error, None));
            }
            if limit {
                return Err(Outcome::Limit("DFID successor vector bound".to_owned()));
            }
            self.report.generated += vector.len() as u64;
            deadlock &= vector.is_empty();
            for next in vector {
                if let Err(failure) = complete(&next, false) {
                    self.record(path, failure)?;
                    self.more_level |= !all_nonleaf;
                    return Ok(successors);
                }
                let st = St::state(next.clone());
                let action_st = St::action(state.clone(), next.clone());
                let in_model = allowed(self.model, &self.model.constraints, &st)?
                    && allowed(self.model, &self.model.action_constraints, &action_st)?;
                let old = if in_model {
                    let next_key = self.key(&next)?;
                    let old = self.insert(&next_key, &next)?;
                    all_done &= old & 1 != 0;
                    all_nonleaf &= !self.leaf(old);
                    if !self.completed(old) {
                        successors.push((next.clone(), next_key));
                    }
                    old
                } else {
                    0
                };
                let failure = if old == 0 {
                    properties(
                        self.model,
                        &self.model.invariants,
                        &st,
                        "invariant",
                        2110,
                        2111,
                    )
                } else {
                    None
                }
                .or_else(|| {
                    properties(
                        self.model,
                        &self.model.implied_actions,
                        &action_st,
                        "property",
                        2112,
                        2113,
                    )
                });
                if let Some(failure) = failure {
                    let mut trace = path.to_vec();
                    trace.push(next);
                    self.record(&trace, failure)?;
                    self.more_level |= !all_nonleaf;
                    return Ok(successors);
                }
            }
        }
        if deadlock && self.model.settings.check_deadlock && !self.opts.check.no_deadlock {
            self.record(
                path,
                Outcome::Violation {
                    kind: "deadlock",
                    name: None,
                    code: 2114,
                },
            )?;
            self.more_level |= !all_nonleaf;
            return Ok(successors);
        }
        if all_done || (leaf && all_nonleaf) {
            self.mark(key, true)?;
        }
        if !all_nonleaf {
            self.more_level = true;
        }
        Ok(successors)
    }

    fn iteration(&mut self, rng: &mut JavaRandom) -> Result<bool, Outcome> {
        let mut roots = self.roots.clone();
        self.more_level = false;
        while !roots.is_empty() && self.report.deciding.is_none() {
            let i = index(rng, roots.len());
            let (state, key) = &roots[i];
            if self.completed(self.status(key)?) {
                roots.swap_remove(i);
                continue;
            }
            let state = state.clone();
            let key = key.clone();
            if self.opts.max_trace == 0 {
                return Err(Outcome::Limit("DFID traversal witness bound".to_owned()));
            }
            let mut path = vec![state.clone()];
            let candidates = self.expand(
                &state,
                &key,
                false,
                &path,
                self.opts.max_retained_candidates,
            )?;
            let mut stack = vec![Frame {
                key,
                successors: candidates,
            }];
            loop {
                let depth = stack.len();
                let depth_level = u32::try_from(depth).map_err(|_| {
                    Outcome::Internal("DFID depth exceeds status level representation".to_owned())
                })?;
                let frame = &mut stack[depth - 1];
                let mut selected = None;
                while !frame.successors.is_empty() {
                    let j = index(rng, frame.successors.len());
                    let (s, k) = &frame.successors[j];
                    let status = self.status(k)?;
                    if !self.completed(status) && depth_level < status >> 2 {
                        selected = Some((s.clone(), k.clone()));
                        break;
                    }
                    frame.successors.swap_remove(j);
                }
                if let Some((state, key)) = selected {
                    if path.len() >= self.opts.max_trace {
                        return Err(Outcome::Limit("DFID traversal witness bound".to_owned()));
                    }
                    path.push(state.clone());
                    let leaf = depth_level >= self.level - 1;
                    let retained = stack
                        .iter()
                        .try_fold(0usize, |n, f| n.checked_add(f.successors.len()))
                        .ok_or_else(|| {
                            Outcome::Limit("DFID retained candidate size overflow".to_owned())
                        })?;
                    let budget = self.opts.max_retained_candidates.saturating_sub(retained);
                    let candidates = self.expand(&state, &key, leaf, &path, budget)?;
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
        Ok(self.more_level)
    }
}

fn index(rng: &mut JavaRandom, len: usize) -> usize {
    usize::try_from(rng.next_index(len as u64)).unwrap_or(0)
}

fn complete(state: &StateVals, initial: bool) -> Result<(), Outcome> {
    if state.iter().any(Option::is_none) {
        return Err(Outcome::Error {
            code: if initial { 2106 } else { 2109 },
            message: "incomplete DFID state".to_owned(),
        });
    }
    Ok(())
}

fn allowed(model: &Model, roles: &[Named], st: &St) -> Result<bool, Outcome> {
    for role in roles {
        match model.eval(&role.pred, st) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => return Ok(false),
            Ok(_) => {
                return Err(Outcome::Unsupported(
                    "DFID nonboolean role diagnostic".to_owned(),
                ));
            }
            Err(e) => return Err(classify(&e, None)),
        }
    }
    Ok(true)
}

fn properties(
    model: &Model,
    roles: &[Named],
    st: &St,
    kind: &'static str,
    code: u32,
    _error_code: u32,
) -> Option<Outcome> {
    for role in roles {
        match model.eval(&role.pred, st) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                return Some(Outcome::Violation {
                    kind,
                    name: Some(role.name.clone()),
                    code,
                });
            }
            Ok(_) => {
                return Some(Outcome::Unsupported(
                    "DFID nonboolean property diagnostic".to_owned(),
                ));
            }
            Err(e) => return Some(classify(&e, None)),
        }
    }
    None
}

fn classify(e: &EvalError, measured: Option<(u32, String)>) -> Outcome {
    match e.class() {
        ErrorClass::Unsupported => Outcome::Unsupported(e.message().to_owned()),
        ErrorClass::ResourceLimit => Outcome::Limit(e.message().to_owned()),
        ErrorClass::Bug => Outcome::Internal(e.message().to_owned()),
        ErrorClass::Eval => match measured {
            Some((code, message)) => Outcome::Error { code, message },
            None => Outcome::Unsupported(format!(
                "unverified DFID evaluation-error context: {}",
                e.message()
            )),
        },
    }
}

fn preflight(model: &Model, opts: &Options) -> Option<Outcome> {
    let refusal = |s: &str| Some(Outcome::Unsupported(s.to_owned()));
    if opts.check.workers == 0 {
        return Some(Outcome::Error {
            code: 1102,
            message: "Error: at least one worker required.".to_owned(),
        });
    }
    if opts.check.workers != 1 {
        return Some(if opts.check.profile == dive_eval::Profile::Tlc {
            Outcome::Error {
                code: 1000,
                message: "Depth-First Iterative Deepening mode does not support multiple workers"
                    .to_owned(),
            }
        } else {
            Outcome::Unsupported("DFID multiple workers".to_owned())
        });
    }
    if opts.max_level > 10_000 {
        return Some(Outcome::Limit("DFID iterative level bound".to_owned()));
    }
    if opts.fingerprint_bits == 0 || opts.fingerprint_bits > 64 {
        return refusal("DFID fingerprint bit width outside 1..64");
    }
    if !model.temporals.is_empty() || !model.implied_temporals.is_empty() {
        return Some(if opts.check.profile == dive_eval::Profile::Tlc {
            Outcome::Error { code: 1000, message: "Depth-First Iterative Deepening mode does not support checking liveness properties".to_owned() }
        } else {
            Outcome::Unsupported("DFID temporal properties and fairness".to_owned())
        });
    }
    if let Some(u) = model.unsupported.first() {
        return refusal(&u.feature);
    }
    if model.symmetry.is_some() {
        return refusal("DFID SYMMETRY");
    }
    if model.view.is_some() {
        return refusal("DFID VIEW");
    }
    if model.alias.is_some() {
        return refusal("DFID ALIAS");
    }
    if !matches!(&model.postconditions, Ok(p) if p.is_empty()) {
        return refusal("DFID POSTCONDITION");
    }
    if !model.implied_inits.is_empty() && !model.constraints.is_empty() {
        return refusal(
            "DFID initial properties with state constraints require separate witness validation contract",
        );
    }
    if !model.implied_actions.is_empty()
        && (!model.constraints.is_empty() || !model.action_constraints.is_empty())
    {
        return refusal(
            "DFID action properties with constraints require separate witness validation contract",
        );
    }
    // Runtime names need ordered execution, which this inline backend already
    // provides. Independent witness validation still refuses history it cannot
    // reconstruct; effects and live introspection remain prohibited here.
    if !model.sequential.is_empty() && !model.sequential_names_only {
        return refusal("DFID live checker state/effect reads");
    }
    let predicates = model
        .init
        .iter()
        .map(|i| &i.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|n| &n.pred))
        .chain(model.implied_inits.iter().map(|n| &n.pred))
        .chain(model.implied_actions.iter().map(|n| &n.pred))
        .chain(model.constraints.iter().map(|n| &n.pred))
        .chain(model.action_constraints.iter().map(|n| &n.pred));
    if predicates
        .into_iter()
        .any(|p| !pure::predicate(model, p) || model.evaluator.scope_shares_side_effects(&p.scope))
        || model.assumptions.iter().any(|d| {
            model.ir.defs().get(d.index()).is_none_or(|def| {
                !model.evaluator.scope(*d).is_ok_and(|scope| {
                    pure::predicate(model, &dive_model::Pred::new(def.body, scope))
                })
            })
        })
        || model
            .init_actions
            .iter()
            .chain(&model.actions)
            .any(|a| !pure::predicate(model, &dive_model::Pred::new(a.expr(), a.scope())))
    {
        return refusal("DFID effects or unresolved constant-operator effects");
    }
    None
}
