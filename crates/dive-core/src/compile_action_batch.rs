//! Source-ordered evaluation of one bound action over a short row of concrete successors.

use crate::compile::Compiler;
use crate::plan::{Code, Node, P, Plan};
use crate::{Error, Handle, Machine};

/// A source-bound nonstuttering action evaluated on a group of materialized edges.
pub struct ActionBatch {
    subscript: Subscript,
    complete_image: bool,
    guard_reads: Option<u64>,
    terms: Vec<Term>,
}

enum Subscript {
    Tuple(u64),
    Variable(usize),
}

enum Term {
    Guard(P),
    Match(usize, P),
    CopyTuple(u64),
    Reset(Box<[u32]>),
}

impl Compiler<'_> {
    pub(super) fn action_batch(&self, action: P, changed: P) -> Option<ActionBatch> {
        let subscript = match &self.code.nodes[changed as usize] {
            Node::SameVars { leaves, thunks, .. }
                if thunks.iter().all(|&s| !self.preset_slot(s)) =>
            {
                Subscript::Tuple(leaves.iter().try_fold(0, |mask, &(var, _)| {
                    Some(mask | 1u64.checked_shl(var.into())?)
                })?)
            }
            Node::Eq(x, y) => {
                let (Node::Var(v), Node::Primed(n)) =
                    (&self.code.nodes[*x as usize], &self.code.nodes[*y as usize])
                else {
                    return None;
                };
                if v != n {
                    return None;
                }
                Subscript::Variable(usize::from(*v))
            }
            _ => return None,
        };
        let mut terms = Vec::new();
        self.batch_terms(action, &mut terms, 0)?;
        let complete_image = function_image(&terms, self.ir.vars().len());
        // A bare subscript uses the ordinary scoped wrapper unless the action
        // has a guard prefix followed by one source-defined write per variable.
        // In particular a guard after a write can demand a lazy function body.
        if matches!(subscript, Subscript::Variable(_)) && !complete_image {
            return None;
        }
        let guard_reads = terms.iter().try_fold(0, |mask, term| match term {
            Term::Guard(p) => Some(mask | self.atom_source_reads(*p)?),
            _ => Some(mask),
        });
        Some(ActionBatch {
            subscript,
            complete_image,
            guard_reads,
            terms,
        })
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

// Only a leading source guard followed by disjoint, complete writes can
// use assignments from a concrete generated destination.
fn function_image(terms: &[Term], width: usize) -> bool {
    let mut writes: u64 = 0;
    for term in terms {
        let mask = match term {
            Term::Reset(_) => continue,
            Term::Guard(_) if writes == 0 => continue,
            Term::Guard(_) => return false,
            Term::Match(variable, _) => {
                let Some(bit) = u32::try_from(*variable)
                    .ok()
                    .and_then(|v| 1u64.checked_shl(v))
                else {
                    return false;
                };
                bit
            }
            Term::CopyTuple(mask) => *mask,
        };
        if mask & writes != 0 {
            return false;
        }
        writes |= mask;
    }
    writes.count_ones() as usize == width
}

impl ActionBatch {
    /// Whether each state variable has one source-defined write after the guard prefix.
    #[must_use]
    pub fn accepts_generated_image(&self) -> bool {
        self.complete_image
    }

    /// Sufficient source variables for all the original Boolean guard terms
    /// in this bound batch, under its captured immutable lexical environment.
    #[must_use]
    pub const fn original_guard_reads(&self) -> Option<u64> {
        self.guard_reads
    }

    /// Whether the subscript comparison uses tuple member equality.
    #[must_use]
    pub const fn admits_projected_image(&self) -> bool {
        self.complete_image && matches!(self.subscript, Subscript::Tuple(_))
    }

    /// Evaluate the original source-only Boolean guards of a complete deterministic
    /// action in their original order, including scoped resets and interruption.
    /// Only a quiet result can bypass those guards in the physical edge classifier.
    #[must_use]
    pub fn original_boolean_prefix(
        &self,
        plan: &Plan,
        machine: &mut Machine,
        before: &[Handle],
        fuel: u64,
    ) -> Option<bool> {
        if !self.admits_projected_image() || before.len() != machine.s0.len() {
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
        let result = (|| -> Result<bool, Error> {
            for term in &self.terms {
                match term {
                    Term::Reset(slots) => {
                        for &slot in slots {
                            machine.frame[machine.base + slot as usize] = Handle::EMPTY;
                        }
                    }
                    Term::Guard(p) if !machine.eval_bool(&plan.code, *p)? => return Ok(false),
                    Term::Guard(_) => {}
                    Term::Match(_, _) | Term::CopyTuple(_) => break,
                }
            }
            Ok(true)
        })();
        machine.leave(frame);
        machine.heap.reset();
        result.ok().filter(|_| {
            !machine.heap.poisoned() && machine.warnings.is_empty() && machine.printed.is_empty()
        })
    }

    /// Exact nonstuttering truth from a verified complete image and an independently
    /// proved successful Boolean guard. Retains source order for every comparison
    /// that could refuse or raise a diagnostic.
    #[must_use]
    pub fn original_image_target(
        &self,
        heap: &crate::Heap,
        before: &[Handle],
        image: &[Handle],
        target: &[Handle],
    ) -> Option<bool> {
        let Subscript::Tuple(mut changed) = self.subscript else {
            return None;
        };
        if !self.complete_image || target.len() != before.len() || image.len() != before.len() {
            return None;
        }
        let mut different = false;
        while changed != 0 {
            let var = changed.trailing_zeros() as usize;
            if !heap.eq(*before.get(var)?, *target.get(var)?) {
                different = true;
                break;
            }
            changed &= changed - 1;
        }
        if !different {
            return Some(false);
        }
        if std::ptr::eq(image, target) {
            return Some(true);
        }
        for term in &self.terms {
            match *term {
                Term::Match(var, _)
                    if !crate::compare::equals(heap, *target.get(var)?, *image.get(var)?)
                        .ok()? =>
                {
                    return Some(false);
                }
                Term::CopyTuple(mut mask) => {
                    while mask != 0 {
                        let var = mask.trailing_zeros() as usize;
                        if !heap.eq(*before.get(var)?, *target.get(var)?) {
                            return Some(false);
                        }
                        mask &= mask - 1;
                    }
                }
                _ => {}
            }
        }
        Some(true)
    }

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
        self.evaluate_image(plan, machine, before, targets, fuel, None)
            .map(|(bits, _)| bits)
    }

    /// Uses the normalized destination writes of the same deterministic bound
    /// action on the same closed original source. The caller must certify every
    /// write, unchanged variable, store, warning and producer binding. A true
    /// flag also certifies the identical ordered Boolean guard prefix. Returns
    /// original edge bits and whether a demanded term consumed that evidence.
    #[must_use]
    pub fn evaluate_image(
        &self,
        plan: &Plan,
        machine: &mut Machine,
        before: &[Handle],
        targets: &[&[Handle]],
        fuel: u64,
        emitted: Option<(&[Handle], bool)>,
    ) -> Option<(u64, bool)> {
        let emitted = emitted.filter(|_| self.complete_image);
        if targets.len() > 64
            || emitted.is_some_and(|(row, _)| row.len() != before.len())
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
        let result = self.match_edges(&plan.code, machine, before, targets, emitted);
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
        emitted: Option<(&[Handle], bool)>,
    ) -> Result<(u64, bool), Error> {
        let mut used = false;
        let mut live = 0;
        for (i, row) in targets.iter().enumerate() {
            // Equality on a bare variable may diagnose a cross-class comparison.
            // Tuple-wrapped subscripts compare their members extensionally.
            let unchanged = match self.subscript {
                Subscript::Tuple(mask) => same(m, before, row, mask),
                Subscript::Variable(v) => crate::compare::equals(&m.heap, before[v], row[v])?,
            };
            if !unchanged {
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
                    if emitted.is_some_and(|(_, guard_prefix)| guard_prefix) {
                        used = true;
                    } else if !m.eval_bool(code, *p)? {
                        return Ok((0, used));
                    }
                }
                Term::Match(var, rhs) => {
                    let value = match emitted {
                        Some((row, _)) => {
                            used = true;
                            row[*var]
                        }
                        None => m.eval_norm(code, *rhs)?,
                    };
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
        Ok((live, used))
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

#[cfg(test)]
mod image_tests {
    use super::{Term, function_image};

    #[test]
    fn only_complete_disjoint_writes_can_borrow_the_produced_image() {
        assert!(function_image(
            &[Term::Guard(0), Term::Match(0, 1), Term::CopyTuple(6)],
            3
        ));
        assert!(!function_image(&[Term::Match(0, 0), Term::Guard(1)], 1));
        assert!(!function_image(&[Term::Match(0, 0), Term::CopyTuple(1)], 1));
        assert!(!function_image(&[Term::CopyTuple(1)], 2));
        assert!(!function_image(&[Term::Match(64, 0)], 65));
    }
}
