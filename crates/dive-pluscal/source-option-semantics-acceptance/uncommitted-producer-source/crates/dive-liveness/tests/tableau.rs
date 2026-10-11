//! Tableau language versus direct periodic-word temporal semantics.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_liveness::Graph;
use dive_liveness::formula::{
    Atom, AtomKind, Formula, FormulaError, Limits, Literal, Node, Tableau, TableauNode, compile,
};
use dive_liveness::search::{Acceptance, find_lasso};

fn truth(nodes: &[Node], root: usize, word: &[u8], back: usize) -> bool {
    let mut table: Vec<Vec<bool>> = Vec::new();
    for node in nodes {
        let values = match node {
            Node::Bool(b) => vec![*b; word.len()],
            Node::Atom(a) => word
                .iter()
                .map(|bits| bits & (if a.kind == AtomKind::State { 1 } else { 2 }) != 0)
                .collect(),
            Node::Not(c) => table[*c].iter().map(|b| !b).collect(),
            Node::And(cs) | Node::Or(cs) => {
                let and = matches!(node, Node::And(_));
                (0..word.len())
                    .map(|i| {
                        if and {
                            cs.iter().all(|c| table[*c][i])
                        } else {
                            cs.iter().any(|c| table[*c][i])
                        }
                    })
                    .collect()
            }
            Node::Always(c) | Node::Eventually(c) => {
                let always = matches!(node, Node::Always(_));
                (0..word.len())
                    .map(|start| {
                        let mut seen = vec![false; word.len()];
                        let mut at = start;
                        let mut out = always;
                        while !seen[at] {
                            seen[at] = true;
                            if always {
                                out &= table[*c][at];
                            } else {
                                out |= table[*c][at];
                            }
                            at = if at + 1 == word.len() { back } else { at + 1 };
                        }
                        out
                    })
                    .collect()
            }
        };
        table.push(values);
    }
    table[root][0]
}

fn product(t: &Tableau, word: &[u8], back: usize) -> (Graph, Acceptance) {
    let fits = |pos: usize, node: usize| {
        t.nodes[node].literals.iter().all(|lit| {
            let mask = if lit.atom.kind == AtomKind::State {
                1
            } else {
                2
            };
            (word[pos] & mask != 0) == lit.positive
        })
    };
    let width = t.nodes.len();
    let mut edges = vec![Vec::new(); word.len() * width];
    for pos in 0..word.len() {
        let next = if pos + 1 == word.len() { back } else { pos + 1 };
        for node in 0..width {
            if fits(pos, node) {
                edges[pos * width + node] = t.nodes[node]
                    .successors
                    .iter()
                    .filter(|n| fits(next, **n))
                    .map(|n| next * width + n)
                    .collect();
            }
        }
    }
    let roots = t.roots.iter().copied().filter(|n| fits(0, *n)).collect();
    let recurrent = edges.iter().map(|row| vec![true; row.len()]).collect();
    let states = (0..t.promise_count)
        .map(|p| {
            (0..word.len() * width)
                .map(|v| t.nodes[v % width].fulfilled[p])
                .collect()
        })
        .collect();
    (
        Graph::new(edges, roots).expect("product"),
        Acceptance {
            recurrent,
            states,
            actions: vec![],
        },
    )
}

fn atom(kind: AtomKind) -> Node {
    Node::Atom(Atom { kind, index: 0 })
}

fn cases() -> Vec<Vec<Node>> {
    let seeds = vec![
        vec![Node::Bool(true)],
        vec![Node::Bool(false)],
        vec![atom(AtomKind::State)],
        vec![atom(AtomKind::Action)],
        vec![atom(AtomKind::State), Node::Not(0)],
    ];
    let mut all = seeds.clone();
    for seed in &seeds {
        for a in 0..3 {
            let mut nodes = seed.clone();
            let i = nodes.len() - 1;
            nodes.push(match a {
                0 => Node::Not(i),
                1 => Node::Always(i),
                _ => Node::Eventually(i),
            });
            all.push(nodes.clone());
            for b in 0..3 {
                let mut nodes = nodes.clone();
                let i = nodes.len() - 1;
                nodes.push(match b {
                    0 => Node::Not(i),
                    1 => Node::Always(i),
                    _ => Node::Eventually(i),
                });
                all.push(nodes);
            }
        }
    }
    for a in 0..4 {
        for b in 0..4 {
            for and in [false, true] {
                let mut nodes = vec![atom(AtomKind::State), atom(AtomKind::Action)];
                let left = match a {
                    0 => 0,
                    1 => {
                        nodes.push(Node::Not(0));
                        2
                    }
                    2 => {
                        nodes.push(Node::Always(0));
                        2
                    }
                    _ => {
                        nodes.push(Node::Eventually(0));
                        2
                    }
                };
                let right = if b == 0 {
                    1
                } else {
                    nodes.push(match b {
                        1 => Node::Not(1),
                        2 => Node::Always(1),
                        _ => Node::Eventually(1),
                    });
                    nodes.len() - 1
                };
                let i = nodes.len();
                nodes.push(if and {
                    Node::And(vec![left, right])
                } else {
                    Node::Or(vec![left, right])
                });
                all.push(nodes.clone());
                nodes.push(Node::Not(i));
                all.push(nodes);
            }
        }
    }
    // Multiple promises, mixed action/state predicates and nested alternation.
    all.push(vec![
        atom(AtomKind::State),
        atom(AtomKind::Action),
        Node::Eventually(0),
        Node::Eventually(1),
        Node::And(vec![2, 3]),
        Node::Always(4),
    ]);
    all.push(vec![
        atom(AtomKind::State),
        atom(AtomKind::Action),
        Node::Always(0),
        Node::Always(1),
        Node::Or(vec![2, 3]),
        Node::Eventually(4),
    ]);
    all.push(vec![Node::And(vec![])]);
    all.push(vec![Node::Or(vec![])]);
    all.extend(recurrence_cases());
    all
}

fn recurrence_cases() -> Vec<Vec<Node>> {
    let mut all = Vec::new();
    // Weak fairness has this recurrence disjunction. Also check negation,
    // conjunction, mixed domains, nested temporal children and a third branch.
    for inner in [false, true] {
        let mut nodes = vec![atom(AtomKind::State), atom(AtomKind::Action)];
        let a = if inner {
            nodes.push(Node::Always(0));
            2
        } else {
            0
        };
        nodes.push(Node::Eventually(a));
        nodes.push(Node::Always(nodes.len() - 1));
        let left = nodes.len() - 1;
        nodes.push(Node::Eventually(1));
        nodes.push(Node::Always(nodes.len() - 1));
        let right = nodes.len() - 1;
        nodes.push(Node::Or(vec![left, right]));
        all.push(nodes.clone());
        nodes.push(Node::Not(nodes.len() - 1));
        all.push(nodes.clone());
        nodes.push(Node::And(vec![left, nodes.len() - 1]));
        all.push(nodes);
    }
    all.push(vec![
        atom(AtomKind::State),
        atom(AtomKind::Action),
        Node::Not(0),
        Node::Eventually(0),
        Node::Always(3),
        Node::Eventually(1),
        Node::Always(5),
        Node::Eventually(2),
        Node::Always(7),
        Node::Or(vec![4, 6, 8]),
    ]);
    all
}

#[test]
fn recurrent_disjunction_uses_one_live_promise_and_keeps_action_edges() {
    let nodes = vec![
        atom(AtomKind::State),
        atom(AtomKind::Action),
        Node::Eventually(0),
        Node::Always(2),
        Node::Eventually(1),
        Node::Always(4),
        Node::Or(vec![3, 5]),
    ];
    let t = compile(
        &Formula::new(nodes, 6).expect("recurrence"),
        Limits::default(),
    )
    .expect("tableau");
    assert_eq!(
        t.promise_count, 1,
        "discarded normalization promises are not live"
    );
    assert_eq!(t.nodes.len(), 3);
    for (word, back, holds) in [
        (vec![1, 0], 0, true),
        (vec![0, 2], 0, true),
        (vec![1, 0], 1, false),
        (vec![2, 0], 1, false),
        (vec![0], 0, false),
    ] {
        let (g, acceptance) = product(&t, &word, back);
        assert_eq!(
            find_lasso(&g, &acceptance).expect("search").is_some(),
            holds
        );
    }
}

#[test]
fn all_short_two_atom_words_match_direct_semantics_at_every_back_edge() {
    for nodes in cases() {
        let root = nodes.len() - 1;
        let f = Formula::new(nodes.clone(), root).expect("formula");
        let t = compile(&f, Limits::default()).expect("tableau");
        assert_eq!(compile(&f, Limits::default()).expect("repeat"), t);
        for len in 1..=4 {
            for bits in 0..(1_u32 << (2 * len)) {
                let word: Vec<u8> = (0..len)
                    .map(|i| u8::try_from((bits >> (2 * i)) & 3).expect("two bits"))
                    .collect();
                for back in 0..len {
                    let states: Vec<Vec<bool>> = word.iter().map(|b| vec![b & 1 != 0]).collect();
                    let actions: Vec<Vec<bool>> = word.iter().map(|b| vec![b & 2 != 0]).collect();
                    let direct = f
                        .evaluate_word(&states, &actions, back, 100_000)
                        .expect("word evaluation");
                    assert_eq!(
                        direct[0],
                        truth(&nodes, root, &word, back),
                        "direct {nodes:?} word={word:?} back={back}"
                    );
                    let (g, acceptance) = product(&t, &word, back);
                    let actual = find_lasso(&g, &acceptance).expect("acceptance").is_some();
                    assert_eq!(
                        actual,
                        truth(&nodes, root, &word, back),
                        "{nodes:?} word={word:?} back={back}"
                    );
                }
            }
        }
    }
}

#[test]
fn unfulfilled_eventuality_cannot_defer_forever() {
    let f = Formula::new(vec![atom(AtomKind::State), Node::Eventually(0)], 1).expect("formula");
    let t = compile(&f, Limits::default()).expect("tableau");
    let (g, mut acceptance) = product(&t, &[0], 0);
    assert_eq!(find_lasso(&g, &acceptance).expect("search"), None);
    // A planted omission of the promise check admits the bogus forever-defer path.
    acceptance.states.clear();
    assert!(find_lasso(&g, &acceptance).expect("planted bug").is_some());
}

#[test]
fn shared_next_obligations_preserve_distinct_promises_and_successor_order() {
    let f = Formula::new(
        vec![
            atom(AtomKind::State),
            atom(AtomKind::Action),
            Node::Eventually(0),
            Node::Eventually(1),
            Node::And(vec![2, 3]),
            Node::Always(4),
        ],
        5,
    )
    .expect("two recurring obligations");
    let state = Literal {
        atom: Atom {
            kind: AtomKind::State,
            index: 0,
        },
        positive: true,
    };
    let action = Literal {
        atom: Atom {
            kind: AtomKind::Action,
            index: 0,
        },
        positive: true,
    };
    // Captured before expansion reuse: promise bits, literal domains and
    // successor order must stay identical, including the second node's order.
    let expected = Tableau {
        roots: vec![0, 1, 2, 3],
        promise_count: 2,
        nodes: vec![
            TableauNode {
                literals: vec![state, action],
                successors: vec![0, 1, 2, 3],
                fulfilled: vec![true, true],
            },
            TableauNode {
                literals: vec![state],
                successors: vec![0, 2, 1, 3],
                fulfilled: vec![true, false],
            },
            TableauNode {
                literals: vec![action],
                successors: vec![0, 1, 2, 3],
                fulfilled: vec![false, true],
            },
            TableauNode {
                literals: vec![],
                successors: vec![0, 1, 2, 3],
                fulfilled: vec![false, false],
            },
        ],
    };
    // The old construction requires 84 work items. Reusing the root expansion
    // uses 72 while retaining the exact automaton and both acceptance promises.
    let limits = Limits {
        expansion_steps: 72,
        ..Limits::default()
    };
    assert_eq!(compile(&f, limits).expect("shared expansion"), expected);
    assert_eq!(
        compile(
            &f,
            Limits {
                expansion_steps: 71,
                ..limits
            }
        ),
        Err(FormulaError::Limit("expansion steps"))
    );
    let (g, acceptance) = product(&expected, &[1, 2], 0);
    assert!(
        find_lasso(&g, &acceptance)
            .expect("alternating fulfillment")
            .is_some()
    );
    let (g, acceptance) = product(&expected, &[1], 0);
    assert!(
        find_lasso(&g, &acceptance)
            .expect("missing action promise")
            .is_none()
    );
}

#[test]
fn malformed_graphs_and_limits_are_errors_not_empty_languages() {
    assert!(Formula::new(vec![], 0).is_err());
    assert!(Formula::new(vec![Node::Not(0)], 0).is_err());
    assert!(Formula::new(vec![Node::Bool(true), Node::Or(vec![2])], 1).is_err());
    let f = Formula::new(vec![atom(AtomKind::State), Node::Eventually(0)], 1).expect("formula");
    for limits in [
        Limits {
            formula_nodes: 0,
            ..Limits::default()
        },
        Limits {
            tableau_nodes: 0,
            ..Limits::default()
        },
        Limits {
            expansion_steps: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(compile(&f, limits), Err(FormulaError::Limit(_))));
    }
    let wide =
        Formula::new(vec![Node::Bool(true), Node::And(vec![0; 10_000])], 1).expect("wide formula");
    assert_eq!(
        compile(&wide, Limits::default()),
        Err(FormulaError::Limit("formula nodes"))
    );
    let mut nodes = vec![Node::Bool(true)];
    for i in 0..300 {
        nodes.push(Node::Not(i));
    }
    let f = Formula::new(nodes, 300).expect("flat deeply nested formula");
    assert_eq!(
        compile(&f, Limits::default()),
        Err(FormulaError::Limit("normalization depth"))
    );
}

#[test]
fn direct_word_truth_checks_shapes_domains_and_cell_bounds() {
    let f = Formula::new(vec![atom(AtomKind::State), Node::Eventually(0)], 1).expect("formula");
    let states = vec![vec![true], vec![false]];
    let actions = vec![vec![false], vec![true]];
    assert_eq!(
        f.evaluate_word(&states, &actions, 1, 4)
            .expect("prefix truth"),
        vec![true, false]
    );
    assert!(matches!(
        f.evaluate_word(&states, &actions, 1, 3),
        Err(FormulaError::Limit(_))
    ));
    assert!(f.evaluate_word(&states, &[], 1, 4).is_err());
    assert!(f.evaluate_word(&[], &[], 0, 4).is_err());
    assert!(f.evaluate_word(&states, &actions, 2, 4).is_err());
    let a = Formula::new(vec![atom(AtomKind::Action)], 0).expect("action formula");
    assert_eq!(
        a.evaluate_word(&states, &actions, 1, 4)
            .expect("action truth"),
        vec![false, true]
    );
    assert!(a.evaluate_word(&states, &[vec![], vec![]], 1, 4).is_err());
}
