//! Query-local name-addressed states. Concrete graph states remain indexed.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use dive_ir::DeclId;
use dive_value::Value;

use crate::core::Core;
use crate::{ContextError, ContextErrorKind, EvalError, Evaluator, StateRef};

/// A partial existential assignment keyed by canonical IR variable name.
///
/// Hidden declarations with the same name share one slot, including collisions
/// with root names. These assignments belong to one evaluator and never add
/// variables to an ordinary graph state, fingerprint or serialized trace.
#[derive(Clone, Debug)]
pub struct NamedState {
    pub(crate) context: Weak<Core>,
    pub(crate) values: BTreeMap<Arc<str>, Value>,
}

impl NamedState {
    /// The assigned value of a canonical name; absence means unassigned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// The actual assigned names and values, in name order.
    pub fn assignments(&self) -> impl ExactSizeIterator<Item = (&str, &Value)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_ref(), value))
    }

    /// Number of assigned names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no name is assigned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn empty(core: &Arc<Core>) -> Arc<Self> {
        Arc::new(Self {
            context: Arc::downgrade(core),
            values: BTreeMap::new(),
        })
    }

    pub(crate) fn with(self: &Arc<Self>, name: &str, value: Value) -> Arc<Self> {
        self.clone().with_owned(name, value)
    }

    pub(crate) fn with_owned(mut self: Arc<Self>, name: &str, value: Value) -> Arc<Self> {
        // Cache keys, captured states and retained alternatives hold strong
        // Arcs, so only a genuinely unshared query state is changed in place.
        Arc::make_mut(&mut self)
            .values
            .insert(Arc::from(name), value);
        self
    }
}

impl Evaluator {
    pub(crate) fn validate_state_context(&self, st: &crate::St) -> crate::Result<()> {
        for state in [&st.s0, &st.s1] {
            if let StateRef::Named(state) = state
                && !std::ptr::eq(state.context.as_ptr(), Arc::as_ptr(&self.core))
            {
                return Err(ContextError::new(
                    ContextErrorKind::ForeignContext,
                    "named state belongs to another evaluator",
                )
                .into());
            }
        }
        Ok(())
    }

    /// Constructs a query-local partial assignment from real variable declarations.
    ///
    /// Duplicate declarations of the same canonical name must agree. This checks
    /// stored value names without interning, enumeration or callbacks. Opaque
    /// deferred values cannot be supplied through this checked public constructor.
    /// The resulting state is not an ordinary Next transition or a graph state.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use dive_eval::{Evaluator, Model, Options, St, StateRef};
    /// use dive_value::Value;
    /// let analysis = dive_sema::analyze_source("M.tla",
    ///     "---- MODULE M ----\nVARIABLE x\nRead == x\n====\n", &Default::default());
    /// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    /// let read = ir.root_def("Read").expect("Read");
    /// let x = ir.vars()[0];
    /// let evaluator = Evaluator::new(Model::new(ir), Options::tlc()).expect("evaluator");
    /// let assignment = evaluator.named_state([(x, Value::TRUE)]).expect("assignment");
    /// let mut state = St::empty();
    /// state.s0 = StateRef::Named(assignment);
    /// assert_eq!(evaluator.eval_def(read, &state).expect("read"), Value::TRUE);
    /// ```
    ///
    /// # Errors
    /// An unknown/nonvariable declaration, inconsistent duplicate assignment or
    /// stored value whose name-table ownership cannot be checked.
    pub fn named_state(
        &self,
        assignments: impl IntoIterator<Item = (DeclId, Value)>,
    ) -> crate::Result<Arc<NamedState>> {
        let mut values = BTreeMap::new();
        for (decl, value) in assignments {
            let declaration = self.core.ir.decls().get(decl.index()).ok_or_else(|| {
                ContextError::new(
                    ContextErrorKind::UnknownId,
                    "named-state variable declaration",
                )
            })?;
            if !declaration.variable {
                return Err(ContextError::new(
                    ContextErrorKind::Structure,
                    "named-state key is not a variable",
                )
                .into());
            }
            value.validate_names(&self.core.names).map_err(|error| {
                ContextError::new(ContextErrorKind::ForeignContext, error.to_string())
            })?;
            if values
                .get(declaration.name.as_str())
                .is_some_and(|old| old != &value)
            {
                return Err(EvalError::general(format!(
                    "Conflicting assignments to query variable {}.",
                    declaration.name,
                )));
            }
            values.insert(Arc::<str>::from(declaration.name.as_str()), value);
        }
        Ok(Arc::new(NamedState {
            context: Arc::downgrade(&self.core),
            values,
        }))
    }
}

impl Core {
    /// One variable-read rule for indexed graph states and named query states.
    pub(crate) fn state_variable<'a>(
        self: &Arc<Self>,
        state: &'a StateRef,
        name: &str,
        root: Option<usize>,
    ) -> crate::Result<Option<&'a Value>> {
        match state {
            StateRef::Vals(values) => Ok(root.and_then(|i| values.get(i)).and_then(Option::as_ref)),
            StateRef::Named(state) => {
                if !std::ptr::eq(state.context.as_ptr(), Arc::as_ptr(self)) {
                    return Err(ContextError::new(
                        ContextErrorKind::ForeignContext,
                        "named state belongs to another evaluator",
                    )
                    .into());
                }
                Ok(state.get(name))
            }
            StateRef::Null | StateRef::Empty => Ok(None),
        }
    }
}

#[cfg(test)]
#[path = "named_state_tests.rs"]
mod tests;
