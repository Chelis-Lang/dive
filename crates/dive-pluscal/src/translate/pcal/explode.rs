//! Explosion into flat labelled steps (`pcal.PcalTranslate`).
//!
//! `while`, labelled `if`/`either` and inner labels become separate steps;
//! control flow becomes `pc` tests and assignments, and procedure calls and
//! returns become pushes and pops of the `stack` frame records.

use super::Fail;
use super::ast::{Algorithm, Body, Kind, LabeledStmt, Loc, SingleAssign, Stmt, VarDecl};
use super::expr::{Expr, Kind as TokKind};
use std::fmt::Write as _;

/// What a call needs to know about a procedure.
#[derive(Clone, Debug)]
pub(crate) struct ProcInfo {
    pub name: String,
    pub params: Vec<VarDecl>,
    pub decls: Vec<VarDecl>,
    pub initial_pc: String,
}

pub(crate) struct Exploder<'a> {
    procs: &'a [ProcInfo],
    omit_pc: bool,
    current_procedure: Option<String>,
}

fn quoted(label: &str) -> String {
    format!("\"{label}\"")
}

/// `pc := next`.
pub(super) fn update_pc(next: &str) -> Stmt {
    Stmt::new(
        Kind::Assign(vec![SingleAssign {
            var: "pc".into(),
            sub: Expr::empty(),
            rhs: Expr::line(&[(&quoted(next), TokKind::Str)]),
            loc: Loc::default(),
        }]),
        Loc::default(),
        0,
    )
}

/// `await pc = label`.
fn check_pc(label: &str) -> Stmt {
    Stmt::new(
        Kind::When(Expr::line(&[
            ("pc", TokKind::Added),
            ("=", TokKind::Builtin),
            (&quoted(label), TokKind::Str),
        ])),
        Loc::default(),
        0,
    )
}

/// `TRUE` or `(TRUE)` (`PcalTranslate.IsTRUE`).
fn is_true(expr: &Expr) -> bool {
    if expr.lines.len() != 1 {
        return false;
    }
    let line: Vec<&str> = expr.lines[0].iter().map(|t| t.text.as_str()).collect();
    matches!(line.as_slice(), ["TRUE"] | ["(", "TRUE", ")"])
}

fn assign(var: &str, rhs: Expr, loc: Loc, at: usize) -> Stmt {
    Stmt::new(
        Kind::Assign(vec![SingleAssign {
            var: var.to_owned(),
            sub: Expr::empty(),
            rhs,
            loc,
        }]),
        loc,
        at,
    )
}

/// `Head(stack).field`.
fn head_field(field: &str) -> Expr {
    Expr::parse(&format!("Head(stack).{field}"), &["stack"])
}

/// The new top-of-stack record pushed by a call.
fn frame(to: &str, pc: &str, info: &ProcInfo, rest: &str) -> Expr {
    let mut text = format!("<< [ procedure |->  \"{to}\",\n     pc        |->  {pc}");
    for decl in info.decls.iter().chain(&info.params) {
        let _ = write!(text, ",\n     {:<9} |->  {}", decl.var, decl.var);
    }
    // MakeNewStackTopExprPretty places the final \o at column 2 before
    // normalization subtracts the first token's column 1.
    text.push_str(" ] >>\n \\o ");
    text.push_str(rest);
    // Only the stack itself is a translator-added variable here; the record
    // fields are skipped as labels when subscripts are added.
    Expr::parse(&text, &["stack"])
}

impl<'a> Exploder<'a> {
    pub(crate) fn new(procs: &'a [ProcInfo], omit_pc: bool) -> Self {
        Self {
            procs,
            omit_pc,
            current_procedure: None,
        }
    }

    /// `PcalTranslate.Explode`.
    pub(crate) fn explode(&mut self, alg: &mut Algorithm) -> Result<(), Fail> {
        for procedure in &mut alg.prcds {
            self.current_procedure = Some(procedure.name.clone());
            let body = std::mem::take(&mut procedure.body);
            procedure.body = self.explode_seq(body, "Error")?;
            self.current_procedure = None;
        }
        match &mut alg.body {
            Body::Uni { body, .. } => {
                let seq = std::mem::take(body);
                *body = self.explode_seq(seq, "Done")?;
            }
            Body::Multi(processes) => {
                for process in processes {
                    let seq = std::mem::take(&mut process.body);
                    process.body = self.explode_seq(seq, "Done")?;
                }
            }
        }
        Ok(())
    }

    fn explode_seq(&mut self, seq: Vec<LabeledStmt>, next: &str) -> Result<Vec<LabeledStmt>, Fail> {
        let labels: Vec<String> = seq.iter().map(|l| l.label.clone()).collect();
        let mut result = Vec::new();
        for (i, labeled) in seq.into_iter().enumerate() {
            let continuation = labels.get(i + 1).map_or(next, String::as_str).to_owned();
            result.extend(self.explode_labeled(labeled, &continuation)?);
        }
        Ok(result)
    }

    fn explode_labeled(&mut self, ast: LabeledStmt, next: &str) -> Result<Vec<LabeledStmt>, Fail> {
        if matches!(ast.stmts.first().map(|s| &s.kind), Some(Kind::While { .. })) {
            return self.explode_while(ast, next);
        }
        let (mut stmts, rest) = self.with_goto(ast.stmts, next)?;
        let mut result = Vec::new();
        // pcal.trans drops the step itself when the pc is omitted; that only
        // happens for bodies that explode through `explode_while`.
        if !self.omit_pc {
            stmts.insert(0, check_pc(&ast.label));
            result.push(LabeledStmt {
                label: ast.label,
                stmts,
                loc: ast.loc,
            });
        }
        result.extend(rest);
        Ok(result)
    }

    fn explode_while(&mut self, ast: LabeledStmt, next: &str) -> Result<Vec<LabeledStmt>, Fail> {
        let mut stmts = ast.stmts.into_iter();
        let Some(Stmt {
            kind: Kind::While { test, unlab, lab },
            ..
        }) = stmts.next()
        else {
            return Err(Fail::internal("while explosion without a while"));
        };
        let rest: Vec<Stmt> = stmts.collect();
        let mut new_stmts = Vec::new();
        if !self.omit_pc {
            new_stmts.push(check_pc(&ast.label));
        }
        let unlab_next = lab.first().map_or(ast.label.clone(), |l| l.label.clone());
        let (body, body_rest) = self.with_goto(unlab, &unlab_next)?;
        let (after, after_rest) = self.with_goto(rest, next)?;
        let trivial = is_true(&test);
        if trivial {
            new_stmts.extend(body);
        } else {
            new_stmts.push(Stmt::new(
                Kind::If {
                    test,
                    then: body,
                    els: after,
                },
                Loc::default(),
                0,
            ));
        }
        let mut result = vec![LabeledStmt {
            label: ast.label.clone(),
            stmts: new_stmts,
            loc: ast.loc,
        }];
        let labels: Vec<String> = lab.iter().map(|l| l.label.clone()).collect();
        for (i, labeled) in lab.into_iter().enumerate() {
            let continuation = labels
                .get(i + 1)
                .cloned()
                .unwrap_or_else(|| ast.label.clone());
            result.extend(self.explode_labeled(labeled, &continuation)?);
        }
        result.extend(body_rest);
        if !trivial {
            result.extend(after_rest);
        }
        Ok(result)
    }

    fn with_goto(
        &mut self,
        stmts: Vec<Stmt>,
        next: &str,
    ) -> Result<(Vec<Stmt>, Vec<LabeledStmt>), Fail> {
        let (mut stmts, rest, needs_goto) = self.explode_last(stmts, next)?;
        if needs_goto {
            stmts.push(update_pc(next));
        }
        Ok((stmts, rest))
    }

    /// `CopyAndExplodeLastStmt`: explodes the last statement of `stmts`;
    /// returns the statements, the labelled steps split off, and whether a
    /// `pc := next` must still be appended.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per statement class, mirroring CopyAndExplodeLastStmt"
    )]
    fn explode_last(
        &mut self,
        mut stmts: Vec<Stmt>,
        next: &str,
    ) -> Result<(Vec<Stmt>, Vec<LabeledStmt>, bool), Fail> {
        let mut rest = Vec::new();
        let mut needs_goto = false;
        let Some(last) = stmts.pop() else {
            return Ok((stmts, rest, !self.omit_pc));
        };
        let Stmt { lbl, loc, at, kind } = last;
        let wrap = |kind: Kind| Stmt {
            lbl: lbl.clone(),
            loc,
            at,
            kind,
        };
        match kind {
            Kind::LabelIf {
                test,
                unlab_then,
                lab_then,
                unlab_else,
                lab_else,
            } => {
                let then_next = lab_then
                    .first()
                    .map_or(next.to_owned(), |l| l.label.clone());
                let else_next = lab_else
                    .first()
                    .map_or(next.to_owned(), |l| l.label.clone());
                let (then, then_rest) = self.with_goto(unlab_then, &then_next)?;
                let (els, else_rest) = self.with_goto(unlab_else, &else_next)?;
                stmts.push(wrap(Kind::If { test, then, els }));
                rest.extend(self.explode_seq(lab_then, next)?);
                rest.extend(self.explode_seq(lab_else, next)?);
                rest.extend(then_rest);
                rest.extend(else_rest);
            }
            Kind::LabelEither(clauses) => {
                let mut ors = Vec::new();
                for clause in clauses {
                    let clause_next = clause
                        .lab
                        .first()
                        .map_or(next.to_owned(), |l| l.label.clone());
                    let (body, body_rest) = self.with_goto(clause.unlab, &clause_next)?;
                    ors.push(body);
                    rest.extend(body_rest);
                    rest.extend(self.explode_seq(clause.lab, next)?);
                }
                stmts.push(wrap(Kind::Either(ors)));
            }
            Kind::Goto(to) => stmts.push(update_pc(&to)),
            Kind::Call { to, args } => {
                stmts.extend(self.explode_call(&to, args, next, loc, at)?);
            }
            Kind::Return { from } => {
                stmts.extend(self.explode_return(from, loc, at)?);
            }
            Kind::CallReturn { to, from, args } => {
                stmts.extend(self.explode_call_return(&to, from, args, loc, at)?);
            }
            Kind::CallGoto { to, after, args } => {
                stmts.extend(self.explode_call(&to, args, &after, loc, at)?);
            }
            Kind::If { test, then, els } => {
                let (mut then, then_rest, then_goto) = self.explode_last(then, next)?;
                let (mut els, else_rest, else_goto) = self.explode_last(els, next)?;
                rest.extend(then_rest);
                rest.extend(else_rest);
                if !self.omit_pc {
                    needs_goto = then_goto && else_goto;
                    if !needs_goto {
                        if then_goto {
                            then.push(update_pc(next));
                        }
                        if else_goto {
                            els.push(update_pc(next));
                        }
                    }
                }
                stmts.push(wrap(Kind::If { test, then, els }));
            }
            Kind::Either(ors) => {
                needs_goto = true;
                let mut flags = Vec::new();
                let mut new_ors = Vec::new();
                for or in ors {
                    let (body, body_rest, goto) = self.explode_last(or, next)?;
                    rest.extend(body_rest);
                    needs_goto = needs_goto && goto;
                    flags.push(goto);
                    new_ors.push(body);
                }
                if !self.omit_pc && !needs_goto {
                    for (body, goto) in new_ors.iter_mut().zip(flags) {
                        if goto {
                            body.push(update_pc(next));
                        }
                    }
                }
                stmts.push(wrap(Kind::Either(new_ors)));
            }
            Kind::With {
                var,
                is_eq,
                exp,
                body,
            } => {
                let (body, body_rest, goto) = self.explode_last(body, next)?;
                rest.extend(body_rest);
                needs_goto = goto;
                stmts.push(wrap(Kind::With {
                    var,
                    is_eq,
                    exp,
                    body,
                }));
            }
            other => {
                needs_goto = true;
                stmts.push(wrap(other));
            }
        }
        if self.omit_pc {
            needs_goto = false;
        }
        Ok((stmts, rest, needs_goto))
    }

    fn find(&self, name: &str) -> Option<&'a ProcInfo> {
        self.procs.iter().find(|p| p.name == name)
    }

    fn explode_call(
        &mut self,
        to: &str,
        args: Vec<Expr>,
        next: &str,
        loc: Loc,
        at: usize,
    ) -> Result<Vec<Stmt>, Fail> {
        let Some(info) = self.find(to) else {
            return Err(Fail::at(
                format!(
                    "Call of non-existent procedure {to}\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        };
        let mut ass = vec![SingleAssign {
            var: "stack".into(),
            sub: Expr::empty(),
            rhs: frame(to, &quoted(next), info, "stack"),
            loc,
        }];
        if info.params.len() != args.len() {
            return Err(Fail::at(
                format!(
                    "Procedure {to} called with wrong number of arguments\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        }
        for (decl, arg) in info.params.iter().zip(args) {
            ass.push(SingleAssign {
                var: decl.var.clone(),
                sub: Expr::empty(),
                rhs: arg,
                loc,
            });
        }
        let mut result = vec![Stmt::new(Kind::Assign(ass), loc, at)];
        for decl in &info.decls {
            result.push(assign(&decl.var, decl.val.clone(), loc, at));
        }
        result.push(update_pc(&info.initial_pc));
        Ok(result)
    }

    fn explode_return(
        &mut self,
        from: Option<String>,
        loc: Loc,
        at: usize,
    ) -> Result<Vec<Stmt>, Fail> {
        let Some(from) = from.or_else(|| self.current_procedure.clone()) else {
            return Err(Fail::at(
                format!(
                    "`return' statement not in procedure\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        };
        let Some(info) = self.find(&from) else {
            return Err(Fail::at(
                format!(
                    "Error in procedure (perhaps name used elsewhere)\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        };
        let mut result = vec![assign("pc", head_field("pc"), loc, at)];
        for decl in info.decls.iter().chain(&info.params) {
            result.push(assign(&decl.var, head_field(&decl.var), loc, at));
        }
        result.push(assign(
            "stack",
            Expr::parse("Tail(stack)", &["stack"]),
            loc,
            at,
        ));
        Ok(result)
    }

    fn explode_call_return(
        &mut self,
        to: &str,
        from: Option<String>,
        args: Vec<Expr>,
        loc: Loc,
        at: usize,
    ) -> Result<Vec<Stmt>, Fail> {
        let Some(from) = from else {
            return Err(Fail::at(
                format!(
                    "`return' statement following `call' at {} not in a procedure",
                    loc.describe()
                ),
                at,
            ));
        };
        let Some(from_info) = self.find(&from) else {
            return Err(Fail::at(
                format!(
                    "Error in procedure (perhaps name used elsewhere)\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        };
        let Some(to_info) = self.find(to) else {
            return Err(Fail::at(
                format!(
                    "Call of non-existent procedure {to}\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        };
        let mut ass = Vec::new();
        if from != to {
            for decl in &from_info.decls {
                ass.push(SingleAssign {
                    var: decl.var.clone(),
                    sub: Expr::empty(),
                    rhs: head_field(&decl.var),
                    loc,
                });
            }
            ass.push(SingleAssign {
                var: "stack".into(),
                sub: Expr::empty(),
                rhs: frame(to, "Head(stack).pc", to_info, "Tail(stack)"),
                loc,
            });
        }
        if to_info.params.len() != args.len() {
            return Err(Fail::at(
                format!(
                    "Procedure {to} called with wrong number of arguments\n    at {}",
                    loc.describe()
                ),
                at,
            ));
        }
        for (decl, arg) in to_info.params.iter().zip(args) {
            ass.push(SingleAssign {
                var: decl.var.clone(),
                sub: Expr::empty(),
                rhs: arg,
                loc,
            });
        }
        let mut result = vec![Stmt::new(Kind::Assign(ass), loc, at)];
        for decl in &to_info.decls {
            result.push(assign(&decl.var, decl.val.clone(), loc, at));
        }
        result.push(update_pc(&to_info.initial_pc));
        Ok(result)
    }
}
