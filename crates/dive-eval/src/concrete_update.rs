//! Evaluate a one-entry update equality against a supplied pair of concrete states.

use std::sync::{Arc, OnceLock};

use dive_ir::{Expr, ExprId};
use dive_value::{DomainLookup, Fcn, Value};

use crate::core::{Core, Lit};
use crate::env::{Env, St, StateRef};
use crate::error::Result;

enum Changed {
    None,
    One(usize),
    More,
}

pub(crate) struct TableChange {
    source: Fcn,
    target: Fcn,
    changed: Changed,
}

#[derive(Default)]
pub(crate) struct ConcreteTables {
    pub(crate) difference: OnceLock<Option<TableChange>>,
    pub(crate) guard: OnceLock<Guard>,
}

pub(crate) struct Guard(pub(crate) ExprId, pub(crate) Fcn, pub(crate) bool);

impl std::fmt::Debug for ConcreteTables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcreteTables")
            .field("compared", &self.difference.get().is_some())
            .field("tested", &self.guard.get().is_some())
            .finish()
    }
}

pub(crate) type ChangeSlot = Arc<ConcreteTables>;

pub(crate) fn concrete_slot(st: &St) -> Option<ChangeSlot> {
    let (StateRef::Vals(source), StateRef::Vals(_)) = (&st.s0, &st.s1) else {
        return None;
    };
    source
        .iter()
        .any(|v| matches!(v, Some(Value::Fcn(table)) if table.len() >= 32))
        .then(|| Arc::new(ConcreteTables::default()))
}

impl Core {
    /// Prove the scalar table domains agree, retain the actual two functions
    /// and record whether at most one matching domain position changed.
    fn concrete_difference(&self, source: Fcn, target: Fcn) -> Result<Option<TableChange>> {
        if source.len() < 32 || source.len() != target.len() {
            return Ok(None);
        }
        let mut changed = Changed::None;
        for index in 0..source.len() {
            self.loop_work()?;
            let (Some((key, old)), Some((other, new))) =
                (source.entry_cow(index), target.entry_cow(index))
            else {
                return Ok(None);
            };
            if !matches!(key.as_ref(), Value::Int(_) | Value::Model(_))
                || !matches!(other.as_ref(), Value::Int(_) | Value::Model(_))
                || key != other
                || source.checked_domain_index(&key) != DomainLookup::Found(index)
                || target.checked_domain_index(&key) != DomainLookup::Found(index)
                || !matches!(old, Value::Bool(_) | Value::Int(_) | Value::Str(_))
                || std::mem::discriminant(old) != std::mem::discriminant(new)
            {
                return Ok(None);
            }
            if old != new {
                changed = if matches!(changed, Changed::None) {
                    Changed::One(index)
                } else {
                    Changed::More
                };
            }
        }
        Ok(Some(TableChange {
            source,
            target,
            changed,
        }))
    }

    pub(crate) fn concrete_update_equal(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        left: ExprId,
        right: ExprId,
    ) -> Result<Option<bool>> {
        if env.dynamic.change_slot.is_none() {
            return Ok(None);
        }
        let Expr::Var { var, primed: true } = self.ir.node(left).expr else {
            return Ok(None);
        };
        let Expr::Except { base, updates } = &self.ir.node(right).expr else {
            return Ok(None);
        };
        if !matches!(self.ir.node(*base).expr, Expr::Var { var: original, primed: false } if original == var)
        {
            return Ok(None);
        }
        let [update] = updates.as_ref() else {
            return Ok(None);
        };
        let [key] = *update.path else { return Ok(None) };
        let Lit::Val(literal @ (Value::Str(_) | Value::Int(_) | Value::Bool(_))) =
            &self.lits[update.value.index()]
        else {
            return Ok(None);
        };
        let Some(key @ (Value::Model(_) | Value::Int(_))) = self.settled(env, key) else {
            return Ok(None);
        };
        let Value::Fcn(target) = self.eval_v(env, st, left)? else {
            return Ok(None);
        };
        let Value::Fcn(source) = self.eval_v(env, st, *base)? else {
            return Ok(None);
        };
        let DomainLookup::Found(index) = source.checked_domain_index(&key) else {
            return Ok(None);
        };
        let Some(slot) = &env.dynamic.change_slot else {
            return Ok(None);
        };
        let slot = &slot.difference;
        if slot.get().is_none() {
            let fresh = self.concrete_difference(source.clone(), target.clone())?;
            let _ = slot.set(fresh);
        }
        let Some(Some(change)) = slot.get() else {
            return Ok(None);
        };
        if !change.source.ptr_eq(&source) || !change.target.ptr_eq(&target) {
            return Ok(None);
        }
        let Some((_, value)) = target.entry_cow(index) else {
            return Ok(None);
        };
        if std::mem::discriminant(value) != std::mem::discriminant(literal) {
            return Ok(None);
        }
        let fits = match change.changed {
            Changed::None => true,
            Changed::One(at) => at == index,
            Changed::More => false,
        };
        Ok(Some(fits && value == literal))
    }
}
