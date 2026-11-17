//! What a bound model uses that dive does not check yet, each refused as
//! `unsupported` by name.

use std::collections::BTreeSet;

use dive_eval::{Scope, SlotView};
use dive_ir::{Arg, DefId, Effects, Expr, ExprId, OpRef, StdOp, StdSupport};

use crate::bind::Binder;
use crate::model::{Model, Pred, Unsupported, UnsupportedKind};
use crate::tables::Look;

mod tuple_calls;

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
    let ir = &*model.ir;
    let (roots, _) = checking_path(b, model);
    let reach = ir.reachable_defs(&roots);
    let used = ir.reachable_defs(&startup_roots(b, model, roots.clone()));
    // A refused start-up evaluation of an impure definition matters only if
    // the checking path uses the definition (TLC may have given it a value)
    // or TLC's evaluation of it may have had a lasting effect (output, a
    // random draw, a register, a write or shared input state). Under TLC's
    // value or exploration order, the strings that evaluation would have
    // created (`IOEnv`'s keys and values, `ToString`) change later order, so
    // every such refusal stands there.
    let ordered_names = b.opts.eval.ctx.order == dive_value::ValueOrder::Tlc
        || b.opts.eval.exploration == dive_eval::ExplorationOrder::Tlc;
    for refusal in &b.startup_refusals {
        let reached = used.get(refusal.def.index()).copied().unwrap_or(true);
        if reached || ordered_names || !effect_free(b, refusal.def) {
            b.ev()
                .restore_external_input_refusal(refusal.input_refusal.clone());
            out.push(Unsupported {
                feature: refusal.what.clone(),
                kind: UnsupportedKind::Feature,
            });
        }
    }
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
    if let Some(s) = &model.symmetry
        && !model.unreduced_symmetry_liveness()
    {
        out.push(Unsupported {
            feature: format!("SYMMETRY {s}"),
            kind: UnsupportedKind::Feature,
        });
    }
    // Java overrides dive does not implement, reachable from what is checked.
    let mut refused: BTreeSet<String> = BTreeSet::new();
    for r in ir.refusals() {
        if reach.get(r.def.index()).copied().unwrap_or(false) {
            let dive_ir::Feature::StdOp(op) = &r.feature;
            if matches!(op, StdOp::IoUtilsIOEnv | StdOp::IoUtilsDeserialize)
                && b.ev().options().host_access.allows(*op)
            {
                continue;
            }
            refused.insert(r.feature.name());
        }
    }
    // Overridden operators used directly as roles.
    for d in &roots {
        if let Some(op) = ir.def(*d).std
            && op.support() != dive_ir::StdSupport::Core
        {
            if matches!(op, StdOp::IoUtilsIOEnv | StdOp::IoUtilsDeserialize)
                && b.ev().options().host_access.allows(op)
            {
                continue;
            }
            refused.insert(dive_ir::Feature::StdOp(op).name());
        }
    }
    out.extend(refused.into_iter().map(|feature| Unsupported {
        feature,
        kind: UnsupportedKind::Feature,
    }));
    out
}

/// Every definition any role may evaluate: the checking path's `roots`, plus
/// temporal properties, `ALIAS`, `POSTCONDITION`s and `SYMMETRY`.
fn startup_roots(b: &Binder<'_>, model: &Model, roots: Vec<DefId>) -> Vec<DefId> {
    let mut out: BTreeSet<DefId> = roots.into_iter().collect();
    let mut exprs = Vec::new();
    let mut preds: Vec<&Pred> = Vec::new();
    for group in [
        &model.temporals,
        &model.implied_temporals,
        &model.configured_properties,
    ] {
        preds.extend(group.iter().map(|n| &n.pred));
    }
    if let Some(Ok(alias)) = &model.alias {
        preds.push(&alias.pred);
    }
    if let Ok(post) = &model.postconditions {
        preds.extend(post.iter().map(|n| &n.pred));
    }
    for p in preds {
        role_defs(b, p.expr, &p.scope, &mut out, &mut exprs, 0);
    }
    if let Some(Look::Def(d) | Look::Java(_, d)) = &model.symmetry_definition {
        out.insert(*d);
    }
    out.into_iter().collect()
}

/// Impure Java overrides whose evaluation only reads: TLC's evaluation of
/// them leaves nothing behind that a later evaluation can observe.
fn reads_only(op: StdOp) -> bool {
    !op.impure()
        || matches!(
            op,
            StdOp::IoUtilsIOEnv | StdOp::TlcJavaTime | StdOp::TlcTLCGet
        )
}

/// Whether evaluating `def` (everything it can reach, through constants the
/// configuration binds) can have no lasting effect: every impure operator it
/// may apply only reads. Conservative: an instance substitution or an
/// operator parameter that may be impure counts as an effect.
fn effect_free(b: &Binder<'_>, def: DefId) -> bool {
    effect_free_with(b, def, false)
}

/// Whether evaluating `def` (everything it can reach, through constants the
/// configuration binds) applies no impure operator at all, not even one that
/// only reads: its value cannot depend on when it is evaluated.
pub(crate) fn pure(b: &Binder<'_>, def: DefId) -> bool {
    effect_free_with(b, def, true)
}

/// [`effect_free`], or with `strict` [`pure`], which also follows the
/// configuration's overrides of the definitions it reaches and the
/// substitutions of instances.
fn effect_free_with(b: &Binder<'_>, def: DefId, strict: bool) -> bool {
    let reads_only = |op: StdOp| if strict { !op.impure() } else { reads_only(op) };
    let ir = &*b.t.ir;
    let mut seen = vec![false; ir.defs().len()];
    let mut work = vec![def];
    // An instance's substitution is read dynamically: its value is a binding
    // of an enclosing `Subst`, whose expressions and operators the walk
    // visits there, or else the declaration itself, a constant (whose
    // configured target's body is evaluated at each use) or a variable.
    // Only `strict` follows that; otherwise it counts as an effect.
    let substitutions = strict;
    let constant = |c: dive_ir::ConstId, work: &mut Vec<DefId>| match b.t.const_look(c) {
        Some(Look::Def(d)) => {
            work.push(d);
            true
        }
        Some(Look::Java(op, _)) => reads_only(op),
        Some(Look::Value(_) | Look::Literal(_)) | None => true,
    };
    while let Some(d) = work.pop() {
        match seen.get_mut(d.index()) {
            Some(s) if !*s => *s = true,
            Some(_) => continue,
            None => return false,
        }
        if strict {
            match b.t.lookup_def(d) {
                Look::Def(r) if r != d => work.push(r),
                Look::Java(op, r) => {
                    if !reads_only(op) {
                        return false;
                    }
                    if r != d {
                        work.push(r);
                    }
                }
                _ => {}
            }
        }
        let def = ir.def(d);
        if let Some(op) = def.std {
            if !reads_only(op) {
                return false;
            }
            continue;
        }
        let mut exprs = vec![def.body];
        while let Some(e) = exprs.pop() {
            let node = ir.node(e);
            match &node.expr {
                Expr::Def(x) => work.push(*x),
                Expr::Let { defs, .. } => work.extend(defs.iter().copied()),
                Expr::Const(c) => {
                    if !constant(*c, &mut work) {
                        return false;
                    }
                }
                Expr::SubstRef { decl, .. } => {
                    if !substitutions {
                        return false;
                    }
                    if let Some(c) = ir.decl(*decl).constant
                        && !constant(c, &mut work)
                    {
                        return false;
                    }
                }
                _ => {}
            }
            for r in node.expr.op_refs() {
                let ok = match r {
                    OpRef::Def(x) => {
                        work.push(*x);
                        true
                    }
                    OpRef::Std(op) => reads_only(*op),
                    OpRef::Const(c) => constant(*c, &mut work),
                    OpRef::Subst(decl) => {
                        substitutions
                            && ir
                                .decl(*decl)
                                .constant
                                .is_none_or(|c| constant(c, &mut work))
                    }
                    OpRef::Param(_) => !node.effects.intersects(Effects::IMPURE),
                    OpRef::Builtin(_) | OpRef::Unbound(_) => true,
                };
                if !ok {
                    return false;
                }
            }
            node.expr.for_each_child(|child| exprs.push(child));
        }
    }
    true
}

/// The definitions a role's expression uses directly, including through the
/// arguments its scope binds lazily.
pub(crate) fn role_defs(
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

/// Why `model` must be checked in sequential mode,
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
pub(crate) fn sequential(b: &Binder<'_>, model: &Model) -> crate::bind::R<Sequential> {
    let ir = &*model.ir;
    let (roots, mut exprs) = checking_path(b, model);
    let fixed = startup_fixed(b, ir);
    let reach = reachable_unfixed(b.ev(), ir, &roots, &fixed);
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
            } else if !fixed.get(i).copied().unwrap_or(false) {
                exprs.push(d.body);
            }
        }
    }
    let tlc_cdot = b.opts.eval.action_composition == dive_eval::ActionComposition::Tlc;
    // A start-up-fixed definition inside a lazily evaluated filter or
    // function is a value, never evaluated again.
    let impure = |e: ExprId| b.ev().impure_after_startup(e);
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
    let mut classified = classify_sequential(out, names, composition);
    // Replay and checkpoint admission retain the conservative source inventory.
    // Only the actual worker restriction can be discharged by this proof.
    tuple_calls::remove_proved_concat(b, model, &mut classified.reasons)?;
    Ok(classified)
}

/// The definitions whose value the binder fixed at start-up
/// ([`dive_eval::Evaluator::startup_fixed`]): checking reads the kept value
/// and never evaluates the body again.
fn startup_fixed(b: &Binder<'_>, ir: &dive_ir::Ir) -> Vec<bool> {
    let ev = b.ev();
    (0..ir.defs().len())
        .map(|i| u32::try_from(i).is_ok_and(|i| ev.startup_fixed(DefId(i))))
        .collect()
}

/// [`dive_ir::Ir::reachable_defs`], except that a start-up-fixed definition
/// is reached without following its body, and that a configured redirect
/// (`Op <- D`, `C <- D`) reaches `D`, whose body each use evaluates.
fn reachable_unfixed(
    ev: &dive_eval::Evaluator,
    ir: &dive_ir::Ir,
    roots: &[DefId],
    fixed: &[bool],
) -> Vec<bool> {
    let constant = |c: &dive_ir::ConstId| match ev.constant_binding(*c) {
        Some(dive_eval::ConstBinding::Def(d)) => Some(d),
        _ => None,
    };
    let mut seen = vec![false; ir.defs().len()];
    let mut work: Vec<DefId> = roots.to_vec();
    while let Some(d) = work.pop() {
        let Some(s) = seen.get_mut(d.index()) else {
            continue;
        };
        if *s {
            continue;
        }
        *s = true;
        work.push(ev.redirected(d));
        let def = ir.def(d);
        if def.std.is_some() || fixed.get(d.index()).copied().unwrap_or(false) {
            continue;
        }
        let mut exprs = vec![def.body];
        while let Some(e) = exprs.pop() {
            let node = ir.node(e);
            if let Expr::Def(x) = &node.expr {
                work.push(*x);
            }
            if let Expr::Let { defs, .. } = &node.expr {
                work.extend(defs.iter().copied());
            }
            if let Expr::Const(c) = &node.expr {
                work.extend(constant(c));
            }
            for r in node.expr.op_refs() {
                match r {
                    OpRef::Def(x) => work.push(*x),
                    OpRef::Const(c) => work.extend(constant(c)),
                    _ => {}
                }
            }
            node.expr.for_each_child(|child| exprs.push(child));
        }
    }
    seen
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
