//! The borrowed input changes topology ownership, not atom callbacks or their priority.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite callback and malformed-graph controls"
)]

use super::*;
use crate::Graph;
use crate::product::BehaviorView;
use dive_value::{Ctx, DeferredKind, DeferredSet, Value, ValueError};
use std::sync::{Arc, Mutex};

type Calls = Arc<Mutex<Vec<&'static str>>>;
type LabelResult = Result<Vec<Option<crate::search::Lasso>>, String>;

#[derive(Debug)]
struct Probe {
    calls: Calls,
    fail: bool,
}
impl DeferredSet for Probe {
    fn kind(&self) -> DeferredKind {
        self.calls.lock().expect("calls").push("kind");
        DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
        self.calls.lock().expect("calls").push("elements");
        Ok(vec![Value::Int(0)])
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
        self.calls.lock().expect("calls").push("member");
        Some(if self.fail {
            Err(ValueError::new(
                dive_value::ErrorKind::Membership,
                "borrowed-topology-provider-error",
            ))
        } else {
            Ok(true)
        })
    }
}

fn fixture(options: &CheckOptions, fail: bool) -> (Model, capture::BehaviorGraph, Calls) {
    let analysis = dive_sema::analyze_source(
        "Borrowed.tla",
        "---- MODULE Borrowed ----\nVARIABLE x\nInit == x = {0}\nNext == x' = x\nP == []<>(0 \\in x)\n====\n",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    let model = dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model");
    let mut graph = match crate::test_capture::explore(&model, options, capture::Limits::default())
    {
        capture::Exploration::Complete(complete) => complete.graph,
        other @ capture::Exploration::Stopped(_) => panic!("finite capture: {other:?}"),
    };
    let calls = Arc::new(Mutex::new(vec![]));
    let provider = Value::deferred(
        Arc::new(Probe {
            calls: Arc::clone(&calls),
            fail,
        }),
        options.eval.ctx,
    );
    graph.states[0] = Arc::from([Some(provider)]);
    calls.lock().expect("calls").clear();
    (model, graph, calls)
}

fn captured_result(
    model: &Model,
    graph: &capture::BehaviorGraph,
    options: &CheckOptions,
    policy: GraphLabelPolicy,
) -> Result<Option<CandidateWord>, AnalysisError> {
    let limits = Limits::default();
    let control = evaluator_control(options).expect("control");
    let plans = prepare(model, limits, &control).expect("plans");
    analyze_captured(
        model,
        &plans,
        usize::MAX,
        graph,
        limits,
        AcceptancePolicy::Cells,
        policy,
        &mut Stage::Unknown,
        &mut EvaluationErrors::default(),
        &control,
    )
}

fn label_path(
    borrowed: bool,
    options: &CheckOptions,
    fail: bool,
) -> (LabelResult, Vec<&'static str>) {
    let (model, graph, calls) = fixture(options, fail);
    let limits = Limits::default();
    let control = evaluator_control(options).expect("control");
    let plans = prepare(&model, limits, &control).expect("plan");
    let dense = Graph::new(
        graph
            .edges
            .iter()
            .map(|row| row.iter().map(|edge| edge.to).collect())
            .collect(),
        graph.roots.clone(),
    )
    .expect("original copied topology");
    let shape = if borrowed {
        BehaviorView::checked_captured(&graph).expect("borrowed topology")
    } else {
        BehaviorView::Dense(&dense)
    };
    let prepared = &plans[0];
    assert!(
        prepared
            .plan
            .atoms
            .iter()
            .all(|atom| atom.kind == AtomKind::State)
    );
    calls.lock().expect("calls").clear();
    let result = (|| {
        let truth = labels(
            &model,
            &prepared.plan,
            &graph,
            &mut CapturedStates(&graph),
            shape,
            &model.property_claims()[0].name,
            GraphLabelPolicy::Cells(limits.atom_cells),
            &(std::collections::BTreeSet::new(), None),
            &mut EvaluationErrors::default(),
            &control,
        )
        .map_err(|error| format!("{error:?}"))?;
        let mut words = vec![];
        for group in &prepared.groups {
            let product = ImplicitProduct::new(
                shape,
                &group.tableau,
                &truth,
                limits.product,
                AcceptancePolicy::Cells,
            )
            .map_err(|error| format!("{error:?}"))?;
            for conditions in &group.alternatives {
                words.push(
                    product
                        .find_lasso_with_conditions(&truth, conditions)
                        .map_err(|error| format!("{error:?}"))?,
                );
            }
        }
        Ok(words)
    })();
    let observed = calls.lock().expect("calls").clone();
    (result, observed)
}

#[test]
fn copied_and_borrowed_labels_pair_actual_provider_calls_and_errors() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for fail in [false, true] {
            let original = label_path(false, &options, fail);
            let borrowed = label_path(true, &options, fail);
            assert_eq!(borrowed, original);
            assert!(
                borrowed.1.contains(&"member"),
                "the real deferred callback must run"
            );
            if fail {
                assert!(
                    borrowed
                        .0
                        .expect_err("provider error")
                        .contains("borrowed-topology-provider-error")
                );
            } else {
                assert!(borrowed.0.is_ok());
            }
        }
    }
}

#[test]
fn malformed_public_topology_precedes_policy_state_shape_and_callbacks() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for bad_root in [true, false] {
            for missing_states in [false, true] {
                let (model, mut graph, calls) = fixture(&options, true);
                if bad_root {
                    graph.roots.insert(0, 7);
                }
                graph.edges[0][0].to = 8;
                if missing_states {
                    graph.states.clear();
                }
                calls.lock().expect("calls").clear();
                let expected = if bad_root {
                    crate::GraphError {
                        vertex: 7,
                        source: None,
                    }
                } else {
                    crate::GraphError {
                        vertex: 8,
                        source: Some(0),
                    }
                };
                let Err(error) =
                    captured_result(&model, &graph, &options, GraphLabelPolicy::Cells(0))
                else {
                    panic!("malformed topology cannot produce a word or hold");
                };
                assert!(
                    matches!(error, AnalysisError::Product(product::ProductError::Shape(ref text)) if text == &expected.to_string())
                );
                assert!(calls.lock().expect("calls").is_empty());
            }
        }
    }
}

#[test]
fn policy_then_state_shape_keep_priority_before_provider_callbacks() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let (model, mut graph, calls) = fixture(&options, true);
        graph.states.clear();
        for policy in [GraphLabelPolicy::Cells(0), GraphLabelPolicy::NoCellLimit] {
            calls.lock().expect("calls").clear();
            let Err(error) = captured_result(&model, &graph, &options, policy) else {
                panic!("missing state rows cannot produce FALSE labels");
            };
            match policy {
                GraphLabelPolicy::Cells(_) => {
                    assert!(matches!(error, AnalysisError::Limit("atom cells")));
                }
                GraphLabelPolicy::NoCellLimit => assert!(
                    matches!(error, AnalysisError::Product(product::ProductError::Shape(ref text)) if text == "state predicate dimensions")
                ),
            }
            assert!(calls.lock().expect("calls").is_empty());
        }
    }
}
