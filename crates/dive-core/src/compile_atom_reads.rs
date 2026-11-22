//! Source projections for bound predicates on materialized graph rows.
use super::{Compiler, G, GNode, Node, P};
use crate::plan::Unch;

#[derive(Default)]
struct Writes {
    assigned: u64,
    copied: u64,
    reads: u64,
}

impl Compiler<'_> {
    pub(super) fn atom_source_reads(&self, p: P) -> Option<u64> {
        self.atom_reads_within(p, &mut 1024)
    }

    fn atom_reads_within(&self, p: P, work: &mut usize) -> Option<u64> {
        *work = work.checked_sub(1)?;
        let info = self.info.get(p as usize)?;
        if info.primed != 0 || info.effect {
            return None;
        }
        if !info.calls {
            return Some(info.vars);
        }
        match self.code.nodes.get(p as usize)? {
            Node::Not(body) | Node::Scope { body, .. } | Node::Lazy { body, .. } => {
                self.atom_reads_within(*body, work)
            }
            Node::And(parts) => parts.iter().try_fold(0, |mask, &part| {
                self.atom_reads_within(part, work).map(|other| mask | other)
            }),
            Node::Enabled { free, .. } if free.iter().all(|&slot| self.preset_slot(slot)) => {
                self.enabled_reads_within(p, work)
            }
            _ => None,
        }
    }

    /// Search a literal prefix of this generator. Refuting it means no
    /// continuation could run, so neither assignments nor the changed
    /// subscript would have been demanded in the original search.
    pub(super) fn enabled_prefix(&mut self, p: P) -> Result<Option<(P, u64)>, super::Error> {
        if self.m.heap.store().semantics().tlc_comparisons
            || self.code.nodes.len() + super::MAX_GUARD_DEPTH + 2 >= super::MAX_NODES
            || self.code.gnodes.len() + super::MAX_GUARD_DEPTH + 2 >= super::MAX_NODES
        {
            return Ok(None);
        }
        let Some(Node::Enabled { g, free }) = self.code.nodes.get(p as usize).cloned() else {
            return Ok(None);
        };
        let action = match self.code.gnodes.get(g as usize) {
            Some(GNode::StepAngle { action, .. }) => *action,
            _ => g,
        };
        let Some((prefix, reads)) = self.leading_source(action, 0) else {
            return Ok(None);
        };
        let predicate = self.push(Node::Enabled { g: prefix, free })?;
        Ok(Some((predicate, reads)))
    }

    fn leading_source(&mut self, g: G, depth: usize) -> Option<(G, u64)> {
        if depth >= super::MAX_GUARD_DEPTH {
            return None;
        }
        match self.code.gnodes.get(g as usize)?.clone() {
            GNode::Scope { body, reset } => {
                let (body, reads) = self.leading_source(body, depth + 1)?;
                Some((self.gpush(GNode::Scope { body, reset }), reads))
            }
            GNode::Conj(items) => {
                let mut reads = 0;
                let mut count = 0;
                let mut work = 1024;
                for &(generator, predicate) in &items {
                    let Some(mask) = self
                        .guard_generator_reads(generator, &mut work)
                        .zip(self.atom_source_reads(predicate))
                        .map(|(g, p)| g | p)
                    else {
                        break;
                    };
                    reads |= mask;
                    count += 1;
                }
                if count == 0 || count == items.len() {
                    return None;
                }
                Some((self.gpush(GNode::Conj(items[..count].into())), reads))
            }
            _ => None,
        }
    }

    /// A generator may have different instructions from its boolean form.
    /// Include the actual source reads on both paths, and admit no generator
    /// that can bind a destination or inspect a partial primed row.
    fn guard_generator_reads(&self, g: G, work: &mut usize) -> Option<u64> {
        *work = work.checked_sub(1)?;
        match self.code.gnodes.get(g as usize)? {
            GNode::Guard(p) | GNode::LabelGuard(p, _) => self.atom_reads_within(*p, work),
            GNode::Scope { body, .. } => self.guard_generator_reads(*body, work),
            GNode::Conj(items) | GNode::LabelArm(items, _) => {
                items.iter().try_fold(0, |mask, &(g, p)| {
                    Some(
                        mask | self.guard_generator_reads(g, work)?
                            | self.atom_reads_within(p, work)?,
                    )
                })
            }
            GNode::Disj(items) => items.iter().try_fold(0, |mask, &g| {
                self.guard_generator_reads(g, work)
                    .map(|other| mask | other)
            }),
            GNode::Exists {
                binds,
                body,
                body_p,
            }
            | GNode::Forall {
                binds,
                body,
                body_p,
            } => {
                let mask = self.guard_generator_reads(*body, work)?
                    | self.atom_reads_within(*body_p, work)?;
                binds.iter().try_fold(mask, |mask, b| {
                    self.atom_reads_within(b.domain, work)
                        .map(|domain| mask | domain)
                })
            }
            GNode::If { cond, then, other } => Some(
                self.atom_reads_within(*cond, work)?
                    | self.guard_generator_reads(*then, work)?
                    | self.guard_generator_reads(*other, work)?,
            ),
            _ => None,
        }
    }

    /// A straight-line generator binds each destination at most once. A
    /// variable copied by UNCHANGED need not be in the truth key unless its
    /// source value is read elsewhere. On materialized rows its contribution
    /// to a direct nonstuttering comparison is always equality. Leave other
    /// generators, substitutions with forced subscripts, and TLC's
    /// shape-sensitive comparisons to the ordinary query.
    pub(super) fn enabled_atom_reads(&self, p: P) -> Option<u64> {
        self.enabled_reads_within(p, &mut 1024)
    }

    fn enabled_reads_within(&self, p: P, work: &mut usize) -> Option<u64> {
        if self.m.heap.store().semantics().tlc_comparisons {
            return None;
        }
        let Node::Enabled { g, .. } = self.code.nodes.get(p as usize)? else {
            return None;
        };
        let (action, subscript) = match self.code.gnodes.get(*g as usize)? {
            GNode::StepAngle { action, same } => {
                let Node::SameVars { leaves, thunks, .. } = self.code.nodes.get(*same as usize)?
                else {
                    return None;
                };
                if thunks.iter().any(|&slot| self.preset_slot(slot)) {
                    return None;
                }
                (
                    *action,
                    leaves.iter().fold(0, |mask, &(v, _)| mask | (1u64 << v)),
                )
            }
            _ => (*g, 0),
        };
        let mut writes = Writes::default();
        let width = u32::try_from(self.ir.vars().len()).ok()?;
        let all = u64::MAX.checked_shr(64u32.checked_sub(width)?).unwrap_or(0);
        self.atom_writes(action, all, &mut writes, 0, work)?;
        // A partial subscript could have unassigned variables. Preserve the
        // ordinary search and any error it raises on those inputs.
        ((writes.assigned | writes.copied) & subscript == subscript)
            .then_some(writes.reads | (subscript & !writes.copied))
    }

    fn atom_writes(
        &self,
        g: G,
        all: u64,
        w: &mut Writes,
        depth: usize,
        work: &mut usize,
    ) -> Option<()> {
        *work = work.checked_sub(1)?;
        if depth > super::MAX_GUARD_DEPTH || w.assigned | w.copied == all {
            // A later conjunction item can switch to its boolean form once
            // the destination is complete. This proof covers generator mode.
            return None;
        }
        match self.code.gnodes.get(g as usize)? {
            GNode::Conj(items) => {
                for &(item, _) in items {
                    self.atom_writes(item, all, w, depth + 1, work)?;
                }
            }
            GNode::Scope { body, .. } => self.atom_writes(*body, all, w, depth + 1, work)?,
            GNode::Guard(p) | GNode::LabelGuard(p, _) => {
                w.reads |= self.atom_reads_within(*p, work)?;
            }
            GNode::Assign { var, rhs } => {
                let bit = 1u64 << var;
                if (w.assigned | w.copied) & bit != 0 {
                    return None;
                }
                w.reads |= self.atom_reads_within(*rhs, work)?;
                w.assigned |= bit;
            }
            GNode::Unchanged(items) => {
                for item in items {
                    let Unch::Var { var, .. } = item else {
                        return None;
                    };
                    let bit = 1u64 << var;
                    if (w.assigned | w.copied) & bit != 0 {
                        return None;
                    }
                    w.copied |= bit;
                }
            }
            _ => return None,
        }
        Some(())
    }
}

#[derive(Default)]
struct AssignmentProof {
    written: u64,
    guard_update: bool,
    different_boolean_guard: bool,
    suffix_update: bool,
}

impl AssignmentProof {
    fn claim(&mut self, var: u16) -> Option<()> {
        let bit = 1u64.checked_shl(u32::from(var))?;
        if self.written & bit != 0 {
            return None;
        }
        self.written |= bit;
        Some(())
    }
}

impl Compiler<'_> {
    pub(super) fn closed_enabled(&self, p: P) -> Option<super::ClosedEnabled> {
        if self.m.heap.store().semantics().tlc_comparisons {
            return None;
        }
        self.closed_query(p, 0, &mut 8192)
    }

    fn closed_query(&self, p: P, depth: usize, work: &mut usize) -> Option<super::ClosedEnabled> {
        let Node::Enabled { g, .. } = self.code.nodes.get(p as usize)? else {
            return None;
        };
        let (action, subscript) = match self.code.gnodes.get(*g as usize)? {
            GNode::StepAngle { action, same } => {
                let mask = match self.code.nodes.get(*same as usize)? {
                    Node::SameVars { leaves, thunks, .. }
                        if thunks.iter().all(|&s| !self.preset_slot(s)) =>
                    {
                        leaves.iter().fold(0, |mask, &(v, _)| mask | (1u64 << v))
                    }
                    Node::Eq(before, after) => {
                        match (
                            &self.code.nodes[*before as usize],
                            &self.code.nodes[*after as usize],
                        ) {
                            (Node::Var(source), Node::Primed(target)) if source == target => {
                                1u64 << source
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                };
                (*action, Some(mask))
            }
            _ => (*g, None),
        };
        let mut proof = AssignmentProof::default();
        self.closed_assignments(action, &mut proof, depth, work)?;
        let width = u32::try_from(self.ir.vars().len()).ok()?;
        let all = u64::MAX.checked_shr(64u32.checked_sub(width)?).unwrap_or(0);
        (proof.written == all).then_some(super::ClosedEnabled {
            subscript,
            quiet_suffix: !proof.suffix_update,
            quiet_guards: !proof.guard_update,
            boolean_guard_prefix: !proof.different_boolean_guard,
        })
    }

    /// Inspect the bound value computation, including every retained operand
    /// of wrappers, thunks and lazy set/function operations. A nested ENABLED
    /// gets the same generator proof in its fresh frame. Refuse recursive and
    /// library calls rather than inheriting a summary of their diagnostics.
    fn contains_update(&self, p: P, depth: usize, work: &mut usize) -> Option<bool> {
        *work = work.checked_sub(1)?;
        if depth > super::MAX_GUARD_DEPTH {
            return None;
        }
        let n = self.code.nodes.get(p as usize)?;
        match n {
            Node::Call { .. }
            | Node::Composition { .. }
            | Node::LibHo(..)
            | Node::Lib { .. }
            | Node::Primed(_) => return None,
            Node::Enabled { .. } => {
                let nested = self.closed_query(p, depth + 1, work)?;
                return Some(!(nested.quiet_guards && nested.quiet_suffix));
            }
            _ => {}
        }
        let mut operands = Vec::new();
        super::push_operands(n, &mut operands);
        let mut update = matches!(n, Node::Except { .. });
        for operand in operands {
            update |= self.contains_update(operand, depth + 1, work)?;
        }
        Some(update)
    }

    /// A source guard cannot bind a destination or read a partial primed
    /// state. Inspect its actual generator as well as its boolean form.
    fn source_guard_update(&self, g: G, depth: usize, work: &mut usize) -> Option<bool> {
        *work = work.checked_sub(1)?;
        if depth > super::MAX_GUARD_DEPTH {
            return None;
        }
        let depth = depth + 1;
        match self.code.gnodes.get(g as usize)? {
            GNode::Scope { body, .. } => self.source_guard_update(*body, depth, work),
            GNode::Guard(p) | GNode::LabelGuard(p, _) => self.contains_update(*p, depth, work),
            GNode::Conj(items) | GNode::LabelArm(items, _) => {
                items.iter().try_fold(false, |seen, &(g, p)| {
                    Some(
                        seen | self.source_guard_update(g, depth, work)?
                            | self.contains_update(p, depth, work)?,
                    )
                })
            }
            GNode::Disj(arms) => arms.iter().try_fold(false, |seen, &g| {
                Some(seen | self.source_guard_update(g, depth, work)?)
            }),
            GNode::Exists {
                binds,
                body,
                body_p,
            }
            | GNode::Forall {
                binds,
                body,
                body_p,
            } => {
                let nested = self.source_guard_update(*body, depth, work)?
                    | self.contains_update(*body_p, depth, work)?;
                binds.iter().try_fold(nested, |seen, b| {
                    Some(seen | self.contains_update(b.domain, depth, work)?)
                })
            }
            GNode::FcnAction { binds, arg, body } => {
                let nested = self.contains_update(*arg, depth, work)?
                    | self.source_guard_update(*body, depth, work)?;
                binds.iter().try_fold(nested, |seen, b| {
                    Some(seen | self.contains_update(b.domain, depth, work)?)
                })
            }
            GNode::Implies { lhs, rhs } => Some(
                self.contains_update(*lhs, depth, work)?
                    | self.source_guard_update(*rhs, depth, work)?,
            ),
            GNode::If { cond, then, other } => Some(
                self.contains_update(*cond, depth, work)?
                    | self.source_guard_update(*then, depth, work)?
                    | self.source_guard_update(*other, depth, work)?,
            ),
            _ => None,
        }
    }

    fn same_guard_order(&self, generator: G, value: P, depth: usize, work: &mut usize) -> bool {
        let Some(remaining) = work.checked_sub(1) else {
            return false;
        };
        *work = remaining;
        if depth >= super::MAX_GUARD_DEPTH {
            return false;
        }
        match (
            &self.code.gnodes[generator as usize],
            &self.code.nodes[value as usize],
        ) {
            (GNode::Guard(guard), _) => self.same_source_value(*guard, value, depth + 1, work),
            (GNode::Scope { body: g, reset: a }, Node::Scope { body: p, reset: b }) => {
                a == b && self.same_guard_order(*g, *p, depth + 1, work)
            }
            (GNode::Conj(parts), Node::And(operands)) => {
                parts.len() == operands.len()
                    && parts
                        .iter()
                        .zip(operands)
                        .all(|(&(g, _), &p)| self.same_guard_order(g, p, depth + 1, work))
            }
            _ => false,
        }
    }

    /// Check a deliberately small value language structurally. P nodes for
    /// the same source conjunct can be emitted twice, once by `guard_expr` and
    /// once by the Boolean conjunction compiler. An opcode alone cannot
    /// identify operands, lexical slots, selectors or a comparison operator.
    fn same_source_value(&self, left: P, right: P, depth: usize, work: &mut usize) -> bool {
        let Some(remaining) = work.checked_sub(1) else {
            return false;
        };
        *work = remaining;
        if depth >= super::MAX_GUARD_DEPTH {
            return false;
        }
        let left = &self.code.nodes[left as usize];
        let right = &self.code.nodes[right as usize];
        if std::mem::discriminant(left) != std::mem::discriminant(right) {
            return false;
        }
        let equivalent = match (left, right) {
            (Node::Const(a), Node::Const(b)) => return a == b,
            (Node::Var(a), Node::Var(b)) => return a == b,
            (Node::Slot(a), Node::Slot(b)) => return a == b,
            (Node::Select(_, a), Node::Select(_, b)) => a == b,
            (Node::Lazy { slot: a, .. }, Node::Lazy { slot: b, .. }) => a == b,
            (Node::Scope { reset: a, .. }, Node::Scope { reset: b, .. }) => a == b,
            (Node::Arith(a, ..), Node::Arith(b, ..)) => a == b,
            (Node::Cmp(a, ..), Node::Cmp(b, ..)) => a == b,
            (Node::SetBin(a, ..), Node::SetBin(b, ..)) => a == b,
            (Node::InLazy { neg: a, .. }, Node::InLazy { neg: b, .. }) => a == b,
            // All scalar parameters of these opcodes are value operands.
            (
                Node::Not(_)
                | Node::And(_)
                | Node::Or(_)
                | Node::If(..)
                | Node::Eq(..)
                | Node::Neq(..)
                | Node::In(..)
                | Node::NotIn(..)
                | Node::Apply(..)
                | Node::SetEnum(_)
                | Node::Tuple(_)
                | Node::Range(..),
                _,
            ) => true,
            _ => false,
        };
        if !equivalent {
            return false;
        }
        let mut a = Vec::new();
        let mut b = Vec::new();
        super::push_operands(left, &mut a);
        super::push_operands(right, &mut b);
        a.len() == b.len()
            && a.into_iter()
                .zip(b)
                .all(|(a, b)| self.same_source_value(a, b, depth + 1, work))
    }

    fn closed_assignments(
        &self,
        g: G,
        proof: &mut AssignmentProof,
        depth: usize,
        work: &mut usize,
    ) -> Option<()> {
        *work = work.checked_sub(1)?;
        if depth > super::MAX_GUARD_DEPTH {
            return None;
        }
        match self.code.gnodes.get(g as usize)? {
            GNode::Scope { body, .. } => self.closed_assignments(*body, proof, depth + 1, work)?,
            GNode::Conj(items) => {
                for &(item, predicate) in items {
                    if proof.written == 0
                        && let Some(update) = self.contains_update(predicate, depth + 1, work)
                    {
                        let generator = self.source_guard_update(item, depth + 1, work);
                        if let Some(generator_update) = generator {
                            proof.guard_update |= update | generator_update;
                            proof.different_boolean_guard |=
                                !self.same_guard_order(item, predicate, 0, &mut 512);
                            continue;
                        }
                    }
                    self.closed_assignments(item, proof, depth + 1, work)?;
                }
            }
            GNode::Assign { var, rhs } => {
                proof.suffix_update |= self.contains_update(*rhs, depth + 1, work)?;
                proof.claim(*var)?;
            }
            GNode::Unchanged(items) => {
                for item in items {
                    let Unch::Var { var, .. } = item else {
                        return None;
                    };
                    proof.claim(*var)?;
                }
            }
            _ => return None,
        }
        Some(())
    }
}
