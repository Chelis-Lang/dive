//! Scoped postcondition deferral for actual temporal-final composition.
use super::{ExplorationEnd, bug};
use crate::{CheckResult, Verdict, engine};
use dive_ir::{BuiltinOp, Expr, ExprId};
use dive_model::Model;
use std::sync::Arc;

/// Opaque actual bound postconditions retained for temporal-before-post ordering.
/// Admission performs no evaluation; it permits only direct scalar/Boolean
/// syntax under a shared 50,000-visit/text policy and depth 128. User calls,
/// constants, captures, constructors, quantifiers, native operations and effects
/// refuse. This is not an evaluator instruction, heap or byte upper bound.
#[derive(Clone)]
pub struct DeferredPostconditions {
    pub(super) model: Model,
    pub(super) admission: Arc<()>,
}
impl DeferredPostconditions {
    /// Retain actual selected postconditions, their exact evaluator core and a
    /// private admission identity. Cloning the token preserves that identity;
    /// separately admitting even a same-core public Model creates a new identity.
    /// This bound-model endpoint cannot undo startup effects; fresh callers must
    /// run their own source admission before binding.
    ///
    /// # Errors
    /// Invalid ownership/roles or syntax outside the bounded direct grammar.
    pub fn admit(model: &Model) -> Result<Self, Verdict> {
        model.validate_context().map_err(|e| Verdict::Unsupported {
            feature: e.to_string(),
        })?;
        let refusal = || Verdict::Unsupported {
            feature: "deferred postcondition requires bounded direct scalar/Boolean syntax".into(),
        };
        let mut work = 50_000usize;
        let list = match &model.postconditions {
            Ok(list) => list,
            // The binder records cached-value roles without evaluating their
            // post body. ModelChecker reaches this lookup only after finalCheck;
            // preserve that specific recorded role error, never evaluate it.
            Err(error) if error.code() == Some(2233) && error.message.len() < work => {
                return Ok(Self {
                    model: model.clone(),
                    admission: Arc::new(()),
                });
            }
            Err(_) => return Err(refusal()),
        };
        for named in list {
            work = work.checked_sub(named.name.len()).ok_or_else(refusal)?;
            if !grammar(model, named.pred.expr, 0, &mut work) {
                return Err(refusal());
            }
        }
        Ok(Self {
            model: model.clone(),
            admission: Arc::new(()),
        })
    }
    /// Evaluate only against genuine completion from the exact retained run.
    /// The caller must establish the actual final-selection ordering separately.
    /// On the normal success path returned errors decide the outcome; the pinned
    /// counterexample-render path ignores returned post errors but not getter
    /// exceptions. This token alone certifies neither context nor that distinction.
    /// The owned candidate pipeline supplies additional private ordering authority.
    ///
    /// # Errors
    /// A completion from another run, or original postcondition deciding failure.
    pub fn evaluate(&self, end: &ExplorationEnd) -> Result<(), Box<CheckResult>> {
        let mut result = end.safety_result().clone();
        if !self.model.evaluator.shares_run_state(&end.evaluator)
            || end
                .deferred_admission
                .as_ref()
                .is_none_or(|id| !Arc::ptr_eq(id, &self.admission))
        {
            result.verdict = bug("deferred postcondition completion belongs to another run");
        } else if let Some(verdict) = engine::postconditions(&self.model) {
            result.verdict = verdict;
            result.collision = None;
            result.output.extend(self.model.take_output());
        } else {
            return Ok(());
        }
        Err(Box::new(result))
    }
}
fn grammar(model: &Model, expr: ExprId, depth: u32, work: &mut usize) -> bool {
    if depth > 128 || *work == 0 {
        return false;
    }
    *work -= 1;
    let Some(node) = model.ir.nodes().get(expr.index()) else {
        return false;
    };
    let children = match &node.expr {
        Expr::Bool(_) | Expr::Var { primed: false, .. } => return true,
        Expr::Num(text) => {
            let Some(left) = work.checked_sub(text.len()) else {
                return false;
            };
            *work = left;
            return true;
        }
        Expr::Builtin {
            op: BuiltinOp::Eq | BuiltinOp::Neq,
            ..
        }
        | Expr::Junction { .. }
        | Expr::Not(_)
        | Expr::Implies(_, _)
        | Expr::Equiv(_, _)
        | Expr::If { .. } => node.expr.children(),
        _ => return false,
    };
    children
        .into_iter()
        .all(|child| grammar(model, child, depth + 1, work))
}
