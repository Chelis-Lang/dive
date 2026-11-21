//! Independent equality tests on the range of a concrete function.

use std::sync::Arc;

use dive_ir::{Bound, BuiltinOp, Expr, ExprId, JunctionKind, Slot};
use dive_value::{DomainLookup, SetKind, Value};

use crate::core::{Core, Lit};
use crate::env::{Env, St};
use crate::error::Result;

impl Core {
    /// Factor independent witnesses over one table. Every selected entry
    /// must be defined and have the literal's primitive type. Otherwise the
    /// ordinary quantifier retains its short-circuit and error order.
    pub(crate) fn table_quantifier(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        exists: bool,
        bounds: &[Bound],
        body: ExprId,
    ) -> Result<Option<bool>> {
        let [bound] = bounds else { return Ok(None) };
        let [fast_slot, slow_slot] = *bound.vars else {
            return Ok(None);
        };
        if bound.tuple || fast_slot == slow_slot {
            return Ok(None);
        }
        let conjunction = if exists {
            body
        } else if let Expr::Not(negated) = self.ir.node(body).expr {
            negated
        } else {
            return Ok(None);
        };
        let Expr::Junction {
            kind: JunctionKind::And,
            items,
            ..
        } = &self.ir.node(conjunction).expr
        else {
            return Ok(None);
        };
        let [left, right] = items.as_ref() else {
            return Ok(None);
        };
        let (Some((left_table, left_slot, first)), Some((right_table, right_slot, second))) = (
            self.table_comparison(*left, BuiltinOp::Eq),
            self.table_comparison(*right, BuiltinOp::Eq),
        ) else {
            return Ok(None);
        };
        if self.ir.node(left_table).expr != self.ir.node(right_table).expr
            || !((left_slot == fast_slot && right_slot == slow_slot)
                || (left_slot == slow_slot && right_slot == fast_slot))
        {
            return Ok(None);
        }
        let Some(domain) = bound.domain else {
            return Ok(None);
        };
        let Expr::Const(id) = self.ir.node(domain).expr else {
            return Ok(None);
        };
        let Some(Some(crate::ConstBinding::Value(Value::Set(set)))) = self.consts.get(id.index())
        else {
            return Ok(None);
        };
        let Some(keys) = set.stored_elements() else {
            return Ok(None);
        };
        // Avoid speculatively forcing set providers or inspecting function keys.
        let already_ordered =
            set.kind() == SetKind::Enum && set.order() == self.opts.explore_order();
        if keys.len() < 32
            || (!already_ordered
                && !keys
                    .iter()
                    .all(|v| matches!(v, Value::Model(_) | Value::Int(_))))
        {
            return Ok(None);
        }
        let keys = self.explore(set)?;
        let Value::Fcn(table) = self.eval_v(env, st, left_table)? else {
            return Ok(None);
        };
        let mut found_first = false;
        let mut found_second = false;
        let mut first_column_can_witness = false;
        for (row, key) in keys.iter().enumerate() {
            self.loop_work()?;
            let DomainLookup::Found(index) = table.checked_domain_index(key) else {
                return Ok(None);
            };
            let Some((_, value)) = table.entry_cow(index) else {
                return Ok(None);
            };
            if std::mem::discriminant(value) != std::mem::discriminant(first)
                || std::mem::discriminant(value) != std::mem::discriminant(second)
            {
                return Ok(None);
            }
            // The first binder advances fastest. A witness in the first
            // column precedes every uninspected row in later columns.
            if row == 0 {
                first_column_can_witness = value
                    == if left_slot == slow_slot {
                        first
                    } else {
                        second
                    };
            }
            found_first |= value == first;
            found_second |= value == second;
            if first_column_can_witness && found_first && found_second {
                return Ok(Some(exists));
            }
        }
        Ok(Some((found_first && found_second) == exists))
    }

    /// Repeated candidate actions can test the same unary predicate of the
    /// given source table. Save a result only for an explicit constant domain
    /// and literal comparison whose inspected primitive entries are defined.
    pub(crate) fn concrete_table_guard(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        exists: bool,
        bounds: &[Bound],
        body: ExprId,
        expr: ExprId,
    ) -> Result<Option<bool>> {
        let Some(slot) = env.dynamic.change_slot.clone() else {
            return Ok(None);
        };
        let [bound] = bounds else { return Ok(None) };
        let [variable] = *bound.vars else {
            return Ok(None);
        };
        if bound.tuple {
            return Ok(None);
        }
        let Expr::Builtin {
            op: op @ (BuiltinOp::Eq | BuiltinOp::Neq),
            ..
        } = self.ir.node(body).expr
        else {
            return Ok(None);
        };
        let Some((read, variable_read, literal)) = self.table_comparison(body, op) else {
            return Ok(None);
        };
        if variable_read != variable {
            return Ok(None);
        }
        let Some(domain) = bound.domain else {
            return Ok(None);
        };
        let Expr::Const(id) = self.ir.node(domain).expr else {
            return Ok(None);
        };
        let Some(Some(crate::ConstBinding::Value(Value::Set(set)))) = self.consts.get(id.index())
        else {
            return Ok(None);
        };
        // Preserve enumeration and its errors in the ordinary evaluator for
        // lazy, unordered, and non-enumerated domains.
        if set.kind() != SetKind::Enum || set.order() != self.opts.explore_order() {
            return Ok(None);
        }
        let Some(keys) = set.stored_elements() else {
            return Ok(None);
        };
        if keys.len() < 32 {
            return Ok(None);
        }
        let keys = self.explore(set)?;
        let Value::Fcn(table) = self.eval_v(env, st, read)? else {
            return Ok(None);
        };
        if let Some(saved) = slot.guard.get() {
            self.loop_work()?;
            return Ok((saved.0 == expr && saved.1.ptr_eq(&table)).then_some(saved.2));
        }
        let mut result = !exists;
        for key in keys.iter() {
            self.loop_work()?;
            let DomainLookup::Found(index) = table.checked_domain_index(key) else {
                return Ok(None);
            };
            let Some((_, value)) = table.entry_cow(index) else {
                return Ok(None);
            };
            if std::mem::discriminant(value) != std::mem::discriminant(literal) {
                return Ok(None);
            }
            if ((value == literal) == (op == BuiltinOp::Eq)) == exists {
                result = exists;
                break;
            }
        }
        let _ = slot
            .guard
            .set(crate::concrete_update::Guard(expr, table, result));
        Ok(Some(result))
    }

    fn table_comparison(
        &self,
        predicate: ExprId,
        expected: BuiltinOp,
    ) -> Option<(ExprId, Slot, &Value)> {
        let Expr::Builtin { op, args } = &self.ir.node(predicate).expr else {
            return None;
        };
        if *op != expected {
            return None;
        }
        let [left, right] = args.as_ref() else {
            return None;
        };
        for (read, literal) in [(*left, *right), (*right, *left)] {
            let Expr::FcnApply { fcn, arg } = self.ir.node(read).expr else {
                continue;
            };
            if !matches!(self.ir.node(fcn).expr, Expr::Var { primed: false, .. }) {
                continue;
            }
            let Expr::Local(binding) = &self.ir.node(arg).expr else {
                continue;
            };
            if binding.up == 0
                && let Lit::Val(value @ (Value::Str(_) | Value::Int(_) | Value::Bool(_))) =
                    &self.lits[literal.index()]
            {
                return Some((fcn, binding.slot, value));
            }
        }
        None
    }
}
