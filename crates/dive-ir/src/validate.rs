//! Structural invariants of a lowered IR.
//!
//! [`check`] verifies what consumers rely on, and returns every violation
//! found. Lowering an accepted analysis must give none; the corpus tests
//! check that on every module.

use crate::analysis::{LocalKind, local_kinds, walk_frames};
use crate::expr::{Arg, Expr, OpRef, Pass};
use crate::ids::{DefId, ExprId};
use crate::{DefScope, Ir, SlotKind};

/// Checks the IR's invariants:
///
/// 1. **References resolve.** Every expression, definition, declaration,
///    variable, constant and slot id is in range; every [`Expr::Local`] and
///    [`OpRef::Param`] names a slot of a live frame.
/// 2. **Trees.** Each expression node has exactly one parent (or is a
///    definition's body), and its children have smaller ids.
/// 3. **Frames.** A `LET` or `LAMBDA` definition's parent is the definition
///    whose frame it is created in, and its depth is consistent.
/// 4. **Laziness points.** Arguments of user operators, constants and
///    parameters, zero-arity `LET` definitions and substitutions are
///    [`Pass::Lazy`] or [`Pass::Strict`]; arguments of Java overrides are
///    [`Pass::Direct`]; a strict point is inert.
/// 5. **Declarations.** [`Expr::Var`] and [`Expr::Const`] name declarations
///    no instance substitutes; [`Expr::SubstRef`] one that is substituted or
///    has no other meaning.
/// 6. **Recursion.** The definition call graph is cyclic only through
///    `RECURSIVE` definitions.
#[must_use]
pub fn check(ir: &Ir) -> Vec<String> {
    let mut v = Vec::new();
    ids_in_range(ir, &mut v);
    if !v.is_empty() {
        return v;
    }
    trees(ir, &mut v);
    frames(ir, &mut v);
    laziness(ir, &mut v);
    decls(ir, &mut v);
    recursion(ir, &mut v);
    v
}

fn ids_in_range(ir: &Ir, v: &mut Vec<String>) {
    let ne = ir.exprs.len();
    let nd = ir.defs.len();
    let mut prime_ids = std::collections::BTreeSet::new();
    for (expr, origin) in &ir.prime_origins {
        if !matches!(
            ir.exprs.get(expr.index()).map(|n| &n.expr),
            Some(Expr::Var { primed: true, .. } | Expr::SubstRef { primed: true, .. })
        ) || !prime_ids.insert(*expr)
            || origin.file.is_some_and(|f| f.index() >= ir.files.len())
        {
            v.push(format!("#{}: invalid prime operand origin", expr.index()));
        }
    }
    let check_ref = |r: &OpRef, v: &mut Vec<String>, at: usize| match r {
        OpRef::Def(d) if d.index() >= nd => v.push(format!("#{at}: callee d{} out of range", d.0)),
        OpRef::Const(c) if c.index() >= ir.consts.len() => {
            v.push(format!("#{at}: const{} out of range", c.0));
        }
        OpRef::Subst(d) if d.index() >= ir.decls.len() => {
            v.push(format!("#{at}: decl{} out of range", d.0));
        }
        _ => {}
    };
    for (i, n) in ir.exprs.iter().enumerate() {
        n.expr.for_each_child(|c| {
            if c.index() >= ne {
                v.push(format!("#{i}: child #{} out of range", c.0));
            }
        });
        for r in n.expr.op_refs() {
            check_ref(r, v, i);
        }
        match &n.expr {
            Expr::Var { var, .. } if var.index() >= ir.vars.len() => {
                v.push(format!("#{i}: var{} out of range", var.0));
            }
            Expr::Const(c) if c.index() >= ir.consts.len() => {
                v.push(format!("#{i}: const{} out of range", c.0));
            }
            Expr::SubstRef { decl, .. } if decl.index() >= ir.decls.len() => {
                v.push(format!("#{i}: decl{} out of range", decl.0));
            }
            Expr::Def(d) if d.index() >= nd => v.push(format!("#{i}: d{} out of range", d.0)),
            Expr::Let { defs, .. } => {
                for d in defs {
                    if d.index() >= nd {
                        v.push(format!("#{i}: LET d{} out of range", d.0));
                    }
                }
            }
            Expr::Subst { bindings, .. } => {
                for b in bindings {
                    if b.decl.index() >= ir.decls.len() {
                        v.push(format!("#{i}: substituted decl{} out of range", b.decl.0));
                    }
                }
            }
            _ => {}
        }
    }
    for (i, d) in ir.defs.iter().enumerate() {
        if d.body.index() >= ne {
            v.push(format!("d{i}: body out of range"));
        }
        if d.module.index() >= ir.modules.len() {
            v.push(format!("d{i}: module out of range"));
        }
    }
    for (i, &d) in ir.vars.iter().enumerate() {
        if ir
            .decls
            .get(d.index())
            .and_then(|x| x.var)
            .map(super::ids::VarId::index)
            != Some(i)
        {
            v.push(format!("var{i}: declaration does not point back"));
        }
    }
    for (i, &d) in ir.consts.iter().enumerate() {
        if ir
            .decls
            .get(d.index())
            .and_then(|x| x.constant)
            .map(super::ids::ConstId::index)
            != Some(i)
        {
            v.push(format!("const{i}: declaration does not point back"));
        }
    }
}

fn trees(ir: &Ir, v: &mut Vec<String>) {
    let mut parent = vec![false; ir.exprs.len()];
    for d in &ir.defs {
        if let Some(p) = parent.get_mut(d.body.index()) {
            if *p {
                v.push(format!("#{}: body of two definitions", d.body.0));
            }
            *p = true;
        }
    }
    for (i, n) in ir.exprs.iter().enumerate() {
        n.expr.for_each_child(|c| {
            if c.index() >= i {
                v.push(format!("#{i}: child #{} is not older", c.0));
            }
            if let Some(p) = parent.get_mut(c.index()) {
                if *p {
                    v.push(format!("#{}: has two parents", c.0));
                }
                *p = true;
            }
        });
    }
    for (i, p) in parent.iter().enumerate() {
        if !p {
            v.push(format!("#{i}: orphan expression"));
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one check per kind of frame reference"
)]
fn frames(ir: &Ir, v: &mut Vec<String>) {
    let mut reached = vec![false; ir.exprs.len()];
    walk_frames(ir, |e, stack| {
        if let Some(r) = reached.get_mut(e.index()) {
            *r = true;
        }
        let Some(node) = ir.exprs.get(e.index()) else {
            return;
        };
        let owner = stack.last().copied();
        let depth = stack.len().saturating_sub(1);
        let check_local = |up: u32, slot: usize, what: &str, v: &mut Vec<String>| {
            let target = stack
                .len()
                .checked_sub(1 + up as usize)
                .and_then(|i| stack.get(i))
                .and_then(|d| ir.defs.get(d.index()));
            match target {
                None => v.push(format!(
                    "#{}: {what} goes {up} frames out of {}",
                    e.0,
                    stack.len()
                )),
                Some(def) if slot >= def.frame.len() => {
                    v.push(format!(
                        "#{}: {what} slot {slot} not in a frame of {}",
                        e.0,
                        def.frame.len()
                    ));
                }
                Some(_) => {}
            }
        };
        match &node.expr {
            Expr::Local(l) => check_local(l.up, l.slot.index(), "local", v),
            Expr::Let { defs, .. } => {
                for d in defs {
                    let Some(def) = ir.defs.get(d.index()) else {
                        continue;
                    };
                    let (parent, want_depth) = match def.scope {
                        DefScope::LetValue { parent, slot, .. } => {
                            let ok = owner
                                .and_then(|o| ir.defs.get(o.index()))
                                .and_then(|o| o.frame.get(slot.index()))
                                .is_some_and(|s| s.kind == SlotKind::LetValue(*d));
                            if !ok {
                                v.push(format!("d{}: LET slot s{} does not hold it", d.0, slot.0));
                            }
                            (parent, depth)
                        }
                        DefScope::Local { parent } => (parent, depth + 1),
                        DefScope::TopLevel | DefScope::Inline { .. } => {
                            v.push(format!("d{}: top-level or inline definition in a LET", d.0));
                            continue;
                        }
                    };
                    if Some(parent) != owner {
                        v.push(format!(
                            "d{}: parent d{} is not the LET's frame",
                            d.0, parent.0
                        ));
                    }
                    if def.depth as usize != want_depth {
                        v.push(format!(
                            "d{}: depth {} should be {want_depth}",
                            d.0, def.depth
                        ));
                    }
                }
            }
            _ => {}
        }
        let mut used: Vec<DefId> = Vec::new();
        for r in node.expr.op_refs() {
            match r {
                OpRef::Param(l) => check_local(l.up, l.slot.index(), "parameter", v),
                OpRef::Def(d) => used.push(*d),
                _ => {}
            }
        }
        if let Expr::Def(d) = &node.expr {
            used.push(*d);
        }
        for d in used {
            if let Some(def) = ir.defs.get(d.index())
                && let DefScope::Inline { parent } = def.scope
            {
                if Some(parent) != owner {
                    v.push(format!(
                        "d{}: inline definition's parent d{} is not its frame",
                        d.0, parent.0
                    ));
                }
                if def.depth as usize != depth + 1 {
                    v.push(format!(
                        "d{}: depth {} should be {}",
                        d.0,
                        def.depth,
                        depth + 1
                    ));
                }
            }
        }
    });
    for (i, r) in reached.iter().enumerate() {
        if !r {
            v.push(format!("#{i}: not reached from any definition's frame"));
        }
    }
    for (i, k) in local_kinds(ir).iter().enumerate() {
        if *k == Some(LocalKind::Invalid) {
            v.push(format!("#{i}: local reference does not resolve"));
        }
    }
}

fn laziness(ir: &Ir, v: &mut Vec<String>) {
    let check = |at: usize, a: &Arg, want_lazy: bool, v: &mut Vec<String>| {
        if let Arg::Expr { expr, pass } = a {
            match (want_lazy, pass) {
                (true, Pass::Direct) => v.push(format!("#{at}: laziness point passed directly")),
                (false, Pass::Lazy | Pass::Strict) => {
                    v.push(format!(
                        "#{at}: built-in argument marked as a laziness point"
                    ));
                }
                _ => {}
            }
            if *pass == Pass::Strict && !inert(ir, *expr) {
                v.push(format!("#{at}: strict argument #{} is not inert", expr.0));
            }
        }
    };
    for (i, n) in ir.exprs.iter().enumerate() {
        match &n.expr {
            Expr::Apply { callee, args } => {
                let lazy = !matches!(callee, OpRef::Std(_) | OpRef::Builtin(_));
                for a in args {
                    check(i, a, lazy, v);
                }
            }
            Expr::Subst { bindings, .. } => {
                for b in bindings {
                    check(i, &b.value, true, v);
                }
            }
            _ => {}
        }
    }
    for (i, d) in ir.defs.iter().enumerate() {
        if let DefScope::LetValue { pass, .. } = d.scope {
            if pass == Pass::Direct {
                v.push(format!("d{i}: LET value passed directly"));
            }
            if pass == Pass::Strict && !inert(ir, d.body) {
                v.push(format!("d{i}: strict LET value is not inert"));
            }
        }
    }
}

fn inert(ir: &Ir, e: ExprId) -> bool {
    ir.exprs
        .get(e.index())
        .is_some_and(|n| n.effects.is_inert())
}

fn decls(ir: &Ir, v: &mut Vec<String>) {
    for (i, n) in ir.exprs.iter().enumerate() {
        match &n.expr {
            Expr::Var { var, .. } => {
                let d = ir
                    .vars
                    .get(var.index())
                    .and_then(|d| ir.decls.get(d.index()));
                if d.is_none_or(|d| d.substituted || !d.variable) {
                    v.push(format!(
                        "#{i}: Var of a substituted or non-variable declaration"
                    ));
                }
            }
            Expr::Const(c) => {
                let d = ir
                    .consts
                    .get(c.index())
                    .and_then(|d| ir.decls.get(d.index()));
                if d.is_none_or(|d| d.substituted || d.variable) {
                    v.push(format!(
                        "#{i}: Const of a substituted or non-constant declaration"
                    ));
                }
            }
            Expr::SubstRef { decl, .. } => {
                let d = ir.decls.get(decl.index());
                if d.is_none_or(|d| !d.substituted && (d.var.is_some() || d.constant.is_some())) {
                    v.push(format!("#{i}: SubstRef of an unsubstituted declaration"));
                }
            }
            _ => {}
        }
    }
}

fn recursion(ir: &Ir, v: &mut Vec<String>) {
    // Edges from every definition to the definitions its body uses,
    // excluding those of RECURSIVE definitions: what remains must be acyclic.
    let n = ir.defs.len();
    let mut owner_of = vec![None; ir.exprs.len()];
    for (i, d) in ir.defs.iter().enumerate() {
        let mut work = vec![d.body];
        while let Some(e) = work.pop() {
            if let Some(o) = owner_of.get_mut(e.index())
                && o.is_none()
            {
                *o = Some(i);
                if let Some(node) = ir.exprs.get(e.index()) {
                    node.expr.for_each_child(|child| work.push(child));
                }
            }
        }
    }
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (e, node) in ir.exprs.iter().enumerate() {
        let Some(Some(from)) = owner_of.get(e) else {
            continue;
        };
        if ir.defs.get(*from).is_some_and(|d| d.recursive) {
            continue;
        }
        let mut add = |d: DefId| {
            if let Some(es) = edges.get_mut(*from) {
                es.push(d.index());
            }
        };
        if let Expr::Def(d) = &node.expr {
            add(*d);
        }
        if let Expr::Let { defs, .. } = &node.expr {
            for d in defs {
                add(*d);
            }
        }
        for r in node.expr.op_refs() {
            if let OpRef::Def(d) = r {
                add(*d);
            }
        }
    }
    // Iterative DFS for a cycle.
    let mut state = vec![0u8; n];
    for start in 0..n {
        if state.get(start).copied() != Some(0) {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        if let Some(s) = state.get_mut(start) {
            *s = 1;
        }
        while let Some(&mut (node, ref mut next)) = stack.last_mut() {
            let succ = edges.get(node).and_then(|es| es.get(*next)).copied();
            *next += 1;
            if let Some(s) = succ {
                match state.get(s).copied() {
                    Some(0) => {
                        if let Some(x) = state.get_mut(s) {
                            *x = 1;
                        }
                        stack.push((s, 0));
                    }
                    Some(1) => {
                        let name = ir.defs.get(s).map_or("?", |d| d.name.as_str());
                        v.push(format!(
                            "d{s} ({name}): on a cycle with no RECURSIVE definition"
                        ));
                    }
                    _ => {}
                }
            } else {
                if let Some(x) = state.get_mut(node) {
                    *x = 2;
                }
                stack.pop();
            }
        }
    }
}
