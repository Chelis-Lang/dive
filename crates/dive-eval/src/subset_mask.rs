//! Closed-context inspection and original-observation singleton learning.
//! Metadata alone certifies neither Boolean results nor operative permission.

use crate::core::Core;
use crate::env::control;
use crate::lazy::Filter;
use crate::{Evaluator, Result, Scope};
use ::std::sync::{Arc, Weak};
use dive_ir::{Expr, QuantKind};
use dive_value::{ConcreteSubsetStream, DeferredSet, SubsetScratch, Value};

pub(crate) struct ClosedForall {
    filter: Weak<dyn DeferredSet>,
    source: dive_value::Set,
}

impl ClosedForall {
    pub(crate) fn inspect(
        core: &Arc<Core>,
        owner: &Arc<dyn DeferredSet>,
        source: &ConcreteSubsetStream,
    ) -> Result<Option<Self>> {
        if ::std::any::Any::type_id(owner.as_ref()) != ::std::any::TypeId::of::<Filter>() {
            return Ok(None);
        }
        let Some(filter) = owner
            .as_any()
            .and_then(|owner| owner.downcast_ref::<Filter>())
        else {
            return Ok(None);
        };
        if !Arc::ptr_eq(core, &filter.core)
            || filter.tuple
            || filter.slots.len() != 1
            || filter.st.control & (control::PRIMED | control::ENABLED) != 0
        {
            return Ok(None);
        }
        let Value::Set(input) = &filter.in_val else {
            return Ok(None);
        };
        if !source.belongs_to(input) {
            return Ok(None);
        }
        let Expr::SetFilter { bound, pred } = core.ir.expr(filter.node) else {
            return Ok(None);
        };
        if *pred != filter.pred
            || bound.tuple
            || bound.vars.len() != 1
            || bound.vars[0].index() != filter.slots[0]
        {
            return Ok(None);
        }
        let mut predicate = filter.pred;
        loop {
            core.host.check_control()?;
            match core.ir.expr(predicate) {
                Expr::Label { body, .. } | Expr::Selected(body) => predicate = *body,
                _ => break,
            }
        }
        let Expr::Quant {
            kind: QuantKind::Forall,
            bounds,
            body,
        } = core.ir.expr(predicate)
        else {
            return Ok(None);
        };
        let [quantified] = bounds.as_slice() else {
            return Ok(None);
        };
        if quantified.vars.is_empty() || (!quantified.tuple && quantified.vars.len() != 1) {
            return Ok(None);
        }
        let Some(domain) = quantified.domain else {
            return Ok(None);
        };
        let Expr::Local(local) = core.ir.expr(domain) else {
            return Ok(None);
        };
        if local.up != 0 || local.slot.index() != filter.slots[0] {
            return Ok(None);
        }
        let evaluator = Evaluator { core: core.clone() };
        let scope = Scope {
            env: filter.env.clone(),
            owner: core.ir.clone(),
            names: core.names.clone(),
            context: Arc::downgrade(core),
            wrapper: None,
        };
        if !crate::stability::closed_subset_forall(
            &evaluator,
            scope,
            predicate,
            *body,
            filter.slots[0],
            &filter.st,
            source.base_elements(),
        )? {
            return Ok(None);
        }
        Ok(Some(Self {
            filter: Arc::downgrade(owner),
            source: input.clone(),
        }))
    }

    pub(crate) fn belongs_to(
        &self,
        owner: &Arc<dyn DeferredSet>,
        source: &ConcreteSubsetStream,
    ) -> bool {
        self.filter.ptr_eq(&Arc::downgrade(owner)) && source.belongs_to(&self.source)
    }
}

/// Only the actual predicate-entry depths are compared; no depth is waived.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Entry {
    evaluation: u32,
    value: u32,
}

impl Entry {
    fn current() -> Option<Self> {
        (!dive_value::quiet_operation_active()).then(|| Self {
            evaluation: Core::evaluation_depth(),
            value: dive_value::checked_operation_depth(),
        })
    }
}

/// Learning belongs to one authentic filter/source, not an expression cache.
/// No truth is inferred until every original singleton has returned Boolean.
#[derive(Default)]
pub(crate) struct SingletonMask {
    phase: Phase,
    scratch: SubsetScratch,
    #[cfg(test)]
    commits: usize,
}

#[derive(Default)]
enum Phase {
    #[default]
    First,
    Empty(Entry),
    Learning {
        proof: ClosedForall,
        entry: Entry,
        allowed: Vec<usize>,
        observed: usize,
    },
    Ready {
        proof: ClosedForall,
        entry: Entry,
        allowed: Vec<usize>,
    },
    Off,
}

impl SingletonMask {
    /// Called after any previously yielded value's original sink has returned.
    /// In particular, no proof/allocation happens before the empty sink.
    pub(crate) fn before_next(
        &mut self,
        core: &Arc<Core>,
        owner: &Arc<dyn DeferredSet>,
        source: &mut ConcreteSubsetStream,
    ) -> Result<()> {
        if let Phase::Empty(entry) = &self.phase {
            let entry = *entry;
            self.phase = Phase::Off;
            if Entry::current() != Some(entry) || source.base_elements().is_empty() {
                return Ok(());
            }
            let Some(proof) = ClosedForall::inspect(core, owner, source)? else {
                return Ok(());
            };
            let mut allowed = Vec::new();
            // Optional optimization storage: refusal leaves the original next
            // candidate untouched and uses the ordinary cursor.
            if allowed.try_reserve(source.base_elements().len()).is_err() {
                return Ok(());
            }
            self.phase = Phase::Learning {
                proof,
                entry,
                allowed,
                observed: 0,
            };
        }
        let Phase::Ready {
            proof,
            entry,
            allowed,
        } = &self.phase
        else {
            return Ok(());
        };
        if !proof.belongs_to(owner, source) || Entry::current() != Some(*entry) {
            return Ok(());
        }
        core.host.check_control()?;
        if !source
            .prepare_masked_next(allowed, &mut self.scratch, || {
                core.host
                    .check_control()
                    .map_err(crate::lazy::to_value_error)
            })
            .map_err(crate::EvalError::from)?
        {
            return Ok(());
        }
        // A control hook can reenter/replace a binding. Recheck local depth
        // after all callbacks, then perform only the concrete constant-time
        // position swap under the CURRENT operative binding's mutex.
        core.host.check_control()?;
        if Entry::current() != Some(*entry) {
            return Ok(());
        }
        let committed = self
            .scratch
            .prepare_commit(source)
            .is_some_and(|position| core.host.commit_subset_position(position));
        #[cfg(test)]
        {
            self.commits += usize::from(committed);
        }
        #[cfg(not(test))]
        let _ = committed;
        core.host.check_control()?;
        Ok(())
    }

    /// Observe the original whole predicate result at its original boundary.
    /// False remains a result; errors/panics never publish a learned witness.
    pub(crate) fn observed(
        &mut self,
        source: &ConcreteSubsetStream,
        entry: Option<(u32, u32)>,
        kept: bool,
    ) {
        if matches!(self.phase, Phase::Off | Phase::Ready { .. }) {
            return;
        }
        let current = Entry::current();
        let entry = entry.map(|(evaluation, value)| Entry { evaluation, value });
        if current != entry || entry.is_none() {
            self.phase = Phase::Off;
            return;
        }
        let Some(indices) = source.current_indices() else {
            self.phase = Phase::Off;
            return;
        };
        let phase = ::std::mem::replace(&mut self.phase, Phase::Off);
        self.phase = match phase {
            Phase::First if indices.is_empty() && kept => entry.map_or(Phase::Off, Phase::Empty),
            Phase::Learning {
                proof,
                entry: learned,
                mut allowed,
                observed,
            } if entry == Some(learned) && indices == [observed].as_slice() => {
                if kept {
                    allowed.push(observed); // capacity reserved before learning
                }
                let observed = observed + 1;
                if observed == source.base_elements().len() {
                    Phase::Ready {
                        proof,
                        entry: learned,
                        allowed,
                    }
                } else {
                    Phase::Learning {
                        proof,
                        entry: learned,
                        allowed,
                        observed,
                    }
                }
            }
            ready @ Phase::Ready { .. } => ready,
            _ => Phase::Off,
        };
    }

    #[cfg(test)]
    pub(crate) fn progress(&self) -> (&'static str, usize) {
        let phase = match self.phase {
            Phase::First => "first",
            Phase::Empty(_) => "empty",
            Phase::Learning { .. } => "learning",
            Phase::Ready { .. } => "ready",
            Phase::Off => "off",
        };
        (phase, self.commits)
    }

    pub(crate) fn entry(&self) -> Option<(u32, u32)> {
        if matches!(self.phase, Phase::Off | Phase::Ready { .. }) {
            return None;
        }
        Entry::current().map(|entry| (entry.evaluation, entry.value))
    }
}

#[cfg(test)]
#[path = "subset_mask_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "subset_mask_integration_tests.rs"]
mod integration_tests;
