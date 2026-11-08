//! Conjunctive recurrence conditions outside the temporal-memory tableau.

use super::{Atom, Formula, Node};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Boolean {
    Bool(bool),
    Atom(Atom),
    Not(usize),
    And(Vec<usize>),
    Or(Vec<usize>),
}

/// A pure boolean state/action condition that must hold infinitely often.
/// Conditions are constructed only by solution decomposition; atom ids and
/// domains refer to the original formula's caller-supplied table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Recurrence {
    nodes: Vec<Boolean>,
    root: usize,
}

impl Recurrence {
    /// All referenced predicates, retaining their state/action domains.
    pub(crate) fn atoms(&self) -> impl Iterator<Item = Atom> + '_ {
        self.nodes.iter().filter_map(|node| {
            if let Boolean::Atom(atom) = node {
                Some(*atom)
            } else {
                None
            }
        })
    }

    /// Evaluates this condition at one word position. Action truth must refer
    /// to that position's outgoing edge, including a lasso's closing edge.
    /// This reads supplied truth only; it evaluates no specification code.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn evaluate(&self, atom: impl FnMut(Atom) -> bool) -> bool {
        let mut values: Vec<bool> = Vec::with_capacity(self.nodes.len());
        self.evaluate_with_scratch(atom, &mut values)
    }

    pub(crate) fn evaluate_with_scratch(
        &self,
        mut atom: impl FnMut(Atom) -> bool,
        values: &mut Vec<bool>,
    ) -> bool {
        values.clear();
        for node in &self.nodes {
            let value = match node {
                Boolean::Bool(b) => *b,
                Boolean::Atom(a) => atom(*a),
                Boolean::Not(c) => !values[*c],
                Boolean::And(cs) => cs.iter().all(|c| values[*c]),
                Boolean::Or(cs) => cs.iter().any(|c| values[*c]),
            };
            values.push(value);
        }
        values[self.root]
    }

    pub(crate) fn scratch_len(&self) -> usize {
        self.nodes.len()
    }

    /// The disjunction of the Boolean `roots` of `formula`, or `None` when a
    /// temporal operator is reachable from them.
    pub(crate) fn condition(formula: &Formula, roots: &[usize]) -> Option<Self> {
        let reachable = boolean_reachable(&formula.nodes, roots)?;
        let mut mapped = vec![0; formula.nodes.len()];
        let mut nodes = Vec::new();
        for i in reachable {
            let node = match &formula.nodes[i] {
                Node::Bool(b) => Boolean::Bool(*b),
                Node::Atom(a) => Boolean::Atom(*a),
                Node::Not(c) => Boolean::Not(mapped[*c]),
                Node::And(cs) => Boolean::And(cs.iter().map(|c| mapped[*c]).collect()),
                Node::Or(cs) => Boolean::Or(cs.iter().map(|c| mapped[*c]).collect()),
                Node::Always(_) | Node::Eventually(_) => return None,
            };
            mapped[i] = nodes.len();
            nodes.push(node);
        }
        let root = nodes.len();
        nodes.push(Boolean::Or(roots.iter().map(|i| mapped[*i]).collect()));
        Some(Self { nodes, root })
    }
}

// The Boolean nodes reachable from `roots`, or `None` under a temporal operator.
fn boolean_reachable(nodes: &[Node], roots: &[usize]) -> Option<BTreeSet<usize>> {
    let mut found = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(i) = pending.pop() {
        if !found.insert(i) {
            continue;
        }
        match &nodes[i] {
            Node::Bool(_) | Node::Atom(_) => {}
            Node::Not(c) => pending.push(*c),
            Node::And(cs) | Node::Or(cs) => pending.extend(cs),
            Node::Always(_) | Node::Eventually(_) => return None,
        }
    }
    Some(found)
}

#[cfg(test)]
mod scratch_tests {
    #![allow(clippy::expect_used, reason = "finite recurrence assertions")]
    use super::{Atom, Formula, Node, Recurrence};
    use crate::formula::AtomKind;

    #[test]
    fn reused_scratch_preserves_all_atom_reads_and_boolean_truth() {
        let state = Atom {
            kind: AtomKind::State,
            index: 0,
        };
        let action = Atom {
            kind: AtomKind::Action,
            index: 0,
        };
        let formula = Formula::new(
            vec![
                Node::Atom(state),
                Node::Atom(action),
                Node::Not(1),
                Node::Or(vec![0, 2]),
                Node::Eventually(3),
                Node::Always(4),
            ],
            5,
        )
        .expect("formula");
        // The body of `[]<>(S \/ ~A)`, as solution decomposition extracts it.
        let condition = &Recurrence::condition(&formula, &[3]).expect("recurrence");
        let mut scratch = Vec::new();
        for value in 0..4 {
            let mut reads = Vec::new();
            let mut reused_reads = Vec::new();
            let truth = |atom: Atom| {
                if atom.kind == AtomKind::State {
                    value & 1 != 0
                } else {
                    value & 2 != 0
                }
            };
            let owned = condition.evaluate(|atom| {
                reads.push(atom);
                truth(atom)
            });
            let reused = condition.evaluate_with_scratch(
                |atom| {
                    reused_reads.push(atom);
                    truth(atom)
                },
                &mut scratch,
            );
            assert_eq!(owned, value & 1 != 0 || value & 2 == 0);
            assert_eq!(reused, owned);
            assert_eq!(reads, [state, action]);
            assert_eq!(reused_reads, reads);
            let capacity = scratch.capacity();
            for _ in 0..8 {
                assert_eq!(condition.evaluate_with_scratch(truth, &mut scratch), owned);
                assert_eq!(scratch.capacity(), capacity);
            }
        }
    }
}
