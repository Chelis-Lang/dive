//! Macro expansion (`ParseAlgorithm.ExpandMacrosInStmtSeq`, `ExpandMacroCall`
//! and `SubstituteInStmt`).
//!
//! Arguments are substituted for parameters in every expression of the macro
//! body, parenthesized when they have more than one token. A parameter used
//! as an assignment target must be replaced by an identifier, optionally
//! followed by selectors that are prepended to the target's own selectors.

use super::Fail;
use super::ast::{Clause, Kind, LabeledStmt, Loc, Macro, SingleAssign, Stmt};
use super::expr::{Expr, Kind as TokKind};

/// Expands the macro calls in `stmts` (before labels are grouped).
pub(crate) fn expand_in(stmts: &mut Vec<Stmt>, macros: &[Macro]) -> Result<(), Fail> {
    let mut i = 0;
    while i < stmts.len() {
        match &mut stmts[i].kind {
            Kind::LabelIf {
                unlab_then,
                unlab_else,
                ..
            } => {
                expand_in(unlab_then, macros)?;
                expand_in(unlab_else, macros)?;
            }
            Kind::LabelEither(clauses) => {
                for clause in clauses {
                    expand_in(&mut clause.unlab, macros)?;
                }
            }
            Kind::With { body, .. } => expand_in(body, macros)?,
            Kind::While { unlab, .. } => expand_in(unlab, macros)?,
            Kind::MacroCall { .. } => {
                let call = stmts.remove(i);
                let expansion = expand_call(&call, macros)?;
                let n = expansion.len();
                for (k, stmt) in expansion.into_iter().enumerate() {
                    stmts.insert(i + k, stmt);
                }
                i += n;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

fn expand_call(call: &Stmt, macros: &[Macro]) -> Result<Vec<Stmt>, Fail> {
    let Kind::MacroCall { name, args } = &call.kind else {
        return Err(Fail::internal("macro expansion of a non-call"));
    };
    let Some(definition) = macros.iter().rev().find(|m| &m.name == name) else {
        return Err(Fail::at(
            format!("Macro {name} undefined,\n    at {}", call.loc.describe()),
            call.at,
        ));
    };
    if definition.params.len() != args.len() {
        return Err(Fail::at(
            format!(
                "Macro {name} called with wrong number of arguments at \n    {}",
                call.loc.describe()
            ),
            call.at,
        ));
    }
    let subs: Vec<(String, Expr)> = definition
        .params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    let site = (call.loc.line, call.loc.col);
    let mut result = Vec::with_capacity(definition.body.len());
    for stmt in &definition.body {
        result.push(substitute_stmt(stmt, &subs, Some(site), call.at)?);
    }
    if let Some(first) = result.first_mut() {
        first.lbl.clone_from(&call.lbl);
    }
    Ok(result)
}

fn seq(stmts: &[Stmt], subs: &[(String, Expr)], site: Option<(usize, usize)>, at: usize) -> Result<Vec<Stmt>, Fail> {
    stmts
        .iter()
        .map(|s| substitute_stmt(s, subs, site, at))
        .collect()
}

fn labeled(
    stmts: &[LabeledStmt],
    subs: &[(String, Expr)],
    at: usize,
) -> Result<Vec<LabeledStmt>, Fail> {
    stmts
        .iter()
        .map(|l| {
            Ok(LabeledStmt {
                label: l.label.clone(),
                stmts: seq(&l.stmts, subs, None, at)?,
                loc: l.loc,
            })
        })
        .collect()
}

fn sub(expr: &Expr, subs: &[(String, Expr)]) -> Expr {
    expr.substitute(subs, true)
}

/// `SubstituteInStmt`.
pub(crate) fn substitute_stmt(
    stmt: &Stmt,
    subs: &[(String, Expr)],
    site: Option<(usize, usize)>,
    at: usize,
) -> Result<Stmt, Fail> {
    let mut loc: Loc = stmt.loc;
    if site.is_some() {
        loc.macro_call = site;
    }
    let kind = match &stmt.kind {
        Kind::Assign(ass) => {
            let mut out = Vec::with_capacity(ass.len());
            for a in ass {
                out.push(substitute_assign(a, subs, site, at)?);
            }
            Kind::Assign(out)
        }
        Kind::If { test, then, els } => Kind::If {
            test: sub(test, subs),
            then: seq(then, subs, site, at)?,
            els: seq(els, subs, site, at)?,
        },
        Kind::Either(ors) => Kind::Either(
            ors.iter()
                .map(|o| seq(o, subs, site, at))
                .collect::<Result<_, _>>()?,
        ),
        Kind::With {
            var,
            is_eq,
            exp,
            body,
        } => Kind::With {
            var: var.clone(),
            is_eq: *is_eq,
            exp: sub(exp, subs),
            body: seq(body, subs, site, at)?,
        },
        Kind::When(e) => Kind::When(sub(e, subs)),
        Kind::Assert(e) => Kind::Assert(sub(e, subs)),
        Kind::Print(e) => Kind::Print(sub(e, subs)),
        Kind::Skip => Kind::Skip,
        Kind::While { test, unlab, lab } => Kind::While {
            test: sub(test, subs),
            unlab: seq(unlab, subs, site, at)?,
            lab: labeled(lab, subs, at)?,
        },
        Kind::LabelIf {
            test,
            unlab_then,
            lab_then,
            unlab_else,
            lab_else,
        } => Kind::LabelIf {
            test: sub(test, subs),
            unlab_then: seq(unlab_then, subs, site, at)?,
            lab_then: labeled(lab_then, subs, at)?,
            unlab_else: seq(unlab_else, subs, site, at)?,
            lab_else: labeled(lab_else, subs, at)?,
        },
        Kind::LabelEither(clauses) => Kind::LabelEither(
            clauses
                .iter()
                .map(|c| {
                    Ok(Clause {
                        unlab: seq(&c.unlab, subs, site, at)?,
                        lab: labeled(&c.lab, subs, at)?,
                    })
                })
                .collect::<Result<_, Fail>>()?,
        ),
        Kind::Call { to, args } => Kind::Call {
            to: to.clone(),
            args: args.iter().map(|a| sub(a, subs)).collect(),
        },
        Kind::Return { from } => Kind::Return { from: from.clone() },
        Kind::CallReturn { to, from, args } => Kind::CallReturn {
            to: to.clone(),
            from: from.clone(),
            args: args.iter().map(|a| sub(a, subs)).collect(),
        },
        Kind::CallGoto { to, after, args } => Kind::CallGoto {
            to: to.clone(),
            after: after.clone(),
            args: args.iter().map(|a| sub(a, subs)).collect(),
        },
        Kind::Goto(to) => Kind::Goto(to.clone()),
        Kind::MacroCall { .. } => {
            return Err(Fail::at(
                format!("Found unexpected statement type in macro at{}", loc.describe()),
                stmt.at,
            ));
        }
    };
    Ok(Stmt {
        lbl: stmt.lbl.clone(),
        loc,
        at: stmt.at,
        kind,
    })
}

fn substitute_assign(
    assign: &SingleAssign,
    subs: &[(String, Expr)],
    site: Option<(usize, usize)>,
    at: usize,
) -> Result<SingleAssign, Fail> {
    let mut loc = assign.loc;
    if site.is_some() {
        loc.macro_call = site;
    }
    let mut result = SingleAssign {
        var: assign.var.clone(),
        sub: if assign.sub.is_empty() {
            assign.sub.clone()
        } else {
            sub(&assign.sub, subs)
        },
        rhs: sub(&assign.rhs, subs),
        loc,
    };
    if let Some((_, replacement)) = subs.iter().find(|(param, _)| *param == assign.var) {
        let Some(first) = replacement.first() else {
            return Err(Fail::internal("empty macro argument"));
        };
        if first.kind != TokKind::Ident {
            return Err(Fail::at(
                format!(
                    "Macro expansion substitutes `{}' for assignment variable\n    at {}",
                    first.text,
                    loc.describe()
                ),
                at,
            ));
        }
        result.var.clone_from(&first.text);
        if !replacement.is_one_token() {
            let mut rest = replacement.clone();
            if let Some(line) = rest.lines.first_mut() {
                line.remove(0);
            }
            rest.normalize();
            if result.sub.is_empty() {
                result.sub = rest;
            } else {
                result.sub.prepend(&rest, 0);
            }
        }
    }
    Ok(result)
}
