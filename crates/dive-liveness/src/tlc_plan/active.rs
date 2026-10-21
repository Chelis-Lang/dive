//! Live action metadata without copied normalization trees.

use std::collections::BTreeSet;

use super::{AtomKind, Formula, FormulaError, Limits, Node};
use crate::formula::Atom;

type Metadata = (BTreeSet<usize>, Option<usize>);

struct Work {
    remaining: usize,
}
impl Work {
    fn step(&mut self) -> Result<(), FormulaError> {
        if crate::budget::exhausted() {
            return Err(FormulaError::Limit(crate::budget::EXHAUSTED));
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(FormulaError::Limit("TLC action membership work"))?;
        Ok(())
    }
}

fn children(node: &Node) -> &[usize] {
    match node {
        Node::Not(child) | Node::Always(child) | Node::Eventually(child) => {
            std::slice::from_ref(child)
        }
        Node::And(children) | Node::Or(children) => children,
        Node::Atom(_) | Node::Bool(_) => &[],
    }
}

// Check every reference, including unreachable/constant-absorbed nodes, before
// consulting summaries. Reverse topological propagation bounds reachable depth
// without recursively unfolding shared children.
fn validate(
    nodes: &[Node],
    root: usize,
    input_limit: usize,
    work: &mut Work,
) -> Result<Vec<Option<usize>>, FormulaError> {
    if root >= nodes.len() {
        return Err(FormulaError::Reference("root outside node table".into()));
    }
    let mut input_remaining = input_limit;
    for (index, node) in nodes.iter().enumerate() {
        work.step()?;
        input_remaining = input_remaining
            .checked_sub(1)
            .ok_or(FormulaError::Limit("TLC planning input terms"))?;
        for &child in children(node) {
            work.step()?;
            input_remaining = input_remaining
                .checked_sub(1)
                .ok_or(FormulaError::Limit("TLC planning input terms"))?;
            if child >= index {
                return Err(FormulaError::Reference(format!(
                    "a child of node {index} does not precede it"
                )));
            }
        }
    }
    let mut depths = vec![None; nodes.len()];
    depths[root] = Some(0);
    for index in (0..=root).rev() {
        let Some(depth) = depths[index] else {
            continue;
        };
        work.step()?;
        for &child in children(&nodes[index]) {
            work.step()?;
            let next = depth + 1;
            if next > 256 {
                return Err(FormulaError::Limit("TLC planning depth"));
            }
            depths[child] = Some(depths[child].map_or(next, |old| old.max(next)));
        }
    }
    Ok(depths)
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Bool(bool),
    // Polarity does not change membership or direct atomic classification.
    Atom(Atom),
    Junction,
    Always,
    Eventually,
}
struct Summary {
    kind: Kind,
    children: Vec<usize>,
}
struct Summaries {
    entries: Vec<Summary>,
    mapped: Vec<[Option<usize>; 2]>,
}
impl Summaries {
    fn child(&self, child: usize, negative: bool) -> Result<usize, FormulaError> {
        self.mapped[child][usize::from(negative)].ok_or_else(|| {
            FormulaError::Reference("missing action membership child summary".into())
        })
    }

    fn push(&mut self, kind: Kind, children: Vec<usize>) -> usize {
        let index = self.entries.len();
        self.entries.push(Summary { kind, children });
        index
    }

    fn junction(
        &mut self,
        children: &[usize],
        negative: bool,
        and: bool,
        work: &mut Work,
    ) -> Result<usize, FormulaError> {
        let mut live = Vec::new();
        let mut absorbing = false;
        for &child in children {
            work.step()?;
            let child = self.child(child, negative)?;
            if let Kind::Bool(value) = self.entries[child].kind {
                absorbing |= value != and;
            } else {
                // Retain repeated occurrences. Set membership may deduplicate
                // atom IDs later, but a repeated conjunction is not direct.
                live.push(child);
            }
        }
        Ok(if absorbing {
            self.push(Kind::Bool(!and), Vec::new())
        } else {
            match live.len() {
                0 => self.push(Kind::Bool(and), Vec::new()),
                1 => live[0],
                _ => self.push(Kind::Junction, live),
            }
        })
    }

    fn node(
        &mut self,
        node: &Node,
        negative: bool,
        work: &mut Work,
    ) -> Result<usize, FormulaError> {
        work.step()?;
        Ok(match node {
            Node::Bool(value) => self.push(Kind::Bool(*value != negative), Vec::new()),
            Node::Atom(atom) => self.push(Kind::Atom(*atom), Vec::new()),
            Node::Not(child) => {
                work.step()?;
                self.child(*child, !negative)?
            }
            Node::And(children) | Node::Or(children) => self.junction(
                children,
                negative,
                matches!(node, Node::And(_)) != negative,
                work,
            )?,
            Node::Always(child) | Node::Eventually(child) => {
                work.step()?;
                let always = matches!(node, Node::Always(_)) != negative;
                let mut child = self.child(*child, negative)?;
                if matches!(self.entries[child].kind, Kind::Always) && always
                    || matches!(self.entries[child].kind, Kind::Eventually) && !always
                {
                    child = self.entries[child].children[0];
                }
                // TLC keeps []TRUE and <>FALSE as temporal constructors.
                self.push(
                    if always {
                        Kind::Always
                    } else {
                        Kind::Eventually
                    },
                    vec![child],
                )
            }
        })
    }

    fn direct(&self, root: usize) -> Option<usize> {
        let outer = &self.entries[root];
        let inner = match outer.kind {
            Kind::Always | Kind::Eventually => &self.entries[outer.children[0]],
            _ => return None,
        };
        if !matches!(
            (outer.kind, inner.kind),
            (Kind::Always, Kind::Eventually) | (Kind::Eventually, Kind::Always)
        ) {
            return None;
        }
        match self.entries[inner.children[0]].kind {
            Kind::Atom(atom) if atom.kind == AtomKind::Action => Some(atom.index),
            _ => None,
        }
    }
}

pub(super) fn summarize(formula: &Formula, limits: Limits) -> Result<Metadata, FormulaError> {
    if crate::budget::exhausted() {
        return Err(FormulaError::Limit(crate::budget::EXHAUSTED));
    }
    let mut work = Work {
        remaining: limits.expansion_steps,
    };
    let depths = validate(
        formula.nodes(),
        formula.root(),
        limits.formula_nodes,
        &mut work,
    )?;
    let mut summaries = Summaries {
        entries: Vec::new(),
        mapped: vec![[None; 2]; formula.nodes().len()],
    };
    for (index, node) in formula.nodes().iter().enumerate() {
        if depths[index].is_none() {
            continue;
        }
        for negative in [false, true] {
            let summary = summaries.node(node, negative, &mut work)?;
            summaries.mapped[index][usize::from(negative)] = Some(summary);
        }
    }
    let root = summaries.child(formula.root(), false)?;
    let mut pending = vec![root];
    let mut seen = vec![false; summaries.entries.len()];
    let mut actions = BTreeSet::new();
    while let Some(index) = pending.pop() {
        work.step()?;
        if std::mem::replace(&mut seen[index], true) {
            continue;
        }
        let summary = &summaries.entries[index];
        if let Kind::Atom(atom) = summary.kind
            && atom.kind == AtomKind::Action
        {
            actions.insert(atom.index);
        }
        pending.extend(&summary.children);
    }
    Ok((actions, summaries.direct(root)))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded metadata controls"
)]
mod tests {
    use super::*;

    fn action(index: usize) -> Node {
        Node::Atom(Atom {
            kind: AtomKind::Action,
            index,
        })
    }
    fn unlimited() -> Limits {
        Limits {
            formula_nodes: 1_000_000,
            expansion_steps: 1_000_000,
            ..Limits::default()
        }
    }
    fn compare(nodes: Vec<Node>, root: usize) {
        let formula = Formula::new(nodes, root).expect("checked formula");
        let copied =
            super::super::active_actions_copied(&formula, unlimited()).expect("old summary");
        let actual = summarize(&formula, unlimited()).expect("new summary");
        assert_eq!(actual, copied, "{formula:?}");
        // An action absent after constant simplification cannot influence any
        // position of the original formula's independent infinite-word truth.
        for (size, back) in [(1, 0), (2, 0), (2, 1)] {
            for bits in 0..16usize {
                let states = vec![vec![false]; size];
                let original: Vec<_> = (0..size)
                    .map(|row| {
                        vec![
                            bits & (1 << (row * 2)) != 0,
                            bits & (1 << (row * 2 + 1)) != 0,
                        ]
                    })
                    .collect();
                let mut changed = original.clone();
                for row in &mut changed {
                    for (index, bit) in row.iter_mut().enumerate() {
                        if !actual.0.contains(&index) {
                            *bit = !*bit;
                        }
                    }
                }
                assert_eq!(
                    formula
                        .evaluate_word(&states, &original, back, 10_000)
                        .expect("truth"),
                    formula
                        .evaluate_word(&states, &changed, back, 10_000)
                        .expect("unchanged inactive truth")
                );
            }
        }
    }

    #[test]
    fn bounded_boolean_temporal_matrix_matches_copied_normalization() {
        let leaves = vec![
            Node::Bool(false),
            Node::Bool(true),
            action(0),
            action(1),
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
        ];
        let mut roots = (0..leaves.len()).collect::<Vec<_>>();
        let mut nodes = leaves;
        for index in 0..5 {
            for node in [
                Node::Not(index),
                Node::Always(index),
                Node::Eventually(index),
                Node::And(vec![index]),
                Node::Or(vec![]),
            ] {
                roots.push(nodes.len());
                nodes.push(node);
            }
        }
        for &left in &roots {
            for &right in &roots {
                for and in [false, true] {
                    let mut formula = nodes.clone();
                    let junction = formula.len();
                    formula.push(if and {
                        Node::And(vec![left, right])
                    } else {
                        Node::Or(vec![left, right])
                    });
                    for wrapper in 0..4 {
                        let mut f = formula.clone();
                        let root = match wrapper {
                            0 => junction,
                            1 => {
                                f.push(Node::Not(junction));
                                f.len() - 1
                            }
                            2 => {
                                f.push(Node::Eventually(junction));
                                f.push(Node::Always(f.len() - 1));
                                f.len() - 1
                            }
                            _ => {
                                f.push(Node::Always(junction));
                                f.push(Node::Eventually(f.len() - 1));
                                f.len() - 1
                            }
                        };
                        compare(f, root);
                    }
                }
            }
        }
    }

    #[test]
    fn atomic_direct_roots_keep_negation_and_junction_multiplicity() {
        for negative in [false, true] {
            for gf in [false, true] {
                let mut nodes = vec![action(7)];
                let atom = if negative {
                    nodes.push(Node::Not(0));
                    1
                } else {
                    0
                };
                nodes.push(if gf {
                    Node::Eventually(atom)
                } else {
                    Node::Always(atom)
                });
                let inner = nodes.len() - 1;
                nodes.push(if gf {
                    Node::Always(inner)
                } else {
                    Node::Eventually(inner)
                });
                let temporal = nodes.len() - 1;
                let formula = Formula::new(nodes.clone(), temporal).expect("formula");
                assert_eq!(
                    summarize(&formula, Limits::default()).expect("metadata"),
                    ([7].into(), Some(7))
                );
                for conjunction in [false, true] {
                    let mut repeated = nodes.clone();
                    repeated.push(if conjunction {
                        Node::And(vec![temporal, temporal])
                    } else {
                        Node::Or(vec![temporal, temporal])
                    });
                    let root = repeated.len() - 1;
                    let formula = Formula::new(repeated, root).expect("formula");
                    assert_eq!(
                        summarize(&formula, Limits::default()).expect("metadata"),
                        ([7].into(), None)
                    );
                }
            }
        }
    }

    #[test]
    fn shared_fairness_dag_does_not_pay_for_copied_trees() {
        let formula = Formula::new(
            vec![
                action(7),
                Node::Eventually(0),
                Node::Always(1),
                Node::And(vec![2; 1000]),
            ],
            3,
        )
        .expect("formula");
        assert_eq!(formula.terms(), 1006);
        assert!(matches!(
            super::super::active_actions_copied(&formula, Limits::default()),
            Err(FormulaError::Limit("TLC planning terms"))
        ));
        assert_eq!(
            summarize(&formula, Limits::default()).expect("one summary per original/polarity"),
            ([7].into(), None)
        );
    }

    #[test]
    fn every_reference_is_checked_before_absorbing_constants() {
        for (nodes, root) in [
            (vec![], 0),
            (
                vec![Node::Bool(false), Node::Not(1), Node::And(vec![0, 1])],
                2,
            ),
            (vec![Node::Bool(true), Node::Not(8)], 0),
            (vec![Node::Not(1), Node::Not(0)], 1),
            (vec![Node::Bool(true)], 8),
        ] {
            let mut work = Work { remaining: 1000 };
            assert!(matches!(
                validate(&nodes, root, 1000, &mut work),
                Err(FormulaError::Reference(_))
            ));
        }
    }

    #[test]
    fn exact_input_and_actual_work_limits_are_enforced() {
        let formula = Formula::new(vec![action(7)], 0).expect("formula");
        let exact = Limits {
            formula_nodes: 1,
            expansion_steps: 5,
            ..Limits::default()
        };
        assert_eq!(
            summarize(&formula, exact).expect("exact actual work"),
            ([7].into(), None)
        );
        for limits in [
            Limits {
                formula_nodes: 0,
                ..exact
            },
            Limits {
                expansion_steps: 0,
                ..exact
            },
            Limits {
                expansion_steps: 4,
                ..exact
            },
        ] {
            assert!(matches!(
                summarize(&formula, limits),
                Err(FormulaError::Limit(_))
            ));
        }
        let watchdog = crate::budget::expired_for_test();
        let _scope = crate::budget::enter(watchdog.budget());
        assert_eq!(
            summarize(&formula, exact),
            Err(FormulaError::Limit(crate::budget::EXHAUSTED))
        );
    }

    #[test]
    fn source_depth_is_bounded_without_recursive_stack_use() {
        let mut nodes = vec![action(7)];
        for child in 0..256 {
            nodes.push(Node::Always(child));
        }
        let formula = Formula::new(nodes.clone(), 256).expect("formula");
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || {
                assert_eq!(
                    summarize(&formula, Limits::default()).expect("iterative depth boundary"),
                    ([7].into(), None)
                );
            })
            .expect("small stack")
            .join()
            .expect("no stack overflow");
        nodes.push(Node::Always(256));
        let formula = Formula::new(nodes, 257).expect("formula");
        assert_eq!(
            summarize(&formula, Limits::default()),
            Err(FormulaError::Limit("TLC planning depth"))
        );
    }
}
