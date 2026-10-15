//! Structural preflight for finite binder domains. No deferred predicate is
//! evaluated here, and every component is bounded before enumeration allocates.

use dive_value::{Ctx, Set, SetKind, Value};

use crate::{EvalError, Result};

pub(crate) struct Budget {
    remaining: usize,
    limit: usize,
    depth: u32,
    ctx: Ctx,
}

impl Budget {
    pub(crate) const fn new(limit: usize, depth: u32, ctx: Ctx) -> Self {
        Self {
            remaining: limit,
            limit,
            depth,
            ctx,
        }
    }

    fn charge(&mut self, count: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or_else(|| EvalError::resource_limit("quantifier domain materialization work"))?;
        Ok(())
    }

    fn component(&mut self, value: &Value, depth: u32) -> Result<usize> {
        let Value::Set(set) = value else {
            return Err(EvalError::unsupported(
                "finite binder domain component is not a set",
            ));
        };
        self.set(set, depth)
    }

    fn value(&mut self, value: &Value, depth: u32) -> Result<()> {
        if depth >= self.depth {
            return Err(EvalError::resource_limit("quantifier domain nesting"));
        }
        match value {
            // Atomic infinite-set values require no enumeration; they are not
            // themselves accepted as binder domains.
            Value::Set(set)
                if matches!(set.kind(), SetKind::Nat | SetKind::Int | SetKind::Strings) =>
            {
                Ok(())
            }
            Value::Set(set) => self.set(set, depth).map(|_| ()),
            Value::Fcn(function) => {
                self.charge(function.len().checked_mul(2).ok_or_else(|| {
                    EvalError::resource_limit("quantifier domain materialization work")
                })?)?;
                for (key, value) in function.iter() {
                    self.value(&key, depth + 1)?;
                    self.value(value, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn explicit(&mut self, set: &Set, depth: u32) -> Result<usize> {
        // Use a cardinality upper bound without normalizing raw operands:
        // TLC's normalizing size/enumeration runs only after preflight,
        // in its original domain/range error order.
        if let Some(raw) = set.raw_elements() {
            self.charge(raw.len())?;
            for value in raw {
                self.value(value, depth)?;
            }
            Ok(raw.len())
        } else {
            let size = usize::try_from(set.size(self.ctx)?)
                .map_err(|_| EvalError::resource_limit("quantifier domain entries"))?;
            self.charge(size)?;
            for value in set.elements(self.ctx)?.iter() {
                self.value(value, depth)?;
            }
            Ok(size)
        }
    }

    fn filtered(&mut self, set: &Set, depth: u32) -> Result<usize> {
        let filter = set
            .as_deferred()
            .and_then(|source| source.as_any())
            .and_then(|source| source.downcast_ref::<crate::lazy::Filter>())
            .ok_or_else(|| EvalError::unsupported("opaque deferred binder domain"))?;
        let forbidden = dive_ir::Effects::IMPURE
            | dive_ir::Effects::LIVE_TLCGET
            | dive_ir::Effects::TEMPORAL
            | dive_ir::Effects::USES_ENABLED
            | dive_ir::Effects::OPEN;
        // Preserve immediate definite-effect refusals before any charged OPEN
        // inspection, regardless of redirect iteration order or remaining work.
        // Only the separate walk below resolves scalar constant reads.
        if filter
            .core
            .ir
            .node(filter.pred)
            .effects
            .intersects(forbidden)
            || crate::env::filter_has_unproved_captures(&filter.core.ir, &filter.env, forbidden)
            || filter.core.redirects.values().any(|target| {
                filter
                    .core
                    .ir
                    .def(*target)
                    .effects
                    .intersects(forbidden.without(dive_ir::Effects::OPEN))
            })
        {
            return Err(EvalError::unsupported(
                "effectful or unresolved deferred binder predicate",
            ));
        }
        for &target in filter.core.redirects.values() {
            let effects = filter.core.ir.def(target).effects;
            if effects.intersects(dive_ir::Effects::OPEN)
                && !self.scalar_redirect(&filter.core, target, &mut Vec::new(), 0)?
            {
                return Err(EvalError::unsupported(
                    "effectful or unresolved deferred binder predicate",
                ));
            }
        }
        // Structural input and the conservative output-vector upper bound are
        // charged before size/explore invokes the captured predicate. Do not
        // evaluate, normalize or cache the filter during this preflight.
        let count = self.component(&filter.in_val, depth)?;
        self.charge(count)?;
        Ok(count)
    }

    // Non-evaluating inspection of one OPEN redirect, not a predicate purity
    // checker. No local definitions, substitutions, operator values or deferred
    // constants qualify. Every inspected source item uses the existing budget.
    fn scalar_redirect(
        &mut self,
        core: &crate::core::Core,
        target: dive_ir::DefId,
        active: &mut Vec<dive_ir::DefId>,
        depth: u32,
    ) -> Result<bool> {
        self.charge(1)?;
        let mut target = target;
        for _ in 0..64 {
            self.charge(1)?;
            match core.redirects.get(&target) {
                Some(&next) if next != target => target = next,
                _ => break,
            }
        }
        let definition = core.ir.def(target);
        if depth >= self.depth {
            return Err(EvalError::resource_limit(
                "deferred binder redirect nesting",
            ));
        }
        self.charge(definition.params.len())?;
        if definition.scope != dive_ir::DefScope::TopLevel
            || definition.recursive
            || definition.std.is_some()
            || definition.params.iter().any(|param| param.arity != 0)
            || core.def_values.contains_key(&target)
            || active.contains(&target)
        {
            return Ok(false);
        }
        active.push(target);
        let result = self.scalar_source(core, definition.body, active, depth + 1);
        active.pop();
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit restricted source constructors; no evaluation or context projection"
    )]
    fn scalar_source(
        &mut self,
        core: &crate::core::Core,
        expr: dive_ir::ExprId,
        active: &mut Vec<dive_ir::DefId>,
        depth: u32,
    ) -> Result<bool> {
        use dive_ir::{Arg, Effects, Expr, OpRef};
        self.charge(1)?;
        if depth >= self.depth {
            return Err(EvalError::resource_limit(
                "deferred binder redirect nesting",
            ));
        }
        let node = core.ir.node(expr);
        if node.effects.intersects(
            Effects::IMPURE | Effects::LIVE_TLCGET | Effects::TEMPORAL | Effects::USES_ENABLED,
        ) {
            return Ok(false);
        }
        match &node.expr {
            Expr::Const(constant) => {
                return Ok(matches!(
                    core.consts.get(constant.index()),
                    Some(Some(crate::ConstBinding::Value(
                        Value::Bool(_)
                            | Value::Int(_)
                            | Value::BigInt(_)
                            | Value::Str(_)
                            | Value::Model(_)
                    )))
                ));
            }
            Expr::Local(local) => return Ok(local.up == 0),
            Expr::Def(target) => return self.scalar_redirect(core, *target, active, depth + 1),
            Expr::Apply { callee, args } => {
                self.charge(args.len())?;
                if args.iter().any(|arg| matches!(arg, Arg::Op { .. })) {
                    return Ok(false);
                }
                let target = match callee {
                    OpRef::Def(target) => Some(*target),
                    OpRef::Std(op) => core.std_redirects.get(op).copied(),
                    OpRef::Builtin(_) => None,
                    _ => return Ok(false),
                };
                if let Some(target) = target
                    && !self.scalar_redirect(core, target, active, depth + 1)?
                {
                    return Ok(false);
                }
                for arg in args {
                    if let Arg::Expr { expr, .. } = arg
                        && !self.scalar_source(core, *expr, active, depth + 1)?
                    {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            Expr::Bool(_)
            | Expr::Num(_)
            | Expr::Decimal { .. }
            | Expr::Str(_)
            | Expr::Var { .. } => return Ok(true),
            Expr::Builtin { args, .. }
            | Expr::Junction { items: args, .. }
            | Expr::SetEnum(args)
            | Expr::Tuple(args)
            | Expr::Cartesian(args) => {
                for &child in args {
                    if !self.scalar_source(core, child, active, depth + 1)? {
                        return Ok(false);
                    }
                }
            }
            Expr::Not(child)
            | Expr::Selected(child)
            | Expr::Prime(child)
            | Expr::Unchanged(child)
            | Expr::Label { body: child, .. }
            | Expr::RecordSelect { record: child, .. } => {
                return self.scalar_source(core, *child, active, depth + 1);
            }
            Expr::FcnSet {
                domain: a,
                range: b,
            }
            | Expr::FcnApply { fcn: a, arg: b }
            | Expr::Implies(a, b)
            | Expr::Equiv(a, b) => {
                return Ok(self.scalar_source(core, *a, active, depth + 1)?
                    && self.scalar_source(core, *b, active, depth + 1)?);
            }
            Expr::FcnCons { bounds, body }
            | Expr::SetMap { bounds, expr: body }
            | Expr::Quant { bounds, body, .. } => {
                for bound in bounds {
                    self.charge(1)?;
                    let Some(domain) = bound.domain else {
                        return Ok(false);
                    };
                    if !self.scalar_source(core, domain, active, depth + 1)? {
                        return Ok(false);
                    }
                }
                return self.scalar_source(core, *body, active, depth + 1);
            }
            // More involved lexical/dynamic constructs retain the old refusal.
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(crate) fn set(&mut self, set: &Set, depth: u32) -> Result<usize> {
        if depth >= self.depth {
            return Err(EvalError::resource_limit("quantifier domain nesting"));
        }
        let child_depth = depth + 1;
        let width;
        let count = match set.kind() {
            SetKind::Enum | SetKind::Raw => self.explicit(set, child_depth)?,
            SetKind::Interval => {
                let (low, high) = set
                    .as_interval()
                    .ok_or_else(|| EvalError::bug("interval kind without bounds"))?;
                let size = if high < low {
                    0
                } else {
                    usize::try_from(high.to_big() - low.to_big() + 1_u32)
                        .map_err(|_| EvalError::resource_limit("quantifier domain entries"))?
                };
                self.charge(size)?;
                size
            }
            SetKind::Subset => {
                let components = set.components();
                let base = &components[0].1;
                width = self.component(base, child_depth)?;
                let count = u32::try_from(width)
                    .ok()
                    .and_then(|n| 1_usize.checked_shl(n))
                    .filter(|n| *n <= self.limit)
                    .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
                self.charge(count.checked_mul(width.max(1)).ok_or_else(|| {
                    EvalError::resource_limit("quantifier domain materialization work")
                })?)?;
                count
            }
            SetKind::FcnSet => {
                let (domain, range) = set
                    .as_fcn_set()
                    .ok_or_else(|| EvalError::bug("function-set kind without components"))?;
                width = self.component(domain, child_depth)?;
                if width == 0 {
                    // TLC never touches the range of the singleton empty function.
                    self.charge(1)?;
                    return Ok(1);
                }
                let range_count = self.component(range, child_depth)?;
                let count = u32::try_from(width)
                    .ok()
                    .and_then(|n| range_count.checked_pow(n))
                    .filter(|n| *n <= self.limit)
                    .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
                self.charge(
                    count
                        .checked_mul(width)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or_else(|| {
                            EvalError::resource_limit("quantifier domain materialization work")
                        })?,
                )?;
                count
            }
            SetKind::Product | SetKind::RcdSet => {
                width = set.as_product().map_or_else(
                    || {
                        set.as_rcd_set()
                            .map_or(0, <[(dive_value::Atom, Value)]>::len)
                    },
                    <[Value]>::len,
                );
                self.charge(width)?;
                let mut count = 1_usize;
                for (_, component) in set.components() {
                    count = count
                        .checked_mul(self.component(&component, child_depth)?)
                        .filter(|n| *n <= self.limit)
                        .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
                }
                self.charge(count.checked_mul(width.max(1)).ok_or_else(|| {
                    EvalError::resource_limit("quantifier domain materialization work")
                })?)?;
                count
            }
            SetKind::Deferred => self.filtered(set, child_depth)?,
            _ => {
                return Err(EvalError::unsupported(
                    "bounded bindings require finite domains; infinite sets are not supported",
                ));
            }
        };
        if count > self.limit {
            return Err(EvalError::resource_limit("quantifier domain entries"));
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "test assertions")]
    use super::Budget;
    use dive_value::{Ctx, Value};
    #[test]
    fn structural_preflight_does_not_normalize_raw_operands() {
        let context = Ctx::tlc();
        let value = Value::set_raw(vec![2.into(), 1.into(), 1.into()], context);
        let set = value.as_set().expect("constructed set");
        assert!(set.raw_elements().is_some());
        assert_eq!(Budget::new(3, 256, context).set(set, 0).ok(), Some(3));
        assert!(set.raw_elements().is_some());
    }

    #[test]
    fn redirect_metadata_uses_the_original_budget_and_depth() {
        let analysis = dive_sema::analyze_source(
            "Budget.tla",
            "---- MODULE Budget ----\nCONSTANT C\nP(unused) == C\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted());
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let target = ir.root_def("P").expect("P");
        let mut model = crate::Model::new(ir);
        model.constants[0] = Some(crate::ConstBinding::Value(Value::Int(1)));
        let evaluator = crate::Evaluator::new(model, crate::Options::tlc()).expect("evaluator");
        // One definition, one redirect lookup, one formal, one expression. The
        // scalar value is inspected by tag, never evaluated or compared.
        let mut exact = Budget::new(4, 256, Ctx::tlc());
        assert!(
            exact
                .scalar_redirect(&evaluator.core, target, &mut Vec::new(), 0)
                .expect("four metadata units")
        );
        assert_eq!(exact.remaining, 0);
        for (limit, depth) in [(3, 256), (4, 1)] {
            let error = Budget::new(limit, depth, Ctx::tlc())
                .scalar_redirect(&evaluator.core, target, &mut Vec::new(), 0)
                .expect_err("existing work or depth bound");
            assert_eq!(error.class(), crate::ErrorClass::ResourceLimit);
        }
    }
}
