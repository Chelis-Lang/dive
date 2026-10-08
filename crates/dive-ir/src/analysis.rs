//! Passes over a lowered IR: the frame-aware walk, the effects analysis and
//! structural hashes.

use crate::effects::{Effects, tlcget_string_is_live};
use crate::expr::{Arg, BuiltinOp, Expr, OpRef};
use crate::ids::{DefId, ExprId};
use crate::std_op::StdOp;
use crate::{DefScope, Ir, SlotKind};

/// Visits every expression node reachable from the definitions' bodies with
/// the stack of definitions whose frames are live (innermost last).
///
/// Top-level definitions start a walk; `LET` definitions are walked where
/// the `LET` is, in the current frame for zero-arity ones and in a new frame
/// for the others; inline definitions (`LAMBDA`s) are walked where they occur.
pub(crate) fn walk_frames(ir: &Ir, mut visit: impl FnMut(ExprId, &[DefId])) {
    let mut stack = Vec::new();
    for (i, d) in ir.defs.iter().enumerate() {
        if d.scope == DefScope::TopLevel {
            stack.clear();
            stack.push(DefId::from_index(i));
            walk(ir, d.body, &mut stack, &mut visit);
        }
    }
}

fn walk(ir: &Ir, e: ExprId, stack: &mut Vec<DefId>, visit: &mut impl FnMut(ExprId, &[DefId])) {
    // An explicit work list keeps deep expressions off the call stack. Each
    // item carries the frame-stack length it runs at.
    enum Item {
        Expr(ExprId, usize),
        Push(DefId),
        Pop,
    }
    let base = stack.len();
    let mut work = vec![Item::Expr(e, base)];
    while let Some(item) = work.pop() {
        match item {
            Item::Push(d) => stack.push(d),
            Item::Pop => {
                stack.pop();
            }
            Item::Expr(e, depth) => {
                stack.truncate(depth);
                let Some(node) = ir.exprs.get(e.index()) else {
                    continue;
                };
                visit(e, stack);
                // Work items run last-in first-out: push in reverse order.
                let mut items = Vec::new();
                if let Expr::Let { defs, .. } = &node.expr {
                    for &d in defs {
                        let Some(def) = ir.defs.get(d.index()) else {
                            continue;
                        };
                        match def.scope {
                            DefScope::LetValue { .. } => items.push(Item::Expr(def.body, depth)),
                            DefScope::Local { .. } => {
                                items.push(Item::Push(d));
                                items.push(Item::Expr(def.body, depth + 1));
                                items.push(Item::Pop);
                            }
                            DefScope::TopLevel | DefScope::Inline { .. } => {}
                        }
                    }
                }
                for r in node.expr.op_refs() {
                    if let OpRef::Def(d) = r
                        && let Some(def) = ir.defs.get(d.index())
                        && matches!(def.scope, DefScope::Inline { .. })
                    {
                        items.push(Item::Push(*d));
                        items.push(Item::Expr(def.body, depth + 1));
                        items.push(Item::Pop);
                    }
                }
                if let Expr::Def(d) = &node.expr
                    && let Some(def) = ir.defs.get(d.index())
                    && matches!(def.scope, DefScope::Inline { .. })
                {
                    items.push(Item::Push(*d));
                    items.push(Item::Expr(def.body, depth + 1));
                    items.push(Item::Pop);
                }
                for c in node.expr.children() {
                    items.push(Item::Expr(c, depth));
                }
                work.extend(items.into_iter().rev());
            }
        }
    }
    stack.truncate(base);
}

/// What a [`Expr::Local`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalKind {
    /// A value: a bound variable or a recursive function's name.
    Value,
    /// A parameter thunk.
    Param,
    /// A `LET` thunk.
    Let(DefId),
    /// The reference does not resolve.
    Invalid,
}

/// Resolves every `Local` node to the kind of slot it reads.
pub(crate) fn local_kinds(ir: &Ir) -> Vec<Option<LocalKind>> {
    let mut out = vec![None; ir.exprs.len()];
    walk_frames(ir, |e, stack| {
        if let Some(Expr::Local(r)) = ir.exprs.get(e.index()).map(|n| &n.expr) {
            let kind = stack
                .len()
                .checked_sub(1 + r.up as usize)
                .and_then(|i| stack.get(i))
                .and_then(|d| ir.defs.get(d.index()))
                .and_then(|d| d.frame.get(r.slot.index()))
                .map_or(LocalKind::Invalid, |s| match s.kind {
                    SlotKind::Bound | SlotKind::FcnName => LocalKind::Value,
                    SlotKind::Param => LocalKind::Param,
                    SlotKind::LetValue(d) => LocalKind::Let(d),
                });
            if let Some(o) = out.get_mut(e.index()) {
                *o = Some(kind);
            }
        }
    });
    out
}

fn builtin_effects(b: BuiltinOp) -> Effects {
    match b {
        BuiltinOp::Enabled => Effects::USES_ENABLED,
        BuiltinOp::Always | BuiltinOp::Eventually | BuiltinOp::LeadsTo | BuiltinOp::WhilePlus => {
            Effects::TEMPORAL
        }
        _ => Effects::NONE,
    }
}

fn std_effects(op: StdOp, key: Option<&Expr>) -> Effects {
    let mut e = Effects::MAY_FAIL;
    if op == StdOp::TlcTLCGet {
        let live = match key {
            Some(Expr::Str(s)) => tlcget_string_is_live(s),
            _ => true,
        };
        if live {
            e |= Effects::IMPURE | Effects::LIVE_TLCGET;
        }
    } else if op.impure() {
        e |= Effects::IMPURE;
    }
    e
}

/// Computes node and definition effects to a fixed point (ADR 0013 §6).
pub(crate) fn compute_effects(ir: &mut Ir) {
    let locals = local_kinds(ir);
    let mut def_eff = vec![Effects::NONE; ir.defs.len()];
    for (i, d) in ir.defs.iter().enumerate() {
        if let (Some(op), Some(e)) = (d.std, def_eff.get_mut(i)) {
            *e = std_effects(op, None);
        }
    }
    let mut node_eff = vec![Effects::NONE; ir.exprs.len()];
    loop {
        // Operators passed as arguments anywhere: what an operator parameter
        // may be.
        let mut escape = Effects::NONE;
        for n in &ir.exprs {
            let ops: Vec<&OpRef> = match &n.expr {
                Expr::Apply { args, .. } => args
                    .iter()
                    .filter_map(|a| if let Arg::Op(o) = a { Some(o) } else { None })
                    .collect(),
                Expr::Subst { bindings, .. } => bindings
                    .iter()
                    .filter_map(|b| {
                        if let Arg::Op(o) = &b.value {
                            Some(o)
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            for o in ops {
                escape |= opref_effects(ir, o, &def_eff, Effects::NONE);
            }
        }
        for i in 0..ir.exprs.len() {
            let e = node_effects(ir, i, &node_eff, &def_eff, &locals, escape);
            if let Some(slot) = node_eff.get_mut(i) {
                *slot = e;
            }
        }
        let mut changed = false;
        for (i, d) in ir.defs.iter().enumerate() {
            if d.std.is_some() {
                continue;
            }
            let e = node_eff
                .get(d.body.index())
                .copied()
                .unwrap_or(Effects::MAY_FAIL);
            if let Some(slot) = def_eff.get_mut(i) {
                let new = slot.union(e);
                if new != *slot {
                    *slot = new;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (n, e) in ir.exprs.iter_mut().zip(node_eff) {
        n.effects = e;
    }
    for (d, e) in ir.defs.iter_mut().zip(def_eff) {
        d.effects = e;
    }
}

fn opref_effects(ir: &Ir, r: &OpRef, def_eff: &[Effects], escape: Effects) -> Effects {
    match r {
        OpRef::Def(d) => {
            def_eff.get(d.index()).copied().unwrap_or(Effects::NONE) | Effects::MAY_FAIL
        }
        OpRef::Param(_) => escape | Effects::MAY_FAIL,
        OpRef::Std(op) => std_effects(*op, None),
        OpRef::Builtin(b) => builtin_effects(*b) | Effects::MAY_FAIL,
        OpRef::Const(_) => Effects::OPEN | Effects::MAY_FAIL,
        OpRef::Subst(decl) => {
            let open = ir
                .decls
                .get(decl.index())
                .is_some_and(|d| d.constant.is_some());
            escape | Effects::MAY_FAIL | if open { Effects::OPEN } else { Effects::NONE }
        }
        OpRef::Unbound(_) => Effects::MAY_FAIL,
    }
}

fn node_effects(
    ir: &Ir,
    i: usize,
    node_eff: &[Effects],
    def_eff: &[Effects],
    locals: &[Option<LocalKind>],
    escape: Effects,
) -> Effects {
    let Some(node) = ir.exprs.get(i) else {
        return Effects::MAY_FAIL;
    };
    let mut e = Effects::NONE;
    for c in node.expr.children() {
        e |= node_eff
            .get(c.index())
            .copied()
            .unwrap_or(Effects::MAY_FAIL);
    }
    let own = match &node.expr {
        Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) | Expr::Tuple(_) | Expr::Record(_) => {
            Effects::NONE
        }
        Expr::Label { .. } => Effects::NONE,
        Expr::Local(_) => match locals.get(i).copied().flatten() {
            Some(LocalKind::Value) => Effects::NONE,
            Some(LocalKind::Let(d)) => {
                def_eff.get(d.index()).copied().unwrap_or(Effects::NONE) | Effects::MAY_FAIL
            }
            _ => Effects::MAY_FAIL,
        },
        Expr::Const(_) => Effects::OPEN | Effects::MAY_FAIL,
        Expr::SubstRef { decl, .. } => {
            let open = ir
                .decls
                .get(decl.index())
                .is_some_and(|d| d.constant.is_some());
            Effects::MAY_FAIL | if open { Effects::OPEN } else { Effects::NONE }
        }
        Expr::Def(d) => {
            def_eff.get(d.index()).copied().unwrap_or(Effects::NONE) | Effects::MAY_FAIL
        }
        Expr::Apply { callee, args } => {
            let mut c = match callee {
                OpRef::Std(op) => {
                    let key = args.first().and_then(|a| match a {
                        Arg::Expr { expr, .. } => ir.exprs.get(expr.index()).map(|n| &n.expr),
                        Arg::Op(_) => None,
                    });
                    std_effects(*op, key)
                }
                other => opref_effects(ir, other, def_eff, escape),
            };
            for a in args {
                if let Arg::Op(o) = a {
                    c |= opref_effects(ir, o, def_eff, escape);
                }
            }
            c
        }
        Expr::Subst { bindings, .. } => {
            let mut c = Effects::NONE;
            for b in bindings {
                if let Arg::Op(o) = &b.value {
                    c |= opref_effects(ir, o, def_eff, escape);
                }
            }
            c
        }
        Expr::Let { defs, .. } => {
            let mut c = Effects::NONE;
            for d in defs {
                if let Some(def) = ir.defs.get(d.index())
                    && matches!(def.scope, DefScope::LetValue { .. })
                {
                    // A thunk whose body may never be forced; its effects are
                    // counted where it is read. Its creation cannot fail.
                    c |= def_eff
                        .get(d.index())
                        .copied()
                        .unwrap_or(Effects::NONE)
                        .without(Effects::MAY_FAIL);
                }
            }
            c
        }
        Expr::Enabled(_) => Effects::USES_ENABLED | Effects::MAY_FAIL,
        Expr::Always(_)
        | Expr::Eventually(_)
        | Expr::LeadsTo(..)
        | Expr::WhilePlus(..)
        | Expr::Wf { .. }
        | Expr::Sf { .. }
        | Expr::TemporalQuant { .. } => Effects::TEMPORAL | Effects::MAY_FAIL,
        Expr::SetEnum(items) if items.len() <= 1 => Effects::NONE,
        _ => Effects::MAY_FAIL,
    };
    e | own
}

/// 64-bit FNV-1a, used for structural hashes because its output is fixed
/// across platforms and Rust versions.
pub(crate) struct Fnv(u64);

impl Fnv {
    pub(crate) fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub(crate) fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    pub(crate) fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.bytes(s.as_bytes());
    }

    pub(crate) fn u64(&mut self, n: u64) {
        self.bytes(&n.to_le_bytes());
    }

    pub(crate) fn finish(&self) -> u64 {
        self.0
    }
}

/// Computes every definition's structural hash.
pub(crate) fn compute_hashes(ir: &mut Ir) {
    let hashes: Vec<u64> = (0..ir.defs.len())
        .map(|i| {
            let mut h = Fnv::new();
            hash_def(ir, DefId::from_index(i), &mut h);
            h.finish()
        })
        .collect();
    for (d, h) in ir.defs.iter_mut().zip(hashes) {
        d.hash = h;
    }
}

fn def_name(ir: &Ir, d: DefId) -> String {
    ir.defs.get(d.index()).map_or_else(String::new, |def| {
        let m = ir
            .modules
            .get(def.module.index())
            .map_or("", |m| m.name.as_str());
        format!("{m}!{}", def.name)
    })
}

fn decl_name(ir: &Ir, d: crate::DeclId) -> String {
    ir.decls.get(d.index()).map_or_else(String::new, |decl| {
        let m = ir
            .modules
            .get(decl.module.index())
            .map_or("", |m| m.name.as_str());
        format!("{m}!{}", decl.name)
    })
}

fn hash_def(ir: &Ir, d: DefId, h: &mut Fnv) {
    let Some(def) = ir.defs.get(d.index()) else {
        return;
    };
    h.str(&format!("{:?}", def.kind));
    h.u64(def.params.len() as u64);
    for p in &def.params {
        h.str(&p.name);
        h.u64(u64::from(p.arity));
    }
    h.u64(u64::from(def.recursive));
    hash_expr(ir, def.body, h);
}

fn hash_opref(ir: &Ir, r: &OpRef, h: &mut Fnv) {
    match r {
        OpRef::Def(d) => {
            let local = ir
                .defs
                .get(d.index())
                .is_some_and(|def| def.scope != DefScope::TopLevel);
            if local {
                // Hashed where it is defined (the LET or the LAMBDA); a
                // recursive LET operator refers to itself.
                h.str("local");
                h.str(ir.defs.get(d.index()).map_or("", |x| x.name.as_str()));
            } else {
                h.str("def");
                h.str(&def_name(ir, *d));
            }
        }
        OpRef::Param(l) => {
            h.str("param");
            h.u64(u64::from(l.up));
            h.u64(u64::from(l.slot.0));
        }
        OpRef::Std(op) => {
            h.str("std");
            h.str(&op.qualified_name());
        }
        OpRef::Builtin(b) => {
            h.str("builtin");
            h.str(b.sany_name());
        }
        OpRef::Const(c) => {
            h.str("const");
            if let Some(&d) = ir.consts.get(c.index()) {
                h.str(&decl_name(ir, d));
            }
        }
        OpRef::Subst(d) => {
            h.str("subst");
            h.str(&decl_name(ir, *d));
        }
        OpRef::Unbound(n) => {
            h.str("unbound");
            h.str(n);
        }
    }
}

#[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
fn hash_expr(ir: &Ir, e: ExprId, h: &mut Fnv) {
    let mut work = vec![e];
    while let Some(e) = work.pop() {
        let Some(node) = ir.exprs.get(e.index()) else {
            continue;
        };
        let expr = &node.expr;
        // The variant's name, then its non-child payload.
        h.str(expr.tag());
        match expr {
            Expr::Bool(b) => h.u64(u64::from(*b)),
            Expr::Num(s) | Expr::Str(s) | Expr::Unbound(s) => h.str(s),
            Expr::Decimal { int, frac } => {
                h.str(int);
                h.str(frac);
            }
            Expr::Var { var, primed } => {
                if let Some(&d) = ir.vars.get(var.index()) {
                    h.str(&decl_name(ir, d));
                }
                h.u64(u64::from(*primed));
            }
            Expr::Const(c) => {
                if let Some(&d) = ir.consts.get(c.index()) {
                    h.str(&decl_name(ir, d));
                }
            }
            Expr::Local(l) => {
                h.u64(u64::from(l.up));
                h.u64(u64::from(l.slot.0));
            }
            Expr::SubstRef { decl, primed } => {
                h.str(&decl_name(ir, *decl));
                h.u64(u64::from(*primed));
            }
            Expr::Def(d) => {
                let local = ir
                    .defs
                    .get(d.index())
                    .is_some_and(|def| def.scope != DefScope::TopLevel);
                h.str(&if local {
                    String::from("local")
                } else {
                    def_name(ir, *d)
                });
            }
            Expr::Apply { callee, args } => {
                hash_opref(ir, callee, h);
                for a in args {
                    match a {
                        Arg::Expr { pass, .. } => h.str(&format!("{pass:?}")),
                        Arg::Op(o) => hash_opref(ir, o, h),
                    }
                }
            }
            Expr::Builtin { op, .. } => h.str(op.sany_name()),
            Expr::Junction {
                kind,
                bulleted,
                items,
            } => {
                h.str(&format!("{kind:?}"));
                h.u64(u64::from(*bulleted));
                h.u64(items.len() as u64);
            }
            Expr::Quant { kind, bounds, .. } => {
                h.str(&format!("{kind:?}"));
                for b in bounds {
                    h.u64(b.vars.len() as u64);
                    h.u64(u64::from(b.tuple));
                }
            }
            Expr::RecordSelect { field, .. } => h.str(field),
            Expr::Record(fs) | Expr::RecordSet(fs) => {
                for f in fs {
                    h.str(&f.name);
                }
            }
            Expr::Label { name, .. } => h.str(name),
            Expr::Let { defs, .. } => {
                for &d in defs {
                    hash_def(ir, d, h);
                }
            }
            Expr::Subst { bindings, .. } => {
                for b in bindings {
                    h.str(&decl_name(ir, b.decl));
                    if let Arg::Op(o) = &b.value {
                        hash_opref(ir, o, h);
                    }
                }
            }
            Expr::Case { arms, other } => {
                h.u64(arms.len() as u64);
                h.u64(u64::from(other.is_some()));
            }
            Expr::TemporalQuant { exists, vars, .. } => {
                h.u64(u64::from(*exists));
                h.u64(vars.len() as u64);
            }
            Expr::Except { updates, .. } => {
                for u in updates {
                    h.u64(u.path.len() as u64);
                }
            }
            _ => {}
        }
        let mut inline: Vec<crate::DefId> = expr
            .op_refs()
            .into_iter()
            .filter_map(|a| {
                if let OpRef::Def(d) = a {
                    Some(*d)
                } else {
                    None
                }
            })
            .collect();
        if let Expr::Def(d) = expr {
            inline.push(*d);
        }
        for d in inline {
            if ir
                .defs
                .get(d.index())
                .is_some_and(|def| matches!(def.scope, DefScope::Inline { .. }))
            {
                hash_def(ir, d, h);
            }
        }
        let children = expr.children();
        h.u64(children.len() as u64);
        work.extend(children.into_iter().rev());
    }
}
