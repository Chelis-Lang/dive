//! Closed origins for binary scalar-comparison operator values.
//!
//! Operators are second-class: they originate in explicit operator actuals,
//! substitutions, configuration or retained slots, never in an `Ev`/`Value`.
//! Every leaf of that complete source closure must be one of the exact scalar
//! kernels below. Forwarding cannot mint another operator. The caller still
//! walks every value actual and obtains no result-kind fact from a callback.

use super::{
    Analysis, Arg, ConstBinding, DefId, Expr, ExprId, OpRef, OpVal, Result, Sources, StdOp, push,
    reserve,
};
use dive_ir::BuiltinOp;

impl Analysis<'_> {
    pub(super) fn scalar_reference(&self, op: &OpRef) -> bool {
        if let OpRef::Builtin(op) = op {
            return matches!(
                op,
                BuiltinOp::Eq
                    | BuiltinOp::Neq
                    | BuiltinOp::In
                    | BuiltinOp::NotIn
                    | BuiltinOp::Subseteq
            );
        }
        matches!(
            self.native(op, 2),
            Some(
                StdOp::NaturalsLt
                    | StdOp::NaturalsLeq
                    | StdOp::NaturalsGt
                    | StdOp::NaturalsGeq
                    | StdOp::IntegersLt
                    | StdOp::IntegersLeq
                    | StdOp::IntegersGt
                    | StdOp::IntegersGeq
            )
        )
    }

    pub(super) fn scalar_operator(&self, op: &OpVal) -> bool {
        match op {
            OpVal::Builtin(op) => self.scalar_reference(&OpRef::Builtin(*op)),
            OpVal::Std(op) => self.scalar_reference(&OpRef::Std(*op)),
            OpVal::Def {
                def,
                parent: None,
                dynamic: None,
            } => self.scalar_reference(&OpRef::Def(*def)),
            _ => false,
        }
    }

    // Deliberately separate from user_body: tuple-result equations continue
    // treating every formal/capture as unknown, including operator parameters.
    pub(super) fn call_body(&self, def: DefId, args: &[Arg]) -> Option<ExprId> {
        let target = self.ev.ir().defs().get(def.index())?;
        (target.std.is_none()
            && self.ev.redirected(def) == def
            && !self.ev.is_value_override(def)
            && target.params.len() == args.len()
            && target
                .params
                .iter()
                .zip(args)
                .all(|(parameter, arg)| match arg {
                    Arg::Expr { .. } => parameter.arity == 0,
                    Arg::Op { .. } => parameter.arity == 2,
                }))
        .then_some(target.body)
    }

    fn scalar_actual(&self, op: &OpRef) -> Option<bool> {
        if self.scalar_reference(op) {
            Some(true)
        } else if matches!(op, OpRef::Param(_) | OpRef::Subst(_)) {
            Some(false) // Forwarded; a genuine primitive leaf must exist.
        } else {
            None
        }
    }

    fn scalar_actuals<'a>(&self, args: impl IntoIterator<Item = &'a Arg>) -> Option<bool> {
        let mut primitive = false;
        for arg in args {
            if let Arg::Op { op, .. } = arg {
                primitive |= self.scalar_actual(op)?;
            }
        }
        Some(primitive)
    }

    pub(super) fn closed_scalar_callbacks(&self, sources: &Sources) -> Result<Option<bool>> {
        for binding in self.ev.core.consts.iter().flatten() {
            self.poll()?;
            if let ConstBinding::Def(def) = binding
                && !self.ev.ir().def(*def).params.is_empty()
            {
                return Ok(None); // A configured operator is not a source leaf.
            }
        }
        let mut primitive = sources.scalar_callbacks;
        let mut seen = Vec::new();
        reserve(&mut seen, self.ev.ir().nodes().len())?;
        seen.resize(self.ev.ir().nodes().len(), false);
        let mut work = Vec::new();
        reserve(&mut work, sources.exprs.len())?;
        work.extend_from_slice(&sources.exprs);
        while let Some(id) = work.pop() {
            self.poll()?;
            let Some(visited) = seen.get_mut(id.index()) else {
                return Ok(None);
            };
            if *visited {
                continue;
            }
            *visited = true;
            let expr = self.ev.ir().expr(id);
            match expr {
                Expr::Apply { callee, args } => {
                    let Some(found) = self.scalar_actuals(args) else {
                        return Ok(None);
                    };
                    primitive |= found;
                    if let OpRef::Def(def) = callee
                        && self.native(callee, args.len()).is_none()
                    {
                        let Some(body) = self.call_body(*def, args) else {
                            return Ok(None);
                        };
                        push(&mut work, body)?;
                    }
                }
                Expr::Def(def) => {
                    if let Some(body) = self.user_body(*def, 0) {
                        push(&mut work, body)?;
                    }
                }
                Expr::Let { defs, .. } => {
                    for def in defs {
                        push(&mut work, self.ev.ir().def(*def).body)?;
                    }
                }
                Expr::Subst { bindings, .. } => {
                    let Some(found) = self.scalar_actuals(bindings.iter().map(|b| &b.value)) else {
                        return Ok(None);
                    };
                    primitive |= found;
                }
                _ => {}
            }
            expr.try_for_each_child(|child| push(&mut work, child))?;
        }
        Ok(Some(primitive))
    }
}
