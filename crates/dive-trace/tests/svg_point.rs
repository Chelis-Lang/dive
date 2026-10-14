//! `PointOnLine` is independently evaluated without the successor enumerator.
#![allow(clippy::expect_used, reason = "functional assertions")]
use dive_trace::{TraceDoc, Validation, validate_doc};
use std::{path::PathBuf, sync::Arc};

#[test]
fn standalone_point_rejects_mathematical_floor_substitution() {
    let source = include_str!("../../dive-api/tests/fixtures/svg-point/Point.tla");
    let config = dive_config::parse(include_str!(
        "../../dive-api/tests/fixtures/svg-point/Point.cfg"
    ))
    .into_config()
    .expect("cfg");
    let mut sema = dive_sema::Options::default();
    sema.search.after_standard = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-eval/tests/fixtures/svg-point"),
    ];
    let analysis = dive_sema::analyze_source("Point.tla", source, &sema);
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    for opts in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::dive(),
    ] {
        let model = dive_model::bind(ir.clone(), &config, &opts).expect("bound independent model");
        let valid = TraceDoc::behavior(vec![serde_json::json!({"p":{"record":{"x":0,"y":1}}})])
            .expect("word");
        assert_eq!(
            validate_doc(&model, &valid, &[]).0.validation,
            Validation::Valid
        );
        let forged = TraceDoc::behavior(vec![serde_json::json!({"p":{"record":{"x":-1,"y":1}}})])
            .expect("word");
        assert!(matches!(
            validate_doc(&model, &forged, &[]).0.validation,
            Validation::Invalid(_)
        ));
    }
}
