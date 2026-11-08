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

    // Metadata inspection itself is charged once per node by the buffer plan.
    pub(crate) fn work_metadata(&self) -> Option<(usize, usize)> {
        let references = self.nodes.iter().try_fold(0_usize, |n, node| {
            n.checked_add(match node {
                Boolean::Bool(_) | Boolean::Atom(_) => 0,
                Boolean::Not(_) => 1,
                Boolean::And(cs) | Boolean::Or(cs) => cs.len(),
            })
        })?;
        Some((self.nodes.len(), references))
    }

    // All nodes still execute in source order. And/Or child inspection retains
    // ordinary short circuiting, but does not skip earlier atom nodes.
    pub(crate) fn evaluate_bounded(
        &self,
        mut atom: impl FnMut(Atom) -> bool,
        values: &mut [bool],
        remaining: &mut usize,
    ) -> Option<bool> {
        for (index, node) in self.nodes.iter().enumerate() {
            *remaining = remaining.checked_sub(1)?;
            let value = match node {
                Boolean::Bool(b) => *b,
                Boolean::Atom(a) => atom(*a),
                Boolean::Not(c) => {
                    *remaining = remaining.checked_sub(1)?;
                    !values[*c]
                }
                Boolean::And(cs) | Boolean::Or(cs) => {
                    let and = matches!(node, Boolean::And(_));
                    let mut result = and;
                    for c in cs {
                        *remaining = remaining.checked_sub(1)?;
                        result = values[*c];
                        if result != and {
                            break;
                        }
                    }
                    result
                }
            };
            values[index] = value;
        }
        Some(values[self.root])
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

#[cfg(test)]
mod bounded_tests {
    use super::*;

    #[test]
    fn work_counts_visited_children_but_atoms_remain_eager() {
        let condition = Recurrence {
            nodes: vec![
                Boolean::Atom(Atom {
                    kind: super::super::AtomKind::State,
                    index: 0,
                }),
                Boolean::Atom(Atom {
                    kind: super::super::AtomKind::State,
                    index: 1,
                }),
                Boolean::Or(vec![0, 1]),
                Boolean::Not(2),
                Boolean::And(vec![2, 3]),
            ],
            root: 4,
        };
        for first in [false, true] {
            let mut seen = Vec::new();
            let mut scratch = [false; 5];
            let mut remaining = 100;
            let actual = condition.evaluate_bounded(
                |a| {
                    seen.push(a.index);
                    a.index == 0 && first
                },
                &mut scratch,
                &mut remaining,
            );
            assert_eq!(seen, vec![0, 1]);
            assert_eq!(actual, Some(false));
            // Five nodes, Or visits 1/2, Not visits 1, And visits 2/1.
            assert_eq!(remaining, 91);
            let mut exact = 9;
            assert_eq!(
                condition.evaluate_bounded(|a| a.index == 0 && first, &mut scratch, &mut exact),
                actual
            );
            assert_eq!(exact, 0);
            let mut short = 8;
            assert_eq!(
                condition.evaluate_bounded(|a| a.index == 0 && first, &mut scratch, &mut short),
                None
            );
        }
    }
}
