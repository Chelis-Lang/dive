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
                for &(_, predicate) in &items {
                    let Some(mask) = self.atom_source_reads(predicate) else {
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
