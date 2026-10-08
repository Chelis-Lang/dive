//! A deterministic text dump of the IR, for debugging and snapshot tests.
//!
//! Expressions print as s-expressions. Ids are printed as `#n` (expressions
//! are not numbered, since each node has exactly one parent), definitions as
//! `d<n>`, declarations as `decl<n>`, slots as `s<n>`, and a local reference
//! as `s<n>^<up>`.

use crate::expr::{Arg, Bound, Expr, OpRef, Pass};
use crate::ids::ExprId;
use crate::{DefScope, Ir, Symbol};
use std::fmt::Write as _;

#[allow(clippy::too_many_lines, reason = "one section per table of the IR")]
pub(crate) fn print(ir: &Ir) -> String {
    let mut out = String::new();
    for (i, f) in ir.files.iter().enumerate() {
        let _ = writeln!(
            out,
            "file f{i} {:?} module {}{}",
            f.display,
            f.module,
            if f.standard { " standard" } else { "" }
        );
    }
    for (i, m) in ir.modules.iter().enumerate() {
        let _ = write!(out, "module m{i} {} nesting {}", m.name, m.nesting);
        if m.standard {
            out.push_str(" standard");
        }
        if m.instantiated {
            out.push_str(" instantiated");
        }
        if i == ir.root.index() {
            out.push_str(" root");
        }
        out.push('\n');
        for (name, sym) in &m.context {
            let s = match sym {
                Symbol::Def(d) => format!("d{}", d.0),
                Symbol::Decl(d) => format!("decl{}", d.0),
                Symbol::Instance => "instance".to_owned(),
            };
            let _ = writeln!(out, "  {name} = {s}");
        }
        for a in &m.assumptions {
            let _ = writeln!(out, "  assume d{}", a.0);
        }
        for t in &m.theorems {
            let _ = writeln!(out, "  theorem d{}", t.0);
        }
    }
    for (i, d) in ir.decls.iter().enumerate() {
        let _ = write!(
            out,
            "decl{i} {} {} m{} arity {}",
            if d.variable { "VARIABLE" } else { "CONSTANT" },
            d.name,
            d.module.0,
            d.arity
        );
        if let Some(v) = d.var {
            let _ = write!(out, " var{}", v.0);
        }
        if let Some(c) = d.constant {
            let _ = write!(out, " const{}", c.0);
        }
        if d.substituted {
            out.push_str(" substituted");
        }
        out.push('\n');
    }
    for (i, d) in ir.defs.iter().enumerate() {
        let scope = match d.scope {
            DefScope::TopLevel => "top".to_owned(),
            DefScope::Local { parent } => format!("local in d{}", parent.0),
            DefScope::Inline { parent } => format!("inline in d{}", parent.0),
            DefScope::LetValue { parent, slot, pass } => {
                format!("let s{} of d{} {pass:?}", slot.0, parent.0)
            }
        };
        let _ = write!(
            out,
            "d{i} {:?} {:?} m{} [{scope}] depth {} level {:?} effects {:?} hash {:016x}",
            d.name,
            d.kind,
            d.module.0,
            d.depth,
            d.level,
            d.effects.names(),
            d.hash
        );
        if d.recursive {
            out.push_str(" recursive");
        }
        if d.local {
            out.push_str(" LOCAL");
        }
        if let Some(s) = d.std {
            let _ = write!(out, " std {s:?}");
        }
        if let Some(s) = d.source {
            let _ = write!(out, " from d{}", s.0);
        }
        let _ = write!(out, " at {}", ir.location(&d.origin));
        out.push('\n');
        if !d.params.is_empty() {
            let ps: Vec<String> = d
                .params
                .iter()
                .map(|p| {
                    if p.arity == 0 {
                        p.name.clone()
                    } else {
                        format!("{}/{}", p.name, p.arity)
                    }
                })
                .collect();
            let _ = writeln!(out, "  params {}", ps.join(" "));
        }
        if !d.frame.is_empty() {
            let fs: Vec<String> = d
                .frame
                .iter()
                .enumerate()
                .map(|(j, s)| format!("s{j}={}:{:?}", s.name, s.kind))
                .collect();
            let _ = writeln!(out, "  frame {}", fs.join(" "));
        }
        out.push_str("  body ");
        expr(ir, d.body, &mut out);
        out.push('\n');
    }
    for r in &ir.refusals {
        let _ = writeln!(out, "refusal {} in d{}", r.feature.name(), r.def.0);
    }
    out
}

fn opref(r: &OpRef) -> String {
    match r {
        OpRef::Def(d) => format!("d{}", d.0),
        OpRef::Param(l) => format!("s{}^{}", l.slot.0, l.up),
        OpRef::Std(s) => format!("std:{s:?}"),
        OpRef::Builtin(b) => format!("builtin:{}", b.sany_name()),
        OpRef::Const(c) => format!("const{}", c.0),
        OpRef::Subst(d) => format!("subst:decl{}", d.0),
        OpRef::Unbound(n) => format!("unbound:{n}"),
    }
}

fn bounds(ir: &Ir, bs: &[Bound], out: &mut String) {
    out.push('[');
    for (i, b) in bs.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let vs: Vec<String> = b.vars.iter().map(|s| format!("s{}", s.0)).collect();
        if b.tuple {
            let _ = write!(out, "<<{}>>", vs.join(" "));
        } else {
            out.push_str(&vs.join(" "));
        }
        if let Some(d) = b.domain {
            out.push_str(" \\in ");
            expr(ir, d, out);
        }
    }
    out.push(']');
}

fn list(ir: &Ir, head: &str, items: &[ExprId], out: &mut String) {
    let _ = write!(out, "({head}");
    for &i in items {
        out.push(' ');
        expr(ir, i, out);
    }
    out.push(')');
}

#[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
fn expr(ir: &Ir, e: ExprId, out: &mut String) {
    let Some(node) = ir.exprs.get(e.index()) else {
        let _ = write!(out, "<bad #{}>", e.0);
        return;
    };
    match &node.expr {
        Expr::Bool(b) => out.push_str(if *b { "TRUE" } else { "FALSE" }),
        Expr::Num(n) => out.push_str(n),
        Expr::Decimal { int, frac } => {
            let _ = write!(out, "{int}.{frac}");
        }
        Expr::Str(s) => {
            let _ = write!(out, "{s:?}");
        }
        Expr::Var { var, primed } => {
            let _ = write!(out, "var{}{}", var.0, if *primed { "'" } else { "" });
        }
        Expr::Const(c) => {
            let _ = write!(out, "const{}", c.0);
        }
        Expr::Local(l) => {
            let _ = write!(out, "s{}^{}", l.slot.0, l.up);
        }
        Expr::SubstRef { decl, primed } => {
            let _ = write!(
                out,
                "subst:decl{}{}",
                decl.0,
                if *primed { "'" } else { "" }
            );
        }
        Expr::Def(d) => {
            let _ = write!(out, "d{}", d.0);
        }
        Expr::Apply { callee, args } => {
            let _ = write!(out, "({}", opref(callee));
            for a in args {
                out.push(' ');
                match a {
                    Arg::Expr { expr: x, pass } => {
                        match pass {
                            Pass::Lazy => out.push('~'),
                            Pass::Strict => out.push('!'),
                            Pass::Direct => {}
                        }
                        expr(ir, *x, out);
                    }
                    Arg::Op { op: o, .. } => {
                        let _ = write!(out, "&{}", opref(o));
                    }
                }
            }
            out.push(')');
        }
        Expr::Builtin { op, args } => list(ir, op.sany_name(), args, out),
        Expr::Junction {
            kind,
            bulleted,
            items,
        } => {
            let head = format!("{kind:?}{}", if *bulleted { "-list" } else { "" });
            list(ir, &head, items, out);
        }
        Expr::Not(a) => list(ir, "not", &[*a], out),
        Expr::Implies(a, b) => list(ir, "=>", &[*a, *b], out),
        Expr::Equiv(a, b) => list(ir, "<=>", &[*a, *b], out),
        Expr::If { cond, then, other } => list(ir, "if", &[*cond, *then, *other], out),
        Expr::Case { arms, other } => {
            out.push_str("(case");
            for a in arms {
                out.push_str(" [");
                expr(ir, a.guard, out);
                out.push_str(" -> ");
                expr(ir, a.value, out);
                out.push(']');
            }
            if let Some(o) = other {
                out.push_str(" [other ");
                expr(ir, *o, out);
                out.push(']');
            }
            out.push(')');
        }
        Expr::Quant {
            kind,
            bounds: bs,
            body,
        } => {
            let _ = write!(out, "({kind:?} ");
            bounds(ir, bs, out);
            out.push(' ');
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::SetFilter { bound, pred } => {
            out.push_str("(filter ");
            bounds(ir, std::slice::from_ref(bound), out);
            out.push(' ');
            expr(ir, *pred, out);
            out.push(')');
        }
        Expr::SetMap {
            expr: x,
            bounds: bs,
        } => {
            out.push_str("(map ");
            expr(ir, *x, out);
            out.push(' ');
            bounds(ir, bs, out);
            out.push(')');
        }
        Expr::FcnCons { bounds: bs, body } => {
            out.push_str("(fcn ");
            bounds(ir, bs, out);
            out.push(' ');
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::RecFcn {
            name,
            bounds: bs,
            body,
        } => {
            let _ = write!(out, "(recfcn s{} ", name.0);
            bounds(ir, bs, out);
            out.push(' ');
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::SetEnum(items) => list(ir, "set", items, out),
        Expr::Tuple(items) => list(ir, "tuple", items, out),
        Expr::Cartesian(items) => list(ir, "\\X", items, out),
        Expr::Record(fs) | Expr::RecordSet(fs) => {
            out.push_str(if matches!(node.expr, Expr::Record(_)) {
                "(record"
            } else {
                "(recordset"
            });
            for f in fs {
                let _ = write!(out, " {}:", f.name);
                expr(ir, f.value, out);
            }
            out.push(')');
        }
        Expr::FcnSet { domain, range } => list(ir, "->", &[*domain, *range], out),
        Expr::FcnApply { fcn, arg } => list(ir, "apply", &[*fcn, *arg], out),
        Expr::RecordSelect { record, field } => {
            out.push_str("(.");
            let _ = write!(out, "{field} ");
            expr(ir, *record, out);
            out.push(')');
        }
        Expr::Except { base, updates } => {
            out.push_str("(except ");
            expr(ir, *base, out);
            for u in updates {
                out.push_str(" [");
                for &p in &u.path {
                    out.push('!');
                    expr(ir, p, out);
                }
                out.push_str(" = ");
                expr(ir, u.value, out);
                out.push(']');
            }
            out.push(')');
        }
        Expr::ExceptAt => out.push('@'),
        Expr::Let { defs, body } => {
            out.push_str("(let [");
            let ds: Vec<String> = defs.iter().map(|d| format!("d{}", d.0)).collect();
            out.push_str(&ds.join(" "));
            out.push_str("] ");
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::Subst { bindings, body } => {
            out.push_str("(subst [");
            for (i, b) in bindings.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                let _ = write!(out, "decl{}<-", b.decl.0);
                match &b.value {
                    Arg::Expr { expr: x, pass } => {
                        if *pass == Pass::Strict {
                            out.push('!');
                        }
                        expr(ir, *x, out);
                    }
                    Arg::Op { op: o, .. } => {
                        let _ = write!(out, "&{}", opref(o));
                    }
                }
            }
            out.push_str("] ");
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::Label { name, body } => {
            let _ = write!(out, "({name}:: ");
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::Selected(a) => list(ir, "selected", &[*a], out),
        Expr::Prime(a) => list(ir, "prime", &[*a], out),
        Expr::Unchanged(a) => list(ir, "unchanged", &[*a], out),
        Expr::StepBox { action, sub } => list(ir, "[]_", &[*action, *sub], out),
        Expr::StepAngle { action, sub } => list(ir, "<>_", &[*action, *sub], out),
        Expr::Enabled(a) => list(ir, "enabled", &[*a], out),
        Expr::Cdot(a, b) => list(ir, "cdot", &[*a, *b], out),
        Expr::Always(a) => list(ir, "[]", &[*a], out),
        Expr::Eventually(a) => list(ir, "<>", &[*a], out),
        Expr::LeadsTo(a, b) => list(ir, "~>", &[*a, *b], out),
        Expr::WhilePlus(a, b) => list(ir, "-+->", &[*a, *b], out),
        Expr::Wf { sub, action } => list(ir, "WF", &[*sub, *action], out),
        Expr::Sf { sub, action } => list(ir, "SF", &[*sub, *action], out),
        Expr::TemporalQuant { exists, vars, body } => {
            let vs: Vec<String> = vars.iter().map(|s| format!("s{}", s.0)).collect();
            let _ = write!(
                out,
                "({} [{}] ",
                if *exists { "EE" } else { "AA" },
                vs.join(" ")
            );
            expr(ir, *body, out);
            out.push(')');
        }
        Expr::Unbound(n) => {
            let _ = write!(out, "(unbound {n})");
        }
        Expr::Unevaluable(u) => {
            let _ = write!(out, "(unevaluable {u:?})");
        }
    }
}
