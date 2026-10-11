//! The checks (ADR 0016 §1).
//!
//! Every check evaluates a predicate of the model over *given* states with
//! the core evaluator (`Model::eval`): `Init` as a state predicate on the
//! first state, the next-state relation as an action predicate on each pair
//! of states. Nothing here constructs a state; the successor enumerator is
//! not even compiled into this crate (ADR 0016 §2).
//!
//! **Random draws** (`RandomElement`, the `Randomization` operators) are
//! replayed from the checking run's seed when a [`Replay`] is given (ADR
//! 0016, amendment 2): the initial predicate draws from a fresh start-up
//! stream, and the step out of state `k` from the stream TLC seeds with
//! state `k`'s fingerprint, as the checker's own trace reconstruction does
//! (D-002). The predicate draws in its own evaluation order, from the
//! start of the stream; the successor generator may have drawn for other
//! successors first. Random checks are decisive only when a conservative
//! certificate proves the two evaluation orders consume the same draws.
//! Otherwise the validator reports unsupported, for either a match or a
//! mismatch. A finite search of stream offsets cannot establish validity
//! or invalidity. A draw anywhere else is refused by name.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dive_eval::{AssumptionOutcome, ErrorClass, EvalError, Host, HostError, St, StateRef};
use dive_ir::StdOp;
use dive_model::{Model, Pred};
use dive_stdlib::host::RunHost;
use dive_stdlib::random::{Stream, Streams};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Value};

use crate::format::Ending;
use crate::trace::Trace;

/// Which check failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    /// The trace's shape: empty, a variable missing or extra, a value that
    /// does not read.
    Shape,
    /// The claimed length differs from the number of states.
    Length,
    /// An `ASSUME` of the specification does not hold.
    Assumption,
    /// The first state does not satisfy the initial predicate.
    Init,
    /// A step does not satisfy the next-state relation.
    Next,
    /// A step's label names an action that does not hold on it.
    Label,
    /// A state that must be in the model violates a `CONSTRAINT`.
    Constraint,
    /// A step that must be in the model violates an `ACTION_CONSTRAINT`.
    ActionConstraint,
    /// An invariant fails before the end, so the search would have stopped
    /// there (or, at the end, an invariant listed before the claimed one
    /// fails).
    Invariant,
    /// An implied initial predicate fails on the first state.
    ImpliedInit,
    /// An action property fails on a step before the end.
    ImpliedAction,
    /// The claimed ending does not hold: the invariant or property is not
    /// violated, or the model has no such invariant or property.
    Ending,
    /// A temporal conjunct of the behavior specification is false.
    Fairness,
    /// A claimed temporal property holds, or a violated property was omitted.
    Temporal,
}

impl Check {
    /// A stable name for reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Length => "length",
            Self::Assumption => "assumption",
            Self::Init => "init",
            Self::Next => "next",
            Self::Label => "label",
            Self::Constraint => "constraint",
            Self::ActionConstraint => "action-constraint",
            Self::Invariant => "invariant",
            Self::ImpliedInit => "implied-init",
            Self::ImpliedAction => "implied-action",
            Self::Ending => "ending",
            Self::Fairness => "fairness",
            Self::Temporal => "temporal",
        }
    }
}

/// The first check a trace fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The state (0-based) the check is about; for a step, the state it
    /// leads to.
    pub state: usize,
    /// The check.
    pub check: Check,
    /// What failed, precisely enough to file as a bug.
    pub detail: String,
}

/// The validator's verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Validation {
    /// Every check passed: the trace is a behavior of the specification and
    /// ends as claimed.
    Valid,
    /// A check failed.
    Invalid(Failure),
    /// Every check that could be made passed, but part of the claim cannot
    /// be checked independently. The text names it. Never a verdict.
    Unsupported(String),
}

impl Validation {
    /// Whether the trace validated.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// `valid`, `invalid` or `unsupported`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid(_) => "invalid",
            Self::Unsupported(_) => "unsupported",
        }
    }
}

/// The verdict with what the validator noticed along the way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The verdict.
    pub validation: Validation,
    /// Remarks: labels that could not be checked, how a TLC document was
    /// read. They never change the verdict.
    pub notes: Vec<String>,
    /// The number of states in the trace.
    pub length: usize,
}

/// The reason given for a deadlock ending (ADR 0016, amendment 1).
pub const DEADLOCK_UNSUPPORTED: &str = "deadlock: the independent finite-cover check could not decide absence of successors; every other check passed (ADR 0016, amendment 1)";

/// Runtime string tokens depend on evaluations outside the supplied trace.
/// Reading witness strings must never establish the order that validates them.
pub const RUNTIME_NAMES_UNSUPPORTED: &str = "runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)";

/// The checking run's random streams, for replaying its draws: the seed
/// and seeding mode ([`Streams`]) and the fingerprint polynomial's index
/// (`-fp`), which keys the stream of each expanded state.
///
/// The per-state stream is keyed by TLC's state fingerprint (FP64 over the
/// values in TLC's variable order, `dive_ir::Ir::tlc_var_order`), which is
/// what both of dive's profiles seed with; the validator computes it itself
/// from the trace's values.
///
/// ```
/// use dive_stdlib::random::{RandomMode, Streams};
/// use dive_trace::Replay;
/// let r = Replay::new(Streams::new(0, RandomMode::Tlc), 0).ok_or("index 0 exists")?;
/// assert_eq!(r.fp_index(), 0);
/// assert!(Replay::new(Streams::new(0, RandomMode::Tlc), 131).is_none());
/// # Ok::<(), &str>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Replay {
    streams: Streams,
    fp_index: usize,
}

impl Replay {
    /// The streams of a run with `streams` and fingerprint polynomial
    /// `fp_index`; `None` when the index is out of range (`0..131`).
    #[must_use]
    pub fn new(streams: Streams, fp_index: usize) -> Option<Self> {
        TlcFingerprint::new(fp_index).map(|_| Self { streams, fp_index })
    }

    /// The run's streams.
    #[must_use]
    pub const fn streams(&self) -> Streams {
        self.streams
    }

    /// The fingerprint polynomial's index.
    #[must_use]
    pub const fn fp_index(&self) -> usize {
        self.fp_index
    }
}

/// A random draw made while a check replayed the run's stream: the
/// operator, its arguments and the value drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Draw {
    /// The state (0-based) whose check drew it: the initial predicate for
    /// state 0, the step into state `k` otherwise.
    pub(crate) state: usize,
    pub(crate) op: StdOp,
    pub(crate) args: Vec<Value>,
    pub(crate) value: Value,
}

/// Which stream a replayed check draws from.
#[derive(Clone, Copy)]
enum Place {
    /// A fresh start-up stream (`ASSUME`s, the initial predicate).
    Startup,
    /// The stream of expanding this state.
    Expand(usize),
}

/// What stopped a run early.
pub(crate) enum Stop {
    Invalid(Failure),
    Unsupported(String),
}

impl From<Stop> for Validation {
    fn from(s: Stop) -> Self {
        match s {
            Stop::Invalid(f) => Self::Invalid(f),
            Stop::Unsupported(u) => Self::Unsupported(u),
        }
    }
}

/// What the end of the trace must show.
enum Goal {
    Invariant(usize),
    ImpliedInit(usize),
    ImpliedAction(usize),
    Deadlock,
    Behavior,
    Liveness(&'static str),
    ErrorEnd(&'static str),
}

/// One validation run. `passed` counts the checks passed, in order: the
/// TLC reader uses it to compare interpretations of a lossy document.
pub(crate) struct Run<'a> {
    model: &'a Model,
    host: Arc<TraceHost>,
    /// The host installed before the run, restored after it.
    inner: Option<Arc<dyn Host>>,
    /// TLC's state fingerprint, for the per-state streams.
    fingerprint: Option<TlcFingerprint>,
    pub(crate) passed: u64,
    pub(crate) notes: BTreeSet<String>,
    /// The draws the initial predicate and the steps made, when recording.
    pub(crate) draws: Vec<Draw>,
    /// Concrete periodic predicates must obey the finite-binding entry bounds.
    periodic: bool,
}

impl<'a> Run<'a> {
    pub(crate) fn new(model: &'a Model, replay: Option<&Replay>) -> Self {
        let inner = model.evaluator.host();
        let host = Arc::new(TraceHost {
            level: AtomicI64::new(0),
            inner: inner.clone(),
            replay: replay.map(|r| RunHost::new(r.streams, None)),
            placed: AtomicBool::new(false),
            draws: AtomicU64::new(0),
            log: Mutex::new(None),
        });
        model.evaluator.set_host(host.clone());
        Self {
            model,
            host,
            inner,
            fingerprint: replay.and_then(|r| TlcFingerprint::new(r.fp_index)),
            passed: 0,
            notes: BTreeSet::new(),
            draws: Vec::new(),
            periodic: false,
        }
    }

    /// Records the draws of the initial predicate and of the steps into
    /// [`Run::draws`].
    pub(crate) fn record_draws(&mut self) {
        *self.host.log.lock().unwrap_or_else(PoisonError::into_inner) = Some(Vec::new());
    }

    /// The stream a check at `at` draws from, at its start. `None` without
    /// a replay, under `SYMMETRY` or an unproved VIEW (not guessed), or
    /// for a state whose transformed fingerprint cannot be computed;
    /// draws then stay refused.
    fn base(&self, trace: &Trace, at: Place) -> Option<Stream> {
        let streams = self.host.replay.as_ref()?.streams();
        match at {
            Place::Startup => Some(streams.startup()),
            Place::Expand(k) => {
                let m = self.model;
                if m.symmetry.is_some() || !crate::random_position::pure_random_view(m) {
                    return None;
                }
                Some(streams.state(self.state_fingerprint(&trace.states[k])?))
            }
        }
    }

    /// Points the replay host at `base`, `skip` generator steps in, so that
    /// draws are allowed until [`Run::unplace`].
    fn place(&self, base: Option<Stream>, skip: u64) {
        if let (Some(replay), Some(base)) = (&self.host.replay, base) {
            replay.set_stream(base);
            replay.skip(skip);
            self.host.placed.store(true, Ordering::Relaxed);
        }
    }

    /// Makes a check that draws from the run's stream at `at`. Draws only
    /// establish a result if `aligned` proves the generator and predicate
    /// consume the same stream positions. Successful certified draws are
    /// recorded as state `record`'s, if given.
    fn replayed(
        &mut self,
        trace: &Trace,
        at: Place,
        record: Option<usize>,
        aligned: bool,
        check: impl Fn(&mut Self) -> Result<(), Stop>,
    ) -> Result<(), Stop> {
        let base = self.base(trace, at);
        self.place(base, 0);
        let before = self.draw_count();
        let r = check(self);
        self.unplace();
        let drew = self.draw_count() != before;
        if drew && !aligned {
            self.discard_draws();
            return Err(Stop::Unsupported(
                "random draws: the successor generator may have consumed draws for other successors; the independent validator cannot prove the predicate's stream position (ADR 0016, amendment 2)".to_owned(),
            ));
        }
        match (&r, record, base) {
            (Ok(()), Some(k), Some(_)) => self.collect_draws(k),
            _ => self.discard_draws(),
        }
        r
    }

    fn unplace(&self) {
        self.host.placed.store(false, Ordering::Relaxed);
    }

    /// TLC's stream identity: FP64 of an admitted VIEW value, otherwise
    /// concrete values in TLC's variable order. Symmetry is still refused.
    fn state_fingerprint(&self, state: &crate::trace::State) -> Option<u64> {
        let f = self.fingerprint.as_ref()?;
        let ctx: Ctx = self.model.evaluator.options().ctx;
        if let Some(view) = &self.model.view {
            let values = state.iter().cloned().collect::<Option<Vec<_>>>()?;
            dive_model::random_view::admit_values(&values).ok()?;
            dive_model::symmetry::Reduction::normalize_concrete(&values, ctx).ok()?;
            let st = St::state(state.clone());
            let value = self.model.eval(&view.pred, &st).ok()?;
            dive_model::random_view::admit_values(std::slice::from_ref(&value)).ok()?;
            return f.fingerprint(&value, ctx).ok();
        }
        let ordered: Option<Vec<Value>> = self
            .model
            .ir
            .tlc_var_order()
            .iter()
            .map(|v| state.get(v.index()).cloned().flatten())
            .collect();
        f.fingerprint_state(&ordered?, ctx).ok()
    }

    /// Moves the draws logged while checking state `k` into
    /// [`Run::draws`].
    fn collect_draws(&mut self, k: usize) {
        let mut log = self.host.log.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(log) = log.as_mut() {
            for (op, args, value) in log.drain(..) {
                self.draws.push(Draw {
                    state: k,
                    op,
                    args,
                    value,
                });
            }
        }
    }

    /// Forgets the draws logged since the last [`Run::collect_draws`].
    fn discard_draws(&self) {
        if let Some(log) = self
            .host
            .log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            log.clear();
        }
    }

    fn draw_count(&self) -> u64 {
        self.host.draws.load(Ordering::Relaxed)
    }

    /// Validates `trace`.
    pub(crate) fn validate(&mut self, trace: &Trace) -> Validation {
        if !crate::runtime_order::admitted(self.model) {
            return Validation::Unsupported(RUNTIME_NAMES_UNSUPPORTED.to_owned());
        }
        match self.run(trace) {
            Ok(None) => Validation::Valid,
            Ok(Some(why)) => Validation::Unsupported(why),
            Err(stop) => stop.into(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the checks of ADR 0016 §1, in the order the search makes them"
    )]
    fn run(&mut self, trace: &Trace) -> Result<Option<String>, Stop> {
        let m = self.model;
        let n = trace.states.len();
        if n == 0 {
            return Err(invalid(0, Check::Shape, "the trace has no states"));
        }
        if trace.labels.len() != n {
            return Err(invalid(
                0,
                Check::Shape,
                "the trace has a label list of the wrong length",
            ));
        }
        if let Some(len) = trace.length
            && len != n
        {
            return Err(invalid(
                n - 1,
                Check::Length,
                format!("the trace claims {len} states but has {n}"),
            ));
        }
        let vars = m.ir.vars().len();
        for (k, s) in trace.states.iter().enumerate() {
            if s.len() != vars || s.iter().any(Option::is_none) {
                return Err(invalid(
                    k,
                    Check::Shape,
                    "the state does not assign every variable",
                ));
            }
        }
        let named_lasso = !trace.properties.is_empty();
        if named_lasso {
            self.prepare_periodic(&trace.ending, &trace.properties)?;
        }
        // The ASSUMEs draw from the start-up stream, after the constant
        // definitions (evaluated when the model was bound).
        let startup = self.base(trace, Place::Startup);
        self.place(startup, 0);
        let before = self.draw_count();
        let assumed = self.assumptions();
        self.unplace();
        self.discard_draws();
        if let Err(Stop::Invalid(f)) = &assumed
            && self.draw_count() != before
        {
            return Err(Stop::Unsupported(format!(
                "an ASSUME that draws random values is false on a fresh start-up stream ({}), but the run drew its constant definitions from that stream first, which the validator does not replay",
                f.detail
            )));
        }
        assumed?;
        let last = n - 1;
        let goal = self.goal(&trace.ending, last)?;
        let full_last = !matches!(goal, Goal::Invariant(_));
        let behavior = matches!(goal, Goal::Behavior);
        for k in 0..n {
            let here = St::state(trace.states[k].clone());
            let step =
                (k > 0).then(|| St::action(trace.states[k - 1].clone(), trace.states[k].clone()));
            match &step {
                None => {
                    // TLC rebuilds a counterexample's initial state from a
                    // fresh start-up stream (D-002), and so does dive.
                    let aligned = m.init.len() == 1
                        && crate::random_position::aligned(m, &m.init[0].pred, &here, true);
                    self.replayed(trace, Place::Startup, Some(0), aligned, |run| {
                        m.init.iter().try_for_each(|init| {
                            run.require(&init.pred, &here, 0, Check::Init, || {
                                format!(
                                    "the initial predicate {} is false in the first state",
                                    m.location(&init.pred)
                                )
                            })
                        })
                    })?;
                }
                Some(step) => {
                    // TLC reseeds with the fingerprint of the state it
                    // expands, for every action of the next-state relation.
                    let aligned = m
                        .next
                        .as_ref()
                        .is_some_and(|p| crate::random_position::aligned(m, p, step, false));
                    self.replayed(trace, Place::Expand(k - 1), Some(k), aligned, |run| {
                        if named_lasso && trace.states[k - 1] == trace.states[k] {
                            Ok(()) // The implicit full-state stuttering closure.
                        } else {
                            run.next(step, k, "the step into this state")
                        }
                    })?;
                    if let Some(label) = &trace.labels[k] {
                        self.label(trace, &label.name, step, k)?;
                    }
                }
            }
            if behavior {
                continue;
            }
            let is_last = k == last;
            if !is_last || full_last {
                for c in &m.constraints {
                    self.require(&c.pred, &here, k, Check::Constraint, || {
                        format!("CONSTRAINT {} is false, but only the last state of an invariant violation may lie outside the model", c.name)
                    })?;
                }
                if let Some(step) = &step
                    && !(named_lasso && trace.states[k - 1] == trace.states[k])
                {
                    for c in &m.action_constraints {
                        self.require(&c.pred, step, k, Check::ActionConstraint, || {
                            format!("ACTION_CONSTRAINT {} is false, but only the last step of an invariant violation may lie outside the model", c.name)
                        })?;
                    }
                }
            }
            match (&goal, is_last) {
                (Goal::Invariant(i), true) => {
                    self.claimed(&m.invariants, *i, &here, k, Check::Invariant, "invariant")?;
                }
                _ => {
                    for inv in &m.invariants {
                        self.require(&inv.pred, &here, k, Check::Invariant, || {
                            format!("invariant {} is false here, so the search would have stopped at this state", inv.name)
                        })?;
                    }
                }
            }
            match (&goal, &step, is_last) {
                (Goal::Invariant(_), _, true) => {}
                (Goal::ImpliedInit(i), None, _) => {
                    self.claimed(
                        &m.implied_inits,
                        *i,
                        &here,
                        k,
                        Check::ImpliedInit,
                        "property",
                    )?;
                }
                (Goal::ImpliedAction(i), Some(step), true) => {
                    self.claimed(
                        &m.implied_actions,
                        *i,
                        step,
                        k,
                        Check::ImpliedAction,
                        "property",
                    )?;
                }
                (_, None, _) => {
                    for p in &m.implied_inits {
                        self.require(&p.pred, &here, k, Check::ImpliedInit, || {
                            format!("property {} is false in the first state", p.name)
                        })?;
                    }
                }
                (_, Some(step), _) => {
                    for p in &m.implied_actions {
                        self.require(&p.pred, step, k, Check::ImpliedAction, || {
                            format!("action property {} is false on this step, so the search would have stopped here", p.name)
                        })?;
                    }
                }
            }
        }
        if named_lasso {
            self.periodic_end(trace)?;
            return Ok(None);
        }
        if let Ending::Lasso { back_to } = trace.ending {
            if m.view.is_some() {
                // TLC closes the loop on the VIEW's value, not the state's:
                // the last state's successor only has the same view as the
                // state it loops to (Examples' MCDistributedReplicatedLog).
                self.notes.insert("VIEW: TLC's loop closes on the view, not the state; the loop step is not checked".to_owned());
            } else {
                let step = St::action(trace.states[last].clone(), trace.states[back_to].clone());
                let what = format!(
                    "the loop from the last state back to state {} (numbered from 1)",
                    back_to + 1
                );
                let aligned = m
                    .next
                    .as_ref()
                    .is_some_and(|p| crate::random_position::aligned(m, p, &step, false));
                self.replayed(trace, Place::Expand(last), None, aligned, |run| {
                    run.next(&step, last, &what)
                })?;
            }
        }
        Ok(match goal {
            Goal::Deadlock => {
                use crate::exists::{Decision, Limits, exists_successor};
                if let Some(next) = &m.next {
                    match exists_successor(m, next, &trace.states[last], None, Limits::default()) {
                        Ok(Decision::Disabled) => None,
                        Ok(Decision::Enabled { .. }) => {
                            return Err(invalid(
                                last,
                                Check::Ending,
                                "the claimed deadlock has a successor",
                            ));
                        }
                        Err(e) => {
                            self.notes.insert(e.to_string());
                            Some(DEADLOCK_UNSUPPORTED.to_owned())
                        }
                    }
                } else {
                    Some(DEADLOCK_UNSUPPORTED.to_owned())
                }
            }
            Goal::Liveness(why) | Goal::ErrorEnd(why) => Some(why.to_owned()),
            Goal::Invariant(_) | Goal::ImpliedInit(_) | Goal::ImpliedAction(_) | Goal::Behavior => {
                None
            }
        })
    }

    pub(crate) fn prepare_periodic(
        &mut self,
        ending: &Ending,
        properties: &[String],
    ) -> Result<(), Stop> {
        if !matches!(ending, Ending::Lasso { .. } | Ending::Stuttering) {
            return Err(invalid(
                0,
                Check::Ending,
                "temporal property names require a periodic ending",
            ));
        }
        if self.model.view.is_some() || self.model.symmetry.is_some() {
            return Err(Stop::Unsupported(
                "periodic validation under VIEW or SYMMETRY needs concrete closing-state evidence"
                    .into(),
            ));
        }
        let mut names = BTreeSet::new();
        let mut declared = BTreeSet::new();
        if self
            .model
            .property_claims()
            .iter()
            .any(|p| !declared.insert(&p.name))
        {
            return Err(Stop::Unsupported(
                "split temporal properties with ambiguous repeated names".into(),
            ));
        }
        for name in properties {
            if name.is_empty() || !names.insert(name) {
                return Err(invalid(
                    0,
                    Check::Ending,
                    "empty or repeated temporal property name",
                ));
            }
            if !self.model.property_claims().iter().any(|p| p.name == *name) {
                return Err(invalid(
                    0,
                    Check::Ending,
                    format!("the model has no temporal property named `{name}`"),
                ));
            }
        }
        let check = |pred: &Pred| {
            crate::temporal::check_periodic_context(self.model, pred)
                .map_err(|e| Stop::Unsupported(e.to_string()))
        };
        for init in &self.model.init {
            check(&init.pred)?;
        }
        if let Some(next) = &self.model.next {
            check(next)?;
        }
        for p in self
            .model
            .constraints
            .iter()
            .chain(&self.model.action_constraints)
            .chain(&self.model.invariants)
            .chain(&self.model.implied_inits)
            .chain(&self.model.implied_actions)
        {
            check(&p.pred)?;
        }
        for &d in &self.model.assumptions {
            check(&Pred::new(
                self.model.ir.def(d).body,
                self.model
                    .evaluator
                    .scope(d)
                    .map_err(|e| Stop::Unsupported(e.to_string()))?,
            ))?;
        }
        self.periodic = true;
        Ok(())
    }

    fn periodic_end(&mut self, trace: &Trace) -> Result<(), Stop> {
        let last = trace.states.len() - 1;
        let back = match trace.ending {
            Ending::Lasso { back_to } => back_to,
            Ending::Stuttering => last,
            _ => return Err(Stop::Unsupported("internal nonperiodic ending".into())),
        };
        let stutter = trace.states[last] == trace.states[back];
        let step = St::action(trace.states[last].clone(), trace.states[back].clone());
        if !stutter {
            self.next(&step, last, "the closing step")?;
            for c in &self.model.action_constraints {
                self.require(&c.pred, &step, last, Check::ActionConstraint, || {
                    format!("ACTION_CONSTRAINT {} is false on the closing step", c.name)
                })?;
            }
        }
        for p in &self.model.implied_actions {
            self.require(&p.pred, &step, last, Check::ImpliedAction, || {
                format!("action property {} is false on the closing step", p.name)
            })?;
        }
        for p in &self.model.temporals {
            if !self.temporal_truth(&p.pred, trace, back)? {
                return Err(invalid(
                    0,
                    Check::Fairness,
                    format!(
                        "behavior temporal conjunct {} is false on this word",
                        p.name
                    ),
                ));
            }
            self.passed += 1;
        }
        for p in self.model.property_claims() {
            let holds = self.temporal_truth(&p.pred, trace, back)?;
            let claimed = trace.properties.contains(&p.name);
            if claimed && holds {
                return Err(invalid(
                    0,
                    Check::Temporal,
                    format!("temporal property {} holds on this word", p.name),
                ));
            }
            if !claimed && !holds {
                return Err(invalid(
                    0,
                    Check::Temporal,
                    format!("violated temporal property {} was omitted", p.name),
                ));
            }
            self.passed += 1;
        }
        Ok(())
    }

    fn temporal_truth(&self, pred: &Pred, trace: &Trace, back: usize) -> Result<bool, Stop> {
        use crate::temporal::TemporalError;
        match crate::temporal::evaluate_lasso(self.model, pred, &trace.states, back) {
            Ok(values) => values
                .first()
                .copied()
                .ok_or_else(|| Stop::Unsupported("internal empty temporal truth vector".into())),
            Err(TemporalError::Invalid(detail)) => Err(invalid(0, Check::Temporal, detail)),
            Err(TemporalError::Unsupported(detail)) => Err(Stop::Unsupported(detail)),
        }
    }

    fn goal(&self, ending: &Ending, last: usize) -> Result<Goal, Stop> {
        let m = self.model;
        let find =
            |list: &[dive_model::Named], name: &str| list.iter().position(|n| n.name == name);
        Ok(match ending {
            Ending::Invariant(name) => Goal::Invariant(find(&m.invariants, name).ok_or_else(|| {
                invalid(last, Check::Ending, format!("the model has no invariant named `{name}`"))
            })?),
            Ending::Property(name) if last == 0 => {
                Goal::ImpliedInit(find(&m.implied_inits, name).ok_or_else(|| {
                    invalid(
                        last,
                        Check::Ending,
                        format!("the model has no implied initial predicate named `{name}`, and a one-state trace cannot violate an action property"),
                    )
                })?)
            }
            Ending::Property(name) => Goal::ImpliedAction(find(&m.implied_actions, name).ok_or_else(|| {
                invalid(last, Check::Ending, format!("the model has no action property named `{name}`"))
            })?),
            Ending::Deadlock => {
                if !m.settings.check_deadlock {
                    return Err(invalid(
                        last,
                        Check::Ending,
                        "the model does not check for deadlock (CHECK_DEADLOCK FALSE or -deadlock)",
                    ));
                }
                Goal::Deadlock
            }
            Ending::Behavior => Goal::Behavior,
            Ending::Lasso { back_to } => {
                if *back_to > last {
                    return Err(invalid(last, Check::Shape, format!("the lasso loops back to state {back_to}, past the end")));
                }
                Goal::Liveness("liveness: the steps and the loop back (unless noted) were checked, but named temporal-property claims are missing")
            }
            Ending::Stuttering => Goal::Liveness("liveness: the steps were checked, but named temporal-property claims are missing"),
            Ending::Assertion | Ending::Error { .. } => Goal::ErrorEnd("an error while expanding the last state: the steps were checked, but re-raising the error needs the successor generator, which the independent validator does not have"),
        })
    }

    fn assumptions(&mut self) -> Result<(), Stop> {
        match self.model.check_assumptions() {
            AssumptionOutcome::AllTrue => {
                self.passed += 1;
                Ok(())
            }
            AssumptionOutcome::False { message, .. } => Err(invalid(0, Check::Assumption, message)),
            AssumptionOutcome::Error { error, message, .. } => match error.class() {
                ErrorClass::Eval => Err(invalid(0, Check::Assumption, message)),
                _ => Err(refused(&error)),
            },
        }
    }

    fn next(&mut self, step: &St, k: usize, what: &str) -> Result<(), Stop> {
        let m = self.model;
        let Some(next) = &m.next else {
            return Err(invalid(
                k,
                Check::Next,
                "the model has no next-state relation, so a trace has one state",
            ));
        };
        self.require(next, step, k, Check::Next, || {
            format!(
                "{what} does not satisfy the next-state relation {}",
                m.location(next)
            )
        })
    }

    /// The label's definition must hold on the step, if it can be checked.
    /// TLC labels a step with the definition its action was split from, and
    /// that action implies the definition's body.
    fn label(&mut self, trace: &Trace, name: &str, step: &St, k: usize) -> Result<(), Stop> {
        let m = self.model;
        let ev = &m.evaluator;
        let Some(d) = m.ir.root_def(name) else {
            self.notes.insert(format!(
                "label `{name}` names no definition of the root module; not checked"
            ));
            return Ok(());
        };
        let d = ev.redirected(d);
        if !m.ir.def(d).params.is_empty() || ev.is_value_override(d) {
            self.notes.insert(format!(
                "label `{name}` names an operator with parameters; not checked"
            ));
            return Ok(());
        }
        // Alone, the label's definition draws from the start of the step's
        // stream, but in the search it may come after other actions that
        // drew: a label whose evaluation draws is not checked.
        let base = self.base(trace, Place::Expand(k - 1));
        self.place(base, 0);
        let before = self.draw_count();
        let r = ev.eval_def(d, step);
        self.unplace();
        self.discard_draws();
        if self.draw_count() != before {
            self.notes.insert(format!(
                "label `{name}` draws random values on its own; not checked"
            ));
            return Ok(());
        }
        match r {
            Ok(Value::Bool(true)) => {
                self.passed += 1;
                Ok(())
            }
            Ok(Value::Bool(false)) => Err(invalid(
                k,
                Check::Label,
                format!("the step is labelled `{name}`, but `{name}` is false on it"),
            )),
            _ => {
                self.notes.insert(format!(
                    "label `{name}` could not be evaluated on its own; not checked"
                ));
                Ok(())
            }
        }
    }

    /// The claimed invariant or property: those listed before it hold, it
    /// does not.
    fn claimed(
        &mut self,
        list: &[dive_model::Named],
        i: usize,
        st: &St,
        k: usize,
        check: Check,
        what: &str,
    ) -> Result<(), Stop> {
        for earlier in &list[..i] {
            self.require(&earlier.pred, st, k, check, || {
                format!("{what} {} is also false here and comes first in the configuration, so it would be reported instead of {}", earlier.name, list[i].name)
            })?;
        }
        let p = &list[i];
        if self.holds(&p.pred, st, k, check)? {
            return Err(invalid(
                k,
                Check::Ending,
                format!(
                    "{what} {} holds at the end of the trace, so the trace does not violate it",
                    p.name
                ),
            ));
        }
        self.passed += 1;
        Ok(())
    }

    fn require(
        &mut self,
        pred: &Pred,
        st: &St,
        k: usize,
        check: Check,
        detail: impl FnOnce() -> String,
    ) -> Result<(), Stop> {
        if self.holds(pred, st, k, check)? {
            self.passed += 1;
            Ok(())
        } else {
            Err(invalid(k, check, detail()))
        }
    }

    fn holds(&self, pred: &Pred, st: &St, k: usize, check: Check) -> Result<bool, Stop> {
        let m = self.model;
        // TLC's level of `s0`: state k (numbered from 0) is at level k + 1;
        // a step into state k is evaluated with the state before it as s0.
        let level = if matches!(st.s1, StateRef::Vals(_)) {
            k
        } else {
            k + 1
        };
        self.host
            .level
            .store(i64::try_from(level).unwrap_or(i64::MAX), Ordering::Relaxed);
        if self.periodic {
            m.evaluator
                .check_finite_bindings(&pred.scope, pred.expr, st, 4096)
                .map_err(|e| Stop::Unsupported(e.to_string()))?;
        }
        match m.eval(pred, st) {
            Ok(Value::Bool(b)) => Ok(b),
            Ok(v) => Err(invalid(
                k,
                check,
                format!(
                    "{} is not a boolean here: {}",
                    m.location(pred),
                    v.kind_name()
                ),
            )),
            Err(e) => match e.class() {
                ErrorClass::Eval => Err(invalid(
                    k,
                    check,
                    format!("evaluating {} fails: {}", m.location(pred), e.message()),
                )),
                _ => Err(refused(&e)),
            },
        }
    }
}

fn invalid(state: usize, check: Check, detail: impl Into<String>) -> Stop {
    Stop::Invalid(Failure {
        state,
        check,
        detail: detail.into(),
    })
}

fn refused(e: &EvalError) -> Stop {
    Stop::Unsupported(match e.class() {
        ErrorClass::Unsupported => format!("the evaluator refuses: {}", e.message()),
        ErrorClass::ResourceLimit => format!("resource limit: {}", e.message()),
        _ => format!("internal evaluator error (a bug): {e}"),
    })
}

impl Drop for Run<'_> {
    fn drop(&mut self) {
        let _ = self.model.evaluator.replace_host(self.inner.take());
    }
}

/// A logged draw: the operator, its arguments and the value drawn.
type Logged = (StdOp, Vec<Value>, Value);

/// The reason a random draw is refused when the validator cannot place it
/// in the run's streams.
pub const RANDOM_UNPLACED: &str = "drawn outside the initial predicate and the next-state relation (in an ASSUME after constant draws, an invariant, a constraint, a property or a label), or under an unproved VIEW or SYMMETRY: the run draws it at a position of its stream that only the successor generator determines, so the independent validator cannot replay it (ADR 0016, amendment 2)";

/// What the validator provides of a checking run: the level of the state
/// being checked (`TLCGet("level")`), `TLCGet("config")` from the host the
/// model was bound with, if any, and, with a [`Replay`], the run's random
/// streams for the checks that can place them. Registers are the
/// evaluator's own and follow the trace's history: a register a
/// specification writes while checking depends on the order of TLC's
/// search, so the validator sees the values the trace's own steps wrote.
/// The live counters depend on the search, not on the trace, so they are
/// refused by name.
#[derive(Debug)]
struct TraceHost {
    level: AtomicI64,
    inner: Option<Arc<dyn Host>>,
    /// The run's streams, in `dive-stdlib`'s [`RunHost`], whose random
    /// operators are the checker's own (TLC's algorithms), when replaying.
    replay: Option<RunHost>,
    /// Whether the replay host points at the stream of the check being
    /// made; a draw is refused otherwise.
    placed: AtomicBool,
    /// Draws attempted so far.
    draws: AtomicU64,
    /// The draws logged while recording.
    log: Mutex<Option<Vec<Logged>>>,
}

impl Host for TraceHost {
    fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Option<Result<Value, HostError>> {
        let replay = self.replay.as_ref()?;
        self.draws.fetch_add(1, Ordering::Relaxed);
        if !self.placed.load(Ordering::Relaxed) {
            return Some(Err(HostError {
                code: 0,
                message: format!("{}, {RANDOM_UNPLACED}", op.qualified_name()),
            }));
        }
        let r = replay.random(op, args, ctx)?;
        if let Ok(v) = &r
            && let Some(log) = self
                .log
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_mut()
        {
            log.push((op, args.to_vec(), v.clone()));
        }
        Some(r)
    }

    fn level(&self) -> Option<i64> {
        Some(self.level.load(Ordering::Relaxed))
    }

    fn config(&self) -> Option<dive_eval::RunConfig> {
        self.inner.as_ref().and_then(|h| h.config())
    }
}

/// Validates a trace already read against `model`. With `replay`, random
/// draws in the initial predicate and the next-state relation are replayed
/// from the checking run's streams; without, they are refused by name.
#[must_use]
pub fn validate(model: &Model, trace: &Trace, replay: Option<&Replay>) -> Report {
    let admitted = model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(dive_eval::EvalError::from)
        .and_then(|()| {
            for pred in model
                .init
                .iter()
                .map(|n| &n.pred)
                .chain(model.next.iter())
                .chain(model.invariants.iter().map(|n| &n.pred))
                .chain(model.implied_inits.iter().map(|n| &n.pred))
                .chain(model.implied_actions.iter().map(|n| &n.pred))
                .chain(model.constraints.iter().map(|n| &n.pred))
                .chain(model.action_constraints.iter().map(|n| &n.pred))
                .chain(model.temporals.iter().map(|n| &n.pred))
                .chain(model.implied_temporals.iter().map(|n| &n.pred))
                .chain(model.configured_properties.iter().map(|n| &n.pred))
                .chain(model.view.iter().map(|n| &n.pred))
            {
                model.evaluator.validate_predicate(&pred.scope, pred.expr)?;
            }
            for &def in &model.assumptions {
                model.evaluator.scope(def)?;
            }
            Ok(())
        });
    if let Err(error) = admitted {
        return Report {
            validation: Validation::Unsupported(error.to_string()),
            notes: Vec::new(),
            length: trace.states.len(),
        };
    }
    let mut run = Run::new(model, replay);
    let validation = run.validate(trace);
    Report {
        validation,
        notes: std::mem::take(&mut run.notes).into_iter().collect(),
        length: trace.states.len(),
    }
}
