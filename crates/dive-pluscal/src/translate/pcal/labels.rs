//! Label insertion and step grouping (`ParseAlgorithm.AddLabelsToStmtSeq`,
//! `MakeLabeledStmtSeq`, `FixStmtSeq`, `ClassifyStmtSeq` and `checkBody`),
//! and the order in which `getAlgorithm` applies them.

use super::Fail;
use super::ast::{Algorithm, Body, Clause, Facts, Kind, LabeledStmt, Stmt, VarDecl};
use super::macros;
use crate::Code;

/// Settings that decide label insertion.
pub(crate) struct LabelOptions<'a> {
    /// `-label`: insert missing labels even if the algorithm has labels.
    pub label: bool,
    /// `-labelRoot`.
    pub root: &'a str,
}

struct Ctx<'a> {
    facts: &'a mut Facts,
    options: &'a LabelOptions<'a>,
    /// `ParseAlgorithm.procedures`: every procedure's parameters and locals.
    procedure_vars: Vec<String>,
    current_procedure: Option<String>,
}

/// Expands macros and inserts labels, then groups statements into labelled
/// steps, in `getAlgorithm` order.
pub(crate) fn normalize(
    alg: &mut Algorithm,
    options: &LabelOptions<'_>,
    budget: &mut macros::Budget,
) -> Result<(), Fail> {
    check_duplicate_macros(alg)?;
    let procedure_vars: Vec<String> = alg
        .prcds
        .iter()
        .flat_map(|p| {
            p.params
                .iter()
                .chain(&p.decls)
                .map(|d: &VarDecl| d.var.clone())
        })
        .collect();
    let macros = std::mem::take(&mut alg.macros);
    let mut facts = std::mem::take(&mut alg.facts);
    let result = (|| {
        let mut ctx = Ctx {
            facts: &mut facts,
            options,
            procedure_vars,
            current_procedure: None,
        };
        match &mut alg.body {
            Body::Multi(processes) => {
                let mut omit_stutter = false;
                for process in processes.iter_mut() {
                    macros::expand_in(&mut process.stmts, &macros, budget)?;
                    ctx.add_labels(&mut process.stmts)?;
                    process.body = ctx.make_labeled(std::mem::take(&mut process.stmts))?;
                    ctx.facts.omit_stuttering_when_done = true;
                    check_body(ctx.facts, &process.body);
                    omit_stutter = omit_stutter || ctx.facts.omit_stuttering_when_done;
                }
                ctx.facts.omit_stuttering_when_done = omit_stutter;
                for procedure in &mut alg.prcds {
                    ctx.current_procedure = Some(procedure.name.clone());
                    macros::expand_in(&mut procedure.stmts, &macros, budget)?;
                    ctx.add_labels(&mut procedure.stmts)?;
                    procedure.body = ctx.make_labeled(std::mem::take(&mut procedure.stmts))?;
                }
                if !ctx.facts.added_labels.is_empty() && !ctx.options.label {
                    return Err(added_labels_error(ctx.facts));
                }
                if ctx.facts.goto_done_used {
                    ctx.facts.omit_pc = false;
                    ctx.facts.omit_stuttering_when_done = false;
                }
                if ctx.facts.goto_used {
                    ctx.facts.omit_pc = false;
                }
            }
            Body::Uni { stmts, body } => {
                macros::expand_in(stmts, &macros, budget)?;
                for procedure in &mut alg.prcds {
                    ctx.current_procedure = Some(procedure.name.clone());
                    macros::expand_in(&mut procedure.stmts, &macros, budget)?;
                    ctx.add_labels(&mut procedure.stmts)?;
                    procedure.body = ctx.make_labeled(std::mem::take(&mut procedure.stmts))?;
                }
                // The last procedure stays current for the main body, as in
                // pcal.trans.
                ctx.add_labels(stmts)?;
                *body = ctx.make_labeled(std::mem::take(stmts))?;
                if !ctx.facts.added_labels.is_empty() && ctx.facts.has_label && !ctx.options.label {
                    return Err(added_labels_error(ctx.facts));
                }
                if ctx.facts.goto_used {
                    ctx.facts.omit_pc = false;
                }
                if ctx.facts.goto_done_used {
                    ctx.facts.omit_pc = false;
                    ctx.facts.omit_stuttering_when_done = false;
                } else {
                    check_body(ctx.facts, body);
                }
            }
        }
        Ok(())
    })();
    alg.macros = macros;
    alg.facts = facts;
    result
}

fn check_duplicate_macros(alg: &Algorithm) -> Result<(), Fail> {
    for (i, m) in alg.macros.iter().enumerate() {
        if alg.macros[i + 1..].iter().any(|n| n.name == m.name) {
            return Err(Fail::new(format!(
                "Multiple definitions of macro name `{}'",
                m.name
            )));
        }
    }
    Ok(())
}

fn added_labels_error(facts: &Facts) -> Fail {
    let mut msg = if facts.added_labels.len() > 1 {
        "Missing labels at the following locations:".to_owned()
    } else {
        "Missing label at the following location:".to_owned()
    };
    for (_, loc, _) in &facts.added_labels {
        msg.push_str("\n     ");
        msg.push_str(&loc.describe());
    }
    let at = facts.added_labels.first().map_or(0, |(_, _, at)| *at);
    Fail::at(msg, at).code(Code::Label)
}

/// `checkBody`: the pc and the stuttering disjunct can be omitted only for a
/// body that is a single `while TRUE` loop.
fn check_body(facts: &mut Facts, body: &[LabeledStmt]) {
    let Some(first) = body.first() else {
        return;
    };
    if body.len() > 1 {
        facts.omit_pc = false;
        facts.omit_stuttering_when_done = false;
        return;
    }
    if first.stmts.is_empty() {
        return;
    }
    let single_true_loop = match (&first.stmts[..], first.stmts.first().map(|s| &s.kind)) {
        ([_], Some(Kind::While { test, .. })) => {
            test.lines.len() == 1 && test.lines[0].len() == 1 && test.lines[0][0].text == "TRUE"
        }
        _ => false,
    };
    if !single_true_loop {
        facts.omit_pc = false;
        facts.omit_stuttering_when_done = false;
        return;
    }
    if let Some(Kind::While { unlab, lab, .. }) = first.stmts.first().map(|s| &s.kind) {
        if !lab.is_empty() {
            facts.omit_pc = false;
        }
        if unlab
            .iter()
            .any(|s| matches!(s.kind, Kind::LabelIf { .. } | Kind::LabelEither(_)))
        {
            facts.omit_pc = false;
        }
    }
}

fn disjoint(a: &[String], b: &[String]) -> bool {
    !a.iter().any(|x| b.contains(x))
}

fn union(a: &[String], b: &[String]) -> Vec<String> {
    let mut out = a.to_vec();
    for x in b {
        if !out.contains(x) {
            out.push(x.clone());
        }
    }
    out
}

impl Ctx<'_> {
    fn needs_label(&mut self, stmt: &mut Stmt) {
        if !stmt.lbl.is_empty() {
            return;
        }
        let mut label = format!("{}{}", self.options.root, self.facts.next_label_num);
        self.facts.next_label_num += 1;
        while self.facts.all_labels.contains(&label) {
            label = format!("{}{}", self.options.root, self.facts.next_label_num);
            self.facts.next_label_num += 1;
        }
        stmt.lbl.clone_from(&label);
        self.facts.added_labels.push((label, stmt.loc, stmt.at));
    }

    fn add_labels(&mut self, stmts: &mut [Stmt]) -> Result<(), Fail> {
        let mut out = Vec::new();
        self.inner(stmts, true, false, &[], &mut out)?;
        Ok(())
    }

    /// `InnerAddLabels`.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per statement class, mirroring InnerAddLabels"
    )]
    fn inner(
        &mut self,
        stmts: &mut [Stmt],
        first_labeled: bool,
        in_with: bool,
        in_assigned: &[String],
        out_assigned: &mut Vec<String>,
    ) -> Result<bool, Fail> {
        *out_assigned = in_assigned.to_vec();
        let mut next_needs_label = first_labeled;
        let mut had_or_added = false;
        for stmt in stmts.iter_mut() {
            if !stmt.lbl.is_empty() {
                had_or_added = true;
                out_assigned.clear();
                if in_with {
                    return Err(Fail::at(
                        format!("Label in `with' statement\n    at {}", stmt.loc.describe()),
                        stmt.at,
                    )
                    .code(Code::Label));
                }
            }
            if next_needs_label {
                if in_with {
                    return Err(Fail::at(
                        format!(
                            "Statement follows `call' or `return' inside a `with' statement.\n    at {}",
                            stmt.loc.describe()
                        ),
                        stmt.at,
                    ).code(Code::Label));
                }
                self.needs_label(stmt);
                next_needs_label = false;
                had_or_added = true;
                out_assigned.clear();
            }
            let snapshot = out_assigned.clone();
            match &mut stmt.kind {
                Kind::LabelIf {
                    unlab_then,
                    unlab_else,
                    ..
                } => {
                    let mut then_assigned = Vec::new();
                    let mut else_assigned = Vec::new();
                    let a =
                        self.inner(unlab_then, false, in_with, &snapshot, &mut then_assigned)?;
                    let b =
                        self.inner(unlab_else, false, in_with, &snapshot, &mut else_assigned)?;
                    next_needs_label = b || a;
                    *out_assigned = union(&then_assigned, &else_assigned);
                }
                Kind::LabelEither(clauses) => {
                    let mut new_out = Vec::new();
                    for clause in clauses.iter_mut() {
                        let mut or_assigned = Vec::new();
                        let r = self.inner(
                            &mut clause.unlab,
                            false,
                            in_with,
                            &snapshot,
                            &mut or_assigned,
                        )?;
                        next_needs_label = r || next_needs_label;
                        new_out = union(&or_assigned, &new_out);
                    }
                    *out_assigned = new_out;
                }
                Kind::While { unlab, .. } => {
                    if in_with {
                        return Err(Fail::at(
                            format!(
                                "`while' inside a `with' statement\n    at {}",
                                stmt.loc.describe()
                            ),
                            stmt.at,
                        )
                        .code(Code::Label));
                    }
                    // Label the while itself (needs a split borrow).
                    let mut body = std::mem::take(unlab);
                    self.needs_label(stmt);
                    had_or_added = true;
                    out_assigned.clear();
                    let mut new_out = Vec::new();
                    self.inner(&mut body, false, false, &[], &mut new_out)?;
                    if let Kind::While { unlab, .. } = &mut stmt.kind {
                        *unlab = body;
                    }
                    next_needs_label = false;
                }
                Kind::With { body, .. } => {
                    // pcal.trans compares with the sequence's *incoming*
                    // assignments, not the running set.
                    let new_in = if in_with {
                        in_assigned.to_vec()
                    } else {
                        Vec::new()
                    };
                    let mut new_out = Vec::new();
                    next_needs_label = self.inner(body, false, true, &new_in, &mut new_out)?;
                    *out_assigned = new_out;
                    if !in_with && !disjoint(in_assigned, out_assigned) {
                        self.needs_label(stmt);
                        had_or_added = true;
                        out_assigned.clear();
                    }
                }
                Kind::Assign(ass) => {
                    let mut vars: Vec<String> = Vec::new();
                    for a in ass.iter() {
                        if !vars.contains(&a.var) {
                            vars.push(a.var.clone());
                        }
                    }
                    if disjoint(out_assigned, &vars) {
                        *out_assigned = union(out_assigned, &vars);
                    } else {
                        if in_with {
                            return Err(Fail::at(
                                format!(
                                    "Second assignment to same variable inside a `with' statement\n    at {}",
                                    stmt.loc.describe()
                                ),
                                stmt.at,
                            ).code(Code::Label));
                        }
                        self.needs_label(stmt);
                        had_or_added = true;
                        *out_assigned = vars;
                    }
                    next_needs_label = false;
                }
                kind => {
                    next_needs_label = false;
                    let mut vars = Vec::new();
                    let (is_call_or_return, sets_procedure_vars) = match kind {
                        Kind::Call { to, .. } | Kind::CallGoto { to, .. } => {
                            (true, self.current_procedure.as_deref() == Some(to.as_str()))
                        }
                        Kind::Return { .. } | Kind::CallReturn { .. } => (true, true),
                        Kind::Goto(_) => {
                            next_needs_label = true;
                            (false, false)
                        }
                        _ => (false, false),
                    };
                    if is_call_or_return {
                        next_needs_label = true;
                        vars.push("stack".to_owned());
                        if sets_procedure_vars {
                            vars.extend(self.procedure_vars.iter().cloned());
                        }
                    }
                    if disjoint(out_assigned, &vars) {
                        *out_assigned = union(out_assigned, &vars);
                    } else {
                        if in_with {
                            return Err(Fail::at(
                                format!(
                                    "Call or return makes second assignment to a variable inside a `with' statement\n    at {}",
                                    stmt.loc.describe()
                                ),
                                stmt.at,
                            ).code(Code::Label));
                        }
                        self.needs_label(stmt);
                        had_or_added = true;
                        *out_assigned = vars;
                    }
                }
            }
        }
        Ok(had_or_added || next_needs_label)
    }

    /// `MakeLabeledStmtSeq`: groups statements into labelled steps and checks
    /// where calls, returns, gotos and labels may appear.
    fn make_labeled(&mut self, stmts: Vec<Stmt>) -> Result<Vec<LabeledStmt>, Fail> {
        let mut result = make_inner(stmts)?;
        for labeled in &mut result {
            self.classify_seq(&mut labeled.stmts)?;
        }
        Ok(result)
    }

    fn check_labeled(&self, seq: &mut [LabeledStmt]) -> Result<(), Fail> {
        for labeled in seq {
            self.classify_seq(&mut labeled.stmts)?;
        }
        Ok(())
    }

    /// `ClassifyStmtSeq`: 0 = no transfer, 1 = call/return/goto, 2 = labels.
    fn classify_seq(&self, stmts: &mut [Stmt]) -> Result<u8, Fail> {
        let mut result = 0;
        let len = stmts.len();
        for i in 0..len {
            let next_is_return = stmts
                .get(i + 1)
                .is_some_and(|s| matches!(s.kind, Kind::Return { .. }));
            let stmt = &mut stmts[i];
            match &mut stmt.kind {
                Kind::LabelIf { .. } => {
                    result = self.classify_if(stmt)?;
                }
                Kind::LabelEither(_) => {
                    result = self.classify_either(stmt)?;
                }
                Kind::With { body, .. } => {
                    result = self.classify_seq(body)?;
                    if result == 2 {
                        return Err(Fail::at(
                            format!(
                                "with statement at {} contains a labeled statement",
                                stmt.loc.describe()
                            ),
                            stmt.at,
                        )
                        .code(Code::Label));
                    }
                }
                Kind::While { unlab, lab, .. } => {
                    if i != 0 {
                        return Err(Fail::internal(
                            "ClassifyStmtSeq encountered `while' not at beginning of StmtSeq",
                        ));
                    }
                    self.classify_seq(unlab)?;
                    self.check_labeled(lab)?;
                }
                Kind::Call { .. } => {
                    if !next_is_return {
                        result = 1;
                    }
                }
                Kind::Return { .. } | Kind::CallReturn { .. } => {
                    if self.current_procedure.is_none() {
                        return Err(Fail::at(
                            format!(
                                "return statement not in a procedure at {}",
                                stmt.loc.describe()
                            ),
                            stmt.at,
                        )
                        .code(Code::Label));
                    }
                    result = 1;
                }
                Kind::Goto(_) | Kind::CallGoto { .. } => result = 1,
                _ => {}
            }
            if result > 0 && i != len - 1 {
                let next = &stmts[i + 1];
                return Err(Fail::at(
                    format!(
                        "Translator discovered later than it should have  that\n  Statement at {} must have a label",
                        next.loc.describe()
                    ),
                    next.at,
                )
                .code(Code::Label));
            }
        }
        Ok(result)
    }

    fn classify_if(&self, stmt: &mut Stmt) -> Result<u8, Fail> {
        let Kind::LabelIf {
            test,
            unlab_then,
            lab_then,
            unlab_else,
            lab_else,
        } = &mut stmt.kind
        else {
            return Ok(0);
        };
        let then_class = self.classify_seq(unlab_then)?;
        let mut labeled = false;
        if !lab_then.is_empty() {
            labeled = true;
            self.check_labeled(lab_then)?;
        }
        let else_class = self.classify_seq(unlab_else)?;
        if !lab_else.is_empty() {
            labeled = true;
            self.check_labeled(lab_else)?;
        }
        if labeled || then_class == 2 || else_class == 2 {
            return Ok(2);
        }
        let class = u8::from(then_class + else_class > 0);
        stmt.kind = Kind::If {
            test: std::mem::take(test),
            then: std::mem::take(unlab_then),
            els: std::mem::take(unlab_else),
        };
        Ok(class)
    }

    fn classify_either(&self, stmt: &mut Stmt) -> Result<u8, Fail> {
        let Kind::LabelEither(clauses) = &mut stmt.kind else {
            return Ok(0);
        };
        let mut class = 0;
        for clause in clauses.iter_mut() {
            let unlab = self.classify_seq(&mut clause.unlab)?;
            class = class.max(unlab);
            if !clause.lab.is_empty() {
                class = 2;
                self.check_labeled(&mut clause.lab)?;
            }
        }
        if class < 2 {
            let ors = clauses
                .iter_mut()
                .map(|c| std::mem::take(&mut c.unlab))
                .collect();
            stmt.kind = Kind::Either(ors);
        }
        Ok(class)
    }
}

/// `InnerMakeLabeledStmtSeq`.
fn make_inner(stmts: Vec<Stmt>) -> Result<Vec<LabeledStmt>, Fail> {
    let mut result: Vec<LabeledStmt> = Vec::new();
    for mut stmt in stmts {
        if stmt.lbl.is_empty() {
            match result.last_mut() {
                Some(current) => {
                    fix(&mut stmt)?;
                    current.stmts.push(stmt);
                    continue;
                }
                None => {
                    return Err(Fail::internal(
                        "InnerMakeLabeledStmtSeq found null label starting labeled stmt seq",
                    ));
                }
            }
        }
        let label = std::mem::take(&mut stmt.lbl);
        let loc = stmt.loc;
        fix(&mut stmt)?;
        result.push(LabeledStmt {
            label,
            stmts: vec![stmt],
            loc,
        });
    }
    Ok(result)
}

/// Splits the unlabelled prefix of `seq` from its labelled remainder.
fn split(seq: &mut Vec<Stmt>) -> Result<Vec<LabeledStmt>, Fail> {
    let at = seq
        .iter()
        .position(|s| !s.lbl.is_empty())
        .unwrap_or(seq.len());
    let rest = seq.split_off(at);
    for stmt in seq.iter_mut() {
        fix(stmt)?;
    }
    make_inner(rest)
}

/// `FixStmtSeq` for one statement.
fn fix(stmt: &mut Stmt) -> Result<(), Fail> {
    match &mut stmt.kind {
        Kind::LabelIf {
            unlab_then,
            lab_then,
            unlab_else,
            lab_else,
            ..
        } => {
            *lab_then = split(unlab_then)?;
            *lab_else = split(unlab_else)?;
        }
        Kind::LabelEither(clauses) => {
            for Clause { unlab, lab } in clauses.iter_mut() {
                *lab = split(unlab)?;
            }
        }
        Kind::While { unlab, lab, .. } => {
            *lab = split(unlab)?;
        }
        _ => {}
    }
    Ok(())
}
