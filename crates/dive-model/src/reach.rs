//! What a bound model uses that dive does not check yet (M5 plan, "Refused
//! as `unsupported` in M5"), each named.

use std::collections::BTreeSet;

use dive_eval::{Scope, SlotView};
use dive_ir::{Arg, DefId, Effects, Expr, ExprId, OpRef, StdOp, StdSupport};

use crate::bind::Binder;
use crate::model::{Model, Pred, Unsupported, UnsupportedKind};
use crate::tables::Look;

/// The features of `model` dive refuses, in a stable order.
pub(crate) fn unsupported(b: &Binder<'_>, model: &Model) -> Vec<Unsupported> {
    let mut out: Vec<Unsupported> = b
        .unsupported
        .iter()
        .map(|feature| Unsupported {
            feature: feature.clone(),
            kind: UnsupportedKind::Feature,
        })
        .collect();
    if !model.implied_temporals.is_empty() {
        let names: Vec<&str> = model
            .implied_temporals
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        out.push(Unsupported {
            kind: UnsupportedKind::TemporalProperty,
            feature: format!(
                "temporal properties (liveness checking): {}",
                names.join(", ")
            ),
        });
    }
    if let Some(s) = &model.symmetry {
        out.push(Unsupported {
            feature: format!("SYMMETRY {s}"),
            kind: UnsupportedKind::Feature,
        });
    }
    // Java overrides dive does not implement, reachable from what is checked.
    let ir = &*model.ir;
    let (roots, _) = checking_path(b, model);
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
    out.extend(refused.into_iter().map(|feature| Unsupported {
        feature,
        kind: UnsupportedKind::Feature,
    }));
    out
}

/// The definitions a role's expression uses directly, including through the
/// arguments its scope binds lazily.
fn role_defs(
    b: &Binder<'_>,
    e: ExprId,
    scope: &Scope,
    out: &mut BTreeSet<DefId>,
    exprs: &mut Vec<ExprId>,
    depth: u32,
) {
    if depth > 64 {
        return;
    }
    exprs.push(e);
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
                    role_defs(b, y, &s, out, exprs, depth + 1);
                }
            }
            Expr::SubstRef { decl, .. } => {
                if let SlotView::Lazy(y, s) = b.ev().scope_subst_slot(scope, *decl) {
                    role_defs(b, y, &s, out, exprs, depth + 1);
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

/// The definitions on the checking path (what is evaluated while checking:
/// the initial predicates, the next-state relation, invariants, implied
/// initial predicates and actions, constraints, action constraints and
/// `VIEW`, with the assumptions and the configuration's bindings), and the
/// role expressions themselves, including the lazy arguments their scopes
/// bind.
fn checking_path(b: &Binder<'_>, model: &Model) -> (Vec<DefId>, Vec<ExprId>) {
    let mut roots: BTreeSet<DefId> = BTreeSet::new();
    let mut exprs: Vec<ExprId> = Vec::new();
    for p in role_preds(model) {
        role_defs(b, p.expr, &p.scope, &mut roots, &mut exprs, 0);
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
    (roots.into_iter().collect(), exprs)
}

/// Every role's predicate: initial predicates, the next-state relation,
/// invariants, implied initial predicates and actions, constraints, action
/// constraints and `VIEW`.
fn role_preds(model: &Model) -> Vec<&Pred> {
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
    preds
}

pub(crate) struct Sequential {
    pub reasons: Vec<String>,
    pub names_only: bool,
    pub runtime_names: bool,
    pub stateful_checking: Option<String>,
}

fn native_op(ir: &dive_ir::Ir, op: &OpRef) -> Option<StdOp> {
    match op {
        OpRef::Std(op) => Some(*op),
        OpRef::Def(id) => ir.def(*id).std,
        _ => None,
    }
}

/// What a use of the Java override `op` (applied to a first argument `key`,
/// when known) on the checking path forces sequential mode for, if anything.
fn sequential_op(op: StdOp, key: Option<&Expr>) -> Option<String> {
    let name = op.qualified_name();
    if op == StdOp::TlcTLCSet {
        return Some(format!("{name} (writes TLC's registers)"));
    }
    if op == StdOp::TlcTLCGet {
        return match key {
            Some(Expr::Str(k)) if !dive_ir::effects::tlcget_string_is_live(k) => None,
            Some(Expr::Str(k)) => Some(format!(
                "{name}(\"{k}\") (reads a live counter or a register)"
            )),
            Some(Expr::Num(n)) => Some(format!("{name}({n}) (reads a register)")),
            _ => Some(format!("{name} of a key computed while checking")),
        };
    }
    if matches!(
        op,
        StdOp::TlcRandomElement
            | StdOp::RandomizationRandomSubset
            | StdOp::RandomizationRandomSetOfSubsets
            | StdOp::RandomizationRandomSubsetSet
    ) {
        return Some(format!(
            "{name} (draws from the expanded state's random stream)"
        ));
    }
    matches!(op.support(), StdSupport::Io | StdSupport::Introspection)
        .then(|| format!("{name} ({})", op.support().describe()))
}

/// TLC's action composition counts its intermediate states as generated; the
/// parallel engine's per-source counts double as successor ordinals.
/// Sequential BFS reason for TLC composition intermediate-state accounting.
/// DFID and simulation ignore those intermediate states, as their TLC functors do.
pub const TLC_CDOT: &str = "action composition under `--action-composition=tlc` (TLC counts its intermediate states as generated)";

/// Why `model` must be checked in sequential mode (ADR 0015 §6, ADR 0014 §7),
/// in a stable order; empty when the parallel engine may run it.
///
/// A parallel search evaluates the checking path in an order other than
/// TLC's single worker's. That is invisible except where evaluation reads
/// or changes state shared across evaluations:
/// - `TLCSet` (registers, `TLCSet("exit")`);
/// - `TLCGet` of a live counter or a register, or of a key computed at run
///   time;
/// - an operator that performs I/O or reads the checker's internal state;
/// - runtime name constructors under TLC value or exploration order;
/// - randomness, drawn in TLC from one stream per expanded state that the
///   next-state relation and the checks of its successors share;
/// - side effects (`Print`, ...) inside a lazily evaluated set filter or
///   function, or inside a cached lazy value bound when the model was bound:
///   these are evaluated once, by whichever evaluation forces them first.
///
/// `Print` elsewhere does not force sequential mode: the checker buffers its
/// output and releases it in TLC's order.
pub(crate) fn sequential(b: &Binder<'_>, model: &Model) -> Sequential {
    let ir = &*model.ir;
    let (roots, mut exprs) = checking_path(b, model);
    let reach = ir.reachable_defs(&roots);
    let ordered_names = b.opts.eval.ctx.order == dive_value::ValueOrder::Tlc
        || b.opts.eval.exploration == dive_eval::ExplorationOrder::Tlc;
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut composition = false;
    for (i, d) in ir.defs().iter().enumerate() {
        if reach.get(i).copied().unwrap_or(false) {
            if let Some(op) = d.std {
                // Configuration-selected native operators may have no call
                // node in the source IR. The bound checking roots retain the
                // actual target, and its RNG shares one expansion stream.
                if native_random(op) {
                    out.extend(sequential_op(op, None));
                }
                if ordered_names && op.creates_runtime_names() {
                    names.insert(format!(
                        "{} (runtime names require TLC encounter order)",
                        op.qualified_name()
                    ));
                }
            } else {
                exprs.push(d.body);
            }
        }
    }
    let tlc_cdot = b.opts.eval.action_composition == dive_eval::ActionComposition::Tlc;
    let impure = |e: ExprId| ir.node(e).effects.intersects(Effects::IMPURE);
    let mut seen = vec![false; ir.nodes().len()];
    while let Some(e) = exprs.pop() {
        match seen.get_mut(e.index()) {
            Some(s) if !*s => *s = true,
            _ => continue,
        }
        let expr = ir.expr(e);
        match expr {
            Expr::Apply { callee, args } => {
                if let Some(op) = native_op(ir, callee) {
                    let key = args.first().and_then(|a| match a {
                        Arg::Expr { expr, .. } => Some(ir.expr(*expr)),
                        Arg::Op { .. } => None,
                    });
                    if ordered_names && op.creates_runtime_names() {
                        names.insert(format!(
                            "{} (runtime names require TLC encounter order)",
                            op.qualified_name()
                        ));
                    }
                    out.extend(sequential_op(op, key));
                }
                for a in args {
                    if let Arg::Op { op, .. } = a
                        && let Some(op) = native_op(ir, op)
                    {
                        if ordered_names && op.creates_runtime_names() {
                            names.insert(format!(
                                "{} (runtime names require TLC encounter order)",
                                op.qualified_name()
                            ));
                        }
                        out.extend(sequential_op(op, None));
                    }
                }
            }
            Expr::Cdot(..) if tlc_cdot => {
                composition = true;
            }
            Expr::SetFilter { pred, .. } if impure(*pred) => {
                out.insert("side effects inside a set filter TLC evaluates lazily".to_owned());
            }
            Expr::FcnCons { body, .. } | Expr::RecFcn { body, .. } if impure(*body) => {
                out.insert("side effects inside a function TLC evaluates lazily".to_owned());
            }
            _ => {}
        }
        exprs.extend(expr.children());
    }
    let ev = b.ev();
    let shared = role_preds(model)
        .iter()
        .any(|p| ev.scope_shares_side_effects(&p.scope));
    let shared = shared
        || model
            .init_actions
            .iter()
            .chain(&model.actions)
            .any(|a| ev.action_shares_side_effects(a));
    if shared {
        out.insert(
            "side effects inside a lazily evaluated value bound with the model, which TLC caches (a LET or an operator argument)".to_owned(),
        );
    }
    classify_sequential(out, names, composition)
}

fn classify_sequential(
    mut out: BTreeSet<String>,
    names: BTreeSet<String>,
    composition: bool,
) -> Sequential {
    let stateful_checking = out.first().cloned();
    if composition {
        out.insert(TLC_CDOT.to_owned());
    }
    let runtime_names = !names.is_empty();
    let names_only = out.is_empty() && runtime_names;
    out.extend(names);
    Sequential {
        reasons: out.into_iter().collect(),
        names_only,
        runtime_names,
        stateful_checking,
    }
}

fn native_random(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::TlcRandomElement
            | StdOp::RandomizationRandomSubset
            | StdOp::RandomizationRandomSetOfSubsets
            | StdOp::RandomizationRandomSubsetSet
    )
}
