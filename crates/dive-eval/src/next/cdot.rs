//! Action composition, `A \cdot B`: a port of TLC's `OPCODE_cdot`
//! rules in `Tool.getNextStates`, `Tool.eval` and `Tool.enabled`
//! (`tlc2/tool/impl/Tool.java` at tlaplus `94d0c50`, `:1488-1531`,
//! `:2772-2808`, `:3371-3393`), which TLC runs only with
//! `-Dtlc2.tool.impl.Tool.cdot=true`.
//!
//! `s -(A \cdot B)-> u` iff some `t` has `s -A-> t` and `t -B-> u`. All three
//! rules enumerate the intermediate states `t` with the successor generator
//! (`A` from `s`), then take `B` from each `t`:
//!
//! - **Next states.** The intermediate state starts as `s0.copyWith(s1)`:
//!   the variables the enclosing action already assigned, set to their
//!   *current* values. `A` is enumerated with the enclosing continuation, and
//!   so is `B` from each `t`, whose successors are the composition's.
//!   `nss.incrementStatesGenerated(|iss|)` counts the intermediate states as
//!   generated, so a state whose `A` steps have no `B` step is not a
//!   deadlock. A nested composition's intermediate states are not counted:
//!   its functor is a `StateVec` or an anonymous one, which ignore the count.
//! - **Evaluation.** `A` from `s` into an empty state, with no continuation;
//!   true iff `B` holds between some `t` and the given successor.
//! - **ENABLED.** As for next states, with `enabled` for `B`.
//!
//! Where the continuation is empty and nothing is assigned yet, these rules
//! compute the TLA+ meaning exactly. Elsewhere the continuation is evaluated
//! after `A` as well (against the intermediate state) and assigned variables
//! are pre-set in `t`, which is not what `A \cdot B` means: TLC's mode
//! ([`ActionComposition::Tlc`]) reproduces it, while the default
//! ([`ActionComposition::Tla`]) refuses those uses by name and counts only the
//! composed successors as generated.

use std::cell::Cell;
use std::sync::Arc;

use dive_ir::ExprId;
use dive_ir::actions::ActionName;
use dive_value::Value;

use super::{Action, Acts, Enab, EnabledState, Flow, Gen, StateVals, Successor};
use crate::core::Core;
use crate::env::{Env, Ev, St, StateRef};
use crate::error::{EvalError, Result};
use crate::options::ActionComposition;

thread_local! {
    /// Whether this thread is running a stage of a bounded composition. A
    /// composition nested in a stage keeps its named refusal.
    static COMPOSING: Cell<bool> = const { Cell::new(false) };
}

/// TLC's message when the switch is off (`Tool.java:1528`).
pub(crate) fn off_error(core: &Core, e: ExprId) -> EvalError {
    EvalError::general(format!(
        "The current version of TLC does not support action composition.  An incomplete implementation can be enabled via the tlc2.tool.impl.Tool.cdot=true java property.\n{}",
        core.loc(e)
    ))
}

/// The refusal of a composition whose meaning TLC's algorithm distorts, in
/// the default mode.
fn distorted(core: &Core, e: ExprId) -> EvalError {
    EvalError::unsupported(format!(
        "action composition `A \\cdot B` with conjuncts after it or variables assigned before it (TLC's algorithm evaluates them against the intermediate state; `--action-composition=tlc` reproduces that) ({})",
        core.loc(e)
    ))
}

/// `s0.copyWith(s1)`: `s0`'s values of the variables `s1` assigns.
fn copy_with(s0: &StateRef, s1: &StateVals) -> StateVals {
    let current = match s0 {
        StateRef::Vals(v) => Some(v),
        _ => None,
    };
    s1.iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_ref()
                .and_then(|_| current.and_then(|c| c.get(i).cloned().flatten()))
        })
        .collect()
}

impl Core {
    fn bounded_composition(&self) -> bool {
        self.host
            .get()
            .is_some_and(|host| host.requires_bounded_action_composition())
    }

    fn unproved_composition<T>(&self) -> Result<T> {
        // Replay remembers this refusal even if an optional label swallows it.
        self.check_action_composition()?;
        Err(EvalError::unsupported(
            "unproved bounded action composition",
        ))
    }

    /// Whether both operands of a bounded composition, and every lazy value
    /// or operator their environment captures, are free of effects, nested
    /// searches and temporal operators, without evaluating anything.
    ///
    /// That is all the bounded relation needs. Its intermediate and final
    /// states come from the ordinary successor generator, and the operands
    /// are evaluated call by name: a lazy value (`LazyValue`) is reused only
    /// for the states it was computed in ([`crate::env::Thunk::cached`]), and
    /// a lazy function keeps its creation state. So a `LET` definition or an
    /// operator argument that reads a variable is recomputed in the
    /// intermediate state, as substitution requires, and an `EXCEPT`, a
    /// function constructor or a `CHOOSE` in either operand means what it
    /// means anywhere else in replay. Effects are excluded because the
    /// stages run them in an order and number of their own, and `ENABLED`
    /// and temporal operators because they start nested searches.
    fn composition_operands_pure(self: &Arc<Self>, env: &Env, a: ExprId, b: ExprId) -> bool {
        use dive_ir::Effects;
        let searches = Effects::LIVE_TLCGET
            .union(Effects::USES_ENABLED)
            .union(Effects::TEMPORAL);
        let forbidden = searches.union(Effects::IMPURE);
        let ir = &self.ir;
        // IR flags follow syntactic references only, and a configured `<-`
        // reaches another body: refuse when any target is impure, and check
        // the search flags on every target.
        let redirected = self
            .redirects
            .iter()
            .filter(|(source, target)| source != target)
            .map(|(_, target)| *target)
            .chain(
                self.consts
                    .iter()
                    .flatten()
                    .filter_map(|binding| match binding {
                        crate::ConstBinding::Def(target) => Some(*target),
                        crate::ConstBinding::Value(_) => None,
                    }),
            );
        for target in redirected {
            let Some(def) = ir.defs().get(self.redirect(target).index()) else {
                return false;
            };
            if def.effects.intersects(searches) {
                return false;
            }
        }
        let evaluator = crate::Evaluator {
            core: self.clone(),
            collected: None,
        };
        if evaluator.impure_redirect() {
            return false;
        }
        [a, b].into_iter().all(|operand| {
            ir.nodes()
                .get(operand.index())
                .is_some_and(|node| !node.effects.intersects(forbidden))
        }) && !crate::env::filter_has_unproved_captures(ir, env, forbidden)
    }

    /// Reuse the ordinary generator, never a checker callback or a
    /// separately implemented relation search. All first-stage work finishes
    /// before any tail or outward observation.
    fn bounded_cdot_intermediates(
        self: &Arc<Self>,
        env: &Env,
        s0: &StateRef,
        a: ExprId,
        b: ExprId,
    ) -> Result<Vec<StateVals>> {
        if self.opts.action_composition != ActionComposition::Tla {
            return self.unproved_composition();
        }
        if COMPOSING.with(Cell::get) || !self.composition_operands_pure(env, a, b) {
            return self.unproved_composition();
        }
        let current = self.composition_current(s0)?;
        if !matches!(&current, StateRef::Vals(values) if values.len() == self.ir.vars().len() && values.iter().all(Option::is_some))
        {
            return self.unproved_composition();
        }
        if let Some(host) = self.host.get() {
            host.check_bounded_action_composition()
                .map_err(crate::host::host_error)?;
        }
        let empty = || vec![None; self.ir.vars().len()].into();
        let intermediates = self.cdot_steps(env, &current, empty(), a, false)?;
        if intermediates
            .iter()
            .any(|state| state.iter().any(Option::is_none))
        {
            return self.unproved_composition();
        }
        Ok(intermediates)
    }

    fn bounded_cdot_tail(
        self: &Arc<Self>,
        env: &Env,
        intermediate: StateVals,
        b: ExprId,
    ) -> Result<Vec<StateVals>> {
        self.loop_work()?;
        let empty = vec![None; self.ir.vars().len()].into();
        let final_states = self.cdot_steps(env, &StateRef::Vals(intermediate), empty, b, true)?;
        if final_states.len() > 1
            || final_states
                .iter()
                .any(|state| state.iter().any(Option::is_none))
        {
            return self.unproved_composition();
        }
        Ok(final_states)
    }

    fn composition_named(self: &Arc<Self>, state: &crate::NamedState) -> Result<StateVals> {
        if !std::ptr::eq(state.context.as_ptr(), Arc::as_ptr(self)) {
            return Err(EvalError::unsupported(
                "foreign named action-composition state",
            ));
        }
        if state
            .assignments()
            .any(|(name, _)| self.root_var_named(name).is_none())
        {
            return Err(EvalError::unsupported(
                "action composition with assigned hidden ENABLED variables",
            ));
        }
        Ok(self
            .ir
            .vars()
            .iter()
            .map(|decl| state.get(&self.ir.decl(*decl).name).cloned())
            .collect())
    }

    fn composition_current(self: &Arc<Self>, state: &StateRef) -> Result<StateRef> {
        match state {
            StateRef::Named(state) => Ok(StateRef::Vals(self.composition_named(state)?)),
            _ => Ok(state.clone()),
        }
    }

    /// The states `A` leads to from `s0`, with `t` as the partial successor
    /// and no continuation (`getNextStates(Action.UNKNOWN, A,
    /// ActionItemList.Empty, c, s0, t, iss)`), duplicates included.
    fn cdot_steps(
        self: &Arc<Self>,
        env: &Env,
        s0: &StateRef,
        t: StateVals,
        a: ExprId,
        bounded_tail: bool,
    ) -> Result<Vec<StateVals>> {
        if !self.bounded_composition() {
            self.check_action_composition()?;
        }
        let s0 = self.composition_current(s0)?;
        let action = Action {
            owner: self.ir.clone(),
            names: self.names.clone(),
            context: Arc::downgrade(self),
            expr: a,
            env: env.clone(),
            name: ActionName::Unnamed,
            params: Vec::new(),
            label: String::new(),
            id: usize::MAX,
        };
        let mut iss: Vec<StateVals> = Vec::new();
        let mut collect = |s: &Successor<'_>| {
            iss.push(StateVals::from(s.state));
            Flow::Continue
        };
        let mut g = Gen {
            core: self,
            init: false,
            diagnostic: false,
            s0,
            s1: t,
            action: &action,
            ordinal: 0,
            emitted: 0,
            intermediate: 0,
            sink: &mut collect,
            choices: None,
        };
        let result = if self.bounded_composition() {
            let outer = COMPOSING.with(|composing| composing.replace(true));
            let result = g.step(a, &None, env);
            COMPOSING.with(|composing| composing.set(outer));
            result
        } else {
            g.step(a, &None, env)
        };
        if let Err(error) = result {
            // A tail that emitted before failing is not a proved deterministic
            // operation. In particular its late error cannot certify a
            // before-first-successor endpoint after hiding the earlier row.
            if bounded_tail && !iss.is_empty() && error.class() == crate::ErrorClass::Eval {
                return self.unproved_composition();
            }
            return Err(error);
        }
        Ok(iss)
    }

    /// `Tool.eval` of `A \cdot B` (`:2772-2808`).
    pub(crate) fn eval_cdot(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        a: ExprId,
        b: ExprId,
        e: ExprId,
    ) -> Result<Ev> {
        if self.bounded_composition() {
            let intermediates = self.bounded_cdot_intermediates(env, &st.s0, a, b)?;
            let target = self.composition_current(&st.s1)?;
            let StateRef::Vals(target) = target else {
                return self.unproved_composition();
            };
            if target.len() != self.ir.vars().len() || target.iter().any(Option::is_none) {
                return self.unproved_composition();
            }
            for intermediate in intermediates {
                for successor in self.bounded_cdot_tail(env, intermediate, b)? {
                    self.loop_work()?;
                    let mut equal = true;
                    for (left, right) in successor.iter().zip(target.iter()) {
                        let (Some(left), Some(right)) = (left, right) else {
                            return self.unproved_composition();
                        };
                        if !self.equals(left, right)? {
                            equal = false;
                            break;
                        }
                    }
                    if equal {
                        return Ok(Ev::V(Value::TRUE));
                    }
                }
            }
            return Ok(Ev::V(Value::FALSE));
        }
        if self.opts.action_composition == ActionComposition::Off {
            return Err(off_error(self, e));
        }
        let empty: StateVals = vec![None; self.ir.vars().len()].into();
        for t in self.cdot_steps(env, &st.s0, empty, a, false)? {
            self.loop_work()?;
            let st2 = St {
                s0: StateRef::Vals(t),
                s1: st.s1.clone(),
                control: st.control,
            };
            match self.eval(&mut env.clone(), &st2, b)? {
                Ev::V(Value::Bool(true)) => return Ok(Ev::V(Value::Bool(true))),
                Ev::V(Value::Bool(false)) => {}
                other => {
                    // TLC casts to `BoolValue`; the cast fails.
                    return Err(EvalError::general(format!(
                        "Attempted to evaluate an expression of form A \\cdot B, but B was {}.\n{}",
                        self.ev_string(&other),
                        self.loc(e)
                    )));
                }
            }
        }
        Ok(Ev::V(Value::Bool(false)))
    }
}

impl Gen<'_, '_, '_> {
    /// `getNextStates` of `A \cdot B` (`:1488-1531`).
    pub(super) fn cdot(
        &mut self,
        a: ExprId,
        b: ExprId,
        acts: &Acts,
        env: &Env,
        e: ExprId,
    ) -> Result<Flow> {
        if self.core.bounded_composition() {
            if acts.is_some() || self.s1.iter().any(Option::is_some) {
                return self.core.unproved_composition();
            }
            let intermediates = self.core.bounded_cdot_intermediates(env, &self.s0, a, b)?;
            for intermediate in intermediates {
                for successor in self.core.bounded_cdot_tail(env, intermediate, b)? {
                    self.core.loop_work()?;
                    self.s1 = successor;
                    if self.emit() == Flow::Stop {
                        self.s1 = vec![None; self.core.ir.vars().len()].into();
                        return Ok(Flow::Stop);
                    }
                }
            }
            self.s1 = vec![None; self.core.ir.vars().len()].into();
            return Ok(Flow::Continue);
        }
        if self.choices.is_some() {
            return Err(EvalError::unsupported(
                "constructive generation of action composition",
            ));
        }
        let core = self.core;
        let mode = core.opts.action_composition;
        match mode {
            ActionComposition::Off => return Err(off_error(core, e)),
            ActionComposition::Tla if acts.is_some() || self.s1.iter().any(Option::is_some) => {
                return Err(distorted(core, e));
            }
            _ => {}
        }
        // The private A-side collection bypasses the caller's successor sink
        // in both modes. Ask the host before entering that intermediate search.
        core.check_action_composition()?;
        // s -A-> t
        let mut iss: Vec<StateVals> = Vec::new();
        {
            let mut collect = |s: &Successor<'_>| {
                iss.push(StateVals::from(s.state));
                Flow::Continue
            };
            let mut g = Gen {
                core,
                init: false,
                diagnostic: false,
                s0: self.s0.clone(),
                s1: copy_with(&core.composition_current(&self.s0)?, &self.s1),
                action: self.action,
                ordinal: 0,
                emitted: 0,
                intermediate: 0,
                sink: &mut collect,
                choices: None,
            };
            g.step(a, acts, env)?;
        }
        if mode == ActionComposition::Tlc {
            self.intermediate += iss.len() as u64;
        }
        // t -B-> u
        let action = self.action;
        for t in iss {
            core.loop_work()?;
            let mut g = Gen {
                core,
                init: false,
                diagnostic: false,
                s0: StateRef::Vals(t),
                s1: self.s1.clone(),
                action,
                ordinal: self.ordinal,
                emitted: 0,
                intermediate: 0,
                sink: &mut *self.sink,
                choices: None,
            };
            let r = g.step(b, acts, env);
            let n = g.emitted;
            self.emitted += n;
            self.ordinal += n;
            if r? == Flow::Stop {
                return Ok(Flow::Stop);
            }
        }
        Ok(Flow::Continue)
    }
}

impl Enab {
    /// `enabled` of `A \cdot B` (`:3371-3393`).
    pub(super) fn cdot(
        &self,
        a: ExprId,
        b: ExprId,
        acts: &Acts,
        env: &Env,
        s1: &EnabledState,
        e: ExprId,
    ) -> Result<Option<EnabledState>> {
        let core = &self.core;
        if core.bounded_composition() {
            // Only a nonstuttering subscript may follow the complete composed
            // edge. Test it against the original s0, never the intermediate t.
            let mut continuation = acts.as_ref();
            while let Some(item) = continuation {
                if !matches!(item.kind, super::Kind::Changed) {
                    return core.unproved_composition();
                }
                continuation = item.next.as_ref();
            }
            if !s1.is_empty() {
                return core.unproved_composition();
            }
            for intermediate in core.bounded_cdot_intermediates(env, &self.s0, a, b)? {
                for successor in core.bounded_cdot_tail(env, intermediate, b)? {
                    core.loop_work()?;
                    let mut named = crate::NamedState::empty(core);
                    for (declaration, value) in core.ir.vars().iter().zip(successor.iter()) {
                        let Some(value) = value else {
                            return core.unproved_composition();
                        };
                        named = named.with(&core.ir.decl(*declaration).name, value.clone());
                    }
                    if let Some(successor) = self.cont(acts, named)? {
                        return Ok(Some(successor));
                    }
                }
            }
            return Ok(None);
        }
        let root_s1 = core.composition_named(s1)?;
        match core.opts.action_composition {
            ActionComposition::Off => return Err(off_error(core, e)),
            ActionComposition::Tla if acts.is_some() || !s1.is_empty() => {
                return Err(distorted(core, e));
            }
            _ => {}
        }
        let t0 = copy_with(&core.composition_current(&self.s0)?, &root_s1);
        for t in core.cdot_steps(env, &self.s0, t0, a, false)? {
            core.loop_work()?;
            let en = Enab {
                core: core.clone(),
                s0: StateRef::Vals(t),
            };
            if let Some(s2) = en.enabled(b, acts, env, s1.clone())? {
                return Ok(Some(s2));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_with_takes_current_values_of_assigned_variables() {
        let s0 = StateRef::Vals(vec![Some(Value::Int(1)), Some(Value::Int(2))].into());
        let s1: StateVals = vec![None, Some(Value::Int(9))].into();
        assert_eq!(&*copy_with(&s0, &s1), &[None, Some(Value::Int(2))]);
        assert_eq!(&*copy_with(&StateRef::Empty, &s1), &[None, None]);
    }
}
