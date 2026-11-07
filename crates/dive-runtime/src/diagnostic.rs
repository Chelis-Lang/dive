//! Diagnostic replays and TLC's state text, shared by the reference engine
//! and Core's error endings (ADR 0155).

use std::fmt::Write as _;

use dive_eval::next::Flow;
use dive_eval::{EvalError, St};
use dive_model::{Model, Named};
use dive_value::{Ctx, Value};

use crate::{ErrorSite, Verdict};

/// Evaluates a role once, leaving mode-specific nonboolean/error diagnostics to callers.
pub fn role_boolean(m: &Model, c: &Named, st: &St) -> Result<Option<bool>, EvalError> {
    m.eval(&c.pred, st).map(|value| match value {
        Value::Bool(value) => Some(value),
        _ => None,
    })
}

/// Re-evaluates an actual receiver failure with the diagnostic source hook.
/// Existing function materialization retains its role replay; tuple/record
/// receivers additionally require the first history-free Init emission and
/// first invariant with no quotient/constraint pipeline to reconstruct.
#[must_use]
pub fn initial_value_method_replay(
    m: &Model,
    verdict: &Verdict,
    state: &[Option<Value>],
    initial_context: Option<(u64, &crate::budget::EvaluationControl)>,
) -> Option<EvalError> {
    let _source_coverage = m.evaluator.pause_source_coverage();
    let Verdict::EvaluationError {
        site: ErrorSite::Invariant,
        cause,
        invariant: Some(name),
        ..
    } = verdict
    else {
        return None;
    };
    let method = cause.value_method()?.method();
    let role = m.invariants.iter().find(|role| role.name == *name)?;
    if m.ir
        .node(role.pred.expr)
        .effects
        .intersects(dive_ir::Effects::IMPURE)
        || m.evaluator.scope_has_effects(&role.pred.scope)
    {
        return None;
    }
    if method == dive_eval::ValueMethod::FunctionMaterialize {
        let context = St::state(std::sync::Arc::from(state)).diagnostic();
        return role_boolean(m, role, &context).err();
    }
    // Do not emulate Init's seen-state, constraint, VIEW, or prior-invariant
    // pipeline. This extension certifies its first emitted, history-free row
    // and first invariant only; other receiver lifetimes remain unproved.
    let (0, control) = initial_context? else {
        return None;
    };
    if m.invariants.first().is_none_or(|first| first.name != *name)
        || !m.constraints.is_empty()
        || m.view.is_some()
        || m.symmetry.is_some()
        || !receiver_replay_state(m, state, control)
        || m.init_actions.iter().any(|action| {
            m.ir.node(action.expr())
                .effects
                .intersects(dive_ir::Effects::IMPURE)
                || m.evaluator.scope_has_effects(&action.scope())
        })
    {
        return None;
    }
    let mut diagnostic = None;
    let _ = m
        .evaluator
        .init_states_diagnostic(&m.init_actions, &mut |succ| {
            if succ.ordinal == 0
                && receiver_replay_state(m, succ.state, control)
                && succ.state == state
            {
                let context = St::state(std::sync::Arc::from(succ.state)).diagnostic();
                diagnostic = role_boolean(m, role, &context).err();
            }
            Flow::Stop
        });
    diagnostic
}

fn receiver_replay_state(
    m: &Model,
    state: &[Option<Value>],
    control: &crate::budget::EvaluationControl,
) -> bool {
    let mut work = 0usize;
    let mut proceed = || {
        let due = work.trailing_zeros() >= 6;
        work = work.wrapping_add(1);
        !due || dive_eval::HostControl::check(control).is_ok()
    };
    state.iter().all(|value| {
        value.as_ref().is_some_and(|value| {
            // Existing controlled inspector proves the absence of set/provider
            // history, not construction provenance. Actual typed catches plus the
            // first generator emission supply that separate authority.
            value
                .closed_function_cache_heap_bytes(&m.names, &mut proceed)
                .is_some()
        })
    })
}

/// TLC's text of a state (`TLCStateMut.toString`): `/\ x = v` per variable
/// in TLC's variable order (`x = v` for a single variable), `null` for an
/// unassigned one.
pub fn state_text(m: &Model, s: &[Option<Value>], ctx: Ctx) -> String {
    let _source_coverage = m.evaluator.pause_source_coverage();
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
