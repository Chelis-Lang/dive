//! Bounded views of actual recursive functions, never function admission proofs.

use crate::env::Ev;
use crate::{EvalError, Evaluator, LazyFcn, Result, Scope, St, StateRef};
use dive_ir::{Expr, ExprId};
use dive_value::Value;
use std::sync::Arc;

type Columns = Vec<crate::scope::BindingColumn>;

/// Captured source data and concrete bindings of an actual recursive function.
/// This view is not a purity, finiteness, termination or truth certificate.
#[derive(Debug)]
pub struct RecursiveScopes {
    /// Original recursive constructor expression.
    pub constructor: ExprId,
    /// Original function body expression.
    pub body: ExprId,
    /// Captured lexical/dynamic scope with the actual recursive self.
    pub source: Scope,
    /// Captured states, control flags and original invocation budget.
    pub state: St,
    /// Concrete formal bindings in existing exploration order.
    pub bindings: Vec<Scope>,
}

impl Evaluator {
    /// Constructs an owned recursive function without materializing its body.
    /// Domains and the entire body are inspected without evaluation first;
    /// evaluated domains retain the interpreter's lexical and state captures.
    /// Finite domain entries and Cartesian bindings use the existing limits.
    /// This establishes neither termination nor a predicate's truth.
    ///
    /// # Errors
    /// Invalid ownership/shape, effects, nonfinite domains, domain evaluation
    /// errors, or existing work/depth/entry limits.
    pub fn scope_recursive_function_bounded(
        &self,
        scope: &Scope,
        constructor: ExprId,
        st: &St,
        limit: usize,
    ) -> Result<Arc<LazyFcn>> {
        self.admit_scoped_expr(scope, constructor)?;
        self.validate_scope(scope)?;
        if st.selectseq.as_ref().is_some_and(|policy| {
            !std::sync::Weak::ptr_eq(&policy.context, &Arc::downgrade(&self.core))
        }) {
            return Err(EvalError::unsupported(
                "foreign recursive predicate invocation",
            ));
        }
        if !matches!(self.ir().expr(constructor), Expr::RecFcn { .. }) {
            return Err(EvalError::unsupported(
                "recursive function constructor required",
            ));
        }
        self.check_recursive_analysis(scope, constructor, st, limit)?;
        let Ev::F(function) = self.core.eval(&mut scope.env.clone(), st, constructor)? else {
            return Err(EvalError::bug(
                "recursive constructor lost its lazy function",
            ));
        };
        self.recursive_columns(&function, limit)?;
        Ok(function)
    }

    /// Concrete body contexts of an actual owned recursive function.
    /// Every context binds the real recursive self and concrete formal values,
    /// and retains the function's captured states, controls and invocation budget.
    /// No body is evaluated and no function is forced/materialized.
    ///
    /// # Errors
    /// Foreign/stale scope, function, state or invocation; unsupported function
    /// shape; domain/pattern errors; or existing finite domain/binding limits.
    pub fn recursive_function_body_scopes_bounded(
        &self,
        origin: &Scope,
        function: &Arc<LazyFcn>,
        invocation: &St,
        limit: usize,
    ) -> Result<RecursiveScopes> {
        self.validate_scope(origin)?;
        if !Arc::ptr_eq(&function.core, &self.core)
            || !same_state(&function.st.s0, &invocation.s0)
            || !same_state(&function.st.s1, &invocation.s1)
            || (function.st.control & !crate::env::control::KEEP_LAZY)
                != (invocation.control & !crate::env::control::KEEP_LAZY)
            || function.st.selectseq_callback != invocation.selectseq_callback
            || !match (&function.st.selectseq, &invocation.selectseq) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    Arc::ptr_eq(a, b)
                        && std::sync::Weak::ptr_eq(&a.context, &Arc::downgrade(&self.core))
                }
                _ => false,
            }
        {
            return Err(EvalError::unsupported(
                "foreign or stale recursive function context",
            ));
        }
        let scope = Scope {
            env: function.body_env(),
            ..origin.clone()
        };
        self.admit_scoped_expr(&scope, function.node)?;
        let columns = self.recursive_columns(function, limit)?;
        let bindings = self.bounded_body_contexts(&scope, function.node, &columns, limit)?;
        Ok(RecursiveScopes {
            constructor: function.node,
            body: function.body,
            source: scope,
            state: function.st.clone(),
            bindings,
        })
    }

    fn recursive_columns(&self, function: &LazyFcn, limit: usize) -> Result<Columns> {
        let Expr::RecFcn { name, bounds, body } = self.ir().expr(function.node) else {
            return Err(EvalError::unsupported("opaque recursive function origin"));
        };
        if function.rec_slot != Some(name.index())
            || *body != function.body
            || bounds.len() != function.bounds.len()
            || !function.excepts.is_empty()
        {
            return Err(EvalError::unsupported("modified recursive function shape"));
        }
        let mut budget =
            crate::bounded_domain::Budget::new(limit, self.core.opts.max_depth, self.core.ctx);
        let mut entries = 0_usize;
        let mut columns = Vec::new();
        for (source, bound) in bounds.iter().zip(&function.bounds) {
            if source.tuple != bound.tuple
                || !source
                    .vars
                    .iter()
                    .map(|s| s.index())
                    .eq(bound.slots.iter().copied())
            {
                return Err(EvalError::unsupported("recursive formal binding shape"));
            }
            let Value::Set(set) = &bound.domain else {
                return Err(EvalError::unsupported(
                    "recursive function domain is not a set",
                ));
            };
            budget.set(set, 0)?;
            let size = usize::try_from(set.size(self.core.ctx)?)
                .map_err(|_| EvalError::resource_limit("quantifier domain entries"))?;
            entries = entries
                .checked_add(size)
                .filter(|n| *n <= limit)
                .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
            let values = self.core.explore(set)?;
            if bound.tuple {
                columns.push((bound.slots.clone(), true, values));
            } else {
                for slot in &bound.slots {
                    columns.push((vec![*slot], false, values.clone()));
                }
            }
        }
        columns
            .iter()
            .try_fold(1_usize, |n, (_, _, ds)| n.checked_mul(ds.len()))
            .filter(|n| *n <= limit)
            .ok_or_else(|| EvalError::resource_limit("quantifier bindings"))?;
        Ok(columns)
    }
}

fn same_state(a: &StateRef, b: &StateRef) -> bool {
    match (a, b) {
        (StateRef::Null, StateRef::Null) | (StateRef::Empty, StateRef::Empty) => true,
        (StateRef::Vals(a), StateRef::Vals(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

#[cfg(test)]
#[path = "recursive_tests.rs"]
mod tests;
