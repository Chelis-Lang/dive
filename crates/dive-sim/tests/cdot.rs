//! Pinned TLC `CdDeadlock` controls: DFID/simulation count emitted states only.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use dive_eval::ActionComposition;
use std::sync::Arc;

fn model(mode: ActionComposition) -> dive_model::Model {
    let source =
        include_str!("../../dive-eval/tests/fixtures/cdot/specs/CdDeadlock/CdDeadlock.tla");
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
fn simulation_composition_counts_only_emitted_states() {
    let options = dive_sim::Options::new(0, dive_stdlib::random::RandomMode::Tlc, 2, 1);
    for mode in [ActionComposition::Tlc, ActionComposition::Tla] {
        let report = dive_sim::simulate(&model(mode), &options);
        assert!(
            matches!(
                report.outcome,
                dive_sim::Outcome::Violation {
                    kind: "deadlock",
                    code: 2114,
                    ..
                }
            ),
            "{:?}",
            report.outcome
        );
        assert_eq!(report.generated, 1);
        assert_eq!(report.traces_started, 1);
        assert_eq!(report.traces[0].states.len(), 1);
    }
    let report = dive_sim::simulate(&model(ActionComposition::Off), &options);
    assert!(
        matches!(report.outcome, dive_sim::Outcome::Unsupported(ref feature) if feature.contains("action composition"))
    );
    assert_eq!(report.generated, 1);
}
