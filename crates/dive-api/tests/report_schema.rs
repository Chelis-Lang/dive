//! Public report versions remain distinct from native trace payload versions.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, DfidRequest, SimulateRequest, TranslationRequest};
use serde_json::{Value, json};
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn envelope(value: &Value, command: &str) {
    assert_eq!(value["schema_version"], 4, "{value}");
    assert_eq!(value["command"], command);
}

#[test]
fn operation_reports_share_version_three_without_changing_trace_payloads() -> Result {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    let source = std::fs::read_to_string(&file)?;
    envelope(
        &dive_api::parse_source("Counter.tla", &source).to_json(false),
        "parse",
    );
    envelope(
        &dive_api::analyze_source("Counter.tla", &source, &dive_api::AnalyzeOptions::default())
            .to_json(),
        "analyze",
    );
    let check = dive_api::check_file(&CheckRequest {
        file: file.clone(),
        compat_tlc: true,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..Default::default()
    })?
    .to_json();
    envelope(&check, "check");
    assert_eq!(check["check_kind"], "safety");
    assert_eq!(check["verdict"], "success");
    assert_eq!(check["statistics"]["distinct"], 5);
    assert!(check.get("execution").is_none());
    assert!(check.get("properties").is_none());
    let simulation = dive_api::simulate_file(&SimulateRequest {
        file: file.clone(),
        seed: Some(0),
        traces: 1,
        depth: Some(2),
        ..Default::default()
    })?
    .to_json();
    envelope(&simulation, "simulate");
    assert_eq!(simulation["exhaustive"], false);
    assert_eq!(simulation["model_proved"], false);
    let dfid = dive_api::dfid_file(&DfidRequest {
        file: file.clone(),
        seed: Some(0),
        max_level: 2,
        ..Default::default()
    })?
    .to_json();
    envelope(&dfid, "dfid");
    assert_eq!(dfid["model_proved"], false);
    assert_eq!(dfid["termination"], "depth-bound");

    let model = SpecModel::load(&file, &file.with_extension("cfg"), &TraceOptions::default())?;
    let mut trace = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])?;
    let report = model.validate(&trace);
    assert_eq!(report.validation, Validation::Valid);
    envelope(&dive_api::trace::report_json(&report), "validate-trace");
    let payload = trace.to_json();
    assert_eq!(payload["format"], "dive-trace");
    assert_eq!(payload["version"], 1);
    assert_eq!(TraceDoc::from_json(&payload)?, trace);
    trace.states[1].insert("x".into(), json!(99));
    let invalid = model.validate(&trace);
    assert!(matches!(invalid.validation, Validation::Invalid(_)));
    let report = dive_api::trace::report_json(&invalid);
    envelope(&report, "validate-trace");
    assert_eq!(report["result"], "invalid");

    let refused = dive_api::trace::report_json(&dive_api::trace::Report {
        validation: Validation::Unsupported("unsupported control".into()),
        notes: Vec::new(),
        length: 0,
    });
    envelope(&refused, "validate-trace");
    assert_eq!(refused["result"], "unsupported");

    let pluscal = "---- MODULE Native ----\nEXTENDS Integers,TLC\n(* --algorithm A variable x = 0; begin L: x := 1; E: skip; end algorithm; *)\n====\n";
    let request = TranslationRequest::default();
    envelope(
        &dive_api::translate_source("Native.tla", pluscal, &request).to_json(),
        "translate",
    );
    envelope(
        &dive_api::original_ast_source("Native.tla", pluscal, &request).to_json(),
        "original-ast",
    );
    envelope(
        &dive_api::rewrite_source("Native.tla", pluscal, &dive_api::RewriteRequest::default())
            .to_json(),
        "translate-rewrite",
    );
    // Rejection reports use the same envelope and keep their original facts.
    let rejected =
        dive_api::parse_source("Bad.tla", "---- MODULE Bad ----\nx == 1 * -2\n====").to_json(false);
    envelope(&rejected, "parse");
    assert_eq!(rejected["accepted"], false);
    Ok(())
}
