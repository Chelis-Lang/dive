//! Results of pure operator calls kept during one start-up evaluation
//! (`Evaluator::precompute`, TLC's `processConstantDefns`), as Core keeps
//! its own while it evaluates deferred definitions (`dive_core`'s
//! `CallCache`).
//!
//! A start-up definition such as `MCReachabilityTestRandomGraphs`'s `Test`
//! applies the same constant operator to the same constant arguments
//! thousands of times (`Seq(Nodes)` with `Seq <- LimitedSeq`);
//! each call rebuilds and canonicalizes the same set. A call is kept only
//! when its result cannot depend on anything but its inputs, and its inputs
//! are already values:
//! - the definition is a top-level, non-recursive, constant-level operator
//!   with no impure operator, no state and no outer binding anywhere it
//!   reaches (the bounded proof of [`Core::state_free`], which also admits
//!   the operator's own parameters, the parameters of the top-level
//!   definitions it calls, which are bound to arguments the proof visits,
//!   and the constants an instance substitutes, and follows every constant
//!   the configuration binds to a definition into that definition);
//! - each argument is a literal, a bound value or a constant bound to a
//!   value, so binding the value directly is indistinguishable from TLC's
//!   lazy binding, and evaluating it can neither fail nor have an effect;
//! - so is each substituted constant the definition reaches, as the call
//!   finds it in its dynamic context: those values are part of the call's
//!   inputs, so `P(Succ)!ExistsPath(m, n)` is kept for each `Succ`
//!   (`MCReachabilityTestRandomGraphs`);
//! - the evaluation runs in the default profile (canonical sets, so a value
//!   carries no normalization history), outside diagnostics and
//!   foreign cache domains.
//!
//! Inputs match by identity (a scalar's value, a compound value's
//! allocation), never by equality, so `1..3` and `{1, 2, 3}` stay apart.
//! Only successful values are kept; a failing call is evaluated again, and
//! fails again, wherever it is used. A definition whose calls keep missing
//! is no longer kept ([`MAX_UNREPEATED`]), which changes only the cost.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use dive_ir::{Arg, DeclId, DefId, DefScope, Expr, ExprId};
use dive_value::Value;

use crate::ConstBinding;
use crate::core::Core;
use crate::env::{Env, Ev, St, Sv, control};
use crate::error::Result;
use crate::options::Comparisons;

/// The most results one start-up evaluation keeps before it starts again.
const MAX_ENTRIES: usize = 4096;

/// How many more of a definition's calls may miss than hit before its calls
/// are no longer kept: a call that is never repeated (a path test applied
/// once to each candidate path) only costs the keeping.
const MAX_UNREPEATED: u32 = 512;

/// How deep a chain of lazy substitutions an argument is followed through.
const MAX_SETTLE_DEPTH: usize = 8;

/// One kept call: the definition, the control flags and the inputs (its
/// arguments, then the substitutions it reads) with its value.
struct Entry {
    def: DefId,
    control: u8,
    inputs: Vec<Value>,
    value: Value,
}

/// The kept calls, by a hash of their identities ([`identity_hash`]).
#[derive(Default)]
pub(crate) struct StartupCalls {
    entries: HashMap<u64, Vec<Entry>>,
    len: usize,
    /// Per definition: how many of its calls were kept and how many were
    /// answered, until it is no longer kept ([`MAX_UNREPEATED`]).
    yields: HashMap<DefId, (u32, u32)>,
    /// How many calls were answered from the kept results.
    pub(crate) hits: u64,
}

impl StartupCalls {
    /// Whether calls of `d` are no longer kept.
    fn declined(&self, d: DefId) -> bool {
        self.yields
            .get(&d)
            .is_some_and(|&(kept, hit)| kept > hit.saturating_add(MAX_UNREPEATED))
    }

    fn get(&mut self, d: DefId, control: u8, inputs: &[Value]) -> Option<Value> {
        let found = self
            .entries
            .get(&identity_hash(d, control, inputs))?
            .iter()
            .find(|e| {
                e.def == d
                    && e.control == control
                    && e.inputs.len() == inputs.len()
                    && e.inputs.iter().zip(inputs).all(|(x, y)| same(x, y))
            })?;
        self.hits += 1;
        let y = self.yields.entry(d).or_default();
        y.1 = y.1.saturating_add(1);
        Some(found.value.clone())
    }

    fn insert(&mut self, d: DefId, control: u8, inputs: Vec<Value>, value: Value) {
        if self.len >= MAX_ENTRIES {
            self.entries.clear();
            self.len = 0;
        }
        let y = self.yields.entry(d).or_default();
        y.0 = y.0.saturating_add(1);
        self.entries
            .entry(identity_hash(d, control, &inputs))
            .or_default()
            .push(Entry {
                def: d,
                control,
                inputs,
                value,
            });
        self.len += 1;
    }
}

/// A hash consistent with [`same`]: a compound value by its allocation, a
/// scalar by its value.
fn identity_hash(d: DefId, control: u8, inputs: &[Value]) -> u64 {
    let mix = |h: u64, x: u64| (h.rotate_left(5) ^ x).wrapping_mul(0x517c_c1b7_2722_0a95);
    let mut h = mix(u64::from(d.0), u64::from(control));
    for v in inputs {
        let x = match v {
            Value::Set(s) => s.addr() as u64,
            Value::Fcn(f) => f.addr() as u64,
            scalar => scalar.hash64(),
        };
        h = mix(h, x);
    }
    h
}

/// Whether two argument values are the same value in the same form: equal
/// scalars, or one compound allocation. The kept arguments hold their
/// allocations, so an address is never reused while it is compared.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Set(x), Value::Set(y)) => x.ptr_eq(y),
        (Value::Fcn(x), Value::Fcn(y)) => x.ptr_eq(y),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::BigInt(x), Value::BigInt(y)) => x == y,
        (Value::Str(x), Value::Str(y)) | (Value::Model(x), Value::Model(y)) => x == y,
        _ => false,
    }
}

impl Core {
    /// Calls `d` on `args` through the start-up call results when the call
    /// qualifies (module documentation); `None` when it does not, and the
    /// caller evaluates it as usual.
    pub(crate) fn startup_call(
        self: &Arc<Self>,
        caller: &Env,
        st: &St,
        d: DefId,
        args: &[Arg],
        e: ExprId,
    ) -> Option<Result<Ev>> {
        if args.is_empty()
            || !st.is(control::PRECOMPUTE)
            || st.is(control::DIAGNOSTIC)
            || self.opts.comparisons != Comparisons::Tla
            || self.expression_cache() != 0
        {
            return None;
        }
        let substs = self.call_cacheable(d)?;
        if self
            .startup_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_none_or(|calls| calls.declined(d))
        {
            return None;
        }
        let mut values = Vec::with_capacity(args.len() + substs.len());
        for a in args {
            let Arg::Expr { expr, .. } = a else {
                return None;
            };
            values.push(self.settled_value(caller, *expr, 0)?);
        }
        // The substitutions the body reads are inputs too: the body is
        // evaluated in the caller's dynamic context, where each is found
        // exactly as `eval_subst_ref` finds it.
        for &decl in substs.iter() {
            values.push(self.settled_subst(caller, decl)?);
        }
        {
            let mut calls = self
                .startup_calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let calls = calls.as_mut()?;
            if let Some(v) = calls.get(d, st.control, &values) {
                return Some(Ok(Ev::V(v)));
            }
        }
        let definition = self.ir.def(d);
        let mut body_env = Env::new(
            d,
            definition.depth,
            definition.frame.len(),
            None,
            caller.dynamic.clone(),
        );
        for (i, v) in values.iter().take(args.len()).enumerate() {
            body_env.bind(i, Sv::Val(v.clone()));
        }
        let r = match self.enter(e) {
            Ok(_g) => self.eval(&mut body_env, st, definition.body),
            Err(error) => Err(error),
        };
        if let Ok(Ev::V(v)) = &r
            && let Some(calls) = self
                .startup_calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_mut()
        {
            calls.insert(d, st.control, values, v.clone());
        }
        Some(r)
    }

    /// The value of an argument whose evaluation cannot fail, has no effect
    /// and reads no state: a literal, a bound value, a constant bound to a
    /// value, or a substitution (through lazy bindings) of one of these.
    fn settled_value(&self, env: &Env, expr: ExprId, depth: usize) -> Option<Value> {
        if let Some(v) = self.settled(env, expr) {
            return Some(v);
        }
        match &self.ir.node(expr).expr {
            Expr::Const(c) => self.const_value(*c),
            Expr::SubstRef {
                decl,
                primed: false,
            } => self.settled_subst_at(env, *decl, depth),
            _ => None,
        }
    }

    /// The value an unprimed use of the substituted constant `decl` has in
    /// `env`'s dynamic context, when finding it can neither fail nor have an
    /// effect (as for [`Self::settled_value`]).
    fn settled_subst(&self, env: &Env, decl: DeclId) -> Option<Value> {
        self.settled_subst_at(env, decl, 0)
    }

    fn settled_subst_at(&self, env: &Env, decl: DeclId, depth: usize) -> Option<Value> {
        let info = self.ir.decl(decl);
        if info.variable || info.var.is_some() {
            return None;
        }
        match env.dynamic.lookup_cut(decl, false) {
            Some(Sv::Val(v)) => Some(v.clone()),
            Some(Sv::Thunk(t)) if depth < MAX_SETTLE_DEPTH => {
                self.settled_value(&t.env, t.expr, depth + 1)
            }
            Some(_) => None,
            None => info.constant.and_then(|c| self.const_value(c)),
        }
    }

    fn const_value(&self, c: dive_ir::ConstId) -> Option<Value> {
        match self.consts.get(c.index()).and_then(Option::as_ref) {
            Some(ConstBinding::Value(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// The substituted constants a call of `d` reads besides its arguments,
    /// when its value depends on nothing else (module documentation);
    /// `None` when a call does not qualify.
    fn call_cacheable(&self, d: DefId) -> Option<Arc<[DeclId]>> {
        let def = self.ir.def(d);
        let qualifies = !def.params.is_empty()
            && def.std.is_none()
            && !def.recursive
            && def.level == dive_ir::Level::Constant
            && def.scope == DefScope::TopLevel
            && matches!(def.kind, dive_ir::DefKind::Operator)
            && !def.effects.intersects(dive_ir::Effects::IMPURE);
        if !qualifies {
            return None;
        }
        self.call_cache_safe[d.index()]
            .get_or_init(|| {
                let mut substs = Vec::new();
                self.state_free(d, true, Some(&mut substs))
                    .then(|| Arc::from(substs))
            })
            .clone()
    }
}

#[cfg(test)]
#[path = "startup_calls_tests.rs"]
mod tests;
