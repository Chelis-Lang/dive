//! Packed labels must retain actual captured-row shape as an invariant.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "malformed capture controls"
)]

use super::*;
use std::sync::Arc;

fn model(property: &str) -> Model {
    let source = format!(
        "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == {property}\n====\n"
    );
    let analysis = dive_sema::analyze_source("M.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model")
}

fn captured(model: &Model) -> capture::BehaviorGraph {
    match dive_check::capture::explore(model, &CheckOptions::tlc(0, 0), capture::Limits::default())
    {
        capture::Exploration::Complete(captured) => captured.graph.clone(),
        other @ capture::Exploration::Stopped(_) => panic!("expected complete capture: {other:?}"),
    }
}

fn run(
    model: &Model,
    graph: &capture::BehaviorGraph,
) -> Result<Option<CandidateWord>, AnalysisError> {
    run_with_policy(
        model,
        graph,
        GraphLabelPolicy::Cells(Limits::default().atom_cells),
    )
}

fn run_with_policy(
    model: &Model,
    graph: &capture::BehaviorGraph,
    policy: GraphLabelPolicy,
) -> Result<Option<CandidateWord>, AnalysisError> {
    let limits = Limits::default();
    let control = evaluator_control(&CheckOptions::tlc(0, 0)).expect("control");
    let plans = prepare(model, limits, &control).expect("plan");
    let mut stage = Stage::Unknown;
    let mut action = EvaluationErrors::default();
    analyze_captured(
        model,
        &plans,
        usize::MAX,
        graph,
        limits,
        AcceptancePolicy::Legacy,
        policy,
        &mut stage,
        &mut action,
        &control,
        None,
    )
}

#[test]
fn missing_or_extra_captured_rows_are_shape_errors_never_false_labels() {
    let model = model("[]<>(x = x)");
    let valid = captured(&model);
    assert!(run(&model, &valid).expect("valid holding graph").is_none());
    let mut missing = valid.clone();
    missing.states.pop();
    let mut extra = valid.clone();
    extra.states.push(valid.states[0].clone());
    let mut extra_edges = valid.clone();
    extra_edges.edges.push(vec![]);
    for malformed in [missing, extra, extra_edges] {
        assert!(matches!(
            run(&model, &malformed),
            Err(AnalysisError::Product(product::ProductError::Shape(_)))
        ));
    }
}

#[test]
fn prefix_empty_rows_and_parallel_or_stutter_edges_remain_valid() {
    let model = model("[]<>(x = x)");
    let mut prefix = captured(&model);
    prefix.edges[1].clear();
    prefix.concrete[1].clear();
    assert!(
        run(&model, &prefix)
            .expect("unexpanded prefix row")
            .is_none()
    );
    let mut parallel = captured(&model);
    let duplicate = parallel.edges[0][0].clone();
    parallel.edges[0].push(duplicate);
    assert!(
        run(&model, &parallel)
            .expect("retained parallel edge")
            .is_none()
    );
}

#[test]
fn active_actions_keep_existing_concrete_row_validation() {
    let model = model("<>[][x' = x]_x");
    let mut malformed = captured(&model);
    malformed.concrete.clear();
    assert!(matches!(
        run(&model, &malformed),
        Err(AnalysisError::Inconsistent(
            "original action target row shape"
        ))
    ));
}

#[test]
fn removing_graph_cell_policy_keeps_shape_validation() {
    let model = model("[]<>(x = x)");
    let mut malformed = captured(&model);
    malformed.states.pop();
    assert!(matches!(
        run_with_policy(&model, &malformed, GraphLabelPolicy::NoCellLimit),
        Err(AnalysisError::Product(product::ProductError::Shape(_)))
    ));
}

#[test]
fn explicit_graph_cells_and_native_size_overflow_have_separate_authority() {
    for (policy, expected) in [
        (GraphLabelPolicy::Cells(0), false),
        (GraphLabelPolicy::Cells(5), false),
        (GraphLabelPolicy::Cells(6), true),
        (GraphLabelPolicy::NoCellLimit, true),
    ] {
        assert_eq!(policy.admit(2, 4, 1).is_ok(), expected);
    }
    assert!(
        GraphLabelPolicy::Cells(8_000_000)
            .admit(8_000_001, 0, 1)
            .is_err()
    );
    assert!(
        GraphLabelPolicy::Cells(usize::MAX)
            .admit(usize::MAX, 1, 1)
            .is_err()
    );
    // Removing this obsolete aggregate check does not allocate anything. Actual
    // packed-layout byte/shape overflow remains covered by packed-truth tests.
    assert!(
        GraphLabelPolicy::NoCellLimit
            .admit(usize::MAX, 1, usize::MAX)
            .is_ok()
    );
}
