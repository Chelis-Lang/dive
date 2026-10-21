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
    match capture::explore(model, &CheckOptions::tlc(0, 0), capture::Limits::default()) {
        capture::Exploration::Complete(captured) => captured.graph.clone(),
        other @ capture::Exploration::Stopped(_) => panic!("expected complete capture: {other:?}"),
    }
}

fn run(
    model: &Model,
    graph: &capture::BehaviorGraph,
) -> Result<Option<CandidateWord>, AnalysisError> {
    let limits = Limits::default();
    let control = evaluator_control(&CheckOptions::tlc(0, 0)).expect("control");
    let plans = prepare(model, limits, &control).expect("plan");
    let mut stage = Stage::Unknown;
    let mut action = None;
    analyze_captured(
        model,
        &plans,
        usize::MAX,
        graph,
        limits,
        AcceptancePolicy::Legacy,
        &mut stage,
        &mut action,
        &control,
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
