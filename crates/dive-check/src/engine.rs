//! The search: ADR 0015's level engine.
//!
//! It reproduces TLC's single-worker breadth-first search
//! (`ModelChecker.DoInitFunctor`, `Worker.run`/`addElement`, tlaplus
//! `94d0c50`) event for event, in the reference order 𝒪 of ADR 0015 §1:
//! levels in order, states in frontier order, successors in enumeration
//! order, and within a successor the phases completeness, constraints,
//! invariants (on new or out-of-model states), implied actions (on every
//! step). The search stops at the first event (E\*): a violation, a
//! deadlock or an error.
//!
//! Two engines share this module's `Search` (the seen-state set, the trace
//! log, the counters, initial states and counterexample replay):
//!
//! - **inline** (`Search::inline_levels`): every level on the calling
//!   thread, inserting each successor into the set as TLC does. Sequential
//!   mode (ADR 0015 §6) uses it, because a spec that writes TLC's registers
//!   or reads its live counters must see them change exactly as TLC's one
//!   worker does.
//! - **parallel** (`parallel.rs`): the three-phase level engine of ADR
//!   0015 §2 on a pool of workers, at any worker count including 1.
//!
//! **Counts at E\*** (ADR 0015 §3). Inline, the level is held as a vector,
//! the state being expanded is `current[i]`, and the next level so far is
//! `next`: generated is the running total, distinct is the set's size (TLC
//! inserts a new state before checking it), and left on the queue is
//! `(m − i − 1) + |next|`. The parallel engine computes the same numbers
//! from §3's closed forms.
//!
//! **Counterexamples** (§4) are rebuilt by replay from the trace log:
//! regenerate the initial states, or a parent's successors, and take the
//! recorded ordinal. The replayed end state is checked against the state
//! the search held; any difference is an internal error, never a verdict.

use crate::capture::{BehaviorGraph, Collector};
use std::collections::BTreeSet;
use std::fmt::Write as _;

use dive_eval::next::{Action, Flow, StateVals, Successor};
use dive_eval::{Ec, EvalError, Evaluator, St};
use dive_ir::actions::ActionName;
use dive_store::predecessor::PredecessorLog;
use dive_store::state::{StateFingerprinter, state_key};
use dive_store::trace::StateId;
use dive_store::{ExactSet, FpSet, StateSet, StoreError, StoreMode, collision};
use dive_value::{Ctx, Value};

use dive_eval::{Counter, RunConfig};
use dive_stdlib::host::RunHost;
use dive_stdlib::random::{RandomMode, Streams};

use crate::class;
use crate::options::CheckOptions;
use crate::result::{Collision, Counterexample, ErrorSite, Stats, TraceState, Verdict, Workers};
use dive_model::{Model, Named};

/// The fingerprint functions of fingerprint mode.
pub(crate) struct FpKeyer {
    /// The state fingerprint.
    states: StateFingerprinter,
    /// The fingerprint of a `VIEW` value (`val.fingerPrint(FP64.New())`).
    view: StateFingerprinter,
}

/// A seen-state set the engines can key states for.
pub(crate) trait Keyed: StateSet + Sized + 'static
where
    Self::Key: 'static,
{
    /// What computes keys.
    type Keyer: Sync;

    /// Native immutable key bytes for checkpoint identity.
    fn checkpoint_key(key: &Self::Key) -> Vec<u8>;

    /// The keyer for a run with `opts`, given TLC's variable order.
    fn keyer(opts: &CheckOptions, tlc_order: &[usize]) -> Result<Self::Keyer, String>;

    /// An empty set with `2^bits` shards.
    fn fresh(bits: u32) -> Self;

    /// The key of the state with values `vals` (or, with `view`, of the
    /// state whose view value is `vals[0]`).
    fn key(
        keyer: &Self::Keyer,
        vals: &[Value],
        view: bool,
        ctx: Ctx,
    ) -> Result<Self::Key, StoreError>;

    /// Borrows the unchanged full identity for the fallible disk store.
    fn disk_key(key: &Self::Key) -> dive_store::seen::Key<'_>;

    /// Copies a graph identity key without evaluating the specification.
    fn duplicate_key(key: &Self::Key) -> Self::Key;

    /// TLC's collision figures after a complete search, in fingerprint mode.
    fn collision(&self, distinct: u64, generated: u64) -> Option<Collision>;
}

impl Keyed for FpSet {
    type Keyer = FpKeyer;

    fn checkpoint_key(key: &u64) -> Vec<u8> {
        key.to_le_bytes().to_vec()
    }

    fn keyer(opts: &CheckOptions, tlc_order: &[usize]) -> Result<FpKeyer, String> {
        let bad = || {
            format!(
                "fingerprint polynomial index {} out of range",
                opts.fp_index
            )
        };
        Ok(if opts.tlc_fingerprints {
            FpKeyer {
                states: StateFingerprinter::tlc(opts.fp_index, tlc_order).ok_or_else(bad)?,
                view: StateFingerprinter::tlc(opts.fp_index, &[0]).ok_or_else(bad)?,
            }
        } else {
            FpKeyer {
                states: StateFingerprinter::new(opts.fp_index).ok_or_else(bad)?,
                view: StateFingerprinter::new(opts.fp_index).ok_or_else(bad)?,
            }
        })
    }

    fn fresh(bits: u32) -> Self {
        Self::new(bits)
    }

    fn key(keyer: &FpKeyer, vals: &[Value], view: bool, ctx: Ctx) -> Result<u64, StoreError> {
        if view {
            keyer.view.fingerprint(vals, ctx)
        } else {
            keyer.states.fingerprint(vals, ctx)
        }
    }

    fn disk_key(key: &u64) -> dive_store::seen::Key<'_> {
        dive_store::seen::Key::Fingerprint(*key)
    }

    fn duplicate_key(key: &u64) -> u64 {
        *key
    }

    fn collision(&self, distinct: u64, generated: u64) -> Option<Collision> {
        let f = collision::tlc_success_figures(distinct, generated, || {
            self.tlc_min_gap(collision::TLC_DEFAULT_FP_BITS)
        });
        Some(Collision {
            optimistic: f.optimistic,
            actual: f.actual,
        })
    }
}

impl Keyed for ExactSet {
    type Keyer = ();

    fn checkpoint_key(key: &Box<[u8]>) -> Vec<u8> {
        key.to_vec()
    }

    fn keyer(_: &CheckOptions, _: &[usize]) -> Result<(), String> {
        Ok(())
    }

    fn fresh(bits: u32) -> Self {
        Self::new(bits)
    }

    fn key((): &(), vals: &[Value], _view: bool, ctx: Ctx) -> Result<Box<[u8]>, StoreError> {
        let mut buf = Vec::new();
        state_key(vals, ctx, &mut buf)?;
        Ok(buf.into_boxed_slice())
    }

    fn disk_key(key: &Box<[u8]>) -> dive_store::seen::Key<'_> {
        dive_store::seen::Key::Exact(key)
    }

    fn duplicate_key(key: &Box<[u8]>) -> Box<[u8]> {
        key.clone()
    }

    fn collision(&self, _: u64, _: u64) -> Option<Collision> {
        None
    }
}

/// The run's host (ADR 0014 §7): the random streams of `--seed`, TLC's
/// under `--compat=tlc` and dive's replayable ones otherwise (D-002), the
/// level, and the counters.
pub(crate) fn run_host(opts: &CheckOptions) -> RunHost {
    let mode = match opts.profile {
        dive_eval::Profile::Tlc => RandomMode::Tlc,
        dive_eval::Profile::Dive => RandomMode::Dive,
    };
    RunHost::new(Streams::new(opts.seed, mode), None)
}

/// `TLCGet("config")` (`AbstractChecker.createConfig`). The worker count is
/// the logical one, 1, by default and the requested one under
/// `--compat=tlc`, as TLC reports `-workers` (D-011); the install location
/// is dive's executable.
pub(crate) fn run_config(opts: &CheckOptions, deadlock: bool) -> RunConfig {
    #[allow(clippy::cast_possible_wrap, reason = "Java's long: same bits")]
    let fingerprint = dive_value::fp64::POLYS
        .get(opts.fp_index)
        .copied()
        .unwrap_or(0) as i64;
    RunConfig {
        deadlock,
        workers: match opts.profile {
            dive_eval::Profile::Tlc => i64::try_from(opts.workers).unwrap_or(i64::MAX),
            dive_eval::Profile::Dive => 1,
        },
        seed: opts.seed,
        fingerprint,
        install: std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}

/// The counters while TLC checks assumptions: no state yet, diameter 1
/// (measured on the pinned TLC, fixture `TgAssume`).
pub(crate) fn startup_counters(host: &RunHost) {
    for c in [
        Counter::Generated,
        Counter::Distinct,
        Counter::Queue,
        Counter::Initial,
    ] {
        host.set_counter(c, Some(0));
    }
    host.set_counter(Counter::Diameter, Some(1));
}

/// During the search the live counters are not provided: reading one is
/// refused rather than answered with a count TLC may not report at that
/// point.
fn search_counters(host: &RunHost) {
    for c in [
        Counter::Generated,
        Counter::Distinct,
        Counter::Queue,
        Counter::Initial,
        Counter::Diameter,
        Counter::DurationSecs,
    ] {
        host.set_counter(c, None);
    }
}

/// Whether the specification refers to a random operator, so that expanded
/// states need their own random stream.
fn random_op(op: dive_ir::StdOp) -> bool {
    use dive_ir::StdOp;
    matches!(
        op,
        StdOp::TlcRandomElement
            | StdOp::RandomizationRandomSubset
            | StdOp::RandomizationRandomSetOfSubsets
            | StdOp::RandomizationRandomSubsetSet
    )
}

/// Whether a definition outside the embedded standard modules, or a node that
/// belongs to no definition, can reach a random operator through the
/// definitions it references. A standard module that merely defines a random
/// helper, such as `Randomization`'s `TestRandomSetOfSubsets`, does not make
/// a specification that never calls it random. Any reference counts, used or
/// not, so an unused random helper in a user module still does. A native
/// definition itself is not a reference: `INSTANCE` of a module that extends
/// `TLC` copies `RandomElement` into the instantiating module without calling it.
pub(crate) fn uses_random(ir: &dive_ir::Ir) -> bool {
    use dive_ir::{Expr, OpRef};
    let defs = ir.defs();
    let mut owned = vec![false; ir.nodes().len()];
    // Direct references of each definition's body subtree.
    let mut refs: Vec<Vec<usize>> = Vec::with_capacity(defs.len());
    // Whether a definition's own body calls a random native operator.
    let mut direct = vec![false; defs.len()];
    let referenced = |e: &Expr, out: &mut Vec<usize>| -> bool {
        let mut random = false;
        if let Expr::Def(d) = e {
            out.push(d.index());
        }
        for r in e.op_refs() {
            match r {
                OpRef::Std(op) => random |= random_op(*op),
                OpRef::Def(d) => out.push(d.index()),
                _ => {}
            }
        }
        random
    };
    for (i, def) in defs.iter().enumerate() {
        let mut out = Vec::new();
        let mut stack = vec![def.body];
        while let Some(e) = stack.pop() {
            let Some(slot) = owned.get_mut(e.index()) else {
                // A malformed reference: keep the conservative answer.
                return true;
            };
            *slot = true;
            let node = ir.node(e);
            direct[i] |= referenced(&node.expr, &mut out);
            stack.extend(node.expr.children());
        }
        refs.push(out);
    }
    // Close over references until no definition changes. Natives seed it.
    let mut random: Vec<bool> = defs
        .iter()
        .zip(&direct)
        .map(|(def, direct)| *direct || def.std.is_some_and(random_op))
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..defs.len() {
            if !random[i]
                && refs[i]
                    .iter()
                    .any(|&d| random.get(d).copied().unwrap_or(true))
            {
                random[i] = true;
                changed = true;
            }
        }
    }
    let reaches = |out: &[usize]| out.iter().any(|&d| random.get(d).copied().unwrap_or(true));
    let user = defs
        .iter()
        .zip(direct.iter().zip(&refs))
        .any(|(def, (direct, out))| !ir.module(def.module).standard && (*direct || reaches(out)));
    user || ir.nodes().iter().zip(&owned).any(|(node, owned)| {
        let mut out = Vec::new();
        !owned && (referenced(&node.expr, &mut out) || reaches(&out))
    })
}

// A supplied bound model no longer carries its source cfg. Inspect admitted
// constant bindings and actual redirects without invoking any expression or
// lazy callback. Unused native imports alone must not refuse pure captures.
pub(crate) fn bound_configured_random(model: &Model) -> Result<bool, String> {
    // The shared capture entry admission already verifies full model context.
    // Keep the helper defensive against any future caller as well.
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| e.to_string())?;
    let native = |id: dive_ir::DefId| -> Result<bool, String> {
        if model.ir.defs().get(id.index()).is_none() {
            return Err("DIVE-EVAL-C001: invalid configured native definition".into());
        }
        let id = model.evaluator.redirected(id);
        let def = model
            .ir
            .defs()
            .get(id.index())
            .ok_or_else(|| "DIVE-EVAL-C001: invalid redirected native definition".to_owned())?;
        Ok(def.std.is_some_and(random_op))
    };
    for (index, _) in model.ir.consts().iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| "DIVE-EVAL-C005: constant index overflow".to_owned())?;
        if let Some(dive_eval::ConstBinding::Def(id)) =
            model.evaluator.constant_binding(dive_ir::ConstId(index))
            && native(id)?
        {
            return Ok(true);
        }
    }
    for (index, _) in model.ir.defs().iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| "DIVE-EVAL-C005: definition index overflow".to_owned())?;
        let id = dive_ir::DefId(index);
        if model.evaluator.redirected(id) != id && native(id)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// TLC's level of a state at BFS level `n` (initial states are level 1).
pub(crate) fn level_of(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// How an engine sets `TLCGet("level")` around a successor's checks: the
/// successor's level (`here`) for state checks, the expanded state's
/// (`from`) for action checks.
pub(crate) struct Levels<'a> {
    pub(crate) set: &'a dyn Fn(i64),
    pub(crate) here: i64,
    pub(crate) from: i64,
}

impl Levels<'_> {
    pub(crate) fn here(&self) {
        (self.set)(self.here);
    }

    pub(crate) fn from(&self) {
        (self.set)(self.from);
    }
}

/// What stopped the search, and how to print its counterexample.
pub(crate) struct Stop {
    pub(crate) verdict: Verdict,
    /// Whether the stop happened while generating the initial states.
    pub(crate) in_init: bool,
    /// The counterexample: the path to this state (id and values)...
    pub(crate) path: Option<(StateId, StateVals)>,
    /// ...followed by this state.
    pub(crate) then: Option<TraceState>,
}

/// What TLC's call-stack replay changes after an invariant failed to
/// evaluate on a new state (`Search::replay_after`).
#[derive(Default)]
pub(crate) struct Replayed {
    /// States the replay inserted into the seen-state set.
    pub(crate) distinct: u64,
    /// States it enqueued.
    pub(crate) queued: u64,
    /// The verdict of a further error it met, which decides TLC's class.
    pub(crate) error: Option<Verdict>,
}

impl Replayed {
    /// The stop's verdict once the replay is accounted for.
    pub(crate) fn apply(self, v: Verdict) -> Verdict {
        match self.error {
            Some(Verdict::EvaluationError { cause, .. }) => class::replay_error(v, &cause),
            Some(refusal) if !refusal.is_verdict() => refusal,
            None | Some(_) => v,
        }
    }
}

/// Whether a verdict is an invariant that failed to evaluate, after which
/// TLC's replay goes on past the state (`Search::replay_after`).
pub(crate) fn resumes(v: &Verdict) -> bool {
    matches!(
        v,
        Verdict::EvaluationError {
            site: ErrorSite::Invariant,
            ..
        }
    )
}

/// The name an action is reported under.
pub(crate) fn action_name(ev: &Evaluator, a: &Action) -> Option<String> {
    match a.name() {
        ActionName::Def(d) => Some(ev.ir().def(d).name.clone()),
        ActionName::Std(op) => Some(op.name().to_owned()),
        ActionName::Builtin(name) => Some(name.to_owned()),
        ActionName::Unnamed => None,
    }
}

/// The verdict for an error raised at `site` while exploring, classified as
/// TLC classifies it (`class.rs`).
pub(crate) fn error_verdict(
    e: &EvalError,
    site: ErrorSite,
    control: &crate::budget::EvaluationControl,
) -> Verdict {
    control
        .verdict(e)
        .unwrap_or_else(|| class::search_error(e, site, None))
}

pub(crate) fn store_verdict(e: &StoreError, control: &crate::budget::EvaluationControl) -> Verdict {
    match e {
        StoreError::Predecessor { .. } => Verdict::ResourceLimit {
            message: e.to_string(),
        },
        StoreError::Value(v) => {
            error_verdict(&EvalError::from(v.clone()), ErrorSite::Fingerprint, control)
        }
        other => Verdict::InternalError {
            message: other.to_string(),
        },
    }
}

pub(crate) fn values(s: &[Option<Value>]) -> Option<Vec<Value>> {
    s.iter().cloned().collect()
}

/// Extracts all concrete values, refusing an impossible incomplete key input.
pub(crate) fn complete_values(s: &[Option<Value>]) -> Result<Vec<Value>, Verdict> {
    values(s).ok_or_else(|| Verdict::InternalError {
        message: "state identity requires completely assigned concrete values".into(),
    })
}

#[cfg(test)]
mod concrete_value_tests {
    use super::{Value, Verdict, complete_values};

    #[test]
    fn missing_assignments_cannot_collapse_to_the_valid_zero_variable_identity() {
        assert_eq!(complete_values(&[]), Ok(Vec::new()));
        assert_eq!(
            complete_values(&[Some(Value::Int(7))]),
            Ok(vec![Value::Int(7)])
        );
        for state in [vec![None], vec![Some(Value::Int(7)), None]] {
            assert!(matches!(
                complete_values(&state),
                Err(Verdict::InternalError { .. })
            ));
        }
    }
}

/// Evaluates a role once, leaving mode-specific nonboolean/error diagnostics to callers.
pub(crate) fn role_boolean(m: &Model, c: &Named, st: &St) -> Result<Option<bool>, EvalError> {
    m.eval(&c.pred, st).map(|value| match value {
        Value::Bool(value) => Some(value),
        _ => None,
    })
}

/// Evaluates a check as a boolean. A value that is not a boolean is TLC's
/// `TLC_EXPECTED_VALUE` at the check's location (`Tool.isValid`; fixtures
/// `ErInvNonBool`, `ErConstrNonBool`, `ErActConstrNonBool`).
pub(crate) fn holds(
    m: &Model,
    c: &Named,
    st: &St,
    site: ErrorSite,
    control: &crate::budget::EvaluationControl,
) -> Result<bool, Verdict> {
    match role_boolean(m, c, st) {
        Ok(Some(v)) => Ok(v),
        Ok(None) => {
            let e = EvalError::coded(
                Ec::ExpectedValue,
                format!(
                    "TLC expected a boolean value, but did not find one. {}",
                    m.evaluator.location(c.pred.location)
                ),
            );
            Err(class::search_error(&e, site, Some(&c.name)))
        }
        Err(e) => Err(control
            .verdict(&e)
            .unwrap_or_else(|| class::search_error(&e, site, Some(&c.name)))),
    }
}

/// TLC's text of a state (`TLCStateMut.toString`): `/\ x = v` per variable
/// in TLC's variable order (`x = v` for a single variable), `null` for an
/// unassigned one.
pub(crate) fn state_text(m: &Model, s: &[Option<Value>], ctx: Ctx) -> String {
    let order = m.ir.tlc_var_order();
    let single = order.len() == 1;
    let mut out = String::new();
    for v in order {
        let name = &m.ir.decl(m.ir.vars()[v.index()]).name;
        let val = s.get(v.index()).and_then(Option::as_ref).map_or_else(
            || "null".to_owned(),
            |x| {
                dive_value::print::to_tlc_string(x, ctx).map_or_else(
                    |_| dive_value::print::to_tlc_string_lenient(x, ctx),
                    |t| dive_value::print::ppr(&t),
                )
            },
        );
        if single {
            let _ = writeln!(out, "{name} = {val}");
        } else {
            let _ = writeln!(out, "/\\ {name} = {val}");
        }
    }
    out
}

/// The trace state of an emitted successor (or initial state). A successor
/// that leaves variables unassigned retains text and separate typed bindings.
pub(crate) fn trace_state(m: &Model, succ: &Successor<'_>, initial: bool) -> TraceState {
    let ev = &m.evaluator;
    let (values, text) = match values(succ.state) {
        Some(v) => (v, None),
        None => (
            Vec::new(),
            Some(state_text(m, succ.state, ev.options().ctx)),
        ),
    };
    TraceState {
        label: if initial {
            "<Initial predicate>".to_owned()
        } else {
            succ.action.label().to_owned()
        },
        action: if initial {
            None
        } else {
            action_name(ev, succ.action)
        },
        values,
        text,
        partial: (!succ.complete()).then(|| crate::result::PartialState {
            values: succ.state.to_vec(),
            expression: succ.action.expr(),
        }),
    }
}

/// The verdict for a successor that leaves variables unassigned, with TLC's
/// message: `TLC_INITIAL_STATE` for an initial state (`DoInitFunctor`), and
/// `TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT` naming the action when there
/// are several (`ModelChecker.doNextSetErr`; fixtures `ErInitIncomplete`,
/// `ErNextIncomplete`).
pub(crate) fn incomplete(m: &Model, succ: &Successor<'_>, initial: bool, ctx: Ctx) -> Verdict {
    let variables: Vec<String> = succ
        .unassigned()
        .iter()
        .map(|&i| m.ir.decl(m.ir.vars()[i]).name.clone())
        .collect();
    let message = if initial {
        format!(
            "current state is not a legal state\nWhile working on the initial state:\n{}",
            state_text(m, succ.state, ctx)
        )
    } else {
        let are = if variables.len() > 1 { "s are" } else { " is" };
        let list = variables.join(", ");
        if m.actions.len() == 1 {
            format!(
                "Successor state is not completely specified by the next-state action. The following variable{are} not defined: {list}."
            )
        } else {
            let name = action_name(&m.evaluator, succ.action).unwrap_or_else(|| {
                let l = succ.action.label();
                l.trim_start_matches('<')
                    .split(' ')
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            });
            format!(
                "Successor state is not completely specified by action {name} of the next-state relation. The following variable{are} not defined: {list}."
            )
        }
    };
    Verdict::NotCompletelySpecified {
        initial,
        variables,
        message,
    }
}

/// Whether the state `t` (stepped to from `step`'s `s0`, if any) satisfies
/// every `CONSTRAINT` and `ACTION_CONSTRAINT`, in TLC's order
/// (`Worker.java:434-435`).
pub(crate) fn in_model(
    m: &Model,
    st1: &St,
    step: Option<&St>,
    levels: &Levels<'_>,
    control: &crate::budget::EvaluationControl,
) -> Result<bool, Verdict> {
    levels.here();
    for c in &m.constraints {
        if !holds(m, c, st1, ErrorSite::Constraint, control)? {
            return Ok(false);
        }
    }
    if let Some(st) = step {
        levels.from();
        let r = (|| {
            for c in &m.action_constraints {
                if !holds(m, c, st, ErrorSite::ActionConstraint, control)? {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        levels.here();
        return r;
    }
    Ok(true)
}

/// The dedup key of an in-model state `t` (its `VIEW` value's, with one).
pub(crate) fn state_key_of<S: Keyed>(
    m: &Model,
    keyer: &S::Keyer,
    ctx: Ctx,
    t: &StateVals,
    st1: &St,
    control: &crate::budget::EvaluationControl,
) -> Result<S::Key, Verdict> {
    let concrete = complete_values(t)?;
    let vals = &concrete;
    let canonical = match &m.symmetry_reduction {
        Some(reduction) => {
            let order = if ctx.order == dive_value::ValueOrder::Tlc {
                m.ir.tlc_var_order()
                    .iter()
                    .map(|v| v.index())
                    .collect::<Vec<_>>()
            } else {
                (0..vals.len()).collect()
            };
            Some(
                reduction
                    .canonical_values(vals, &order, ctx)
                    .map_err(|error| crate::model_error(&error))?,
            )
        }
        None => None,
    };
    let vals = canonical.as_deref().unwrap_or(vals);
    let canonical_state;
    let st1 = if canonical.is_some() {
        canonical_state = St::state(vals.iter().cloned().map(Some).collect::<Vec<_>>().into());
        &canonical_state
    } else {
        st1
    };
    match &m.view {
        None => {
            let key = S::key(keyer, vals, false, ctx).map_err(|e| store_verdict(&e, control))?;
            if canonical
                .as_ref()
                .is_some_and(|image| image.as_slice() != concrete.as_slice())
            {
                dive_model::symmetry::Reduction::normalize_concrete(&concrete, ctx)
                    .map_err(|error| crate::model_error(&error))?;
            }
            Ok(key)
        }
        Some(v) => {
            // TLCStateMut.fingerPrint deep-normalizes every concrete variable
            // before VIEW, even without symmetry and even when VIEW ignores it.
            dive_model::symmetry::Reduction::normalize_concrete(&concrete, ctx)
                .map_err(|error| crate::model_error(&error))?;
            match m.eval(&v.pred, st1) {
                Ok(x) => S::key(keyer, &[x], true, ctx).map_err(|e| store_verdict(&e, control)),
                Err(e) => Err(error_verdict(&e, ErrorSite::View, control)),
            }
        }
    }
}

/// The first invariant `t` violates, in cfg order, as the verdict. An
/// invariant that fails to evaluate is classified as on a new state, which
/// TLC's replay does not check again; the caller applies
/// [`class::recurring`] for a state outside the model.
pub(crate) fn check_invariants(
    m: &Model,
    st1: &St,
    initial: bool,
    control: &crate::budget::EvaluationControl,
) -> Option<Verdict> {
    for c in &m.invariants {
        match holds(m, c, st1, ErrorSite::Invariant, control) {
            Ok(true) => {}
            Ok(false) => {
                return Some(Verdict::InvariantViolated {
                    property: c.name.clone(),
                    initial,
                });
            }
            Err(v) => return Some(v),
        }
    }
    None
}

/// The first implied action the step violates, as the verdict.
pub(crate) fn check_implied_actions(
    m: &Model,
    step: &St,
    control: &crate::budget::EvaluationControl,
) -> Option<Verdict> {
    for c in &m.implied_actions {
        match holds(m, c, step, ErrorSite::ImpliedAction, control) {
            Ok(true) => {}
            Ok(false) => {
                return Some(Verdict::ActionPropertyViolated {
                    property: c.name.clone(),
                });
            }
            Err(v) => return Some(v),
        }
    }
    None
}

fn observer_failure(mut message: String) -> Verdict {
    message.insert_str(0, "behavior observer: ");
    Verdict::Unsupported { feature: message }
}

fn observer_bug(message: &str) -> Verdict {
    Verdict::InternalError {
        message: message.into(),
    }
}

fn observer_call(callback: impl FnOnce() -> Result<(), Verdict>) -> Result<(), Verdict> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(verdict)) => Err(verdict),
        Err(_) => Err(Verdict::InternalError {
            message: "behavior observer callback panicked".into(),
        }),
    }
}

/// Explicit state and depth ceilings, allocated only for bounded runs.
pub(crate) struct AdmissionLimits {
    max_states: Option<u64>,
    max_depth: Option<u64>,
    started: std::time::Instant,
}

/// The state of a search: the seen-state set, trace log and counters.
pub(crate) struct Search<'a, 'obs, S: Keyed> {
    pub(crate) m: &'a Model,
    pub(crate) host: &'a RunHost,
    pub(crate) control: std::sync::Arc<crate::budget::EvaluationControl>,
    /// TLC's state fingerprint, which seeds each expanded state's random
    /// stream.
    seeder: StateFingerprinter,
    /// Whether the specification uses a random operator at all.
    random: bool,
    pub(crate) ctx: Ctx,
    pub(crate) keyer: &'a S::Keyer,
    pub(crate) set: S,
    disk: Option<crate::seen::Run>,
    pub(crate) log: PredecessorLog,
    pub(crate) generated: u64,
    pub(crate) depth: u64,
    pub(crate) initial_generated: u64,
    pub(crate) capture: Option<Collector<S::Key>>,
    checkpoint: Option<&'a mut crate::checkpoint::Session>,
    pub(crate) observer: Option<&'obs mut dyn crate::capture::Observer>,
    pub(crate) primary: Option<&'a mut crate::capture::CheckpointHooks<'obs>>,
    pub(crate) prefix_level: Option<u64>,
    /// The run's armed budget (ADR 0107), polled once per expanded state.
    pub(crate) budget: crate::budget::Budget,
    pub(crate) admission: Option<Box<AdmissionLimits>>,
}

impl<S: Keyed> Search<'_, '_, S> {
    /// The next distinct identity must be admitted before any invariant check.
    #[cold]
    #[inline(never)]
    pub(crate) fn admission_stop(&self, depth: u64, distinct: u64) -> Option<Verdict> {
        self.admission_exhausted(depth, distinct.saturating_add(1))
    }

    /// Bulk recovery must fit the same ceilings before admitting its snapshot.
    #[cold]
    #[inline(never)]
    pub(crate) fn admission_exhausted(&self, depth: u64, states: u64) -> Option<Verdict> {
        let limits = self.admission.as_ref()?;
        let (resource, limit, observed) = if let Some(limit) = limits.max_states
            && states > limit
        {
            (crate::budget::Resource::States, limit, states)
        } else if let Some(limit) = limits.max_depth
            && depth > limit
        {
            (crate::budget::Resource::Depth, limit, depth)
        } else {
            return None;
        };
        Some(Verdict::Stopped(Box::new(crate::budget::Stopped {
            resource,
            limit,
            observed,
            elapsed_ms: u64::try_from(limits.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            phase: if self.capture.is_some() {
                crate::budget::Phase::Capture
            } else {
                crate::budget::Phase::Search
            },
        })))
    }

    /// The stop for an exhausted budget, in this search's phase.
    pub(crate) fn budget_stop(&self) -> Option<Stop> {
        let polled = self.budget.poll();
        let phase = if self.capture.is_some() {
            crate::budget::Phase::Capture
        } else {
            crate::budget::Phase::Search
        };
        Some(Stop {
            verdict: match polled {
                Ok(Some(exhausted)) => Verdict::Stopped(Box::new(exhausted.during(phase))),
                Err(error) => Verdict::BudgetMeasurement { error },
                Ok(None) => return None,
            },
            in_init: false,
            path: None,
            then: None,
        })
    }

    fn seen_len(&self) -> u64 {
        self.disk
            .as_ref()
            .map_or_else(|| self.set.len(), crate::seen::Run::semantic_entries)
    }

    fn insert_key(
        &mut self,
        key: S::Key,
        parent: Option<StateId>,
        ordinal: u64,
    ) -> Result<bool, Verdict> {
        if let Some(disk) = &mut self.disk {
            disk.insert(S::disk_key(&key), self.generated, parent, ordinal)
                .map_err(|error| crate::seen::verdict(&error))
        } else {
            Ok(self.set.insert(key))
        }
    }

    fn observe_checkpoint(
        &mut self,
        phase: crate::capture::CheckpointPhase,
        source: StateId,
        values: &StateVals,
        stats: Stats,
    ) -> Result<(), Verdict> {
        if self.primary.is_none() {
            return Ok(());
        }
        if let Some(stop) = self.budget_stop() {
            return Err(stop.verdict);
        }
        let source =
            usize::try_from(source).map_err(|_| observer_bug("checkpoint source range"))?;
        let graph = self
            .capture
            .as_ref()
            .ok_or_else(|| observer_bug("checkpoint without capture"))?
            .graph();
        let edges = graph
            .edges
            .get(source)
            .ok_or_else(|| observer_bug("checkpoint source row"))?;
        let concrete = graph
            .concrete
            .get(source)
            .ok_or_else(|| observer_bug("checkpoint concrete row"))?;
        if let Some(primary) = &mut self.primary {
            primary.invoke(
                crate::capture::ReachedCheckpoint {
                    phase,
                    source,
                    state: values,
                    stats,
                    edges,
                    concrete,
                },
                &self.control,
            )?;
        }
        Ok(())
    }

    fn observe_initial(&mut self, stats: Stats) -> Result<(), Verdict> {
        let (Some(observer), Some(capture)) = (&mut self.observer, &self.capture) else {
            return Ok(());
        };
        observer_call(|| {
            let graph = capture.graph();
            for &id in &graph.roots {
                let vals = graph
                    .states
                    .get(id)
                    .ok_or_else(|| observer_bug("missing initial graph state"))?;
                observer.init(id, vals, stats).map_err(observer_failure)?;
            }
            Ok(())
        })
    }

    /// Publish real completed-level events after all storage and safety work succeeded.
    pub(crate) fn observe_level(
        &mut self,
        level: u64,
        first_id: StateId,
        sources: usize,
        stats: Stats,
    ) -> Result<(), Verdict> {
        let (Some(observer), Some(capture)) = (&mut self.observer, &self.capture) else {
            return Ok(());
        };
        let first = usize::try_from(first_id).map_err(|_| Verdict::InternalError {
            message: "observed source id exceeds address space".into(),
        })?;
        let mut stop = false;
        observer_call(|| {
            let graph = capture.graph();
            let end = first
                .checked_add(sources)
                .ok_or_else(|| observer_bug("observed source range overflow"))?;
            let rows = graph
                .edges
                .get(first..end)
                .ok_or_else(|| observer_bug("missing observed graph rows"))?;
            let originals = graph
                .concrete
                .get(first..end)
                .ok_or_else(|| observer_bug("missing concrete graph rows"))?;
            for (offset, (row, original)) in rows.iter().zip(originals).enumerate() {
                let from = first + offset;
                let mut concrete = original.iter();
                for edge in row {
                    let successor = if edge.ordinal.is_some() {
                        &concrete
                            .next()
                            .ok_or_else(|| observer_bug("missing concrete graph successor"))?
                            .state
                    } else {
                        graph
                            .states
                            .get(from)
                            .ok_or_else(|| observer_bug("missing stutter graph state"))?
                    };
                    observer
                        .edge(from, edge, successor, stats)
                        .map_err(observer_failure)?;
                }
            }
            observer
                .level_done(level, graph, stats)
                .map_err(observer_failure)?;
            stop = observer.stop_after_level();
            Ok(())
        })?;
        if stop {
            if stats.left_on_queue == 0 {
                return Err(observer_bug("prefix stop requires an unexhausted frontier"));
            }
            self.prefix_level = Some(level);
        }
        Ok(())
    }

    /// Native transaction state is only present on admitted pure safety runs.
    pub(crate) fn has_checkpoint(&self) -> bool {
        self.checkpoint.is_some()
    }

    /// Pause before any work from this frontier, at a completed-level barrier.
    pub(crate) fn checkpoint_boundary(
        &mut self,
        frontier: &crate::frontier::Queue,
        level: u64,
    ) -> Option<Verdict> {
        let session = self.checkpoint.take()?;
        if !session.pause_due(level) {
            self.checkpoint = Some(session);
            return None;
        }
        Some(match session.pause::<S>(self, frontier, level) {
            Ok(()) => crate::checkpoint::failure(
                "paused at a committed completed-level boundary; incomplete search, no model verdict",
            ),
            Err(verdict) => verdict,
        })
    }

    /// Record the winning native key without repeating model evaluation.
    pub(crate) fn checkpoint_discovery(
        &mut self,
        key: Vec<u8>,
        state: &StateVals,
        parent: StateId,
        ordinal: u32,
    ) -> Result<(), Verdict> {
        if let Some(session) = self.checkpoint.as_mut() {
            session.record_key(key, state, Some(parent), ordinal, self.ctx)?;
        }
        Ok(())
    }

    /// Begin or resume at the actual saved BFS level, reporting the pool started.
    fn begin_levels(
        &mut self,
        next: crate::frontier::Queue,
        opts: &CheckOptions,
        workers: &mut Workers,
        level: u64,
    ) -> (Option<Stop>, Stats) {
        if !next.is_empty()
            && let Some(verdict) = self.checkpoint_boundary(&next, level)
        {
            workers.used = 1;
            workers.sequential = true;
            workers.reason = Some("native checkpoint paused before the worker pool started".into());
            return (
                Some(Stop {
                    verdict,
                    in_init: false,
                    path: None,
                    then: None,
                }),
                self.stats(next.len() as u64),
            );
        }
        if workers.sequential {
            if level == 1 {
                return self.inline_levels(next, self.m.settings.check_deadlock);
            }
            return self.inline_from(next, self.m.settings.check_deadlock, level);
        }
        let (stop, stats, used) = if level == 1 {
            crate::parallel::levels(self, next, opts, workers.used)
        } else {
            crate::parallel::levels_from(self, next, opts, workers.used, level)
        };
        workers.used = used;
        (stop, stats)
    }

    /// One generated state, inline: TLC's `DoInitFunctor.addElement` (no
    /// parent) or `Worker.addElement`. Returns the verdict that stops the
    /// search, whether the successor ends the counterexample, and whether
    /// TLC's replay goes on past it (an invariant that failed to evaluate on
    /// a new state, [`Search::replay_after`]).
    fn on_state(
        &mut self,
        parent: Option<(&StateVals, StateId)>,
        succ: &Successor<'_>,
        new_level: u64,
        next: &mut crate::frontier::Queue,
    ) -> Option<(Verdict, bool, bool)> {
        let r = self.on_state_checks(parent, succ, new_level, next);
        // Back to the level of the state being expanded.
        self.host.set_level(level_of(new_level - 1));
        r
    }

    #[allow(
        clippy::too_many_lines,
        reason = "preserves TLC ordered deciding-event checks, including native capture admission"
    )]
    fn on_state_checks(
        &mut self,
        parent: Option<(&StateVals, StateId)>,
        succ: &Successor<'_>,
        new_level: u64,
        next: &mut crate::frontier::Queue,
    ) -> Option<(Verdict, bool, bool)> {
        let Some(generated) = self.generated.checked_add(1) else {
            return Some((
                Verdict::ResourceLimit {
                    message: "DIVE-S0401: generated counter overflow".into(),
                },
                false,
                false,
            ));
        };
        if self.checkpoint.is_some() && generated > crate::checkpoint::MAX_COUNTER {
            return Some((
                crate::checkpoint::failure(
                    "generated counter exceeds native positive TLC-long bound",
                ),
                false,
                false,
            ));
        }
        self.generated = generated;
        if let Some(capture) = &self.capture
            && let Err(v) = capture.allow_emitted(self.generated, parent.map(|_| succ.ordinal))
        {
            return Some((v, false, false));
        }
        let host = self.host;
        let set = |l: i64| host.set_level(l);
        let levels = Levels {
            set: &set,
            here: level_of(new_level),
            from: level_of(new_level - 1),
        };
        let initial = parent.is_none();
        let m = self.m;
        if !succ.complete() {
            // TLC prints the incomplete successor at the end of the trace
            // (fixture `ErNextIncomplete`), and an initial state only in
            // its message (`ErInitIncomplete`).
            return Some((incomplete(m, succ, initial, self.ctx), !initial, false));
        }
        let t: StateVals = succ.state.into();
        let st1 = St::state(t.clone());
        let st_step = parent.map(|(s0, _)| St::action(s0.clone(), t.clone()));
        let in_model = match in_model(m, &st1, st_step.as_ref(), &levels, &self.control) {
            Ok(b) => b,
            Err(v) => return Some((v, true, false)),
        };
        let mut fresh = true;
        let mut new_id = None;
        if in_model {
            // A state TLC cannot fingerprint (or whose `VIEW` fails) is not
            // printed: the trace ends at its predecessor (fixtures `ErView*`,
            // `ErFpInfinite`).
            let key = match state_key_of::<S>(m, self.keyer, self.ctx, &t, &st1, &self.control) {
                Ok(k) => k,
                Err(v) => return Some((v, false, false)),
            };
            if self.admission.is_some()
                && let Some(verdict) = self.admission_stop(new_level, self.seen_len())
            {
                let present = if let Some(disk) = &mut self.disk {
                    match disk.admission_contains(
                        S::disk_key(&key),
                        self.generated,
                        parent.map(|(_, id)| id),
                        succ.ordinal,
                    ) {
                        Ok(present) => present,
                        Err(error) => return Some((crate::seen::verdict(&error), false, false)),
                    }
                } else {
                    self.set.contains(&key)
                };
                if !present {
                    return Some((verdict, false, false));
                }
            }
            let graph_key = self.capture.as_ref().map(|_| S::duplicate_key(&key));
            let checkpoint_key = self.checkpoint.as_ref().map(|_| S::duplicate_key(&key));
            let inserted = match self.insert_key(key, parent.map(|(_, id)| id), succ.ordinal) {
                Ok(inserted) => inserted,
                Err(verdict) => return Some((verdict, false, false)),
            };
            if inserted {
                let Ok(ordinal) = u32::try_from(succ.ordinal) else {
                    return Some((
                        Verdict::InternalError {
                            message: "more than 2^32 successors of one state".to_owned(),
                        },
                        false,
                        false,
                    ));
                };
                let id = match parent {
                    None => self.log.push_initial(ordinal),
                    Some((_, p)) => self.log.push(p, ordinal),
                };
                match id {
                    Ok(id) => {
                        new_id = Some(id);
                        if let (Some(session), Some(key)) = (&mut self.checkpoint, &checkpoint_key)
                            && let Err(verdict) = session.record::<S>(
                                key,
                                &t,
                                parent.map(|(_, p)| p),
                                ordinal,
                                self.ctx,
                            )
                        {
                            return Some((verdict, false, false));
                        }
                    }
                    Err(e) => return Some((store_verdict(&e, &self.control), false, false)),
                }
                self.depth = self.depth.max(new_level);
            } else {
                fresh = false;
            }
            if let (Some(capture), Some(key)) = (&mut self.capture, graph_key)
                && let Err(v) =
                    capture.record(key, t.clone(), new_id, parent.map(|(_, id)| id), succ)
            {
                return Some((v, false, false));
            }
        }
        // Only the explicitly enabled checkpoint route enqueues a new root
        // before its invariants. All legacy callers retain the old ordering.
        let mut queued_for_checkpoint = false;
        if initial
            && self.primary.is_some()
            && let Some(id) = new_id
        {
            if let Err(verdict) = next.push(t.clone(), id) {
                return Some((verdict, false, false));
            }
            queued_for_checkpoint = true;
            let stats = Stats {
                initial_generated: self.generated,
                ..self.stats(next.len() as u64)
            };
            if let Err(verdict) =
                self.observe_checkpoint(crate::capture::CheckpointPhase::Init, id, &t, stats)
            {
                let with = self
                    .primary
                    .as_ref()
                    .is_some_and(|hook| hook.has_decision());
                return Some((verdict, with, false));
            }
        }
        if fresh {
            if let Some(v) = check_invariants(m, &st1, initial, &self.control) {
                // TLC's replay checks a state outside the model again, and
                // goes on past a new one (`Search::replay_after`).
                if !in_model {
                    return Some((class::recurring(v), true, false));
                }
                let resume = !initial && resumes(&v);
                return Some((v, true, resume));
            }
            if initial {
                for c in &m.implied_inits {
                    match holds(m, c, &st1, ErrorSite::ImpliedInit, &self.control) {
                        Ok(true) => {}
                        Ok(false) => {
                            return Some((
                                Verdict::PropertyViolatedInitially {
                                    property: c.name.clone(),
                                },
                                true,
                                false,
                            ));
                        }
                        Err(v) => return Some((v, true, false)),
                    }
                }
            }
        }
        if let Some(st) = &st_step {
            levels.from();
            if let Some(v) = check_implied_actions(m, st, &self.control) {
                return Some((v, true, false));
            }
        }
        if let Some(id) = new_id
            && !queued_for_checkpoint
            && let Err(verdict) = next.push(t, id)
        {
            return Some((verdict, false, false));
        }
        None
    }

    /// Generates the initial states (`DoInitFunctor`) into `next`. Returns
    /// the stop and statistics if the search ends there.
    pub(crate) fn initial_states(
        &mut self,
        next: &mut crate::frontier::Queue,
    ) -> Option<(Stop, Stats)> {
        let ev: &Evaluator = &self.m.evaluator;
        let m = self.m;
        let mut stop: Option<Stop> = None;
        search_counters(self.host);
        // Initial states: `DoInitFunctor`, on the start-up stream that the
        // constant definitions and assumptions drew from.
        let ctx = self.ctx;
        let r = ev.init_states(&m.init_actions, &mut |succ: &Successor<'_>| {
            // One relaxed load per initial state: the budget can stop a
            // long `Init` enumeration between its states (ADR 0107).
            if let Some(budget) = self.budget_stop() {
                stop = Some(Stop {
                    in_init: true,
                    ..budget
                });
                return Flow::Stop;
            }
            match self.on_state(None, succ, 1, next) {
                None => Flow::Continue,
                Some((verdict, with, _)) => {
                    // An error in a check of an initial state names the state
                    // (`DoInitFunctor` records it).
                    let verdict = class::init_error(verdict, Some(&state_text(m, succ.state, ctx)));
                    // doInit prints the recorded state for a RuntimeException,
                    // but not for an EvalException (reported as GENERAL).
                    let with = with
                        && !matches!(
                            verdict,
                            Verdict::EvaluationError { reported: 1000, .. }
                                | Verdict::EvaluationError {
                                    site: ErrorSite::ImpliedInit,
                                    ..
                                }
                        );
                    stop = Some(Stop {
                        verdict,
                        in_init: true,
                        path: None,
                        then: if with {
                            Some(trace_state(m, succ, true))
                        } else {
                            None
                        },
                    });
                    Flow::Stop
                }
            }
        });
        self.initial_generated = self.generated;
        if let Err(e) = r {
            stop = Some(Stop {
                verdict: class::init_error(
                    error_verdict(&e.error, ErrorSite::Init, &self.control),
                    None,
                ),
                in_init: true,
                path: None,
                then: None,
            });
        }
        if let Some(s) = stop {
            // After an evaluation error TLC prints its statistics: every
            // state it inserted is on the queue (it enqueues before
            // checking), and the depth is 1 once a state was generated
            // (fixtures `ErInit*`, `ErInvInitState`, `ErConstrInitState`).
            // After a violation it prints none.
            let mut stats = self.stats(self.seen_len());
            stats.depth = if matches!(
                s.verdict,
                Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. }
            ) {
                self.depth
            } else {
                u64::from(self.generated > 0)
            };
            return Some((s, stats));
        }
        // No action at all: success if no state was queued, otherwise TLC's
        // `TLC_STATES_AND_NO_NEXT_ACTION`, without statistics
        // (`ModelChecker.java:233-245`).
        if m.actions.is_empty() && !next.is_empty() {
            let stop = Stop {
                verdict: Verdict::ModelError {
                    code: 2115,
                    name: "TLC_STATES_AND_NO_NEXT_ACTION".to_owned(),
                    message: "No next state actions defined to generate successor states from."
                        .to_owned(),
                },
                in_init: true,
                path: None,
                then: None,
            };
            return Some((stop, self.stats(next.len() as u64)));
        }
        None
    }

    /// Runs every level inline from the frontier `next`: `Worker.run`.
    #[allow(
        clippy::too_many_lines,
        reason = "ordered BFS stop/counting phases with fallible queue reads"
    )]
    pub(crate) fn inline_levels(
        &mut self,
        next: crate::frontier::Queue,
        check_deadlock: bool,
    ) -> (Option<Stop>, Stats) {
        self.inline_from(next, check_deadlock, 1)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "single ordered level loop for ordinary and recovered inline BFS"
    )]
    pub(crate) fn inline_from(
        &mut self,
        mut next: crate::frontier::Queue,
        check_deadlock: bool,
        mut level: u64,
    ) -> (Option<Stop>, Stats) {
        let ev: &Evaluator = &self.m.evaluator;
        let m = self.m;
        let mut stop: Option<Stop> = None;
        while !next.is_empty() {
            if let Some(verdict) = self.checkpoint_boundary(&next, level) {
                return (
                    Some(Stop {
                        verdict,
                        in_init: false,
                        path: None,
                        then: None,
                    }),
                    self.stats(next.len() as u64),
                );
            }
            let empty = next.empty_like();
            let current = std::mem::replace(&mut next, empty);
            let count = current.len();
            let mut first_id = 0;
            for i in 0..count {
                let (vals, id) = match current.read(i, &m.names) {
                    Ok(state) => state,
                    Err(verdict) => {
                        return (
                            Some(Stop {
                                verdict,
                                in_init: false,
                                path: None,
                                then: None,
                            }),
                            self.stats((count - i + next.len()) as u64),
                        );
                    }
                };
                if i == 0 {
                    first_id = id;
                }
                let s0 = &vals;
                if let Some(stop) = self.budget_stop() {
                    let left = (count - i) + next.len();
                    return (Some(stop), self.stats(left as u64));
                }
                if self.host.exit_requested() {
                    // `TLCSet("exit", TRUE)`: TLC's `stop()` ends the search
                    // as a success once the state that asked is done.
                    let left = (count - i) + next.len();
                    return (None, self.stats(left as u64));
                }
                let before = self.generated;
                if let Err(v) = self.seed_state(s0) {
                    let stop = Stop {
                        verdict: v,
                        in_init: false,
                        path: Some((id, s0.clone())),
                        then: None,
                    };
                    let left = (count - i - 1) + next.len();
                    return (Some(stop), self.stats(left as u64));
                }
                self.host.set_level(level_of(level));
                let mut resume: Option<u64> = None;
                let r = ev.successors(&m.actions, s0, &mut |succ: &Successor<'_>| match self
                    .on_state(Some((s0, id)), succ, level + 1, &mut next)
                {
                    None => Flow::Continue,
                    Some((verdict, with, resumes)) => {
                        if resumes {
                            resume = Some(succ.ordinal);
                        }
                        let path = (!matches!(
                            &verdict,
                            Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. }
                        ))
                        .then(|| (id, s0.clone()));
                        stop = Some(Stop {
                            verdict,
                            in_init: false,
                            path,
                            then: if with {
                                Some(trace_state(m, succ, false))
                            } else {
                                None
                            },
                        });
                        Flow::Stop
                    }
                });
                // TLC counts the intermediate states of `\cdot` as generated
                // (`--action-composition=tlc`, D-022), so they also keep the
                // state from being a deadlock.
                let intermediate = match &r {
                    Ok(o) => o.intermediate,
                    Err(e) => e.intermediate,
                };
                self.generated = self.generated.saturating_add(intermediate);
                if let Err(e) = r {
                    // The counterexample ends at the state being expanded
                    // (probe `EvalErrorMidLevel`).
                    stop = Some(Stop {
                        verdict: error_verdict(&e.error, ErrorSite::Next, &self.control),
                        in_init: false,
                        path: (e.error.class() != dive_eval::ErrorClass::Interrupted)
                            .then(|| (id, s0.clone())),
                        then: None,
                    });
                }
                // Composition intermediates consume the capture generation
                // bound too. Check before stutter/level publication, preserving
                // any safety or evaluation event already decided by the source.
                if stop.is_none()
                    && let Some(capture) = &self.capture
                    && let Err(verdict) = capture.allow_generated(self.generated)
                {
                    stop = Some(Stop {
                        verdict,
                        in_init: false,
                        path: None,
                        then: None,
                    });
                }
                if stop.is_none() && check_deadlock && self.generated == before {
                    // No successor at all, counting those outside the model
                    // (probes `ConstraintNoDeadlock`, `DeadlockMidLevel`).
                    stop = Some(Stop {
                        verdict: Verdict::Deadlock,
                        in_init: false,
                        path: Some((id, s0.clone())),
                        then: None,
                    });
                }
                if let Some(mut s) = stop {
                    let left = (count - i - 1) + next.len();
                    let mut stats = self.stats(left as u64);
                    if let Some(ordinal) = resume {
                        let r = if self.disk.is_some() {
                            self.replay_after_disk(s0, id, ordinal, level, &s.verdict)
                        } else {
                            self.replay_after(&self.set, s0, ordinal, level, &BTreeSet::new())
                        };
                        stats.distinct += r.distinct;
                        stats.left_on_queue += r.queued;
                        s.verdict = r.apply(s.verdict);
                    }
                    return (Some(s), stats);
                }
                if let Some(capture) = &mut self.capture
                    && let Err(v) = capture.stutter(id)
                {
                    return (
                        Some(Stop {
                            verdict: v,
                            in_init: false,
                            path: None,
                            then: None,
                        }),
                        self.stats(0),
                    );
                }
                if self.primary.is_some() {
                    let stats = self.stats(((count - i - 1) + next.len()) as u64);
                    if let Err(verdict) = self.observe_checkpoint(
                        crate::capture::CheckpointPhase::SourceRow,
                        id,
                        s0,
                        stats,
                    ) {
                        let path = self
                            .primary
                            .as_ref()
                            .is_some_and(|hook| hook.has_decision())
                            .then(|| (id, s0.clone()));
                        return (
                            Some(Stop {
                                verdict,
                                in_init: false,
                                path,
                                then: None,
                            }),
                            stats,
                        );
                    }
                }
            }
            let stats = self.stats(next.len() as u64);
            if let Err(verdict) = self.observe_level(level, first_id, count, stats) {
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
            if self.prefix_level.is_some() {
                return (None, stats);
            }
            level += 1;
        }
        (None, self.stats(0))
    }

    /// Switches the host to the random stream of expanding `s0`: TLC reseeds
    /// with `fingerprint(s0) ^ seed` (`RandomEnumerableValues`).
    fn seed_state(&self, s0: &StateVals) -> Result<(), Verdict> {
        if !self.random {
            return Ok(());
        }
        let vals = complete_values(s0)?;
        let fp = self
            .seeder
            .fingerprint(&vals, self.ctx)
            .map_err(|e| store_verdict(&e, &self.control))?;
        self.host.expand(fp);
        Ok(())
    }

    /// TLC's call-stack replay after an invariant failed to evaluate on the
    /// new state `t`, successor `ordinal` of `s0` (a state of level `level`)
    /// (`ModelChecker.modelCheckImpl`, `keepCallStack`; `ModelChecker.doNext`).
    ///
    /// The replay expands `s0` again, one action at a time, enumerating each
    /// action in full before checking its successors. `t` is now in the
    /// seen-state set, so its invariants are not checked again and the replay
    /// goes on: past `t` it inserts new states (TLC's distinct count, though
    /// not its generated count, includes them), checks them and enqueues
    /// them, until a violation or an incomplete state ends it quietly, or a
    /// further error ends it with that error's class. Fixtures `ErInv*`
    /// (5 distinct and 2 on the queue where the search found 4 and 1),
    /// `ErInvReplayProp` (an action property violated on `t` itself) and
    /// `ErInvReplayError` (a second error, class `TLC_NESTED_EXPRESSION`).
    ///
    /// `set` is the seen-state set at the stop, apart from the keys in
    /// `unseen` (states the parallel engine found after the stop).
    #[allow(
        clippy::too_many_lines,
        reason = "keep TLC replay error order and bounded admission in one transaction"
    )]
    pub(crate) fn replay_after(
        &self,
        set: &S,
        s0: &StateVals,
        ordinal: u64,
        level: u64,
        unseen: &BTreeSet<S::Key>,
    ) -> Replayed {
        // Diagnostic reconstruction follows an already deciding genuine event.
        // It must retain that event's class/count semantics if the budget trips.
        // This pause affects only this thread; worker control remains active.
        // The inline caller has no workers; parallel commit invokes this only
        // after both expansion and invariant pool barriers completed.
        let _diagnostic = match self.m.evaluator.host_control(None) {
            Ok(scope) => scope,
            Err(error) => {
                return Replayed {
                    error: Some(Verdict::InternalError {
                        message: error.to_string(),
                    }),
                    ..Replayed::default()
                };
            }
        };
        if self.disk.is_some() {
            // Defensive admission for every caller, including future pool
            // integrations: never use Boolean resident membership on disk.
            return Replayed {
                error: Some(Verdict::Unsupported {
                    feature: "disk seen-state invariant-error call-stack replay".into(),
                }),
                ..Replayed::default()
            };
        }
        let m = self.m;
        let ev = &m.evaluator;
        // TLC prints the replay's `Print` output again; dive does not.
        let _quiet = dive_eval::capture::PrintCapture::start();
        let mut out = Replayed::default();
        if self.seed_state(s0).is_err() {
            return out;
        }
        let host = self.host;
        let set_level = |l: i64| host.set_level(l);
        let levels = Levels {
            set: &set_level,
            here: level_of(level + 1),
            from: level_of(level),
        };
        let mut added: BTreeSet<S::Key> = BTreeSet::new();
        let mut base = 0u64;
        let record_error = |v: Verdict, out: &mut Replayed| {
            if !matches!(
                v,
                Verdict::InvariantViolated { .. } | Verdict::ActionPropertyViolated { .. }
            ) {
                out.error = Some(v);
            }
        };
        for a in &m.actions {
            host.set_level(level_of(level));
            let mut succs: Vec<(u64, Option<StateVals>)> = Vec::new();
            let r = ev.next_states(a, s0, base, &mut |succ: &Successor<'_>| {
                succs.push((succ.ordinal, succ.complete().then(|| succ.state.into())));
                Flow::Continue
            });
            match r {
                Ok(o) => base += o.emitted,
                Err(e) => {
                    // The search reached `t` without this error, so it lies
                    // in `t`'s action or a later one.
                    out.error = Some(error_verdict(&e.error, ErrorSite::Next, &self.control));
                    return out;
                }
            }
            for (o, state) in succs {
                if o < ordinal {
                    continue;
                }
                let Some(t) = state else {
                    // `doNextSetErr`: an incomplete state ends it quietly.
                    return out;
                };
                let st1 = St::state(t.clone());
                let step = St::action(s0.clone(), t.clone());
                if o == ordinal {
                    levels.from();
                    if let Some(v) = check_implied_actions(m, &step, &self.control) {
                        record_error(v, &mut out);
                        return out;
                    }
                    continue;
                }
                let inside = match in_model(m, &st1, Some(&step), &levels, &self.control) {
                    Ok(b) => b,
                    Err(v) => {
                        out.error = Some(v);
                        return out;
                    }
                };
                let mut new = true;
                if inside {
                    let key =
                        match state_key_of::<S>(m, self.keyer, self.ctx, &t, &st1, &self.control) {
                            Ok(k) => k,
                            Err(v) => {
                                out.error = Some(v);
                                return out;
                            }
                        };
                    new = (!set.contains(&key) || unseen.contains(&key)) && !added.contains(&key);
                    if new {
                        if self.admission.is_some()
                            && self
                                .admission_stop(
                                    level + 1,
                                    set.len().saturating_sub(unseen.len() as u64) + out.distinct,
                                )
                                .is_some()
                        {
                            // The original evaluation error already decided the
                            // run. Stop its diagnostic replay at the ceiling
                            // without checking an unadmitted state's invariant.
                            return out;
                        }
                        added.insert(key);
                        out.distinct += 1;
                    }
                }
                if new && let Some(v) = check_invariants(m, &st1, false, &self.control) {
                    record_error(v, &mut out);
                    return out;
                }
                levels.from();
                if let Some(v) = check_implied_actions(m, &step, &self.control) {
                    record_error(v, &mut out);
                    return out;
                }
                if inside && new {
                    out.queued += 1;
                }
            }
        }
        out
    }

    /// Separate fallible counterpart: the original source/ordinal and generated
    /// count stay fixed, while actual disk keys are admitted in replay order.
    fn replay_after_disk(
        &mut self,
        s0: &StateVals,
        source: StateId,
        ordinal: u64,
        level: u64,
        original: &Verdict,
    ) -> Replayed {
        // As above, this is diagnostic replay of an existing deciding error,
        // never ordinary search or independent candidate certification.
        let _diagnostic = match self.m.evaluator.host_control(None) {
            Ok(scope) => scope,
            Err(error) => {
                return Replayed {
                    error: Some(Verdict::InternalError {
                        message: error.to_string(),
                    }),
                    ..Replayed::default()
                };
            }
        };

        let Some(mut disk) = self.disk.take() else {
            return Replayed {
                error: Some(Verdict::InternalError {
                    message: "disk replay without owned storage".into(),
                }),
                ..Replayed::default()
            };
        };
        disk.start_replay(crate::seen::SeenReplay {
            original_error: original.clone(),
            predecessor: source,
            stopped_successor_ordinal: ordinal,
            generated: self.generated,
            admitted_keys: 0,
            queued: 0,
            unproved_print_output: false,
            halted_by: None,
        });
        let replay = self.replay_disk_states(&mut disk, s0, source, ordinal, level);
        disk.finish_replay(replay.distinct, replay.queued, replay.error.clone());
        self.disk = Some(disk);
        replay
    }

    #[allow(
        clippy::too_many_lines,
        reason = "keep fallible key admission and TLC replay check ordering in one transaction"
    )]
    fn replay_disk_states(
        &self,
        disk: &mut crate::seen::Run,
        s0: &StateVals,
        source: StateId,
        ordinal: u64,
        level: u64,
    ) -> Replayed {
        let m = self.m;
        let ev = &m.evaluator;
        // TLC prints the replay's `Print` output again; dive does not.
        let _quiet = dive_eval::capture::PrintCapture::start();
        let mut out = Replayed::default();
        if let Err(verdict) = self.seed_state(s0) {
            out.error = Some(verdict);
            return out;
        }
        let host = self.host;
        let set_level = |l: i64| host.set_level(l);
        let levels = Levels {
            set: &set_level,
            here: level_of(level + 1),
            from: level_of(level),
        };
        let mut base = 0u64;
        let record_error = |v: Verdict, out: &mut Replayed| {
            if !matches!(
                v,
                Verdict::InvariantViolated { .. } | Verdict::ActionPropertyViolated { .. }
            ) {
                out.error = Some(v);
            }
        };
        for a in &m.actions {
            host.set_level(level_of(level));
            let mut succs: Vec<(u64, Option<StateVals>)> = Vec::new();
            let r = ev.next_states(a, s0, base, &mut |succ: &Successor<'_>| {
                succs.push((succ.ordinal, succ.complete().then(|| succ.state.into())));
                Flow::Continue
            });
            match r {
                Ok(o) => {
                    let Some(next) = base.checked_add(o.emitted) else {
                        out.error = Some(Verdict::ResourceLimit {
                            message: "disk replay successor ordinal overflow".into(),
                        });
                        return out;
                    };
                    base = next;
                }
                Err(e) => {
                    // The search reached `t` without this error, so it lies
                    // in `t`'s action or a later one.
                    out.error = Some(error_verdict(&e.error, ErrorSite::Next, &self.control));
                    return out;
                }
            }
            for (o, state) in succs {
                if o < ordinal {
                    continue;
                }
                let Some(t) = state else {
                    // `doNextSetErr`: an incomplete state ends it quietly.
                    return out;
                };
                let st1 = St::state(t.clone());
                let step = St::action(s0.clone(), t.clone());
                if o == ordinal {
                    levels.from();
                    if let Some(v) = check_implied_actions(m, &step, &self.control) {
                        record_error(v, &mut out);
                        return out;
                    }
                    continue;
                }
                let inside = match in_model(m, &st1, Some(&step), &levels, &self.control) {
                    Ok(b) => b,
                    Err(v) => {
                        out.error = Some(v);
                        return out;
                    }
                };
                let mut new = true;
                if inside {
                    let key =
                        match state_key_of::<S>(m, self.keyer, self.ctx, &t, &st1, &self.control) {
                            Ok(k) => k,
                            Err(v) => {
                                out.error = Some(v);
                                return out;
                            }
                        };
                    let present =
                        match disk.replay_contains(S::disk_key(&key), self.generated, source, o) {
                            Ok(present) => present,
                            Err(error) => {
                                out.error = Some(crate::seen::verdict(&error));
                                return out;
                            }
                        };
                    new = !present;
                    if new {
                        if self.admission.is_some()
                            && self
                                .admission_stop(level + 1, disk.semantic_entries())
                                .is_some()
                        {
                            // Retain the earlier genuine error; this key and
                            // its invariants lie beyond the admitted prefix.
                            return out;
                        }
                        match disk.replay_insert(S::disk_key(&key), self.generated, source, o) {
                            Ok(dive_store::seen::InsertOutcome::Inserted) => out.distinct += 1,
                            Ok(dive_store::seen::InsertOutcome::Present) => {
                                out.error = Some(Verdict::InternalError {
                                    message: "disk replay identity changed between positioned query and insertion".into(),
                                });
                                return out;
                            }
                            Err(error) => {
                                out.error = Some(crate::seen::verdict(&error));
                                return out;
                            }
                        }
                    }
                }
                if new && let Some(v) = check_invariants(m, &st1, false, &self.control) {
                    record_error(v, &mut out);
                    return out;
                }
                levels.from();
                if let Some(v) = check_implied_actions(m, &step, &self.control) {
                    record_error(v, &mut out);
                    return out;
                }
                if inside && new {
                    out.queued += 1;
                }
            }
        }
        out
    }

    pub(crate) fn stats(&self, left_on_queue: u64) -> Stats {
        Stats {
            generated: self.generated,
            distinct: self.seen_len(),
            left_on_queue,
            depth: self.depth,
            initial_generated: self.initial_generated,
        }
    }

    /// Rebuilds the path to state `id` by replay (ADR 0015 §4).
    fn replay(&self, id: StateId) -> Result<Vec<TraceState>, String> {
        // This only reconstructs a path after search selected its deciding
        // event and joined every worker phase. Preserve that event if the
        // budget trips while regenerating its predecessors. This thread-owned
        // scope does not pause independent witness certification or workers.
        // Replay uses callback execution even inside a fresh owned run.
        let _native_hooks = if self.m.evaluator.host_binding_stamp().native_hooks()
            == dive_eval::NativeHooks::Unrestricted
        {
            self.m
                .evaluator
                .native_hook_scope(dive_eval::NativeHooks::Callback)
        } else {
            None
        };
        let _diagnostic = self
            .m
            .evaluator
            .host_control(None)
            .map_err(|error| error.to_string())?;
        let ev = &self.m.evaluator;
        let path = self.log.path(id).map_err(|e| e.to_string())?;
        let mut out: Vec<TraceState> = Vec::new();
        let mut current: Option<StateVals> = None;
        // Step k expands the state at level k (initial states are level 1).
        for (level, step) in (0u64..).zip(path) {
            let want = u64::from(step.ordinal);
            let mut found: Option<(StateVals, TraceState)> = None;
            let mut sink = |succ: &Successor<'_>| {
                if succ.ordinal == want {
                    let initial = current.is_none();
                    found = Some((succ.state.into(), trace_state(self.m, succ, initial)));
                    Flow::Stop
                } else {
                    Flow::Continue
                }
            };
            // TLC replays with the same seeding: a fresh start-up stream for
            // the initial states, then each predecessor's own (D-002).
            let r = match &current {
                None => {
                    self.host.restart();
                    ev.init_states(&self.m.init_actions, &mut sink)
                }
                Some(s) => {
                    self.seed_state(s)
                        .map_err(|v| format!("replay failed: {v:?}"))?;
                    self.host.set_level(level_of(level));
                    ev.successors(&self.m.actions, s, &mut sink)
                }
            };
            if let Err(e) = r
                && found.is_none()
            {
                return Err(format!("replay failed: {}", e.error));
            }
            let Some((s, t)) = found else {
                return Err(format!(
                    "replay did not regenerate successor {} of state {:?}",
                    step.ordinal, step.parent
                ));
            };
            current = Some(s);
            out.push(t);
        }
        Ok(out)
    }

    /// The counterexample of a stop.
    fn counterexample(&self, stop: &Stop) -> Result<Option<Counterexample>, String> {
        let mut states = Vec::new();
        if let Some((id, held)) = &stop.path {
            states = self.replay(*id)?;
            let last = states.last().map(|t| t.values.as_slice());
            let held = values(held);
            if last != held.as_deref() {
                return Err(format!(
                    "the replayed trace does not end in the state the search expanded (state {id})"
                ));
            }
        }
        if let Some(t) = &stop.then {
            states.push(t.clone());
        }
        Ok((!states.is_empty()).then_some(Counterexample { states }))
    }
}

/// What the engine hands back.
pub(crate) struct Outcome {
    pub seen: Option<crate::SeenUsage>,
    pub graph: Option<BehaviorGraph>,
    pub prefix_level: Option<u64>,
    pub frontier: crate::FrontierUsage,
    pub predecessor: crate::result::PredecessorUsage,
    pub verdict: Verdict,
    pub stats: Option<Stats>,
    pub tlc_prints_stats: bool,
    pub counterexample: Option<Counterexample>,
    pub collision: Option<Collision>,
    /// `Print` output of the search, in order (not of the replay).
    pub output: Vec<String>,
    /// Notes qualifying the result.
    pub notes: Vec<String>,
    /// The workers requested and used.
    pub workers: Workers,
}

/// TLC's `checkPostCondition` after a complete search (`ModelChecker.java:283-288`,
/// `Tool.checkPostConditionWithContext`): each postcondition in order, in the
/// empty context; the first that is not `TRUE` decides.
pub(crate) fn postconditions(
    m: &Model,
    control: &crate::budget::EvaluationControl,
) -> Option<Verdict> {
    let list = match &m.postconditions {
        Ok(list) => list,
        Err(e) => {
            let v = crate::model_error(e);
            return Some(match v {
                Verdict::ModelError { code, message, .. } => Verdict::ModelError {
                    code: 1000,
                    name: "GENERAL".to_owned(),
                    message: class::general_text(&format!(
                        "@!@!@STARTMSG {code}:0 @!@!@\n{message}\n@!@!@ENDMSG {code} @!@!@"
                    )),
                },
                other => other,
            });
        }
    };
    for pc in list {
        // TLC looks the name up only now (`Spec.getPostConditionSpecs`). A
        // definition it evaluated at start-up has become a value, which it
        // refuses as a constant, with `GENERAL` (fixtures `ErPostConstTrue`,
        // `ErPostConstFalse`, `ErPostNonBool`).
        if m.evaluator.precomputed(pc.pred.scope.def()).is_some() {
            return Some(Verdict::ModelError {
                code: 1000,
                name: "GENERAL".to_owned(),
                message: class::general_text(&format!(
                    "@!@!@STARTMSG 2233:0 @!@!@\nThe post condition {} cannot be a constant.\n@!@!@ENDMSG 2233 @!@!@",
                    pc.name
                )),
            });
        }
        let location = m.evaluator.location(pc.pred.location);
        match m.eval(&pc.pred, &St::constant()) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                return Some(Verdict::PostconditionFalse {
                    property: pc.name.clone(),
                    message: format!("Postcondition {} at {location} is false.", pc.name),
                });
            }
            // `Tool.isValid`: `TLC_EXPECTED_VALUE` (fixture `ErPostNonBoolVar`).
            Ok(_) => {
                let e = EvalError::coded(
                    Ec::ExpectedValue,
                    format!("TLC expected a boolean value, but did not find one. {location}"),
                );
                return Some(class::postcondition_error(&e, &pc.name, &location));
            }
            Err(e) => {
                return Some(
                    control
                        .verdict(&e)
                        .unwrap_or_else(|| class::postcondition_error(&e, &pc.name, &location)),
                );
            }
        }
    }
    None
}

/// How many workers a run with `opts` uses on `m`, and why fewer than asked.
pub(crate) fn plan_workers(m: &Model, opts: &CheckOptions) -> Workers {
    let requested = opts.workers.max(1);
    if let Some(first) = m.sequential.first() {
        let more = m.sequential.len() - 1;
        let reason = if more == 0 {
            format!("sequential mode: the specification uses {first}")
        } else {
            format!("sequential mode: the specification uses {first} (and {more} more)")
        };
        return Workers {
            requested,
            used: 1,
            sequential: true,
            reason: Some(reason),
        };
    }
    if opts.sequential {
        return Workers {
            requested,
            used: 1,
            sequential: true,
            reason: Some("sequential mode requested (--sequential)".to_owned()),
        };
    }
    Workers {
        requested,
        used: requested,
        sequential: false,
        reason: None,
    }
}

/// Optional consumers of the shared search. The default has no hooks.
#[derive(Default)]
pub(crate) struct SearchHooks<'a, 'obs> {
    pub(crate) capture: Option<(crate::capture::Limits, bool)>,
    pub(crate) checkpoint: Option<&'a mut crate::checkpoint::Session>,
    pub(crate) observer: Option<&'obs mut dyn crate::capture::Observer>,
    pub(crate) primary: Option<&'a mut crate::capture::CheckpointHooks<'obs>>,
}

/// Runs the search over `m` (assumptions already checked).
pub(crate) fn search(
    m: &Model,
    opts: &CheckOptions,
    host: &RunHost,
    hooks: SearchHooks<'_, '_>,
) -> Outcome {
    match opts.store {
        StoreMode::Fingerprint => search_in::<FpSet>(m, opts, host, hooks),
        StoreMode::Exact => search_in::<ExactSet>(m, opts, host, hooks),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the search and its outcome, in order"
)]
fn search_in<S: Keyed>(
    m: &Model,
    opts: &CheckOptions,
    host: &RunHost,
    hooks: SearchHooks<'_, '_>,
) -> Outcome {
    let control = match crate::budget::EvaluationControl::new(
        &opts.budget,
        if hooks.capture.is_some() {
            crate::budget::Phase::Capture
        } else {
            crate::budget::Phase::Search
        },
    ) {
        Ok(control) => control,
        Err(error) => {
            return Outcome {
                seen: None,
                graph: None,
                prefix_level: None,
                frontier: crate::FrontierUsage::default(),
                predecessor: crate::result::PredecessorUsage::default(),
                verdict: Verdict::InternalError {
                    message: error.to_string(),
                },
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: m.take_output(),
                notes: vec![],
                workers: plan_workers(m, opts),
            };
        }
    };
    let _control_scope = match control.enter(&m.evaluator) {
        Ok(scope) => scope,
        Err(error) => {
            return Outcome {
                seen: None,
                graph: None,
                prefix_level: None,
                frontier: crate::FrontierUsage::default(),
                predecessor: crate::result::PredecessorUsage::default(),
                verdict: Verdict::InternalError {
                    message: error.to_string(),
                },
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: m.take_output(),
                notes: vec![],
                workers: plan_workers(m, opts),
            };
        }
    };
    let SearchHooks {
        capture,
        checkpoint,
        observer,
        primary,
    } = hooks;
    let (capture, concrete_pool) =
        capture.map_or((None, false), |(limits, pool)| (Some(limits), pool));
    let mut notes = Vec::new();
    let mut workers = plan_workers(m, opts);
    if primary.is_some() {
        workers.used = 1;
        workers.sequential = true;
        workers.reason =
            Some("opt-in reached capture checkpoints use the inline W1 executor".into());
    }
    let random = match bound_configured_random(m) {
        Ok(configured) => uses_random(&m.ir) || configured,
        Err(feature) => {
            workers.used = 0;
            return Outcome {
                prefix_level: None,
                seen: None,
                predecessor: crate::result::PredecessorUsage::default(),
                graph: None,
                frontier: crate::FrontierUsage::default(),
                verdict: Verdict::Unsupported { feature },
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: Vec::new(),
                notes,
                workers,
            };
        }
    };
    let tlc_order: Vec<usize> = m.ir.tlc_var_order().iter().map(|v| v.index()).collect();
    let keyer = match S::keyer(opts, &tlc_order) {
        Ok(k) => k,
        Err(message) => {
            return Outcome {
                prefix_level: None,
                seen: None,
                predecessor: crate::result::PredecessorUsage::default(),
                graph: None,
                frontier: crate::FrontierUsage::default(),
                verdict: Verdict::InternalError { message },
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: Vec::new(),
                notes,
                workers,
            };
        }
    };
    let Some(seeder) = StateFingerprinter::tlc(opts.fp_index, &tlc_order) else {
        return Outcome {
            prefix_level: None,
            seen: None,
            predecessor: crate::result::PredecessorUsage::default(),
            graph: None,
            frontier: crate::FrontierUsage::default(),
            verdict: Verdict::InternalError {
                message: format!(
                    "fingerprint polynomial index {} out of range",
                    opts.fp_index
                ),
            },
            stats: None,
            tlc_prints_stats: false,
            counterexample: None,
            collision: None,
            output: Vec::new(),
            notes,
            workers,
        };
    };
    if opts.seen_disk.is_some() {
        workers.used = 1;
        workers.sequential = true;
        workers.reason = Some("disk seen-state retention uses inline search until immutable disk read views are proved".into());
    }
    let disk = match opts
        .seen_disk
        .as_ref()
        .map(|request| crate::seen::Run::open(opts.store, request))
        .transpose()
    {
        Ok(disk) => disk,
        Err(error) => {
            workers.used = 0;
            return Outcome {
                prefix_level: None,
                seen: Some(crate::seen::SeenUsage::creation_failure(
                    opts.seen_disk.as_ref(),
                    &error,
                )),
                predecessor: crate::result::PredecessorUsage::default(),
                graph: None,
                frontier: crate::FrontierUsage::default(),
                verdict: crate::seen::verdict(&error),
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: m.take_output(),
                notes,
                workers,
            };
        }
    };
    let mut s = Search {
        m,
        host,
        seeder,
        random,
        ctx: opts.eval.ctx,
        keyer: &keyer,
        set: S::fresh(dive_store::shard_bits_for(workers.used)),
        disk,
        log: PredecessorLog::new(
            opts.predecessor_memory_bytes,
            opts.predecessor_directory.as_deref(),
        ),
        generated: 0,
        depth: 0,
        initial_generated: 0,
        capture: capture.map(|limits| Collector::new(limits).with_concrete_pool(concrete_pool)),
        checkpoint,
        observer,
        primary,
        prefix_level: None,
        budget: opts.budget.clone(),
        control: control.clone(),
        admission: (opts.max_states.is_some() || opts.max_depth.is_some()).then(|| {
            Box::new(AdmissionLimits {
                max_states: opts.max_states,
                max_depth: opts.max_depth,
                started: std::time::Instant::now(),
            })
        }),
    };
    let accounting = crate::frontier::accounting(opts);
    let mut next = crate::frontier::Queue::new(opts, accounting.clone());
    let recovered = if let Some(session) = s.checkpoint.take() {
        let restored = session.restore::<S>(&mut s, &mut next);
        s.checkpoint = Some(session);
        restored
    } else {
        Ok(None)
    };
    let mut no_actions = false;
    let (stop, stats) = match recovered {
        Ok(Some(level)) => s.begin_levels(next, opts, &mut workers, level),
        Err(verdict) => {
            drop(next);
            workers.used = 1;
            workers.sequential = true;
            workers.reason =
                Some("native checkpoint admission stopped before the worker pool started".into());
            (
                Some(Stop {
                    verdict,
                    in_init: false,
                    path: None,
                    then: None,
                }),
                s.stats(0),
            )
        }
        Ok(None) => {
            if let Some((stop, stats)) = s.initial_states(&mut next) {
                drop(next);
                if !workers.sequential {
                    workers.used = 1;
                    workers.sequential = true;
                    workers.reason = Some("search stopped during inline initial-state enumeration before the worker pool started".into());
                }
                (Some(stop), stats)
            } else {
                // TLC reports depth 1 once Init generated a state, even when
                // the constraints excluded all of them (#231).
                s.depth = s.depth.max(u64::from(s.initial_generated > 0));
                let stats = s.stats(next.len() as u64);
                if let Err(verdict) = s.observe_initial(stats) {
                    workers.used = 1;
                    workers.sequential = true;
                    workers.reason =
                        Some("initialization observer stopped before the worker pool".into());
                    (
                        Some(Stop {
                            verdict,
                            in_init: true,
                            path: None,
                            then: None,
                        }),
                        stats,
                    )
                } else if m.actions.is_empty() {
                    // TLC's `ModelChecker.modelCheck` returns after a
                    // successful Init when no action is bound: it starts no
                    // workers and checks no POSTCONDITION (#231). A nonempty
                    // queue was already reported by `initial_states`.
                    no_actions = true;
                    workers.used = 1;
                    workers.sequential = true;
                    workers.reason = Some("no actions: the search ends after Init".into());
                    drop(next);
                    (None, stats)
                } else {
                    s.begin_levels(next, opts, &mut workers, 1)
                }
            }
        }
    };
    let mut frontier = crate::frontier::evidence(&accounting);
    let mut stop = if frontier.failures.is_empty() {
        stop
    } else {
        frontier.deciding_event_before_failure = Some(stop.as_ref().map_or_else(
            || "completed safety search".into(),
            |s| format!("{:?}", s.verdict),
        ));
        Some(Stop {
            verdict: Verdict::ResourceLimit {
                message: format!("frontier storage failed: {}", frontier.failures.join("; ")),
            },
            in_init: false,
            path: None,
            then: None,
        })
    };
    let mut output = m.take_output();
    let collision = if stop.is_none() {
        if let Some(disk) = &mut s.disk {
            match disk.collision(opts.store, stats.distinct, stats.generated) {
                Ok(collision) => collision,
                Err(error) => {
                    stop = Some(Stop {
                        verdict: crate::seen::verdict(&error),
                        in_init: false,
                        path: None,
                        then: None,
                    });
                    None
                }
            }
        } else {
            s.set.collision(stats.distinct, stats.generated)
        }
    } else {
        None
    };
    let Some(stop) = stop else {
        // checkPostCondition runs after the complete search, with the
        // final counters visible through TLCGet.
        for (counter, value) in [
            (Counter::Generated, stats.generated),
            (Counter::Distinct, stats.distinct),
            (Counter::Queue, stats.left_on_queue),
            (Counter::Initial, s.initial_generated),
            (Counter::Diameter, stats.depth),
        ] {
            host.set_counter(counter, i64::try_from(value).ok());
        }
        let verdict = if no_actions || s.prefix_level.is_some() {
            Verdict::Success
        } else {
            postconditions(m, &control).unwrap_or(Verdict::Success)
        };
        output.extend(m.take_output());
        return crate::seen::finish(
            &mut s.disk,
            finalize_predecessor(
                &mut s.log,
                Outcome {
                    prefix_level: s.prefix_level,
                    seen: None,
                    predecessor: crate::result::PredecessorUsage::default(),
                    graph: s.capture.take().map(Collector::finish),
                    frontier,
                    collision: if verdict == Verdict::Success {
                        collision
                    } else {
                        None
                    },
                    verdict,
                    stats: Some(stats),
                    tlc_prints_stats: true,
                    counterexample: None,
                    output,
                    notes,
                    workers,
                },
            ),
        );
    };
    // TLCTrace's initial-predecessor fast path reuses the original state's
    // object. TLCStateRandom then retains its consumed stream rather than
    // reseeding. A final random successor can disappear from the printed
    // trace. Until that object/printing contract is ported, strict mode
    // refuses this boundary instead of attaching a different full witness.
    let unproved_initial_replay = s.random
        && stop.then.is_some()
        && stop
            .path
            .as_ref()
            .is_some_and(|(id, _)| s.log.get(*id).is_ok_and(|step| step.parent.is_none()));
    // The replay re-evaluates `Init` and `Next`; their output is not
    // repeated, and neither is a warning they raise.
    let cx = {
        let _quiet = dive_eval::capture::PrintCapture::start();
        s.counterexample(&stop)
    };
    let _ = m.take_output();
    let unproved_disk_print_replay = m.evaluator.has_print_event()
        && s.disk
            .as_mut()
            .is_some_and(|disk| disk.refuse_print_replay(&stop.verdict));
    if s.disk
        .as_ref()
        .and_then(crate::seen::Run::replay_evidence)
        .is_some_and(|replay| replay.unproved_print_output)
    {
        notes.push("Actual Print/PrintT events occurred before or during disk invariant-error reconstruction. TLC repeats reconstruction output; this native output contract is unproved. Retained output, counts and original error are diagnostic evidence, not class/output agreement or a certificate.".into());
    }
    if let Some(replay) = s.disk.as_ref().and_then(crate::seen::Run::replay_evidence)
        && replay
            .halted_by
            .as_ref()
            .is_some_and(|halt| !halt.is_verdict())
    {
        notes.push(format!(
            "The search reached {:?}; positioned disk replay stopped at {:?}. Retained counters/word are diagnostic evidence, not completed error replay or a certificate.",
            replay.original_error, replay.halted_by
        ));
    }
    let deciding_event = format!("{:?}", stop.verdict);
    let (verdict, counterexample) = match cx {
        _ if unproved_disk_print_replay => (crate::seen::print_replay_refusal(), None),
        _ if unproved_initial_replay => {
            notes.push(format!("The search reached {:?}; its first-successor random trace has an unproved TLC replay/printing contract (D-002). Statistics record that stopping prefix, not a completed certification.", stop.verdict));
            (
                Verdict::Unsupported {
                    feature: "TLC initial-predecessor random counterexample replay is not proved"
                        .into(),
                },
                None,
            )
        }
        Ok(c) => (stop.verdict, c),
        Err(message) if s.random && s.log.failure().is_none() => {
            // TLC rebuilds the trace the same way, from a fresh start-up
            // stream, and loses it too when draws made before `Init`
            // (constant definitions, `ASSUME`s, constant-level invariants)
            // shift the initial states (D-002, fixture `TmRandom`).
            notes.push(format!(
                "The counterexample cannot be rebuilt: the specification draws random values before Init, and the initial states regenerated from a fresh start-up stream differ, so TLC prints no trace either (D-002). ({message})"
            ));
            (stop.verdict, None)
        }
        Err(message) => (Verdict::InternalError { message }, None),
    };
    if matches!(&m.postconditions, Ok(l) if !l.is_empty()) {
        notes.push(
            "POSTCONDITION is not evaluated after a violation (TLC evaluates it with the counterexample; its result does not change the verdict).".to_owned(),
        );
    }
    let mut final_outcome = finalize_predecessor(
        &mut s.log,
        Outcome {
            prefix_level: None,
            seen: None,
            predecessor: crate::result::PredecessorUsage::default(),
            graph: None,
            frontier,
            // TLC prints statistics after an error in the initial states, not
            // after a violation there (probe `InitViolation`, fixtures `ErInit*`).
            tlc_prints_stats: !stop.in_init || matches!(verdict, Verdict::EvaluationError { .. }),
            verdict,
            stats: Some(stats),
            counterexample,
            collision: None,
            output,
            notes,
            workers,
        },
    );
    if !final_outcome.predecessor.failures.is_empty() {
        final_outcome.predecessor.deciding_event_before_failure = Some(deciding_event);
    }
    crate::seen::finish(&mut s.disk, final_outcome)
}

/// Close storage only after replay, retaining the deciding event on failure.
fn finalize_predecessor(log: &mut PredecessorLog, mut outcome: Outcome) -> Outcome {
    let mut evidence = crate::result::PredecessorUsage {
        storage: log.usage(),
        ..crate::result::PredecessorUsage::default()
    };
    if let Some(failure) = log.failure() {
        evidence.failures.push(failure.to_string());
    }
    match log.close() {
        Ok(()) => evidence.cleanup_completed = true,
        Err(error) => evidence.failures.push(error.to_string()),
    }
    if !evidence.failures.is_empty() {
        evidence.deciding_event_before_failure = Some(format!("{:?}", outcome.verdict));
        outcome.verdict = Verdict::ResourceLimit {
            message: format!(
                "predecessor storage failed: {}",
                evidence.failures.join("; ")
            ),
        };
        outcome.collision = None;
        outcome.counterexample = None;
        outcome.graph = None;
    }
    outcome.predecessor = evidence;
    outcome
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "storage-failure regression controls")]
mod predecessor_tests {
    use super::*;

    #[derive(Debug)]
    struct TripDuringPathReplay {
        visits: std::sync::atomic::AtomicUsize,
        budget: crate::budget::Budget,
    }

    impl dive_eval::Host for TripDuringPathReplay {
        fn check_tlc_access(&self, key: &str, write: bool) -> Result<(), dive_eval::HostError> {
            if key == "level"
                && !write
                && self
                    .visits
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    == 1
            {
                // Init alone calls TLCGet. The first call is ordinary search;
                // the second trips during predecessor-path regeneration,
                // before the long state-dependent predicate loop.
                self.budget.trip_for_test();
            }
            Ok(())
        }
    }

    #[test]
    fn deciding_violation_path_regeneration_preserves_control_and_the_original_event() {
        use std::sync::Arc;
        use std::time::Duration;

        for width in [1, 4] {
            let source = "---- MODULE ReplayBudget ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0 /\\ TLCGet(\"level\")=0 /\\ \\A k \\in 1..4096: k+x=k\nNext == x'=x+1\nInv == x<2\n====\n";
            let analyzed = dive_sema::analyze_source(
                "ReplayBudget.tla",
                source,
                &dive_sema::Options::default(),
            );
            assert!(analyzed.accepted());
            let ir = Arc::new(dive_ir::lower(&analyzed).unwrap());
            let cfg =
                dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n")
                    .into_config()
                    .unwrap();
            let watchdog = crate::budget::Budget::new(Some(Duration::from_secs(3600)), None)
                .start()
                .unwrap()
                .unwrap();
            let mut opts = CheckOptions::tlc(0, 0);
            opts.workers = width;
            opts.budget = watchdog.budget().clone();
            let host = Arc::new(run_host(&opts));
            let model = dive_model::bind(
                ir,
                &cfg,
                &dive_model::BindOptions {
                    eval: opts.eval,
                    symmetry_liveness: opts.symmetry_liveness,
                    no_deadlock: opts.no_deadlock,
                    host: Some(Arc::new(TripDuringPathReplay {
                        visits: std::sync::atomic::AtomicUsize::new(0),
                        budget: opts.budget.clone(),
                    })),
                },
            )
            .unwrap();
            let control =
                crate::budget::EvaluationControl::new(&opts.budget, crate::budget::Phase::Search)
                    .unwrap();
            let _outer = control.enter(&model.evaluator).unwrap();
            let result = search(&model, &opts, &host, SearchHooks::default());
            assert_eq!(
                result.verdict,
                Verdict::InvariantViolated {
                    property: "Inv".into(),
                    initial: false,
                }
            );
            assert!(opts.budget.poll().unwrap().is_some());
            let word = result.counterexample.unwrap();
            assert_eq!(word.states.len(), 3);
            assert_eq!(
                word.states
                    .iter()
                    .map(|state| state.values.clone())
                    .collect::<Vec<_>>(),
                vec![
                    vec![Value::int(0.into())],
                    vec![Value::int(1.into())],
                    vec![Value::int(2.into())]
                ]
            );
            // Search and diagnostic scopes restored the exact outer owner:
            // ordinary regeneration still observes its exhausted budget.
            let interrupted = model
                .evaluator
                .init_states(&model.init_actions, &mut |_| Flow::Continue)
                .unwrap_err();
            assert!(control.owns(&interrupted.error));
        }
    }

    #[test]
    fn diagnostic_text_never_creates_storage_failure_evidence() {
        let mut log = PredecessorLog::new(None, None);
        log.push_initial(0).unwrap();
        let verdict = Verdict::InternalError {
            message: "caller message contains DIVE-S0300".into(),
        };
        let outcome = finalize_predecessor(
            &mut log,
            Outcome {
                prefix_level: None,
                seen: None,
                predecessor: crate::result::PredecessorUsage::default(),
                graph: None,
                frontier: crate::FrontierUsage::default(),
                verdict: verdict.clone(),
                stats: None,
                tlc_prints_stats: false,
                counterexample: None,
                collision: None,
                output: Vec::new(),
                notes: Vec::new(),
                workers: Workers::one(),
            },
        );
        assert_eq!(outcome.verdict, verdict);
        assert_eq!(outcome.predecessor.failures, [] as [String; 0]);
        assert_eq!(outcome.predecessor.deciding_event_before_failure, None);
        assert!(outcome.predecessor.cleanup_completed);
    }

    #[test]
    fn cleanup_failure_preserves_decision_counts_and_actual_workers() {
        let parent =
            std::env::temp_dir().join(format!("dive-predecessor-engine-{}", std::process::id()));
        std::fs::create_dir(&parent).unwrap();
        let mut log = PredecessorLog::new(Some(0), Some(&parent));
        log.push_initial(5).unwrap();
        log.push(0, 7).unwrap();
        let stats = Stats {
            generated: 11,
            distinct: 2,
            depth: 2,
            left_on_queue: 1,
            initial_generated: 1,
        };
        // Delete the backing files before close. Neither a violation nor a
        // successful search may be certified after this cleanup/read failure.
        std::fs::remove_dir_all(&parent).unwrap();
        let verdict = Verdict::InvariantViolated {
            property: "Inv".into(),
            initial: false,
        };
        let outcome = finalize_predecessor(
            &mut log,
            Outcome {
                prefix_level: None,
                seen: None,
                predecessor: crate::result::PredecessorUsage::default(),
                graph: None,
                frontier: crate::FrontierUsage::default(),
                verdict: verdict.clone(),
                stats: Some(stats),
                tlc_prints_stats: true,
                counterexample: Some(Counterexample::default()),
                collision: None,
                output: vec!["observed".into()],
                notes: Vec::new(),
                workers: Workers {
                    requested: 8,
                    used: 4,
                    sequential: false,
                    reason: Some("pool limit".into()),
                },
            },
        );
        assert!(matches!(outcome.verdict, Verdict::ResourceLimit { .. }));
        assert_eq!(outcome.stats, Some(stats));
        assert_eq!(outcome.workers.used, 4);
        assert_eq!(outcome.output, ["observed"]);
        assert!(outcome.counterexample.is_none());
        assert_eq!(
            outcome.predecessor.deciding_event_before_failure,
            Some(format!("{verdict:?}"))
        );
        assert!(!outcome.predecessor.cleanup_completed);
        assert_eq!(outcome.predecessor.storage.records, 2);
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "configured RNG stream regression assertions"
)]
mod configured_stream_tests {
    use super::*;
    use std::sync::Arc;

    fn run(configured: bool, invariant: &str, seed: i64, width: usize) -> Outcome {
        run_case(configured, invariant, seed, width, "")
    }

    fn run_case(
        configured: bool,
        invariant: &str,
        seed: i64,
        width: usize,
        extra: &str,
    ) -> Outcome {
        let call = if configured {
            "C(1..100)"
        } else {
            "RandomElement(1..100)"
        };
        let source = format!(
            "---- MODULE Stream ----\nEXTENDS Naturals, TLC\nCONSTANT C(_)\nVARIABLES n,x\n{extra}\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'={call}\nInv == {invariant}\n====\n"
        );
        let a = dive_sema::analyze_source("Stream.tla", &source, &dive_sema::Options::default());
        assert!(a.accepted());
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let cfg = dive_config::parse("CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n").into_config().expect("cfg");
        let mut opts = CheckOptions::tlc(0, seed);
        opts.workers = width;
        opts.chaos = Some(19);
        let host = Arc::new(run_host(&opts));
        let bind = dive_model::BindOptions {
            eval: opts.eval,
            symmetry_liveness: opts.symmetry_liveness,
            no_deadlock: opts.no_deadlock,
            host: Some(Arc::new(crate::host::EngineHost::new(host.clone()))),
        };
        let model = dive_model::bind(ir, &cfg, &bind).expect("bound model");
        model.evaluator.intern_checker_names();
        let config = run_config(&opts, model.settings.check_deadlock);
        model.evaluator.intern_bfs_config(&config);
        host.set_config(Some(config));
        startup_counters(&host);
        assert!(matches!(
            model.check_assumptions(),
            dive_eval::AssumptionOutcome::AllTrue
        ));
        let assume_output = model.take_output();
        // Compare this internal route independently with the direct source call.
        let mut outcome = search(&model, &opts, &host, SearchHooks::default());
        let mut startup = model
            .output
            .iter()
            .filter_map(|message| match message {
                dive_model::Message::Print(text) => Some(text.clone()),
                dive_model::Message::Warning { .. } => None,
            })
            .collect::<Vec<_>>();
        startup.extend(assume_output);
        startup.extend(outcome.output);
        outcome.output = startup;
        outcome
    }

    #[test]
    fn actual_configured_target_matches_direct_draws_and_inline_order() {
        for seed in [0, 7, 14] {
            for width in [1, 4, 8] {
                for invariant in ["x # 74", "x # 80", "n < 3"] {
                    let configured = run(true, invariant, seed, width);
                    let direct = run(false, invariant, seed, width);
                    assert_eq!(
                        configured.verdict, direct.verdict,
                        "seed{seed} width{width} {invariant}"
                    );
                    assert_eq!(configured.stats, direct.stats);
                    assert_eq!(configured.workers.used, 1);
                    assert!(configured.workers.sequential);
                    let states = |out: &Outcome| {
                        out.counterexample.as_ref().map(|trace| {
                            trace
                                .states
                                .iter()
                                .map(|state| state.values.clone())
                                .collect::<Vec<_>>()
                        })
                    };
                    assert_eq!(states(&configured), states(&direct));
                    if seed == 7 && invariant == "n < 3" {
                        let x = states(&configured)
                            .expect("trace")
                            .into_iter()
                            .map(|state| state[1].clone())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            x,
                            [
                                Value::Int(0),
                                Value::Int(80),
                                Value::Int(94),
                                Value::Int(100)
                            ]
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn configured_startup_draws_preserve_direct_stream_and_print_order() {
        for extra in [
            "ASSUME PrintT(\"startup\")",
            "ASSUME RandomElement(1..100)>0",
            "ASSUME C(1..100)>0",
            "CachedConfigured == C(1..100)",
            "ConfiguredPrint == Print(C(1..100), TRUE)",
            "Cached == RandomElement(1..100)",
            "ASSUME Print(RandomElement(1..100), TRUE)",
            "Unused == RandomElement(1..100)",
        ] {
            for seed in [0, 7, 14] {
                for width in [1, 4, 8] {
                    let configured = run_case(true, "n < 3", seed, width, extra);
                    let direct = run_case(false, "n < 3", seed, width, extra);
                    assert_eq!(configured.verdict, direct.verdict, "{extra} seed{seed}");
                    assert_eq!(configured.stats, direct.stats);
                    assert_eq!(configured.output, direct.output);
                    assert_eq!(configured.workers.used, 1);
                    let states = |out: &Outcome| {
                        out.counterexample.as_ref().map(|trace| {
                            trace
                                .states
                                .iter()
                                .map(|state| state.values.clone())
                                .collect::<Vec<_>>()
                        })
                    };
                    assert_eq!(states(&configured), states(&direct));
                }
            }
        }
    }
}
