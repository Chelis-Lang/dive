//! Observable shared original-tree mutations of `PcalTranslate.Explode`.
//! Labelled entry vectors are shallow copies; nested compound fields remain
//! shared. Empty `CopyAndExplodeLastStmt` inputs produce fresh vectors.

use super::ast::{Algorithm, Body, Kind, LabeledStmt, Loc, Stmt};
use super::{Fail, explode, macros};
use crate::Code;

/// `ClassifyStmtSeq` allocates fresh If/Either nodes. Preserve grouped label
/// locations while making the semantic node coordinates explicit before a
/// generated If/Either can inherit its LabelIf/LabelEither coordinates.
pub(super) fn classified_locations(algorithm: &mut Algorithm) {
    let mut pending: Vec<&mut Stmt> = Vec::new();
    for procedure in &mut algorithm.prcds {
        pending.extend(procedure.body.iter_mut().flat_map(|l| &mut l.stmts));
    }
    match &mut algorithm.body {
        Body::Uni { body, .. } => pending.extend(body.iter_mut().flat_map(|l| &mut l.stmts)),
        Body::Multi(processes) => pending.extend(
            processes
                .iter_mut()
                .flat_map(|p| p.body.iter_mut().flat_map(|l| &mut l.stmts)),
        ),
    }
    while let Some(stmt) = pending.pop() {
        if matches!(stmt.kind, Kind::If { .. } | Kind::Either(_)) {
            stmt.loc = Loc::default();
        }
        match &mut stmt.kind {
            Kind::If { then, els, .. } => {
                pending.extend(then);
                pending.extend(els);
            }
            Kind::LabelIf {
                unlab_then,
                lab_then,
                unlab_else,
                lab_else,
                ..
            } => {
                pending.extend(unlab_then);
                pending.extend(unlab_else);
                pending.extend(lab_then.iter_mut().flat_map(|l| &mut l.stmts));
                pending.extend(lab_else.iter_mut().flat_map(|l| &mut l.stmts));
            }
            Kind::Either(ors) => pending.extend(ors.iter_mut().flatten()),
            Kind::LabelEither(clauses) => {
                for clause in clauses {
                    pending.extend(&mut clause.unlab);
                    pending.extend(clause.lab.iter_mut().flat_map(|l| &mut l.stmts));
                }
            }
            Kind::While { unlab, lab, .. } => {
                pending.extend(unlab);
                pending.extend(lab.iter_mut().flat_map(|l| &mut l.stmts));
            }
            Kind::With { body, .. } => pending.extend(body),
            _ => {}
        }
    }
}

/// The measured procedural positive has copied labelled entry vectors. Keep
/// unported procedural compound/process/ordering ownership explicit, and bind
/// every flat call to an actual declared parameter vector before serialization.
fn flat_procedures(algorithm: &Algorithm) -> Result<bool, Fail> {
    if algorithm.prcds.is_empty() {
        return Ok(false);
    }
    let Body::Uni { body, .. } = &algorithm.body else {
        return Err(pending(0, "process-owned procedure header ownership"));
    };
    if !algorithm.facts.added_labels.is_empty() {
        return Err(pending(0, "automatic procedure label ownership"));
    }
    for procedure in &algorithm.prcds {
        if !procedure.plus_labels.is_empty() || !procedure.minus_labels.is_empty() {
            return Err(pending(0, "procedure fairness-label header ownership"));
        }
    }
    for (owner, labels) in algorithm
        .prcds
        .iter()
        .map(|p| (Some(p.name.as_str()), p.body.as_slice()))
        .chain(std::iter::once((None, body.as_slice())))
    {
        for stmt in labels.iter().flat_map(|l| &l.stmts) {
            let call = match &stmt.kind {
                Kind::Call { to, args } | Kind::CallGoto { to, args, .. } => Some((to, args)),
                Kind::CallReturn { to, from, args } => {
                    if from.as_deref() != owner || owner.is_none() {
                        return Err(pending(stmt.at, "return outside procedure ownership"));
                    }
                    Some((to, args))
                }
                Kind::Return { from } => {
                    if from.as_deref() != owner || owner.is_none() {
                        return Err(pending(stmt.at, "return outside procedure ownership"));
                    }
                    None
                }
                Kind::Assign(assignments) if assignments.len() > 1 => {
                    return Err(pending(stmt.at, "simultaneous-assignment ordering"));
                }
                Kind::If { .. }
                | Kind::LabelIf { .. }
                | Kind::Either(_)
                | Kind::LabelEither(_)
                | Kind::While { .. }
                | Kind::With { .. } => {
                    return Err(pending(stmt.at, "compound procedural transfer ownership"));
                }
                _ => None,
            };
            if let Some((to, args)) = call {
                let Some(target) = algorithm.prcds.iter().find(|p| p.name == *to) else {
                    return Err(pending(stmt.at, "unresolved procedure call ownership"));
                };
                if target.params.len() != args.len() {
                    return Err(pending(stmt.at, "procedure argument-count ownership"));
                }
            }
        }
    }
    Ok(true)
}
fn pending(at: usize, feature: &str) -> Fail {
    Fail::at(format!("unsupported debug AST checksum: {feature}"), at).code(Code::Unsupported)
}

pub(super) fn mutate(algorithm: &mut Algorithm, budget: &mut macros::Budget) -> Result<(), Fail> {
    let flat_procedures = flat_procedures(algorithm)?;
    let mut walker = Walker {
        flat_procedures,
        omit_pc: algorithm.facts.omit_pc,
        budget,
    };
    for procedure in &mut algorithm.prcds {
        walker.labeled(&mut procedure.body, "Error")?;
    }
    match &mut algorithm.body {
        Body::Uni { body, .. } => walker.labeled(body, "Done")?,
        Body::Multi(processes) => {
            for process in processes {
                walker.labeled(&mut process.body, "Done")?;
            }
        }
    }
    Ok(())
}
struct Walker<'a> {
    flat_procedures: bool,
    omit_pc: bool,
    budget: &'a mut macros::Budget,
}
impl Walker<'_> {
    fn update(&mut self, next: &str, at: usize) -> Result<Stmt, Fail> {
        // Reserve the complete generated assignment before constructing tokens.
        self.budget
            .reserve_header(3, 1, next.len().saturating_add(20), at)?;
        Ok(explode::update_pc(next))
    }
    fn labeled(&mut self, labels: &mut [LabeledStmt], next: &str) -> Result<(), Fail> {
        for index in 0..labels.len() {
            let (current, after) = labels.split_at_mut(index + 1);
            let label = &mut current[index];
            let continuation = after.first().map_or(next, |l| l.label.as_str());
            // ExplodeWhile copies unlabDo and the post-while tail separately.
            if let Some(Stmt {
                kind: Kind::While { unlab, lab, .. },
                ..
            }) = label.stmts.first_mut()
            {
                let unlab_next = lab
                    .first()
                    .map_or(label.label.as_str(), |l| l.label.as_str());
                self.sequence(unlab, unlab_next, false)?;
                self.labeled(lab, &label.label)?;
                self.sequence(&mut label.stmts[1..], continuation, false)?;
            } else {
                self.sequence(&mut label.stmts, continuation, false)?;
            }
        }
        Ok(())
    }
    /// A fresh result for an empty input is not assigned back to the original
    /// LabelIf/Clause field. It is only visible if a shared parent replaces the
    /// labelled node by its generated If/Either.
    fn with_goto(&mut self, stmts: &mut Vec<Stmt>, next: &str) -> Result<Option<Vec<Stmt>>, Fail> {
        let empty = stmts.is_empty();
        let needs = self.sequence(stmts, next, true)?;
        if !needs {
            return Ok(None);
        }
        let update = self.update(next, stmts.last().map_or(0, |s| s.at))?;
        if empty {
            Ok(Some(vec![update]))
        } else {
            stmts.push(update);
            Ok(None)
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one shared-vector rule per CopyAndExplodeLastStmt semantic class"
    )]
    fn sequence(&mut self, stmts: &mut [Stmt], next: &str, shared: bool) -> Result<bool, Fail> {
        let Some(last) = stmts.last_mut() else {
            return Ok(!self.omit_pc);
        };
        let at = last.at;
        let mut needs = true;
        let replacement = match &mut last.kind {
            Kind::LabelIf {
                test,
                unlab_then,
                lab_then,
                unlab_else,
                lab_else,
            } => {
                let then_next = lab_then.first().map_or(next, |l| l.label.as_str());
                let else_next = lab_else.first().map_or(next, |l| l.label.as_str());
                let fresh_then = self.with_goto(unlab_then, then_next)?;
                let fresh_else = self.with_goto(unlab_else, else_next)?;
                self.labeled(lab_then, next)?;
                self.labeled(lab_else, next)?;
                needs = false;
                if shared {
                    // This node is removed from a shared source vector, so its
                    // owned fields can move into the new semantic If node.
                    Some(Kind::If {
                        test: std::mem::take(test),
                        then: fresh_then.unwrap_or_else(|| std::mem::take(unlab_then)),
                        els: fresh_else.unwrap_or_else(|| std::mem::take(unlab_else)),
                    })
                } else {
                    None
                }
            }
            Kind::LabelEither(clauses) => {
                let mut generated = Vec::new();
                for clause in clauses {
                    let clause_next = clause.lab.first().map_or(next, |l| l.label.as_str());
                    let fresh = self.with_goto(&mut clause.unlab, clause_next)?;
                    self.labeled(&mut clause.lab, next)?;
                    if shared {
                        generated.push(fresh.unwrap_or_else(|| std::mem::take(&mut clause.unlab)));
                    }
                }
                needs = false;
                if shared {
                    Some(Kind::Either(generated))
                } else {
                    None
                }
            }
            Kind::Goto(target) => {
                needs = false;
                if shared {
                    let replacement = self.update(target, at)?;
                    last.loc = replacement.loc;
                    Some(replacement.kind)
                } else {
                    None
                }
            }
            Kind::If { then, els, .. } => {
                let then_needs = self.sequence(then, next, true)?;
                let else_needs = self.sequence(els, next, true)?;
                needs = then_needs && else_needs;
                if !self.omit_pc && !needs {
                    if then_needs {
                        then.push(self.update(next, at)?);
                    }
                    if else_needs {
                        els.push(self.update(next, at)?);
                    }
                }
                None
            }
            Kind::Either(ors) => {
                let mut flags = Vec::new();
                for clause in ors.iter_mut() {
                    flags.push(self.sequence(clause, next, true)?);
                }
                needs = flags.iter().all(|flag| *flag);
                if !self.omit_pc && !needs {
                    for (clause, flag) in ors.iter_mut().zip(flags) {
                        if flag {
                            clause.push(self.update(next, at)?);
                        }
                    }
                }
                None
            }
            Kind::With { body, .. } => {
                needs = self.sequence(body, next, true)?;
                None
            }
            Kind::Call { .. }
            | Kind::CallReturn { .. }
            | Kind::CallGoto { .. }
            | Kind::Return { .. } => {
                if self.flat_procedures && !shared {
                    needs = false;
                    None
                } else {
                    return Err(pending(at, "procedural transfer ownership"));
                }
            }
            _ => None,
        };
        if let Some(kind) = replacement {
            last.kind = kind;
        }
        Ok(needs && !self.omit_pc)
    }
}
