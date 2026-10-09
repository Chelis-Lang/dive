//! Factored recurrence versus direct periodic words and bounded products.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_liveness::Graph;
use dive_liveness::formula::{
    Atom, AtomKind, Formula, FormulaError, Limits as FormulaLimits, Node, compile,
};
use dive_liveness::product::{Labels, Limits, Product, ProductError};

fn cases() -> Vec<Formula> {
    let base = vec![
        Node::Atom(Atom {
            kind: AtomKind::State,
            index: 0,
        }),
        Node::Atom(Atom {
            kind: AtomKind::Action,
            index: 0,
        }),
        Node::Not(0),
        Node::Eventually(0),
        Node::Always(3),
        Node::Eventually(1),
        Node::Always(5),
    ];
    let mut cases = vec![
        Formula::new(base.clone(), 4).expect("state recurrence"),
        Formula::new(base.clone(), 6).expect("action recurrence"),
    ];
    for root in [
        Node::And(vec![4, 6]),
        Node::Or(vec![4, 6]),
        Node::And(vec![2, 6]),
        Node::And(vec![4, 4, 6]),
        Node::Or(vec![4, 1]),
        Node::And(vec![]),
        Node::Or(vec![]),
    ] {
        let mut nodes = base.clone();
        nodes.push(root);
        let f = Formula::new(nodes.clone(), nodes.len() - 1).expect("formula");
        cases.push(f);
        nodes.push(Node::Not(nodes.len() - 1));
        cases.push(Formula::new(nodes.clone(), nodes.len() - 1).expect("negated formula"));
    }
    // Shared GF under another connective, nested temporal operands (retained),
    // and boolean state/action operands containing both conjunction and negation.
    for operand in [
        Node::And(vec![0, 1]),
        Node::Or(vec![0, 1]),
        Node::Not(1),
        Node::Bool(true),
        Node::Bool(false),
        Node::And(vec![]),
        Node::Or(vec![]),
        Node::Always(0),
    ] {
        let mut nodes = base.clone();
        let i = nodes.len();
        nodes.push(operand);
        nodes.push(Node::Eventually(i));
        nodes.push(Node::Always(i + 1));
        nodes.push(Node::And(vec![i + 2, 4]));
        cases.push(Formula::new(nodes.clone(), nodes.len() - 1).expect("mixed recurrence"));
    }
    cases
}

#[test]
fn every_factored_short_word_matches_original_temporal_truth() {
    for formula in cases() {
        let (core, conditions) = formula.factor_recurrences(4096).expect("factor");
        let tableau = compile(&core, FormulaLimits::default()).expect("tableau");
        for len in 1..=4 {
            for bits in 0..(1_u32 << (2 * len)) {
                let states: Vec<Vec<bool>> =
                    (0..len).map(|i| vec![bits & (1 << (2 * i)) != 0]).collect();
                let actions: Vec<Vec<bool>> =
                    (0..len).map(|i| vec![bits & (2 << (2 * i)) != 0]).collect();
                for back in 0..len {
                    let graph = Graph::new(
                        (0..len)
                            .map(|i| vec![if i + 1 == len { back } else { i + 1 }])
                            .collect(),
                        vec![0],
                    )
                    .expect("word graph");
                    let labels = Labels {
                        states: vec![states.iter().map(|v| v[0]).collect()],
                        actions: vec![actions.clone()],
                    };
                    let expected = formula
                        .evaluate_word(&states, &actions, back, 100_000)
                        .expect("direct word")[0];
                    let product = Product::with_recurrences(
                        &graph,
                        &tableau,
                        &labels,
                        &conditions,
                        Limits::default(),
                    )
                    .expect("factored product");
                    assert_eq!(
                        product.find_lasso().expect("search").is_some(),
                        expected,
                        "{formula:?} bits={bits} len={len} back={back}"
                    );
                }
            }
        }
    }
}

#[test]
fn closing_edge_fulfills_recurrence_but_prefix_alone_does_not() {
    let formula = Formula::new(
        vec![
            Node::Atom(Atom {
                kind: AtomKind::Action,
                index: 0,
            }),
            Node::Eventually(0),
            Node::Always(1),
        ],
        2,
    )
    .expect("recurrence");
    let (core, conditions) = formula.factor_recurrences(4096).expect("factor");
    assert_eq!(conditions.len(), 1);
    let tableau = compile(&core, FormulaLimits::default()).expect("tableau");
    let graph = Graph::new(vec![vec![1], vec![1]], vec![0]).expect("prefix and closing self-loop");
    for (bits, expected) in [
        (vec![vec![false], vec![true]], true),
        (vec![vec![true], vec![false]], false),
    ] {
        let labels = Labels {
            states: vec![],
            actions: vec![bits],
        };
        let p =
            Product::with_recurrences(&graph, &tableau, &labels, &conditions, Limits::default())
                .expect("product");
        assert_eq!(p.find_lasso().expect("search").is_some(), expected);
        // Dropping recurrence is a planted bug: it incorrectly accepts the
        // second word, where the action occurs only in the finite prefix.
        assert!(
            Product::new(&graph, &tableau, &labels, Limits::default())
                .expect("planted omission")
                .find_lasso()
                .expect("search")
                .is_some()
        );
    }
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![false], vec![true]]],
    };
    assert_eq!(
        Product::with_recurrences(
            &graph,
            &tableau,
            &labels,
            &conditions,
            Limits {
                acceptance_cells: 3,
                ..Limits::default()
            }
        ),
        Err(ProductError::Limit("acceptance cells"))
    );
    assert!(matches!(
        Product::with_recurrences(
            &graph,
            &tableau,
            &Labels {
                states: vec![],
                actions: vec![]
            },
            &conditions,
            Limits::default()
        ),
        Err(ProductError::Shape(_))
    ));
}

#[test]
fn shared_conjunctions_and_disjunctions_have_explicit_work_bounds() {
    for and in [true, false] {
        let mut nodes = vec![
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
            Node::Eventually(0),
            Node::Always(1),
        ];
        for _ in 0..20 {
            let i = nodes.len() - 1;
            nodes.push(if and {
                Node::And(vec![i, i])
            } else {
                Node::Or(vec![i, i])
            });
        }
        let f = Formula::new(nodes.clone(), nodes.len() - 1).expect("small shared DAG");
        assert!(matches!(
            f.factor_recurrences(4096),
            Err(FormulaError::Limit(_))
        ));
    }
    let f = Formula::new(vec![Node::Bool(true)], 0).expect("constant");
    assert_eq!(
        f.factor_recurrences(0),
        Err(FormulaError::Limit("formula nodes"))
    );
}
