//! Results of pure operator calls kept during one start-up evaluation
//! (`Evaluator::precompute`, TLC's `processConstantDefns`), as the core keeps
//! its own while it evaluates deferred definitions (`dive_core`'s
//! `CallCache`, `docs/developer/core/04-plan-compiler.md`).
//!
//! A start-up definition such as `MCReachabilityTestRandomGraphs`'s `Test`
//! applies the same constant operator to the same constant arguments
//! thousands of times (`Seq(Nodes)` with `Seq <- LimitedSeq`, issue #846);
//! each call rebuilds and canonicalizes the same set. A call is kept only
//! when its result cannot depend on anything but its arguments, and its
//! arguments are already values:
//! - the definition is a top-level, non-recursive, constant-level operator
//!   of a module that is not instantiated, with no impure operator, no
//!   substitution, no state and no outer binding anywhere it reaches (the
//!   bounded proof of [`Core::cacheable`], which also admits the operator's
//!   own parameters and follows every constant the configuration binds to
//!   a definition into that definition);
//! - each argument is a literal, a bound value or a constant bound to a
//!   value, so binding the value directly is indistinguishable from TLC's
//!   lazy binding, and evaluating it can neither fail nor have an effect;
//! - the evaluation runs in the default profile (canonical sets, so a value
//!   carries no normalization history), outside coverage, diagnostics and
//!   foreign cache domains.
//!
//! Arguments match by identity (a scalar's value, a compound value's
//! allocation), never by equality, so `1..3` and `{1, 2, 3}` stay apart.
//! Only successful values are kept; a failing call is evaluated again, and
//! fails again, wherever it is used.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use dive_ir::{Arg, DefId, DefScope, Expr, ExprId};
use dive_value::Value;

use crate::ConstBinding;
use crate::core::Core;
use crate::env::{Env, Ev, St, Sv, control};
use crate::error::Result;
use crate::options::Comparisons;

/// The most results one start-up evaluation keeps before it starts again.
const MAX_ENTRIES: usize = 4096;

/// How deep a chain of lazy substitutions an argument is followed through.
const MAX_SETTLE_DEPTH: usize = 8;

/// The kept calls: by definition, the control flags and arguments of each
/// call with its value.
#[derive(Default)]
pub(crate) struct StartupCalls {
    entries: HashMap<DefId, Vec<(u8, Vec<Value>, Value)>>,
    len: usize,
    /// How many calls were answered from the kept results.
    pub(crate) hits: u64,
}

impl StartupCalls {
    fn get(&mut self, d: DefId, control: u8, args: &[Value]) -> Option<Value> {
        let found = self.entries.get(&d)?.iter().find(|(c, a, _)| {
            *c == control && a.len() == args.len() && a.iter().zip(args).all(|(x, y)| same(x, y))
        })?;
        self.hits += 1;
        Some(found.2.clone())
    }

    fn insert(&mut self, d: DefId, control: u8, args: Vec<Value>, value: Value) {
        if self.len >= MAX_ENTRIES {
            self.entries.clear();
            self.len = 0;
        }
        self.entries
            .entry(d)
            .or_default()
            .push((control, args, value));
        self.len += 1;
    }
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
            || self.source_coverage.is_some()
            || self.expression_cache() != 0
        {
            return None;
        }
        if !self.call_cacheable(d) {
            return None;
        }
        let mut values = Vec::with_capacity(args.len());
        for a in args {
            let Arg::Expr { expr, .. } = a else {
                return None;
            };
            values.push(self.settled_value(caller, *expr, 0)?);
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
        for (i, v) in values.iter().enumerate() {
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
            } => {
                let info = self.ir.decl(*decl);
                if info.variable || info.var.is_some() {
                    return None;
                }
                match env.dynamic.lookup_cut(*decl, false) {
                    Some(Sv::Val(v)) => Some(v.clone()),
                    Some(Sv::Thunk(t)) if depth < MAX_SETTLE_DEPTH => {
                        self.settled_value(&t.env, t.expr, depth + 1)
                    }
                    Some(_) => None,
                    None => info.constant.and_then(|c| self.const_value(c)),
                }
            }
            _ => None,
        }
    }

    fn const_value(&self, c: dive_ir::ConstId) -> Option<Value> {
        match self.consts.get(c.index()).and_then(Option::as_ref) {
            Some(ConstBinding::Value(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// Whether a call of `d` depends only on its arguments (module
    /// documentation).
    fn call_cacheable(&self, d: DefId) -> bool {
        let def = self.ir.def(d);
        !def.params.is_empty()
            && def.std.is_none()
            && !def.recursive
            && def.level == dive_ir::Level::Constant
            && def.scope == DefScope::TopLevel
            && matches!(def.kind, dive_ir::DefKind::Operator)
            && !self.ir.module(def.module).instantiated
            && !def.effects.intersects(dive_ir::Effects::IMPURE)
            && *self.call_cache_safe[d.index()].get_or_init(|| self.state_free(d, true))
    }
}

#[cfg(test)]
#[path = "startup_calls_tests.rs"]
mod tests;
