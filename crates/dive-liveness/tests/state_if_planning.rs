//! Pinned conditional source conversion, including early evaluation errors.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_liveness::formula::Limits;
use dive_liveness::plan::PlanningError;
use serde_json::Value;
use std::{path::Path, sync::Arc};

#[test]
fn complete_pinned_conditional_selection_preserves_error_phase_and_fallback() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probes/state-if-planning");
    let capture: Value = serde_json::from_slice(
        &std::fs::read(root.join("capture.json")).expect("pinned complete capture"),
    )
    .expect("capture JSON");
    let rows = capture["rows"].as_array().expect("complete rows");
    assert_eq!(rows.len(), 24);
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
        // TLC converts fairness conjuncts, then properties, stopping at the
        // first conversion error.
        let result = model
            .temporals
            .iter()
            .chain(&model.implied_temporals)
            .map(|named| dive_liveness::plan::lower_tlc(&model, &named.pred, Limits::default()))
            .collect::<Result<Vec<_>, _>>();
        if row["fields"]["completion"] == "planned" {
            let plans = result.expect(name);
            let (fairness, properties) = plans.split_at(model.temporals.len());
            let formulas = |plans: &[dive_liveness::plan::Plan]| {
                plans.iter().map(|p| p.formula.clone()).collect::<Vec<_>>()
            };
            let (fairness, properties) = (formulas(fairness), formulas(properties));
            dive_liveness::tlc_plan::classify(
                &fairness.iter().collect::<Vec<_>>(),
                &properties.iter().collect::<Vec<_>>(),
                Limits::default(),
            )
            .expect(name);
        } else {
            let Err(PlanningError::Evaluation { error, source }) = result else {
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
