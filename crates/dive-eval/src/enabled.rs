//! Invocation-local dispatch for clients that implement their own existential
//! procedure. The interpreter supplies the reached action and actual context;
//! it does not supply a successor or a checker decision.

use std::sync::{Arc, Weak};

use dive_ir::ExprId;

use crate::{Core, Env, Evaluator, Result, Scope, St, Value, env::Ev};

/// An existential procedure used by a particular evaluation invocation.
///
/// The callback receives the original, unbranched lexical scope. It owns query
/// branching, completeness and resource accounting. Ordinary interpreter order,
/// argument laziness and host access before reaching the query are unchanged.
/// Implementations must avoid retaining a strong reference to their evaluator:
/// lazy function values can retain invocation states.
pub trait EnabledResolver: std::fmt::Debug + Send + Sync {
    /// Decides the reached action in its actual current state and lexical scope.
    ///
    /// # Errors
    /// An evaluation error, explicit refusal, or exhausted independent budget.
    fn resolve(&self, scope: &Scope, action: ExprId, st: &St) -> Result<bool>;
}

#[derive(Clone, Debug)]
pub(crate) struct EnabledInvocation {
    pub(crate) context: Weak<Core>,
    pub(crate) resolver: Arc<dyn EnabledResolver>,
}

impl Evaluator {
    /// Installs a client-owned existential procedure on this invocation only.
    /// The callback is bound to this evaluator and participates in lazy-cache
    /// identity. It is never installed on the shared evaluator or its host.
    ///
    /// # Errors
    /// A supplied state or existing invocation belongs to another evaluator.
    pub fn with_enabled_resolver(&self, st: &St, resolver: Arc<dyn EnabledResolver>) -> Result<St> {
        self.validate_state_context(st)?;
        let mut result = st.clone();
        result.enabled = Some(EnabledInvocation {
            context: Arc::downgrade(&self.core),
            resolver,
        });
        Ok(result)
    }
}

impl St {
    /// Removes the client existential callback while preserving states,
    /// evaluation control and other invocation budgets. An independent query
    /// interpreter can then handle its own nested queries with shared counters.
    #[must_use]
    pub fn without_enabled_resolver(&self) -> Self {
        Self {
            enabled: None,
            ..self.clone()
        }
    }
}

impl Core {
    pub(crate) fn eval_enabled(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        action: ExprId,
        expr: ExprId,
    ) -> Result<Ev> {
        if let Some(invocation) = &st.enabled {
            let evaluator = Evaluator {
                core: self.clone(),
                collected: None,
            };
            evaluator.validate_state_context(st)?;
            let scope = Scope {
                env: env.clone(),
                owner: self.ir.clone(),
                names: self.names.clone(),
                context: Arc::downgrade(self),
                wrapper: None,
            };
            return invocation
                .resolver
                .resolve(&scope, action, st)
                .map(|value| Ev::V(Value::Bool(value)));
        }
        self.eval_enabled_native(env, st, action, expr)
    }
}
