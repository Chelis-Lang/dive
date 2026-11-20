//! Equality of the actual Core compiler inputs and first native temporal plan.
use super::{Miss, miss};
use crate::core_engine;
use dive_core::compile::{ConstBinding, Role};
use dive_ir::{DefId, ExprId, Slot};
use dive_liveness::{
    formula::{AtomKind, Formula},
    plan::AtomTest,
};
use dive_model::Model;
use dive_value::{Ctx, Value, ValueOrder};
use std::collections::{BTreeMap, BTreeSet};

/// The operands of an owned union keep the operator's identity. Only the
/// ordinary callback-free frontier codec is admitted below that one root.
#[derive(PartialEq, Eq)]
enum BoundValue {
    Plain(Vec<u8>),
    Union {
        order: ValueOrder,
        normalized: bool,
        cached: bool,
        left: Vec<u8>,
        right: Vec<u8>,
    },
}

struct Values<'a> {
    model: &'a Model,
    remaining: usize,
}

impl Values<'_> {
    fn plain(&mut self, v: &Value) -> Result<Vec<u8>, Miss> {
        let bound = dive_value::codec::frontier_size_bound(v, self.remaining)
            .map_err(|e| miss(format!("bounded constant representation: {e}")))?;
        self.remaining = self
            .remaining
            .checked_sub(bound)
            .ok_or_else(|| miss("constant representation cap"))?;
        let mut bytes = Vec::new();
        dive_value::codec::encode(v, self.model.evaluator.options().ctx, &mut bytes)
            .map_err(|e| miss(e.to_string()))?;
        Ok(bytes)
    }
    fn value(&mut self, v: &Value) -> Result<BoundValue, Miss> {
        if let Value::Set(s) = v
            && let Some((a, b)) = self.model.evaluator.owned_union_operands(s)
        {
            return Ok(BoundValue::Union {
                order: s.order(),
                normalized: s.is_tlc_normalized(),
                cached: s.has_cached_elements(),
                left: self.plain(a)?,
                right: self.plain(b)?,
            });
        }
        self.plain(v).map(BoundValue::Plain)
    }
    fn role(&mut self, r: &Role) -> Result<BoundRole, Miss> {
        if !r.substs.is_empty() {
            return Err(miss("INSTANCE substitutions in bound role"));
        }
        let slots = r
            .slots
            .iter()
            .map(|(i, v)| Ok((*i, self.value(v)?)))
            .collect::<Result<_, Miss>>()?;
        Ok(BoundRole {
            name: r.name.clone(),
            expr: r.expr,
            def: r.def,
            slots,
        })
    }
}

#[derive(PartialEq, Eq)]
struct BoundRole {
    name: String,
    expr: ExprId,
    def: DefId,
    slots: Vec<(Slot, BoundValue)>,
}
#[derive(PartialEq, Eq)]
enum Constant {
    Definition(DefId),
    Value(BoundValue),
}

#[derive(PartialEq, Eq)]
pub(super) struct Binding {
    ctx: Ctx,
    action_composition: bool,
    constants: Vec<Option<Constant>>,
    precomputed: BTreeMap<DefId, BoundValue>,
    overrides: BTreeSet<DefId>,
    init: Vec<BoundRole>,
    next: Vec<BoundRole>,
    check_deadlock: bool,
    formula: Formula,
    atoms: Vec<(AtomKind, AtomTest, BoundRole)>,
    reference_names: Vec<String>,
    native_names: Vec<String>,
}

impl Binding {
    pub(super) fn collect(
        prepared: &core_engine::Prepared,
        first: &dive_liveness::plan::Plan,
    ) -> Result<Self, Miss> {
        let m = &prepared.model;
        let h = core_engine::handoff(m, &m.ir, Vec::new()).map_err(miss)?;
        if h.startup_evaluator.is_some()
            || !h.deferred.is_empty()
            || !h.assumptions.is_empty()
            || !h.redirects.is_empty()
            || !h.std_redirects.is_empty()
            || !h.invariants.is_empty()
            || !h.implied_inits.is_empty()
            || !h.implied_actions.is_empty()
            || !h.constraints.is_empty()
            || !h.action_constraints.is_empty()
            || h.view.is_some()
            || !h.symmetry.is_empty()
        {
            return Err(miss(
                "unqualified transition handoff, deferred constant or reduction",
            ));
        }
        let mut values = Values {
            model: m,
            remaining: 1 << 20,
        };
        let constants = h
            .constants
            .iter()
            .map(|c| {
                c.as_ref()
                    .map(|c| match c {
                        ConstBinding::Def(d) => Ok(Constant::Definition(*d)),
                        ConstBinding::Value(v) => values.value(v).map(Constant::Value),
                    })
                    .transpose()
            })
            .collect::<Result<_, Miss>>()?;
        let precomputed = h
            .precomputed
            .iter()
            .map(|(id, v)| Ok((*id, values.value(v)?)))
            .collect::<Result<_, Miss>>()?;
        let init = h
            .init
            .iter()
            .map(|role| values.role(role))
            .collect::<Result<_, _>>()?;
        let next = h
            .next
            .iter()
            .map(|role| values.role(role))
            .collect::<Result<_, _>>()?;
        let atoms = first
            .atoms
            .iter()
            .map(|a| {
                let r = core_engine::role(m, String::new(), a.pred.expr, &a.pred.scope)
                    .map_err(miss)?;
                Ok((a.kind, a.test, values.role(&r)?))
            })
            .collect::<Result<_, Miss>>()?;
        if m.names.len() != m.evaluator.startup_names()
            || prepared.store.strings().order() != dive_core::strings::StringOrder::Text
        {
            return Err(miss("runtime name growth or nonlexical native name order"));
        }
        let reference_names = names(m.names.len(), |i| {
            m.names.resolve(i).map(|a| a.as_str().to_owned())
        })?;
        let native_names = names(prepared.store.strings().len(), |i| {
            prepared.store.strings().text(i).map(|s| s.to_string())
        })?;
        Ok(Self {
            ctx: h.ctx,
            action_composition: h.action_composition,
            constants,
            precomputed,
            overrides: h.overrides.into_iter().collect(),
            init,
            next,
            check_deadlock: h.check_deadlock,
            formula: first.formula.clone(),
            atoms,
            reference_names,
            native_names,
        })
    }
}

fn names(n: usize, mut get: impl FnMut(u32) -> Option<String>) -> Result<Vec<String>, Miss> {
    if n > 65_536 {
        return Err(miss("startup name count cap"));
    }
    (0..n)
        .map(|i| {
            u32::try_from(i)
                .ok()
                .and_then(&mut get)
                .ok_or_else(|| miss("incomplete startup name domain"))
        })
        .collect()
}
