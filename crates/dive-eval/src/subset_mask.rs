//! Independent closed-context gate. No truth, mask or host authority is inferred.
//! The entry is test-only until the operative foundation is integrated.

use crate::core::Core;
use crate::env::control;
use crate::lazy::Filter;
use crate::{Evaluator, Result, Scope};
use ::std::sync::{Arc, Weak};
use dive_ir::{Expr, QuantKind};
use dive_value::{ConcreteSubsetStream, DeferredSet, Value};

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

#[cfg(test)]
#[path = "subset_mask_tests.rs"]
mod tests;
