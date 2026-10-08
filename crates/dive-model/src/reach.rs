//! What a bound model uses that dive does not check yet (M5 plan, "Refused
//! as `unsupported` in M5"), each named.

use std::collections::BTreeSet;

use dive_eval::{Scope, SlotView};
use dive_ir::{Arg, DefId, Expr, ExprId, OpRef};

use crate::bind::Binder;
use crate::model::{Model, Pred, Unsupported};
use crate::tables::Look;

/// The features of `model` dive refuses, in a stable order.
pub(crate) fn unsupported(b: &Binder<'_>, model: &Model) -> Vec<Unsupported> {
    let mut out: Vec<String> = b.unsupported.clone();
    if !model.implied_temporals.is_empty() {
        let names: Vec<&str> = model
            .implied_temporals
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        out.push(format!(
            "temporal properties (liveness checking): {}",
            names.join(", ")
        ));
    }
    if let Some(s) = &model.symmetry {
        out.push(format!("SYMMETRY {s}"));
    }
    // Java overrides dive does not implement, reachable from what is checked.
    let ir = &*model.ir;
    let mut roots: BTreeSet<DefId> = BTreeSet::new();
    let mut preds: Vec<&Pred> = Vec::new();
    preds.extend(model.init.iter().map(|i| &i.pred));
    preds.extend(model.next.iter());
    for group in [
        &model.invariants,
        &model.implied_inits,
        &model.implied_actions,
        &model.constraints,
        &model.action_constraints,
    ] {
        preds.extend(group.iter().map(|n| &n.pred));
    }
    if let Some(v) = &model.view {
        preds.push(&v.pred);
    }
    for p in preds {
        role_defs(b, p.expr, &p.scope, &mut roots, 0);
    }
    roots.extend(model.assumptions.iter().copied());
    for l in b.t.consts.iter().flatten() {
        if let Look::Def(d) | Look::Java(_, d) = l {
            roots.insert(*d);
        }
    }
    for map in [&b.t.tool_obj, &b.t.body_obj] {
        for l in map.values() {
            if let Look::Def(d) | Look::Java(_, d) = l {
                roots.insert(*d);
            }
        }
    }
    let roots: Vec<DefId> = roots.into_iter().collect();
    let reach = ir.reachable_defs(&roots);
    let mut refused: BTreeSet<String> = BTreeSet::new();
    for r in ir.refusals() {
        if reach.get(r.def.index()).copied().unwrap_or(false) {
            refused.insert(r.feature.name());
        }
    }
    // Overridden operators used directly as roles.
    for d in &roots {
        if let Some(op) = ir.def(*d).std
            && op.support() != dive_ir::StdSupport::Core
        {
            refused.insert(dive_ir::Feature::StdOp(op).name());
        }
    }
    out.extend(refused);
    out.into_iter()
        .map(|feature| Unsupported { feature })
        .collect()
}

/// The definitions a role's expression uses directly, including through the
/// arguments its scope binds lazily.
fn role_defs(b: &Binder<'_>, e: ExprId, scope: &Scope, out: &mut BTreeSet<DefId>, depth: u32) {
    if depth > 64 {
        return;
    }
    let ir = &*b.t.ir;
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        let expr = ir.expr(x);
        match expr {
            Expr::Def(d) => {
                out.insert(*d);
            }
            Expr::Local(r) => {
                if let SlotView::Lazy(y, s) = b.ev().scope_slot(scope, *r) {
                    role_defs(b, y, &s, out, depth + 1);
                }
            }
            Expr::SubstRef { decl, .. } => {
                if let SlotView::Lazy(y, s) = b.ev().scope_subst_slot(scope, *decl) {
                    role_defs(b, y, &s, out, depth + 1);
                }
            }
            Expr::Let { defs, .. } => out.extend(defs.iter().copied()),
            _ => {}
        }
        for r in expr.op_refs() {
            if let OpRef::Def(d) = r {
                out.insert(*d);
            }
        }
        if let Expr::Apply { args, .. } = expr {
            for a in args {
                if let Arg::Op {
                    op: OpRef::Def(d), ..
                } = a
                {
                    out.insert(*d);
                }
            }
        }
        stack.extend(expr.children());
    }
}
