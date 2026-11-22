//! Source-ordered evaluation of one bound action over a short row of concrete successors.

use crate::compile::Compiler;
use crate::plan::{Code, Node, P, Plan};
use crate::{Error, Handle, Machine};

/// A source-bound nonstuttering action evaluated on a group of materialized edges.
pub struct ActionBatch {
    changed: u64,
    terms: Vec<Term>,
}

enum Term {
    Guard(P),
    Match(usize, P),
    CopyTuple(u64),
    Reset(Box<[u32]>),
}

impl Compiler<'_> {
    pub(super) fn action_batch(&self, action: P, changed: P) -> Option<ActionBatch> {
        let Node::SameVars { leaves, .. } = &self.code.nodes[changed as usize] else {
            return None;
        };
        let changed = leaves.iter().try_fold(0, |mask, &(var, _)| {
            Some(mask | 1u64.checked_shl(var.into())?)
        })?;
        let mut terms = Vec::new();
        self.batch_terms(action, &mut terms, 0)?;
        Some(ActionBatch { changed, terms })
    }

    // ENABLED constructs its own primed row. A closed capture list binds
    // every outer slot to this atom's preset value, even when the generator
    // reads every source variable and cannot use a projected state key.
    fn batch_source(&self, p: P, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }
        if self.atom_source_reads(p).is_some() {
            return true;
        }
        match &self.code.nodes[p as usize] {
            Node::Not(body) => self.batch_source(*body, depth + 1),
            Node::Enabled { free, .. } => free.iter().all(|&slot| self.preset_slot(slot)),
            _ => false,
        }
    }

    fn batch_terms(&self, p: P, terms: &mut Vec<Term>, depth: usize) -> Option<()> {
        if depth > 32 || terms.len() > 128 {
            return None;
        }
        let node = &self.code.nodes[p as usize];
        match node {
            Node::And(parts) => {
                for &part in parts {
                    self.batch_terms(part, terms, depth + 1)?;
                }
            }
            Node::Scope { body, reset } => {
                terms.push(Term::Reset(reset.clone()));
                self.batch_terms(*body, terms, depth + 1)?;
            }
            // Only the outermost thunk may be elided. No later operand may read its slot.
            Node::Lazy { slot, body } if depth == 0 && !self.preset_slot(*slot) => {
                self.batch_terms(*body, terms, depth + 1)?;
            }
            _ if self.batch_source(p, depth) => terms.push(Term::Guard(p)),
            Node::Eq(lhs, rhs) => {
                match (
                    &self.code.nodes[*lhs as usize],
                    &self.code.nodes[*rhs as usize],
                ) {
                    (Node::Primed(var), _) if self.atom_source_reads(*rhs).is_some() => {
                        terms.push(Term::Match(usize::from(*var), *rhs));
                    }
                    (Node::Tuple(left), Node::Tuple(right)) if left.len() == right.len() => {
                        let mut mask = 0;
                        for (&left, &right) in left.iter().zip(right) {
                            let (Node::Var(before), Node::Primed(after)) = (
                                &self.code.nodes[left as usize],
                                &self.code.nodes[right as usize],
                            ) else {
                                return None;
                            };
                            if before != after {
                                return None;
                            }
                            mask |= 1u64.checked_shl((*before).into())?;
                        }
                        terms.push(Term::CopyTuple(mask));
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
        Some(())
    }
}

impl ActionBatch {
    /// Evaluates this plan on the original source and target rows, in its owning machine.
    /// All handles must already have been proved materialized in the same store.
    /// Any error or visible event leaves ordinary per-edge evaluation in charge.
    #[must_use]
    pub fn evaluate(
        &self,
        plan: &Plan,
        machine: &mut Machine,
        before: &[Handle],
        targets: &[&[Handle]],
        fuel: u64,
    ) -> Option<u64> {
        if targets.len() > 64
            || before.len() != machine.s0.len()
            || targets.iter().any(|r| r.len() != before.len())
        {
            return None;
        }
        machine.s0.copy_from_slice(before);
        machine.s1.fill(Handle::EMPTY);
        machine.set_proven_materialized(before, &[], plan.bound_preset.iter().map(|&(_, v)| v));
        machine.set_fuel(fuel);
        let frame = machine.enter(plan.implied_actions.first()?.frame);
        for &(slot, value) in &plan.bound_preset {
            machine.frame[machine.base + slot as usize] = value;
        }
        machine.warnings.clear();
        machine.printed.clear();
        let result = self.match_edges(&plan.code, machine, before, targets);
        machine.leave(frame);
        machine.heap.reset();
        result.ok().filter(|_| {
            !machine.heap.poisoned() && machine.warnings.is_empty() && machine.printed.is_empty()
        })
    }

    fn match_edges(
        &self,
        code: &Code,
        m: &mut Machine,
        before: &[Handle],
        targets: &[&[Handle]],
    ) -> Result<u64, Error> {
        let mut live = 0;
        for (i, row) in targets.iter().enumerate() {
            if !same(m, before, row, self.changed) {
                live |= 1u64 << i;
            }
        }
        for term in &self.terms {
            if live == 0 {
                break;
            }
            match term {
                Term::Reset(slots) => {
                    for &slot in slots {
                        m.frame[m.base + slot as usize] = Handle::EMPTY;
                    }
                }
                Term::Guard(p) => {
                    if !m.eval_bool(code, *p)? {
                        return Ok(0);
                    }
                }
                Term::Match(var, rhs) => {
                    let value = m.eval_norm(code, *rhs)?;
                    let mut pending = live;
                    while pending != 0 {
                        let i = pending.trailing_zeros() as usize;
                        let target = targets[i][*var];
                        if !crate::compare::equals(&m.heap, target, value)? {
                            live &= !(1u64 << i);
                        }
                        pending &= pending - 1;
                    }
                }
                Term::CopyTuple(mask) => {
                    let mut pending = live;
                    while pending != 0 {
                        let i = pending.trailing_zeros() as usize;
                        if !same(m, before, targets[i], *mask) {
                            live &= !(1u64 << i);
                        }
                        pending &= pending - 1;
                    }
                }
            }
        }
        Ok(live)
    }
}

fn same(m: &Machine, before: &[Handle], after: &[Handle], mut mask: u64) -> bool {
    while mask != 0 {
        let i = mask.trailing_zeros() as usize;
        if !m.heap.eq(before[i], after[i]) {
            return false;
        }
        mask &= mask - 1;
    }
    true
}
