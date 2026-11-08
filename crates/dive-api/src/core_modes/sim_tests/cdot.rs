//! Pinned TLC `CdDeadlock` controls. TLC's simulation reports the deadlock
//! (2114) after one emitted state, as the retired reference simulator did;
//! Core refuses this composition by name in every composition mode (#910).
#![allow(clippy::expect_used, reason = "test fixture construction")]
use super::simulate;

use dive_eval::ActionComposition;
use std::sync::Arc;

fn model(mode: ActionComposition) -> dive_model::Model {
    let source =
        include_str!("../../../../dive-eval/tests/fixtures/cdot/specs/CdDeadlock/CdDeadlock.tla");
    let analysis =
        dive_sema::analyze_source("CdDeadlock.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let mut options = dive_model::BindOptions::tlc();
    options.eval.action_composition = mode;
    dive_model::bind(ir, &config, &options).expect("model")
}

#[test]
fn simulation_composition_is_a_named_core_refusal() {
    let options = dive_sim::Options::new(0, dive_stdlib::random::RandomMode::Tlc, 2, 1);
    for mode in [
        ActionComposition::Tlc,
        ActionComposition::Tla,
        ActionComposition::Off,
    ] {
        let report = simulate(&model(mode), &options);
        assert!(
            matches!(report.outcome, dive_sim::Outcome::Unsupported(ref feature)
                if feature.contains("action composition")),
            "{mode:?}: {:?}",
            report.outcome
        );
        assert!(report.traces.is_empty(), "{mode:?}");
    }
}
