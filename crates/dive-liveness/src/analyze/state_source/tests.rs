//! A lazy value owner must preserve demand, original values and error control flow.
#![allow(clippy::expect_used, clippy::panic, reason = "finite source controls")]

use std::sync::Arc;

use super::*;
use crate::analyze::*;
use crate::product::BehaviorView;
use dive_check::{CheckOptions, capture};
use dive_value::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Read {
    Representative(usize),
    Concrete(usize, usize),
}

/// Values live outside the graph and acquire their `StateVals` allocation only
/// when requested. Metadata remains available without constructing values.
struct Lazy<'a> {
    metadata: &'a capture::BehaviorGraph,
    representatives: Vec<Vec<Option<Value>>>,
    originals: Vec<Vec<Vec<Option<Value>>>>,
    reads: Vec<Read>,
    fail: Option<usize>,
    stopped: bool,
}

impl<'a> Lazy<'a> {
    fn new(graph: &'a capture::BehaviorGraph) -> Self {
        Self {
            metadata: graph,
            representatives: graph.states.iter().map(|state| state.to_vec()).collect(),
            originals: graph
                .concrete
                .iter()
                .map(|row| row.iter().map(|target| target.state.to_vec()).collect())
                .collect(),
            reads: vec![],
            fail: None,
            stopped: false,
        }
    }

    fn read(&mut self, read: Read) -> Result<(), AnalysisError> {
        self.reads.push(read);
        if self.fail == Some(self.reads.len()) {
            return Err(if self.stopped {
                AnalysisError::Stopped(Box::new(dive_check::budget::Stopped {
                    resource: dive_check::budget::Resource::Time,
                    limit: 10,
                    observed: 11,
                    elapsed_ms: 11,
                    phase: dive_check::budget::Phase::Temporal,
                }))
            } else {
                AnalysisError::Limit("lazy source control")
            });
        }
        Ok(())
    }
}

impl StateSource for Lazy<'_> {
    fn representative_count(&self) -> usize {
        self.representatives.len()
    }

    fn representative(
        &mut self,
        vertex: usize,
    ) -> Result<std::borrow::Cow<'_, StateVals>, AnalysisError> {
        self.read(Read::Representative(vertex))?;
        self.representatives
            .get(vertex)
            .cloned()
            .map(Into::into)
            .map(std::borrow::Cow::Owned)
            .ok_or(AnalysisError::Inconsistent("original state reference"))
    }

    fn concrete_count(&self, source: usize) -> Option<usize> {
        self.metadata.concrete.get(source).map(Vec::len)
    }

    fn concrete_metadata(&self, source: usize, original: usize) -> Option<ConcreteMetadata<'_>> {
        self.metadata
            .concrete
            .get(source)?
            .get(original)
            .map(|target| ConcreteMetadata {
                ordinal: target.ordinal,
                label: &target.label,
            })
    }

    fn concrete(&mut self, source: usize, original: usize) -> Result<StateVals, AnalysisError> {
        self.read(Read::Concrete(source, original))?;
        self.originals
            .get(source)
            .and_then(|row| row.get(original))
            .cloned()
            .map(Into::into)
            .ok_or(AnalysisError::Inconsistent(
                "original action target row shape",
            ))
    }
}

fn bound(body: &str, configuration: &str, options: &CheckOptions) -> Model {
    let text = format!("---- MODULE Source ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n");
    let analysis = dive_sema::analyze_source("Source.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse(configuration)
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}

fn captured(model: &Model, options: &CheckOptions) -> capture::BehaviorGraph {
    match capture::explore(model, options, capture::Limits::default()) {
        capture::Exploration::Complete(complete) => complete.graph,
        capture::Exploration::Stopped(result) => panic!("finite capture: {result:?}"),
    }
}

fn run(
    model: &Model,
    options: &CheckOptions,
    graph: &capture::BehaviorGraph,
    states: &mut impl StateSource,
) -> Result<Option<CandidateWord>, AnalysisError> {
    let limits = Limits::default();
    let control = evaluator_control(options).expect("control");
    let plans = prepare(model, limits, &control).expect("plans");
    analyze_source(
        model,
        &plans,
        usize::MAX,
        graph,
        states,
        limits,
        AcceptancePolicy::compact_default(),
        GraphLabelPolicy::NoCellLimit,
        &mut Stage::Unknown,
        &mut EvaluationErrors::default(),
        &control,
        None,
    )
}

fn summary(result: Result<Option<CandidateWord>, AnalysisError>) -> String {
    match result {
        Ok(None) => "no candidate".into(),
        Ok(Some(word)) => format!(
            "{} {:?} {:?} {} {:?}",
            word.target_property, word.properties, word.states, word.back_to, word.edges
        ),
        Err(error) => format!("{error:?}"),
    }
}

fn state(n: i64) -> StateVals {
    Arc::from([Some(Value::Int(n))])
}

/// Same destination, deliberately different original values and ordinal gaps.
fn divergent() -> capture::BehaviorGraph {
    capture::BehaviorGraph {
        states: vec![state(0), state(1), state(2)],
        roots: vec![0],
        edges: vec![
            vec![
                capture::Edge {
                    to: 1,
                    ordinal: Some(3),
                    label: Some("first".into()),
                },
                capture::Edge {
                    to: 1,
                    ordinal: Some(8),
                    label: Some("second".into()),
                },
                capture::Edge {
                    to: 0,
                    ordinal: None,
                    label: None,
                },
            ],
            vec![],
            vec![],
        ],
        concrete: vec![
            vec![
                capture::ConcreteSuccessor {
                    state: state(7),
                    ordinal: 3,
                    label: "first".into(),
                },
                capture::ConcreteSuccessor {
                    state: state(9),
                    ordinal: 8,
                    label: "second".into(),
                },
            ],
            vec![],
            vec![],
        ],
    }
}

#[test]
fn selected_targets_load_only_first_original_per_destination_and_needed_stutter() {
    let graph = divergent();
    let mut lazy = Lazy::new(&graph);
    let selected = selected_targets(&graph, 0, &mut lazy).expect("selected originals");
    assert_eq!(
        selected,
        std::collections::BTreeMap::from([(0, state(0)), (1, state(7))])
    );
    assert_ne!(
        selected[&1], graph.states[1],
        "representatives cannot replace originals"
    );
    assert_eq!(lazy.reads, [Read::Concrete(0, 0), Read::Representative(0)]);
    assert_eq!(
        selected,
        selected_targets(&graph, 0, &mut CapturedStates(&graph)).expect("reference")
    );
    assert!(
        selected_targets(&graph, 1, &mut lazy)
            .expect("frontier")
            .is_empty()
    );
    assert_eq!(lazy.reads.len(), 2, "empty frontier never loads a value");
}

#[test]
fn metadata_checks_do_not_load_duplicate_values_or_hide_malformed_rows() {
    let valid = divergent();
    let mut cases = vec![];
    let mut missing = valid.clone();
    missing.concrete[0].pop();
    cases.push(missing);
    let mut extra = valid.clone();
    let duplicate = extra.concrete[0][0].clone();
    extra.concrete[0].push(duplicate);
    cases.push(extra);
    let mut ordinal = valid.clone();
    ordinal.concrete[0][1].ordinal = 4;
    cases.push(ordinal);
    let mut label = valid.clone();
    label.concrete[0][1].label = "wrong".into();
    cases.push(label);
    let mut target = valid.clone();
    target.edges[0][2].to = 2;
    cases.push(target);
    let mut no_row = valid.clone();
    no_row.concrete.clear();
    cases.push(no_row);
    let mut bad_ref = valid.clone();
    bad_ref.edges[0][0].to = 3;
    cases.push(bad_ref);
    for graph in &cases {
        let mut lazy = Lazy::new(graph);
        let actual = selected_targets(graph, 0, &mut lazy).expect_err("malformed row");
        assert!(matches!(
            actual,
            AnalysisError::Inconsistent("original action target row shape")
        ));
        let expected =
            selected_targets(graph, 0, &mut CapturedStates(graph)).expect_err("reference error");
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        assert!(
            !lazy.reads.contains(&Read::Concrete(0, 1)),
            "duplicate values stay undemanded"
        );
    }
}

#[test]
fn lazy_labels_and_full_candidates_equal_reference_in_both_profiles() {
    let bodies = [
        (
            "Init == x=0\nNext == x'=(x+1)%3\nP == []<>(x=0)",
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        ),
        (
            "Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == <>(x=3)",
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        ),
        (
            "Init == x=0\nNext == x'=(x+1)%3\nP == []<>(ENABLED (x'=x+1))",
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        ),
        (
            "Init == x=0\nA == x'=(x+1)%3\nSpec == Init /\\ [][A]_x /\\ WF_x(A)\nP == []<>(x=2)",
            "SPECIFICATION Spec\nPROPERTY P\n",
        ),
        (
            "Init == x=0\nA == x'=(x+1)%3\nSpec == Init /\\ [][A]_x /\\ SF_x(A)\nP == []<><<A>>_x\nQ == <>[][x'=x]_x",
            "SPECIFICATION Spec\nPROPERTY P\nPROPERTY Q\n",
        ),
    ];
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for (body, config) in bodies {
            let model = bound(body, config, &options);
            let graph = captured(&model, &options);
            let limits = Limits::default();
            let control = evaluator_control(&options).expect("control");
            let plans = prepare(&model, limits, &control).expect("plans");
            let shape = BehaviorView::checked_captured(&graph).expect("topology");
            for prepared in &plans {
                let plan = &prepared.plan;
                let active =
                    crate::tlc_plan::active_actions(&plan.formula, limits.formula).expect("active");
                let label = |states: &mut dyn StateSource| {
                    labels(
                        &model,
                        plan,
                        &graph,
                        states,
                        shape,
                        "P",
                        GraphLabelPolicy::NoCellLimit,
                        &active,
                        &mut EvaluationErrors::default(),
                        &control,
                    )
                    .expect("labels")
                };
                let (expected, expected_edges) = label(&mut CapturedStates(&graph));
                let (actual, actual_edges) = label(&mut Lazy::new(&graph));
                assert_eq!(actual_edges, expected_edges);
                assert_eq!(actual, expected);
            }
            let expected = run(&model, &options, &graph, &mut CapturedStates(&graph));
            let mut metadata = graph.clone();
            metadata.states.clear();
            for target in metadata.concrete.iter_mut().flatten() {
                target.state = Arc::from([]);
            }
            let mut lazy = Lazy::new(&graph);
            assert_eq!(
                summary(run(&model, &options, &metadata, &mut lazy)),
                summary(expected),
                "{body}"
            );
        }
    }
}

#[test]
fn source_errors_and_typed_stops_propagate_at_every_demanded_read() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for property in ["<>(x=3)", "<>[][x'=x]_x"] {
            let body = format!("Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == {property}");
            let model = bound(&body, "INIT Init\nNEXT Next\nPROPERTY P\n", &options);
            let graph = captured(&model, &options);
            let mut normal = Lazy::new(&graph);
            run(&model, &options, &graph, &mut normal).expect("normal analysis");
            assert_ne!(normal.reads, [] as [Read; 0]);
            for failed in 1..=normal.reads.len() {
                for stopped in [false, true] {
                    let mut lazy = Lazy::new(&graph);
                    lazy.fail = Some(failed);
                    lazy.stopped = stopped;
                    let error = run(&model, &options, &graph, &mut lazy)
                        .expect_err("source failure never means success");
                    assert_eq!(lazy.reads, normal.reads[..failed]);
                    if stopped {
                        let AnalysisError::Stopped(actual) = error else {
                            panic!("typed source stop")
                        };
                        assert_eq!(actual.phase, dive_check::budget::Phase::Temporal);
                        assert_eq!((actual.limit, actual.observed), (10, 11));
                    } else {
                        assert!(matches!(error, AnalysisError::Limit("lazy source control")));
                    }
                }
            }
        }
    }
}

#[test]
fn original_word_reads_only_selected_representatives_and_preserves_edge_positions() {
    let mut graph = divergent();
    graph.edges[1].push(capture::Edge {
        to: 1,
        ordinal: None,
        label: None,
    });
    let lasso = crate::search::Lasso {
        root: 0,
        entry: 1,
        prefix: vec![crate::search::Step {
            from: 0,
            edge: 1,
            to: 1,
        }],
        cycle: vec![crate::search::Step {
            from: 1,
            edge: 0,
            to: 1,
        }],
    };
    let mut lazy = Lazy::new(&graph);
    let (states, ids, edges) = original_word(&graph, &mut lazy, None, &lasso).expect("word");
    assert_eq!(states, vec![state(0), state(1)]);
    assert_eq!(ids, [0, 1]);
    assert_eq!(
        edges,
        [graph.edges[0][1].clone(), graph.edges[1][0].clone()]
    );
    assert_eq!(
        lazy.reads,
        [Read::Representative(0), Read::Representative(1)]
    );
    let projection = row_demand::Projection {
        graph: crate::Graph::new(vec![vec![0], vec![0, 0, 1]], vec![1]).expect("projection"),
        sources: vec![1, 0],
        edges: vec![vec![0], vec![0, 1, 2]],
    };
    let projected = crate::search::Lasso {
        root: 1,
        entry: 0,
        prefix: vec![crate::search::Step {
            from: 1,
            edge: 1,
            to: 0,
        }],
        cycle: vec![crate::search::Step {
            from: 0,
            edge: 0,
            to: 0,
        }],
    };
    lazy.reads.clear();
    let actual =
        original_word(&graph, &mut lazy, Some(&projection), &projected).expect("projected word");
    assert_eq!(actual, (states, ids, edges));
    assert_eq!(
        lazy.reads,
        [Read::Representative(0), Read::Representative(1)]
    );
    lazy.reads.clear();
    lazy.fail = Some(1);
    assert!(matches!(
        original_word(&graph, &mut lazy, None, &lasso),
        Err(AnalysisError::Limit("lazy source control"))
    ));
}

#[test]
fn inactive_actions_zero_atoms_and_empty_frontiers_never_request_values() {
    let options = CheckOptions::tlc(0, 0);
    let model = bound(
        "Init == x=0\nNext == x'=x\nP == <>[][x'=x]_x",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
        &options,
    );
    let limits = Limits::default();
    let control = evaluator_control(&options).expect("control");
    let mut plans = prepare(&model, limits, &control).expect("plans");
    let plan = &mut plans[0].plan;
    assert!(plan.atoms.iter().all(|atom| atom.kind == AtomKind::Action));
    let mut graph = captured(&model, &options);
    graph.concrete.clear();
    let shape = BehaviorView::checked_captured(&graph).expect("topology");
    let mut lazy = Lazy::new(&graph);
    lazy.fail = Some(1);
    labels(
        &model,
        plan,
        &graph,
        &mut lazy,
        shape,
        "P",
        GraphLabelPolicy::NoCellLimit,
        &(std::collections::BTreeSet::new(), None),
        &mut EvaluationErrors::default(),
        &control,
    )
    .expect("inactive action ignores concrete shape");
    assert_eq!(lazy.reads, [] as [Read; 0]);
    plan.atoms.clear();
    labels(
        &model,
        plan,
        &graph,
        &mut lazy,
        shape,
        "P",
        GraphLabelPolicy::NoCellLimit,
        &(std::collections::BTreeSet::new(), None),
        &mut EvaluationErrors::default(),
        &control,
    )
    .expect("zero atoms");
    assert_eq!(lazy.reads, [] as [Read; 0]);
    let mut graph = divergent();
    for row in &mut graph.edges {
        row.clear();
    }
    for row in &mut graph.concrete {
        row.clear();
    }
    let mut lazy = Lazy::new(&graph);
    assert!(
        selected_targets(&graph, 0, &mut lazy)
            .expect("frontier")
            .is_empty()
    );
    assert_eq!(lazy.reads, [] as [Read; 0]);
}

#[test]
fn metadata_errors_precede_source_values_and_atom_error_priority_is_unchanged() {
    let mut graph = divergent();
    graph.edges[0][0].label = Some("mismatch".into());
    let mut lazy = Lazy::new(&graph);
    lazy.fail = Some(1);
    assert!(matches!(
        selected_targets(&graph, 0, &mut lazy),
        Err(AnalysisError::Inconsistent(
            "original action target row shape"
        ))
    ));
    assert_eq!(lazy.reads, [] as [Read; 0]);
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound(
            "Init == x=0\nNext == x'=1-x\nP == []<>(1 \\div (x-x)=0)",
            "INIT Init\nNEXT Next\nPROPERTY P\n",
            &options,
        );
        let mut graph = captured(&model, &options);
        graph.concrete.clear();
        let mut lazy = Lazy::new(&graph);
        let actual = run(&model, &options, &graph, &mut lazy);
        assert!(matches!(actual, Err(AnalysisError::Atom { .. })));
        assert_eq!(
            summary(actual),
            summary(run(&model, &options, &graph, &mut CapturedStates(&graph)))
        );
        assert_eq!(lazy.reads, [Read::Representative(0)]);
    }
}

#[test]
fn reference_rows_borrow_and_action_atoms_evaluate_once_per_destination() {
    use dive_value::{Ctx, DeferredKind, DeferredSet};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct Membership(Arc<AtomicUsize>);
    impl DeferredSet for Membership {
        fn kind(&self) -> DeferredKind {
            DeferredKind::Filter
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            Ok(vec![Value::Int(0)])
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Ok(true))
        }
    }
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound(
            "Init == x={0}\nNext == x'=x\nP == <>[][0 \\in x']_x",
            "INIT Init\nNEXT Next\nPROPERTY P\n",
            &options,
        );
        let mut graph = captured(&model, &options);
        let mut adapter = CapturedStates(&graph);
        let borrowed = adapter.representative(0).expect("reference row");
        assert!(
            matches!(borrowed, std::borrow::Cow::Borrowed(row) if std::ptr::eq(row, std::ptr::from_ref(&graph.states[0])))
        );
        let calls = Arc::new(AtomicUsize::new(0));
        graph.concrete[0][0].state = Arc::from([Some(Value::deferred(
            Arc::new(Membership(Arc::clone(&calls))),
            options.eval.ctx,
        ))]);
        let mut duplicate = graph.concrete[0][0].clone();
        duplicate.ordinal = 7;
        graph.concrete[0].push(duplicate);
        let mut edge = graph.edges[0][0].clone();
        edge.ordinal = Some(7);
        graph.edges[0].insert(1, edge);
        let limits = Limits::default();
        let control = evaluator_control(&options).expect("control");
        let plans = prepare(&model, limits, &control).expect("plans");
        let plan = &plans[0].plan;
        let active =
            crate::tlc_plan::active_actions(&plan.formula, limits.formula).expect("actions");
        let shape = BehaviorView::checked_captured(&graph).expect("topology");
        let label = |states: &mut dyn StateSource| {
            calls.store(0, Ordering::Relaxed);
            let (truth, _) = labels(
                &model,
                plan,
                &graph,
                states,
                shape,
                "P",
                GraphLabelPolicy::NoCellLimit,
                &active,
                &mut EvaluationErrors::default(),
                &control,
            )
            .expect("labels");
            assert_eq!(
                calls.load(Ordering::Relaxed),
                1,
                "duplicates and stutter share destination evaluation"
            );
            truth
        };
        let expected = label(&mut CapturedStates(&graph));
        let mut lazy = Lazy::new(&graph);
        assert_eq!(label(&mut lazy), expected);
        assert_eq!(lazy.reads, [Read::Concrete(0, 0), Read::Representative(0)]);
    }
}
