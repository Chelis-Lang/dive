//! Actual pinned conditional source conversion, including early errors.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_liveness::formula::Limits;
use dive_liveness::plan::PlanningError;
use dive_liveness::tlc_plan::{ModelPlanningError, lower_model};
use serde_json::Value;
use std::{path::Path, sync::Arc};

#[test]
fn complete_pinned_conditional_selection_preserves_error_phase_and_fallback() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/probes/state-if-planning");
    let capture: Value = serde_json::from_slice(
        &std::fs::read(root.join("capture.json")).expect("pinned complete capture"),
    )
    .expect("capture JSON");
    let rows = capture["rows"].as_array().expect("complete rows");
    assert_eq!(rows.len(), 18);
    for row in rows {
        let name = row["id"].as_str().expect("case name");
        let directory = root.join("models").join(name);
        let source =
            std::fs::read_to_string(directory.join(format!("{name}.tla"))).expect("source");
        let analysis = dive_sema::analyze_source(
            &format!("{name}.tla"),
            &source,
            &dive_sema::Options::default(),
        );
        if row["fields"]["phase"] == "binding" {
            assert!(!analysis.accepted(), "{name}: reject before planning");
            continue;
        }
        assert!(analysis.accepted(), "{name}: {:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse(
            &std::fs::read_to_string(directory.join(format!("{name}.cfg"))).expect("configuration"),
        )
        .into_config()
        .expect("parsed configuration");
        let model =
            dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bound model");
        let result = lower_model(&model, Limits::default());
        if row["fields"]["completion"] == "planned" {
            let bound = result.expect(name);
            assert_eq!(
                bound.projection.orders.len().to_string(),
                row["fields"]["groups"].as_str().expect("group count"),
                "{name}"
            );
            let group = &bound.projection.orders[0];
            assert_eq!(
                group.check_state.len().to_string(),
                row["fields"]["g0.state"]
            );
            assert_eq!(
                group.check_action.len().to_string(),
                row["fields"]["g0.action"]
            );
        } else {
            let Err(ModelPlanningError::Bound(PlanningError::Evaluation { error, source })) =
                result
            else {
                panic!("{name}: expected actual early evaluation failure, got {result:?}");
            };
            assert_eq!(
                error.ec().code().to_string(),
                row["fields"]["error_code"],
                "{name}"
            );
            assert_eq!(
                error.message().replace('\n', " "),
                row["fields"]["message"],
                "{name}"
            );
            let actual = error
                .origin()
                .expect("original inner error expression retained");
            let origin = model.ir.nodes()[actual.index()].origin;
            assert_eq!(origin.file, source.origin.file, "{name}");
            assert!(
                origin.range.0 < origin.range.1,
                "{name}: inner error source span"
            );
            assert!(
                source.origin.range.0 < source.origin.range.1,
                "{name}: conversion operand span"
            );
        }
        assert!(
            model.output.is_empty(),
            "{name}: no effects during pure planning"
        );
    }
}
