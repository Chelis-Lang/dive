//! Values TLC keeps lazy: functions it holds as lambdas (`FcnLambdaValue`) and
//! the set constructions it does not compute eagerly (`SetPredValue`,
//! `SetCupValue`, `SetCapValue`, `SetDiffValue`, `UnionValue`).
//!
//! Lazy sets live inside [`Value`]s through `dive-value`'s [`DeferredSet`]
//! hook. Lazy functions are evaluator values ([`crate::env::Ev::F`]): they can be
//! applied, have a domain and take `EXCEPT`s without being computed, exactly
//! as TLC's `FcnLambdaValue` (`tlc2/value/impl/FcnLambdaValue.java`).

use std::fmt;
use std::sync::{Arc, OnceLock};

use dive_ir::ExprId;
use dive_value::{Ctx, DeferredKind, DeferredSet, Result as VResult, Value, ValueError, tlc};

use crate::core::Core;
use crate::env::{Env, Ev, St, Sv};
use crate::error::EvalError;

/// One bound group of a function constructor, with its evaluated domain.
#[derive(Clone, Debug)]
pub(crate) struct BoundVal {
    pub(crate) slots: Vec<usize>,
    pub(crate) tuple: bool,
    pub(crate) domain: Value,
}

/// One pending `EXCEPT` of a lazy function (TLC's `ValueExcept`): the path,
/// the position in it the function is at, and the new value.
#[derive(Clone, Debug)]
pub(crate) struct Except {
    pub(crate) path: Arc<[Value]>,
    pub(crate) idx: usize,
    pub(crate) value: Ev,
}

impl Except {
    pub(crate) fn current(&self) -> &Value {
        &self.path[self.idx]
    }

    pub(crate) fn is_last(&self) -> bool {
        self.idx + 1 == self.path.len()
    }

    pub(crate) fn advanced(&self) -> Self {
        Self {
            path: self.path.clone(),
            idx: self.idx + 1,
            value: self.value.clone(),
        }
    }
}

/// TLC's `FcnLambdaValue`: `[x \in S |-> e]` kept as a lambda, or a recursive
/// function `f[x \in S] == e`.
pub struct LazyFcn {
    pub(crate) core: Arc<Core>,
    /// The constructor node, for error locations.
    pub(crate) node: ExprId,
    pub(crate) bounds: Vec<BoundVal>,
    pub(crate) body: ExprId,
    pub(crate) env: Env,
    pub(crate) st: St,
    /// The slot that names the function inside its own body (recursive
    /// functions).
    pub(crate) rec_slot: Option<usize>,
    pub(crate) excepts: Vec<Except>,
    /// `fcnRcd`: the computed function, once materialized.
    pub(crate) materialized: OnceLock<Value>,
}

impl fmt::Debug for LazyFcn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyFcn")
            .field("node", &self.node)
            .field("bounds", &self.bounds)
            .field("excepts", &self.excepts.len())
            .finish_non_exhaustive()
    }
}

impl LazyFcn {
    /// The number of arguments (`FcnParams.length()`).
    pub(crate) fn arity(&self) -> usize {
        self.bounds
            .iter()
            .map(|b| if b.tuple { 1 } else { b.slots.len() })
            .sum()
    }

    /// A copy with one more `EXCEPT` (`FcnLambdaValue.takeExcept`).
    pub(crate) fn with_except(self: &Arc<Self>, ex: Except) -> Arc<Self> {
        let mut excepts = self.excepts.clone();
        excepts.push(ex);
        Arc::new(Self {
            core: self.core.clone(),
            node: self.node,
            bounds: self.bounds.clone(),
            body: self.body,
            env: self.env.clone(),
            st: self.st.clone(),
            rec_slot: self.rec_slot,
            excepts,
            materialized: OnceLock::new(),
        })
    }

    /// The environment for evaluating the body: the captured one, with the
    /// recursive name bound to the function itself.
    pub(crate) fn body_env(self: &Arc<Self>) -> Env {
        let mut env = self.env.clone();
        if let Some(slot) = self.rec_slot {
            env.bind(slot, Sv::Fcn(self.clone()));
        }
        env
    }
}

// ---------------------------------------------------------------------------------
// Deferred sets
// ---------------------------------------------------------------------------------

/// `{x \in S : P}` over a set TLC cannot reduce (`SetPredValue`).
pub(crate) struct Filter {
    pub(crate) core: Arc<Core>,
    pub(crate) node: ExprId,
    pub(crate) slots: Vec<usize>,
    pub(crate) tuple: bool,
    pub(crate) in_val: Value,
    pub(crate) pred: ExprId,
    pub(crate) env: Env,
    pub(crate) st: St,
}

impl fmt::Debug for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter")
            .field("node", &self.node)
            .finish_non_exhaustive()
    }
}

/// Converts an evaluation error into the value layer's error type, keeping
/// it recoverable on the other side ([`crate::core::unwrap_deferred`]).
pub(crate) fn to_value_error(e: EvalError) -> ValueError {
    crate::error::stash(e)
}

impl DeferredSet for Filter {
    fn kind(&self) -> DeferredKind {
        DeferredKind::Filter
    }

    fn elements(&self, _ctx: Ctx) -> VResult<Vec<Value>> {
        self.core.filter_elements(self).map_err(to_value_error)
    }

    fn member(&self, elem: &Value, _ctx: Ctx) -> Option<VResult<bool>> {
        Some(self.core.filter_member(self, elem).map_err(to_value_error))
    }

    fn key(&self) -> Vec<Value> {
        vec![self.in_val.clone(), Value::Int(i64::from(self.pred.0))]
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// `S \cup T`, `S \cap T`, `S \ T` of sets TLC keeps lazy.
pub(crate) struct SetOp {
    pub(crate) core: Arc<Core>,
    pub(crate) kind: DeferredKind,
    pub(crate) a: Value,
    pub(crate) b: Value,
}

impl fmt::Debug for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetOp")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl DeferredSet for SetOp {
    fn kind(&self) -> DeferredKind {
        self.kind
    }

    fn elements(&self, _ctx: Ctx) -> VResult<Vec<Value>> {
        self.core.setop_elements(self).map_err(to_value_error)
    }

    fn member(&self, elem: &Value, ctx: Ctx) -> Option<VResult<bool>> {
        Some(match self.kind {
            DeferredKind::Cup => tlc::member(elem, &self.a, ctx).and_then(|m| {
                if m {
                    Ok(true)
                } else {
                    tlc::member(elem, &self.b, ctx)
                }
            }),
            DeferredKind::Cap => tlc::member(elem, &self.a, ctx).and_then(|m| {
                if m {
                    tlc::member(elem, &self.b, ctx)
                } else {
                    Ok(false)
                }
            }),
            DeferredKind::Diff => tlc::member(elem, &self.a, ctx).and_then(|m| {
                if m {
                    tlc::member(elem, &self.b, ctx).map(|n| !n)
                } else {
                    Ok(false)
                }
            }),
            DeferredKind::Union => self
                .core
                .union_member(&self.a, elem)
                .map_err(to_value_error),
            DeferredKind::Filter | DeferredKind::Map => Ok(false),
        })
    }

    fn key(&self) -> Vec<Value> {
        vec![self.a.clone(), self.b.clone()]
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}
