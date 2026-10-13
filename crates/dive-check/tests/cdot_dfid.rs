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
fn dfid_composition_ignores_intermediates_without_admitting_effect_reasons() {
    for mode in [ActionComposition::Tlc, ActionComposition::Tla] {
        let mut model = model(mode);
        let options = dive_check::dfid::Options::new(dive_check::CheckOptions::tlc(0, 0), 2);
        let report = dive_check::dfid::check(&model, &options);
        assert!(
            matches!(
                report.outcome,
                dive_check::dfid::Outcome::Violation {
                    kind: "deadlock",
                    code: 2114,
                    ..
                }
            ),
            "{:?}",
            report.outcome
        );
        assert_eq!((report.generated, report.distinct, report.level), (1, 1, 2));
        model
            .sequential
            .push("TLCGet live checker state".to_owned());
        model.sequential_names_only = false;
        let report = dive_check::dfid::check(&model, &options);
        assert!(matches!(
            report.outcome,
            dive_check::dfid::Outcome::Unsupported(_)
        ));
        assert_eq!(report.generated, 0);
    }
    let report = dive_check::dfid::check(
        &model(ActionComposition::Off),
        &dive_check::dfid::Options::new(dive_check::CheckOptions::tlc(0, 0), 2),
    );
    assert!(matches!(
        report.outcome,
        dive_check::dfid::Outcome::Unsupported(ref feature) if feature.contains("action composition")
    ));
    assert_eq!(report.generated, 1);
}
