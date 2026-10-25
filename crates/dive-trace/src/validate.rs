//! The checks (ADR 0016 §1).
//!
//! Every check evaluates a predicate of the model over *given* states with
//! the core evaluator (`Model::eval`): `Init` as a state predicate on the
//! first state, the next-state relation as an action predicate on each pair
//! of states. Private register-dependent Init delegates to an independent
//! single-candidate operation from empty slots. Whether a state has a
//! successor (a deadlock ending, `ENABLED`, fairness) is the core evaluator's
//! `ENABLED` search ([`crate::enabled`], ADR 0103).
//!
//! **Random draws** (`RandomElement`, the `Randomization` operators) depend
//! on the checking run's stream position, which only the successor
//! generator determines. A check that draws is refused by name
//! ([`RANDOM_UNSUPPORTED`]), whatever its outcome.
//! Register reads and writes are also refused ([`REGISTERS_UNSUPPORTED`]):
//! the evaluator may retain values written by off-trace checker evaluations.
//!
//! **Runtime names.** Under TLC's value or exploration order, strings and
//! model values compare by the order in which they were first encountered.
//! Names interned when the model was bound have the same order as in TLC.
//! If decoding the trace or any check interned a name after that
//! ([`Model::has_runtime_names`]), the validator cannot know where TLC
//! placed it, and the result, valid or invalid, becomes
//! [`RUNTIME_NAMES_UNSUPPORTED`].

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dive_eval::{AssumptionOutcome, ErrorClass, EvalError, Host, HostError, St, StateRef};
use dive_ir::StdOp;
use dive_model::{Model, Pred};
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

/// Actual progress before cooperative validation interruption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationProgress {
    /// Completed checks in this validation invocation, not exploration work.
    pub checks_passed: u64,
    /// Zero-based supplied state position, when known.
    pub state: Option<usize>,
    /// The check that was interrupted, when known.
    pub check: Option<Check>,
}

/// A controlled validation did not produce a validation report.
///
/// The driver interprets the opaque evaluator interruption. This crate knows
/// neither resource budgets nor the authority that requested interruption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationRunError {
    /// A typed host interruption, never an invalid or unsupported trace.
    Interrupted {
        /// Original evaluator transport, retaining its opaque owner identity.
        error: EvalError,
        /// Actual progress in this invocation.
        progress: ValidationProgress,
    },
    /// Allocation or evaluator resource exhaustion; no validation verdict exists.
    ResourceLimit {
        /// Original resource error, not a semantic capability refusal.
        error: EvalError,
        /// Actual progress before validation could not complete.
        progress: ValidationProgress,
    },
    /// An internal evaluator failure aborted validation, rather than refuting it.
    EvaluationFailure {
        /// Original internal evaluator error.
        error: EvalError,
        /// Work completed before the failure.
        progress: ValidationProgress,
    },
    /// The isolated validation thread could not run or did not return.
    ThreadFailure(String),
    /// Another thread owns the evaluator's control registration.
    ControlConflict(dive_eval::HostControlConflict),
}

impl std::fmt::Display for ValidationRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interrupted { error, .. }
            | Self::ResourceLimit { error, .. }
            | Self::EvaluationFailure { error, .. } => error.fmt(f),
            Self::ThreadFailure(message) => f.write_str(message),
            Self::ControlConflict(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ValidationRunError {}

/// The reason given for a deadlock ending (ADR 0016, amendment 1).
pub const DEADLOCK_UNSUPPORTED: &str = "deadlock: ENABLED of the next-state relation could not be decided on the last state; every other check passed (ADR 0016, amendment 1; ADR 0103)";

/// The reason given when the trace or a check created a name after the
/// model was bound, under an order where names compare by encounter.
pub const RUNTIME_NAMES_UNSUPPORTED: &str = "runtime string encounter order: decoding the trace or checking it created a string or model value that was not interned when the model was bound, and its order relative to other names depends on evaluations outside the trace (#124)";

/// The reason given when a check draws a random value.
pub const RANDOM_UNSUPPORTED: &str = "a random draw: the value depends on the checking run's stream position, which only the successor generator determines, so the independent validator cannot check it";

/// The reason given when a check accesses search-dependent TLC registers.
pub const REGISTERS_UNSUPPORTED: &str = "TLCGet/TLCSet registers: their contents depend on evaluations outside the trace, so the independent validator cannot read or write them";

/// Action composition has no independent intermediate-state decision path.
/// Workspace feature unification must not let replay borrow the checker.
pub const COMPOSITION_UNSUPPORTED: &str = "action composition: independent validation does not enumerate intermediate states with the checker";

/// What stopped a run early.
pub(crate) enum Stop {
    Invalid(Failure),
    Unsupported(String),
    Interrupted {
        error: EvalError,
        state: Option<usize>,
        check: Option<Check>,
    },
    ControlConflict(dive_eval::HostControlConflict),
}

impl Stop {
    fn at(self, state: Option<usize>, check: Check) -> Self {
        match self {
            Self::Interrupted { error, .. } => Self::Interrupted {
                error,
                state,
                check: Some(check),
            },
            other => other,
        }
    }
}

impl From<Stop> for Validation {
    fn from(s: Stop) -> Self {
        match s {
            Stop::Invalid(f) => Self::Invalid(f),
            Stop::Unsupported(u) => Self::Unsupported(u),
            Stop::Interrupted { .. } => Self::Unsupported(
                "the caller interrupted validation; use validate_controlled for typed transport"
                    .into(),
            ),
            Stop::ControlConflict(error) => Self::Unsupported(error.to_string()),
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
    TemporalStateError,
}

/// One validation run. `passed` counts the checks passed, in order: the
/// TLC reader uses it to compare interpretations of a lossy document.
pub(crate) struct Run<'a> {
    model: &'a Model,
    host: Arc<TraceHost>,
    /// The host installed before the run, restored after it.
    inner: Option<Arc<dyn Host>>,
    pub(crate) passed: u64,
    pub(crate) notes: BTreeSet<String>,
    private_init: bool,
    pub(crate) init_incomplete: bool,
    endpoint_prepared: bool,
    endpoint_resource: Option<(EvalError, Option<usize>, Check)>,
    control: Option<Arc<dyn dive_eval::HostControl>>,
}

impl<'a> Run<'a> {
    pub(crate) fn new(model: &'a Model) -> Self {
        let inner = model.evaluator.host();
        let host = Arc::new(TraceHost {
            level: AtomicI64::new(0),
            inner: inner.clone(),
            drew: Mutex::new(None),
            any: AtomicBool::new(false),
            registers: AtomicBool::new(false),
            composition: AtomicBool::new(false),
            register_used: AtomicBool::new(false),
            startup: AtomicBool::new(false),
            effect: Mutex::new(None),
            private: false,
            run_config: Mutex::new(None),
            cache_domain: next_cache_domain(),
        });
        model.evaluator.set_host(host.clone());
        Self {
            model,
            host,
            inner,
            passed: 0,
            notes: BTreeSet::new(),
            private_init: false,
            init_incomplete: false,
            endpoint_prepared: false,
            endpoint_resource: None,
            control: None,
        }
    }

    pub(crate) fn private(model: &'a Model, host: Arc<TraceHost>, initial: bool) -> Self {
        let inner = model.evaluator.replace_host(Some(host.clone()));
        Self {
            model,
            host,
            inner,
            passed: 0,
            notes: BTreeSet::new(),
            private_init: initial,
            init_incomplete: false,
            endpoint_prepared: false,
            endpoint_resource: None,
            control: None,
        }
    }

    /// Validates `trace`.
    // Legacy document/session callers have a closed result type. Controlled
    // callers bypass this conversion through validate_result, including label,
    // ENABLED/fairness and final refusal handling.
    pub(crate) fn validate(&mut self, trace: &Trace) -> Validation {
        self.validate_result(trace).unwrap_or_else(Validation::from)
    }

    fn validate_result(&mut self, trace: &Trace) -> Result<Validation, Stop> {
        let v = match self.run(trace) {
            Ok(None) => Validation::Valid,
            Ok(Some(why)) => Validation::Unsupported(why),
            Err(stop @ (Stop::Interrupted { .. } | Stop::ControlConflict(_))) => return Err(stop),
            Err(stop) => stop.into(),
        };
        let v = self.refusal().map_or(v, Validation::Unsupported);
        if v == Validation::Valid {
            self.poll(None, None)?;
        }
        Ok(v)
    }

    fn poll(&self, state: Option<usize>, check: Option<Check>) -> Result<(), Stop> {
        if let Some(control) = &self.control {
            control.check().map_err(|interrupted| Stop::Interrupted {
                error: EvalError::interrupted(interrupted),
                state,
                check,
            })?;
        }
        Ok(())
    }

    /// Why the run's verdict cannot stand, whatever it is: a check drew a
    /// random value, accessed registers, or a name was created after binding.
    pub(crate) fn refusal(&self) -> Option<String> {
        if let Some(effect) = self
            .host
            .effect
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        {
            return Some(effect);
        }
        if self.host.any.load(Ordering::Relaxed) {
            let op = self
                .host
                .drew
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .map_or_else(|| "a random operator".to_owned(), StdOp::qualified_name);
            return Some(format!("{op}: {RANDOM_UNSUPPORTED}"));
        }
        if self.host.registers.load(Ordering::Relaxed) {
            return Some(REGISTERS_UNSUPPORTED.to_owned());
        }
        if self.host.composition.load(Ordering::Relaxed) {
            return Some(COMPOSITION_UNSUPPORTED.to_owned());
        }
        self.model
            .has_runtime_names()
            .then(|| RUNTIME_NAMES_UNSUPPORTED.to_owned())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the checks of ADR 0016 §1, in the order the search makes them"
    )]
    fn run(&mut self, trace: &Trace) -> Result<Option<String>, Stop> {
        self.poll(None, None)?;
        let m = self.model;
        if matches!(trace.ending, Ending::TemporalStateError { .. })
            && (!self.host.private || !self.endpoint_prepared)
        {
            return Err(Stop::Unsupported(
                "temporal error endpoint requires a privately reconstructed source session".into(),
            ));
        }
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
            if k.is_multiple_of(1024) {
                self.poll(Some(k), Some(Check::Shape))?;
            }
            if s.len() != vars || s.iter().any(Option::is_none) {
                return Err(invalid(
                    k,
                    Check::Shape,
                    "the state does not assign every variable",
                ));
            }
            crate::trace::validate_state_names(m, s).map_err(Stop::Unsupported)?;
        }
        let named_lasso = !trace.properties.is_empty();
        if named_lasso {
            self.host.startup.store(false, Ordering::Relaxed);
            self.prepare_periodic(&trace.ending, &trace.properties)?;
            self.host
                .startup
                .store(self.private_init, Ordering::Relaxed);
        }
        self.assumptions()?;
        if self.private_init {
            self.host
                .startup
                .store(self.private_init, Ordering::Relaxed);
        }
        let last = n - 1;
        let goal = self.goal(&trace.ending, last)?;
        let full_last = !matches!(goal, Goal::Invariant(_));
        let behavior = matches!(goal, Goal::Behavior);
        for k in 0..n {
            if k.is_multiple_of(1024) {
                self.poll(Some(k), None)?;
            }
            let here = St::state(trace.states[k].clone());
            let step =
                (k > 0).then(|| St::action(trace.states[k - 1].clone(), trace.states[k].clone()));
            match &step {
                None => {
                    // Init uses startup level 0; generated-state predicates
                    // keep the separate Clear context in `here` (level 1).
                    if self.private_init {
                        // The direct random relation evaluates its real domain
                        // conservatively, with registers refused. Startup register
                        // effects and random relations cannot share this route.
                        self.host.startup.store(false, Ordering::Relaxed);
                        let random = if self.host.register_used.load(Ordering::Relaxed) {
                            Ok(None)
                        } else {
                            crate::initial::random_check(m, &trace.states[0])
                        };
                        let relation = !matches!(&random, Ok(None));
                        let operation = match random {
                            Ok(Some(member)) => Ok(member),
                            Ok(None) => {
                                self.host.startup.store(true, Ordering::Relaxed);
                                crate::initial::check(m, &trace.states[0])
                            }
                            Err(error) => Err(error),
                        };
                        let member = match operation {
                            Ok(member) => member,
                            Err(feature) => {
                                self.init_incomplete = !relation;
                                return Err(Stop::Unsupported(feature));
                            }
                        };
                        if !member {
                            return Err(invalid(
                                0,
                                Check::Init,
                                "the first state is outside the independent Init relation",
                            ));
                        }
                        self.passed += 1;
                    } else {
                        let initial = St::initial(trace.states[0].clone());
                        for init in &m.init {
                            self.require(&init.pred, &initial, 0, Check::Init, || {
                                format!(
                                    "the initial predicate {} is false in the first state",
                                    m.location(&init.pred)
                                )
                            })?;
                        }
                    }
                    self.host.startup.store(false, Ordering::Relaxed);
                }
                Some(step) => {
                    if !(named_lasso && trace.states[k - 1] == trace.states[k]) {
                        // Otherwise the implicit full-state stuttering closure.
                        self.next(step, k, "the step into this state")?;
                    }
                    if let Some(label) = &trace.labels[k] {
                        self.label(&label.name, step, k)?;
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
                self.next(&step, last, &what)?;
            }
        }
        Ok(match goal {
            Goal::Deadlock => {
                if let Some(next) = &m.next {
                    match crate::enabled::enabled(m, next, None, &trace.states[last]) {
                        Ok(false) => None,
                        Ok(true) => {
                            return Err(invalid(
                                last,
                                Check::Ending,
                                "the claimed deadlock has a successor",
                            ));
                        }
                        Err(e) => {
                            if let crate::enabled::EnabledError::Interrupted(error) = e {
                                return Err(refused(&error).at(Some(last), Check::Ending));
                            }
                            self.notes.insert(e.to_string());
                            Some(DEADLOCK_UNSUPPORTED.to_owned())
                        }
                    }
                } else {
                    Some(DEADLOCK_UNSUPPORTED.to_owned())
                }
            }
            // The fallible entrypoint separately reconstructs the program
            // before this path validation and replays it afterward.
            Goal::TemporalStateError => None,
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
        if self.model.view.is_some()
            || self.model.symmetry_reduction.is_some()
            || (self.model.symmetry.is_some() && !self.model.unreduced_symmetry_liveness())
        {
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
            crate::temporal::check_periodic_context(self.model, pred).map_err(|error| match error {
                crate::temporal::TemporalError::Interrupted(error) => refused(&error),
                other => Stop::Unsupported(other.to_string()),
            })
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
                self.model.evaluator.scope(d).map_err(|error| {
                    if error.class() == ErrorClass::Interrupted {
                        refused(&error).at(None, Check::Assumption)
                    } else {
                        Stop::Unsupported(error.to_string())
                    }
                })?,
            ))?;
        }
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
            Err(TemporalError::Interrupted(error)) => {
                Err(refused(&error).at(None, Check::Temporal))
            }
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
            Ending::TemporalStateError { .. } => Goal::TemporalStateError,
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
        // Binding and ASSUME remain explicit uninterruptible units. This
        // thread-owned frame restores control on every return/unwind without
        // disabling another evaluator or any worker's installed capability.
        let _assumptions = if self.control.is_some() && !self.endpoint_prepared {
            Some(
                self.model
                    .evaluator
                    .host_control(None)
                    .map_err(Stop::ControlConflict)?,
            )
        } else {
            None
        };
        match self.model.check_assumptions() {
            AssumptionOutcome::AllTrue => {
                self.passed += 1;
                Ok(())
            }
            AssumptionOutcome::False { message, .. } => Err(invalid(0, Check::Assumption, message)),
            AssumptionOutcome::Error { error, message, .. } => match error.class() {
                ErrorClass::Eval => Err(invalid(0, Check::Assumption, message)),
                _ => Err(self.evaluation_refusal(error, None, Check::Assumption)),
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
    fn label(&mut self, name: &str, step: &St, k: usize) -> Result<(), Stop> {
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
        let r = ev.eval_def(d, step);
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
            Err(error) if error.class() == ErrorClass::ResourceLimit && self.endpoint_prepared => {
                Err(self.evaluation_refusal(error, Some(k), Check::Label))
            }
            Err(error) if error.class() == ErrorClass::Interrupted => {
                Err(refused(&error).at(Some(k), Check::Label))
            }
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

    fn evaluation_refusal(&mut self, error: EvalError, state: Option<usize>, check: Check) -> Stop {
        // Legacy finite validation keeps its closed report channel. The new
        // fallible endpoint path retains original resource transport separately
        // and intercepts it before constructing any ValidationReport.
        if self.endpoint_prepared && error.class() == ErrorClass::ResourceLimit {
            self.endpoint_resource = Some((error.clone(), state, check));
        }
        refused(&error).at(state, check)
    }

    fn holds(&mut self, pred: &Pred, st: &St, k: usize, check: Check) -> Result<bool, Stop> {
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
                _ => Err(self.evaluation_refusal(e, Some(k), check)),
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
    if e.class() == ErrorClass::Interrupted {
        return Stop::Interrupted {
            error: e.clone(),
            state: None,
            check: None,
        };
    }
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

/// What the validator provides of a checking run: the level of the state
/// being checked (`TLCGet("level")`) and `TLCGet("config")` from the host the
/// model was bound with, if any. Registers and live counters depend on
/// evaluations outside the trace, so they are refused by name. Register
/// access and random draws are remembered so the whole verdict is refused,
/// including when an optional label check swallowed the evaluation error.
#[derive(Debug)]
pub(crate) struct TraceHost {
    level: AtomicI64,
    inner: Option<Arc<dyn Host>>,
    /// The first operator that drew.
    drew: Mutex<Option<StdOp>>,
    /// Whether any check drew.
    any: AtomicBool,
    /// Whether any check attempted a register read or write.
    pub(crate) registers: AtomicBool,
    /// Whether evaluation tried to borrow composition's intermediate search.
    composition: AtomicBool,
    pub(crate) register_used: AtomicBool,
    pub(crate) startup: AtomicBool,
    effect: Mutex<Option<String>>,
    private: bool,
    pub(crate) run_config: Mutex<Option<dive_eval::RunConfig>>,
    /// This host's own expression cache domain: replay reuses lazy values it
    /// computed itself, as TLC's `LazyValue` does, but never values another
    /// consumer cached (ADR 0103).
    cache_domain: u64,
}

/// A cache domain no other replay host has used (0 is ordinary evaluation's).
pub(crate) fn next_cache_domain() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl TraceHost {
    pub(crate) fn startup() -> Arc<Self> {
        Arc::new(Self {
            level: AtomicI64::new(0),
            inner: None,
            drew: Mutex::new(None),
            any: AtomicBool::new(false),
            registers: AtomicBool::new(false),
            composition: AtomicBool::new(false),
            register_used: AtomicBool::new(false),
            startup: AtomicBool::new(true),
            effect: Mutex::new(None),
            private: true,
            run_config: Mutex::new(None),
            cache_domain: next_cache_domain(),
        })
    }

    fn private_host(&self) -> bool {
        self.private
    }

    fn refuse_effect(&self, feature: String) -> Result<(), HostError> {
        let mut effect = self.effect.lock().unwrap_or_else(PoisonError::into_inner);
        if effect.is_none() {
            *effect = Some(feature.clone());
        }
        Err(HostError {
            code: 0,
            message: feature,
        })
    }
}

impl Host for TraceHost {
    fn check_native_access(&self, op: StdOp) -> Result<(), HostError> {
        if self.private_host()
            && matches!(
                op,
                StdOp::TlcRandomElement
                    | StdOp::RandomizationRandomSubset
                    | StdOp::RandomizationRandomSetOfSubsets
                    | StdOp::RandomizationRandomSubsetSet
            )
        {
            if !self.any.swap(true, Ordering::Relaxed) {
                *self.drew.lock().unwrap_or_else(PoisonError::into_inner) = Some(op);
            }
            return Err(HostError {
                code: 0,
                message: format!("{}: {RANDOM_UNSUPPORTED}", op.qualified_name()),
            });
        }
        if self.private_host()
            && op.impure()
            && !matches!(
                op,
                StdOp::TlcTLCGet | StdOp::TlcTLCSet | StdOp::TlcPrint | StdOp::TlcPrintT
            )
        {
            return self.refuse_effect(format!(
                "{}: private trace startup does not support this host effect",
                op.qualified_name()
            ));
        }
        Ok(())
    }

    fn print_output(&self, _text: &str) -> Result<bool, HostError> {
        // Reconstruct Print's value semantics without invoking any sink or
        // counting replay/startup reconstruction as producer output. The
        // evaluator still checks arguments and performs normal rendering.
        Ok(false)
    }

    fn check_tlc_access(&self, key: &str, write: bool) -> Result<(), HostError> {
        if self.private_host()
            && !(key.starts_with("s:")
                || !write && matches!(key, "config" | "level" | "all" | "all:named"))
        {
            return self.refuse_effect(format!(
                "TLC{}({key:?}): private trace startup does not support this host effect",
                if write { "Set" } else { "Get" }
            ));
        }
        Ok(())
    }
    fn check_register_access(&self) -> Result<(), HostError> {
        self.register_used.store(true, Ordering::Relaxed);
        if self.startup.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.registers.store(true, Ordering::Relaxed);
        Err(HostError {
            code: 0,
            message: REGISTERS_UNSUPPORTED.to_owned(),
        })
    }

    fn check_action_composition(&self) -> Result<(), HostError> {
        self.composition.store(true, Ordering::Relaxed);
        Err(HostError {
            code: 0,
            message: COMPOSITION_UNSUPPORTED.to_owned(),
        })
    }

    fn expression_cache_domain(&self) -> u64 {
        self.cache_domain
    }

    fn random(&self, op: StdOp, _args: &[Value], _ctx: Ctx) -> Option<Result<Value, HostError>> {
        if !self.any.swap(true, Ordering::Relaxed) {
            *self.drew.lock().unwrap_or_else(PoisonError::into_inner) = Some(op);
        }
        None
    }

    fn level(&self) -> Option<i64> {
        Some(self.level.load(Ordering::Relaxed))
    }

    fn config(&self) -> Option<dive_eval::RunConfig> {
        self.run_config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .or_else(|| self.inner.as_ref().and_then(|h| h.config()))
    }
}

/// Validates a trace already read against `model`. Random draws are refused
/// by name ([`RANDOM_UNSUPPORTED`]).
#[must_use]
pub fn validate(model: &Model, trace: &Trace) -> Report {
    let admitted = model.validate_context().and_then(|()| {
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
    let mut run = Run::new(model);
    let validation = run.validate(trace);
    Report {
        validation,
        notes: std::mem::take(&mut run.notes).into_iter().collect(),
        length: trace.states.len(),
    }
}

/// Validates supplied states with a generic cooperative host capability.
///
/// Interruption is returned outside [`Validation`]: it is neither an invalid
/// trace nor an unsupported feature. Only the owning driver can interpret its
/// opaque identity. Binding, `ASSUME`, quiet value units and pure bulk/native
/// work retain their interruption limitations. Supplied deferred values may
/// retain a different evaluator, whose work this control does not govern.
/// This does not control TLC JSON
/// interpretation or private [`crate::Session`] reconstruction.
///
/// ```
/// # fn run(model: &dive_model::Model, trace: &dive_trace::Trace,
/// #        control: std::sync::Arc<dyn dive_eval::HostControl>)
/// #        -> Result<dive_trace::Report, dive_trace::ValidationRunError> {
/// dive_trace::validate_controlled(model, trace, control)
/// # }
/// ```
///
/// # Errors
/// Typed host interruption with actual validation progress, or a conflicting
/// control registration owned by another thread. Ordinary invalid/unsupported
/// trace reports remain ordinary reports.
pub fn validate_controlled(
    model: &Model,
    trace: &Trace,
    control: Arc<dyn dive_eval::HostControl>,
) -> Result<Report, ValidationRunError> {
    let _scope = model
        .evaluator
        .host_control(Some(control.clone()))
        .map_err(ValidationRunError::ControlConflict)?;
    let admitted = model.validate_context().and_then(|()| {
        for &def in &model.assumptions {
            model.evaluator.scope(def)?;
        }
        Ok(())
    });
    if let Err(error) = admitted {
        if error.class() == ErrorClass::Interrupted {
            return Err(ValidationRunError::Interrupted {
                error,
                progress: ValidationProgress {
                    checks_passed: 0,
                    state: None,
                    check: None,
                },
            });
        }
        return Ok(Report {
            validation: Validation::Unsupported(error.to_string()),
            notes: Vec::new(),
            length: trace.states.len(),
        });
    }
    let mut run = Run::new(model);
    run.control = Some(control);
    controlled_report(&mut run, trace)
}

#[allow(
    clippy::too_many_lines,
    reason = "finite validation and independent endpoint phases share one fallible report boundary"
)]
fn controlled_report(run: &mut Run<'_>, trace: &Trace) -> Result<Report, ValidationRunError> {
    let endpoint = if matches!(trace.ending, Ending::TemporalStateError { .. }) {
        if !run.host.private {
            return Ok(Report {
                validation: Validation::Unsupported(
                    "temporal error endpoint requires a privately reconstructed source session"
                        .into(),
                ),
                notes: Vec::new(),
                length: trace.states.len(),
            });
        }
        if !trace.properties.is_empty() {
            return Ok(Report {
                validation: Validation::Invalid(Failure {
                    state: 0,
                    check: Check::Shape,
                    detail: "finite error endpoints cannot claim temporal properties".into(),
                }),
                notes: Vec::new(),
                length: trace.states.len(),
            });
        }
        match crate::row_demand::prepare(run.model, &trace.states, run.control.as_deref()) {
            Ok(program) => {
                run.endpoint_prepared = true;
                Some(program)
            }
            Err(error) => return endpoint_refusal(run, trace, error, None),
        }
    } else {
        None
    };
    let result = run.validate_result(trace);
    if let Some((error, state, check)) = run.endpoint_resource.take() {
        return Err(ValidationRunError::ResourceLimit {
            error,
            progress: ValidationProgress {
                checks_passed: run.passed,
                state,
                check: Some(check),
            },
        });
    }
    let mut validation = match result {
        Ok(validation) => validation,
        Err(Stop::Interrupted {
            error,
            state,
            check,
        }) => {
            return Err(ValidationRunError::Interrupted {
                error,
                progress: ValidationProgress {
                    checks_passed: run.passed,
                    state,
                    check,
                },
            });
        }
        Err(Stop::ControlConflict(error)) => {
            return Err(ValidationRunError::ControlConflict(error));
        }
        Err(other) => other.into(),
    };
    if validation == Validation::Valid
        && let Some(program) = endpoint
    {
        let Some(last) = trace.states.len().checked_sub(1) else {
            return Ok(Report {
                validation: Validation::Invalid(Failure {
                    state: 0,
                    check: Check::Shape,
                    detail: "endpoint has no state".into(),
                }),
                notes: Vec::new(),
                length: 0,
            });
        };
        let Ending::TemporalStateError { ec, endpoint } = &trace.ending else {
            return Ok(Report {
                validation: Validation::Unsupported("missing endpoint ending".into()),
                notes: Vec::new(),
                length: trace.states.len(),
            });
        };
        match program.replay(
            run.model,
            &trace.states[last],
            ec,
            endpoint,
            run.control.as_deref(),
        ) {
            Ok(Ok(())) => {}
            Ok(Err(detail)) => {
                validation = Validation::Invalid(Failure {
                    state: last,
                    check: Check::Ending,
                    detail,
                })
            }
            Err(error) => return endpoint_refusal(run, trace, error, Some(last)),
        }
        if let Some(control) = &run.control {
            control
                .check()
                .map_err(|error| ValidationRunError::Interrupted {
                    error: EvalError::interrupted(error),
                    progress: progress(run, Some(last)),
                })?;
        }
    }
    if let Some(feature) = run.refusal() {
        validation = Validation::Unsupported(feature);
    }
    Ok(Report {
        validation,
        notes: std::mem::take(&mut run.notes).into_iter().collect(),
        length: trace.states.len(),
    })
}

fn progress(run: &Run<'_>, state: Option<usize>) -> ValidationProgress {
    ValidationProgress {
        checks_passed: run.passed,
        state,
        check: Some(Check::Ending),
    }
}
fn endpoint_refusal(
    run: &Run<'_>,
    trace: &Trace,
    error: crate::row_demand::FormulaError,
    state: Option<usize>,
) -> Result<Report, ValidationRunError> {
    let feature = match error {
        crate::row_demand::FormulaError::Evaluation(error) => match error.class() {
            ErrorClass::Interrupted => {
                return Err(ValidationRunError::Interrupted {
                    error,
                    progress: progress(run, state),
                });
            }
            ErrorClass::ResourceLimit => {
                return Err(ValidationRunError::ResourceLimit {
                    error,
                    progress: progress(run, state),
                });
            }
            ErrorClass::Bug => {
                return Err(ValidationRunError::EvaluationFailure {
                    error,
                    progress: progress(run, state),
                });
            }
            _ => error.to_string(),
        },
        other => other.to_string(),
    };
    Ok(Report {
        validation: Validation::Unsupported(feature),
        notes: Vec::new(),
        length: trace.states.len(),
    })
}

pub(crate) fn validate_private_controlled(
    model: &Model,
    host: Arc<TraceHost>,
    trace: &Trace,
    control: Option<Arc<dyn dive_eval::HostControl>>,
) -> Result<Report, ValidationRunError> {
    let _scope = control
        .as_ref()
        .map(|control| model.evaluator.host_control(Some(control.clone())))
        .transpose()
        .map_err(ValidationRunError::ControlConflict)?;
    let mut run = Run::private(model, host, false);
    run.control = control;
    controlled_report(&mut run, trace)
}

/// `report`, refused if a name was created after the model was bound
/// ([`RUNTIME_NAMES_UNSUPPORTED`]).
pub(crate) fn guard_runtime_names(model: &Model, mut report: Report) -> Report {
    if model.has_runtime_names() {
        report.validation = Validation::Unsupported(RUNTIME_NAMES_UNSUPPORTED.to_owned());
    }
    report
}

/// Negative controls: by-construction invalid edits of a valid
/// breadth-first counterexample must be rejected.
#[cfg(test)]
mod negative_controls {
    #![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

    use std::sync::Arc;

    use super::{
        Check, ErrorClass, Run, TraceHost, Validation, ValidationRunError, endpoint_refusal,
        validate,
    };
    use crate::{Ending, Trace, TraceDoc};

    const SPEC: &str = "---- MODULE Count ----\nEXTENDS Naturals\nVARIABLE x\n\
        Init == x = 0\nNext == x' = x + 1\nSmall == x < 3\nTiny == x < 5\n====\n";

    fn model() -> dive_model::Model {
        let analysis = dive_sema::analyze_source("Count.tla", SPEC, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Tiny Small\n")
            .into_config()
            .expect("cfg");
        dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
    }

    fn trace(model: &dive_model::Model, xs: &[i64]) -> Trace {
        let states = xs.iter().map(|x| serde_json::json!({ "x": x })).collect();
        let doc = TraceDoc::behavior(states)
            .expect("states")
            .with_ending(Ending::Invariant("Small".to_owned()));
        Trace::decode(model, &doc).expect("decode")
    }

    fn rejected(model: &dive_model::Model, t: &Trace) -> Check {
        match validate(model, t).validation {
            Validation::Invalid(f) => f.check,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn endpoint_allocation_failure_has_no_completed_validation_report() {
        let model = model();
        let trace = trace(&model, &[0, 1]);
        let host = TraceHost::startup();
        let run = Run::private(&model, host, false);
        let error = crate::row_demand::reserve(&mut Vec::<u64>::new(), usize::MAX)
            .expect_err("actual allocation capacity overflow");
        let error =
            endpoint_refusal(&run, &trace, error, Some(1)).expect_err("no report on exhaustion");
        assert!(
            matches!(error,ValidationRunError::ResourceLimit{error,progress} if error.class()==ErrorClass::ResourceLimit&&progress.state==Some(1)&&progress.check==Some(Check::Ending))
        );
    }

    #[test]
    fn edits_of_a_valid_counterexample_are_rejected() {
        let m = model();
        let original = trace(&m, &[0, 1, 2, 3]);
        assert_eq!(validate(&m, &original).validation, Validation::Valid);
        // A dropped middle state, a dropped initial state, swapped states
        // and a changed value are not steps of the specification.
        assert_eq!(rejected(&m, &trace(&m, &[0, 1, 3])), Check::Next);
        assert_eq!(rejected(&m, &trace(&m, &[1, 2, 3])), Check::Init);
        assert_eq!(rejected(&m, &trace(&m, &[0, 2, 1, 3])), Check::Next);
        assert_eq!(rejected(&m, &trace(&m, &[0, 1, 7, 3])), Check::Next);
        // Cut short, the claimed invariant holds at the end.
        assert_eq!(rejected(&m, &trace(&m, &[0, 1, 2])), Check::Ending);
        // Another invariant is not violated at the end.
        let mut claim = original.clone();
        claim.ending = Ending::Invariant("Tiny".to_owned());
        assert_eq!(rejected(&m, &claim), Check::Ending);
        // An invariant the model does not have.
        claim.ending = Ending::Invariant("Missing".to_owned());
        assert_eq!(rejected(&m, &claim), Check::Ending);
    }
}
