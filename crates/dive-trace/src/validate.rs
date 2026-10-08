//! The checks (ADR 0016 §1).
//!
//! Every check evaluates a predicate of the model over *given* states with
//! the core evaluator (`Model::eval`): `Init` as a state predicate on the
//! first state, the next-state relation as an action predicate on each pair
//! of states. Nothing here constructs a state; the successor enumerator is
//! not even compiled into this crate (ADR 0016 §2).

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use dive_eval::{AssumptionOutcome, ErrorClass, EvalError, Host, St, StateRef};
use dive_model::{Model, Pred};
use dive_value::Value;

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
pub const DEADLOCK_UNSUPPORTED: &str = "deadlock: that the last state has no successor cannot be checked without the successor enumerator, which the independent validator does not have (ADR 0016, amendment 1); every other check passed";

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
    pub(crate) passed: u64,
    pub(crate) notes: BTreeSet<String>,
}

impl<'a> Run<'a> {
    pub(crate) fn new(model: &'a Model) -> Self {
        let inner = model.evaluator.host();
        let host = Arc::new(TraceHost {
            level: AtomicI64::new(0),
            inner: inner.clone(),
        });
        model.evaluator.set_host(host.clone());
        Self {
            model,
            host,
            inner,
            passed: 0,
            notes: BTreeSet::new(),
        }
    }

    /// Validates `trace`.
    pub(crate) fn validate(&mut self, trace: &Trace) -> Validation {
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
        self.assumptions()?;
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
                    for init in &m.init {
                        self.require(&init.pred, &here, 0, Check::Init, || {
                            format!(
                                "the initial predicate {} is false in the first state",
                                m.location(&init.pred)
                            )
                        })?;
                    }
                }
                Some(step) => {
                    self.next(step, k, "the step into this state")?;
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
                if let Some(step) = &step {
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
        if let Ending::Lasso { back_to } = trace.ending {
            if m.view.is_some() {
                // TLC closes the loop on the VIEW's value, not the state's:
                // the last state's successor only has the same view as the
                // state it loops to (Examples' MCDistributedReplicatedLog).
                self.notes.insert("VIEW: TLC's loop closes on the view, not the state; the loop step is not checked".to_owned());
            } else {
                let step = St::action(trace.states[last].clone(), trace.states[back_to].clone());
                self.next(
                    &step,
                    last,
                    &format!(
                        "the loop from the last state back to state {} (numbered from 1)",
                        back_to + 1
                    ),
                )?;
            }
        }
        Ok(match goal {
            Goal::Deadlock => Some(DEADLOCK_UNSUPPORTED.to_owned()),
            Goal::Liveness(why) | Goal::ErrorEnd(why) => Some(why.to_owned()),
            Goal::Invariant(_) | Goal::ImpliedInit(_) | Goal::ImpliedAction(_) | Goal::Behavior => {
                None
            }
        })
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
                Goal::Liveness("liveness: the steps and the loop back (unless noted) were checked, but the temporal property is not (liveness counterexamples are not validated yet)")
            }
            Ending::Stuttering => Goal::Liveness("liveness: the steps were checked, but the temporal property is not (liveness counterexamples are not validated yet)"),
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
        match ev.eval_def(d, step) {
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

/// What the validator provides of a checking run: the level of the state
/// being checked (`TLCGet("level")`), and `TLCGet("config")` from the host
/// the model was bound with, if any. Registers are the evaluator's own and
/// follow the trace's history: a register a specification writes while
/// checking depends on the order of TLC's search, so the validator sees the
/// values the trace's own steps wrote. Randomness and the live counters
/// depend on the search, not on the trace, so they are refused by name.
#[derive(Debug)]
struct TraceHost {
    level: AtomicI64,
    inner: Option<Arc<dyn Host>>,
}

impl Host for TraceHost {
    fn level(&self) -> Option<i64> {
        Some(self.level.load(Ordering::Relaxed))
    }

    fn config(&self) -> Option<dive_eval::RunConfig> {
        self.inner.as_ref().and_then(|h| h.config())
    }
}

/// Validates a trace already read against `model`.
#[must_use]
pub fn validate(model: &Model, trace: &Trace) -> Report {
    let mut run = Run::new(model);
    let validation = run.validate(trace);
    Report {
        validation,
        notes: std::mem::take(&mut run.notes).into_iter().collect(),
        length: trace.states.len(),
    }
}
