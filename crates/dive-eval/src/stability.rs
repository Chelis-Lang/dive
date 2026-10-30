//! Optional, callback-free canonical producer closure for a private row driver.
//!
//! This is not a purity claim about arbitrary deferred providers. Unknown
//! constructors/caches decline before the driver starts graph capture.

use std::sync::{Arc, Weak};

use dive_ir::ExprId;

use crate::core::Core;
use crate::{Evaluator, Scope};

#[cfg(test)]
#[path = "stability_tests.rs"]
mod tests;

mod budgeted;

#[cfg(test)]
#[path = "stability/budgeted_tests.rs"]
mod budgeted_tests;

/// A producer/atom closure checked without evaluation for one evaluator.
///
/// This fact covers only the supplied roots, ordinary cache domain, and the
/// canonical representation capability. A driver must additionally prove its
/// source roots are complete and its host/runtime is privately owned. There is
/// no public constructor and this does not grant temporal error authority.
///
/// ```
/// use dive_eval::{Evaluator, Scope};
/// use dive_ir::ExprId;
/// fn inspect(evaluator: &Evaluator, root: (ExprId, Scope)) -> dive_eval::Result<()> {
///     if let Some(fact) = evaluator.canonical_producer_stability_budgeted(&[root], &[], None)? {
///         assert!(fact.belongs_to(evaluator));
///     }
///     Ok(())
/// }
/// # let _ = inspect;
/// ```
#[derive(Clone)]
pub struct ProducerStability {
    owner: Weak<Core>,
}

impl std::fmt::Debug for ProducerStability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProducerStability").finish_non_exhaustive()
    }
}

impl ProducerStability {
    /// Whether this fact belongs to the actual evaluator being inspected.
    #[must_use]
    pub fn belongs_to(&self, evaluator: &Evaluator) -> bool {
        self.owner.ptr_eq(&Arc::downgrade(&evaluator.core))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fact {
    Canonical,
    // Successful enumeration yields canonical elements, but the domain itself
    // is not admitted to reordered predicates, captured values or caches.
    YieldCanonical,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Producer,
    Reordered,
}

impl Evaluator {
    /// Inspects a pure predicate closure over closed function-tree state inputs.
    /// Immutable interval domains and stored closed general functions are
    /// admitted. Function constructors, CHOOSE, EXCEPT, raw/symbolic set
    /// history and ENABLED retain the existing explicit inspection refusal.
    /// Callers must prove the actual input rows are closed and must preserve
    /// EXCEPT warnings/errors through reference fallback. This is not the
    /// canonical producer certificate and does not attest arbitrary providers.
    ///
    /// ```no_run
    /// # fn inspect(ev: &dive_eval::Evaluator, scope: &dive_eval::Scope,
    /// # expr: dive_ir::ExprId) -> dive_eval::Result<()> {
    /// let eligible = ev.closed_predicate_stability_budgeted(scope, expr, None)?;
    /// # let _ = eligible;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// An original interruption or inspection allocation failure.
    pub fn closed_predicate_stability_budgeted(
        &self,
        scope: &Scope,
        expr: ExprId,
        control: Option<&dyn crate::HostControl>,
    ) -> crate::Result<bool> {
        budgeted::closed_predicate(self, scope, expr, control)
    }

    /// Inspects an optional action closure whose single-key EXCEPT operations
    /// require a separate compiler/runtime proof of closed base, immutable
    /// interval binder, exact domain and scalar replacement. This fact alone
    /// never licenses evaluating an update or skipping reference warnings.
    /// EXCEPT @, constructors, CHOOSE and query/effect history remain refused.
    ///
    /// ```no_run
    /// # fn inspect(ev: &dive_eval::Evaluator, scope: &dive_eval::Scope, expr: dive_ir::ExprId) -> dive_eval::Result<()> {
    /// let closure = ev.guarded_action_stability_budgeted(scope, expr, None)?;
    /// // A successful inspection still requires compiler/runtime update guards.
    /// # let _ = closure;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// An original interruption or inspection allocation failure.
    pub fn guarded_action_stability_budgeted(
        &self,
        scope: &Scope,
        expr: ExprId,
        control: Option<&dyn crate::HostControl>,
    ) -> crate::Result<bool> {
        budgeted::closed_action(self, scope, expr, control)
    }

    /// Inspects canonical producer and predicate storage under the run budget.
    ///
    /// This callback-free inspection uses explicit traversal stacks, without a
    /// formula, value-depth or proof-work ceiling. Unknown representation or
    /// transfer semantics still return `None`. An explicit control is polled
    /// when supplied; otherwise the evaluator's installed control is polled.
    /// No provider, expression or native permission callback is invoked.
    /// This grants neither private-host ownership nor completeness of the
    /// supplied producer roots.
    ///
    /// ```no_run
    /// # fn inspect(ev: &dive_eval::Evaluator, root: (dive_ir::ExprId, dive_eval::Scope))
    /// # -> dive_eval::Result<()> {
    /// if let Some(fact) = ev.canonical_producer_stability_budgeted(&[root], &[], None)? {
    ///     assert!(fact.belongs_to(ev));
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// The original typed interruption, or failed traversal allocation.
    pub fn canonical_producer_stability_budgeted(
        &self,
        producers: &[(ExprId, Scope)],
        predicates: &[(ExprId, Scope)],
        control: Option<&dyn crate::HostControl>,
    ) -> crate::Result<Option<ProducerStability>> {
        budgeted::prove(self, producers, predicates, control)
    }
}

// Private fixed-data/independence inspection. Boolean/totality follows only
// from the original completed singleton predicate observations, not this fact.
pub(crate) fn closed_subset_forall(
    evaluator: &Evaluator,
    original: Scope,
    quantifier: ExprId,
    body: ExprId,
    outer_slot: usize,
    state: &crate::St,
    base: &[dive_value::Value],
) -> crate::Result<bool> {
    budgeted::closed_forall(
        evaluator, original, quantifier, body, outer_slot, state, base,
    )
}
