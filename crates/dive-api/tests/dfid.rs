//! Native DFID completion and independently checked witness controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test controls")]
#[path = "support/refused.rs"]
mod refused;

use dive_api::DfidRequest;
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};

fn request(name: &str, level: u32, seed: i64) -> DfidRequest {
    DfidRequest {
        file: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/checker/dfid/{name}.tla")),
        compat_tlc: true,
        max_level: level,
        seed: Some(7),
        worker_seeds: Some(vec![seed; level as usize]),
        ..DfidRequest::default()
    }
}

#[test]
fn completion_certificate_and_zero_exit_are_separate() {
    let report = dive_api::dfid_file(&request("DfidBound", 6, 0)).expect("report");
    let json = report.to_json();
    assert_eq!(report.exit_status(), 0);
    assert_eq!(json["result"], "incomplete");
    assert_eq!(json["termination"], "depth-bound");
    assert_eq!(json["model_proved"], false);
    assert_eq!(json["statistics"]["generated"], 16);
    assert_eq!(json["statistics"]["distinct"], 6);
    assert_eq!(json["profile"], "tlc+overrides");
    assert_eq!(
        json["options"]["worker_seeds"],
        serde_json::json!([0, 0, 0, 0, 0])
    );
    let completed = dive_api::dfid_file(&request("DfidCycle", 6, 0))
        .expect("report")
        .to_json();
    assert_eq!(completed["result"], "complete");
    assert_eq!(completed["model_proved"], true);
    assert_eq!(completed["statistics"]["generated"], 7);
    for bound in [0, 1] {
        let report = dive_api::dfid_file(&request("DfidBound", bound, 0)).expect("report");
        assert_eq!(report.exit_status(), 255);
        assert_eq!(report.to_json()["model_proved"], false);
    }
    let mut native = request("DfidBound", 6, 0);
    native.compat_tlc = false;
    native.worker_seeds = None;
    assert_eq!(
        dive_api::dfid_file(&native).expect("report").exit_status(),
        3
    );
}

#[test]
fn cross_root_witness_is_valid_and_mutant_is_rejected() {
    for seed in [0, 1, 7, -1] {
        let req = request("DfidRootsMixed", 6, seed);
        let report = dive_api::dfid_file(&req).expect("report");
        let json = report.to_json();
        assert_eq!(json["result"], "violation", "{json}");
        assert_eq!(json["violation_validated"], true);
        assert_eq!(json["model_proved"], false);
        assert_eq!(report.exit_status(), 12);
        let mut trace = TraceDoc::from_json(&json["counterexample"]["trace"]).expect("trace");
        assert_eq!(trace.states.len(), 2);
        let model = SpecModel::load(
            &req.file,
            &req.file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: true,
                ..TraceOptions::default()
            },
        )
        .expect("model");
        assert_eq!(model.validate(&trace).validation, Validation::Valid);
        trace.states[1].insert("x".to_owned(), serde_json::json!(99));
        assert!(matches!(
            model.validate(&trace).validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn finite_deadlock_is_independently_certified() {
    let json = dive_api::dfid_file(&request("DfidDeadlock", 6, 0))
        .expect("report")
        .to_json();
    assert_eq!(json["observed_tlc_code"], 2114);
    assert_eq!(json["result"], "violation");
    assert_eq!(json["violation_validated"], true);
    assert_eq!(json["model_proved"], false);
    assert_eq!(json["options"]["workers_used"], 1);
}

#[test]
fn ordered_start_up_names_validate_independently() {
    // `ToString(8)` and `ToString(7)` yield runtime strings whose order CHOOSE
    // depends on. Only the retired reference engine ran this (#791, #910);
    // Core refuses it by name.
    let report = dive_api::dfid_file(&request("DfidSeedNames", 6, 0)).expect("structured report");
    let json = report.to_json();
    assert_eq!(json["result"], "unsupported", "{json}");
    assert_eq!(report.exit_status(), 3, "{json}");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains(refused::FINITE_STRING_ORDER)),
        "{json}"
    );
    assert!(json["counterexample"].is_null(), "{json}");
    assert_eq!(json["violation_validated"], false);
    assert_eq!(json["model_proved"], false);
}

#[test]
fn load_failure_reports_unknown_statistics() {
    let json = dive_api::dfid_file(&request("MissingModel", 6, 0))
        .expect("structured error")
        .to_json();
    assert_eq!(json["options"]["workers_used"], 0);
    assert_eq!(json["options"]["execution_stage"], "model-loading");
    assert_eq!(json["statistics"]["generated"], serde_json::Value::Null);
    assert_eq!(json["model_proved"], false);
}

#[test]
fn zero_width_precedes_even_model_loading() {
    let mut req = request("MissingModel", 6, 0);
    req.workers = Some(0);
    let report = dive_api::dfid_file(&req).expect("parameter report");
    let json = report.to_json();
    assert_eq!(report.exit_status(), 1);
    assert_eq!(json["observed_tlc_code"], 1102);
    assert_eq!(json["options"]["execution_stage"], "parameter-validation");
    assert_eq!(json["options"]["workers_used"], 0);
    assert_eq!(json["statistics"]["generated"], serde_json::Value::Null);
    assert_eq!(json["model_proved"], false);
}

#[test]
fn constrained_initial_property_is_refused_before_uncertifiable_witness() {
    let dir = std::env::temp_dir().join(format!("dive-dfid-constrained-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("directory");
    let file = dir.join("ConstrainedInitial.tla");
    std::fs::write(
        &file,
        "---- MODULE ConstrainedInitial ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x\nLimit == x > 0\nP == x > 0\n====\n",
    )
    .expect("source");
    std::fs::write(
        dir.join("ConstrainedInitial.cfg"),
        "INIT Init\nNEXT Next\nCONSTRAINT Limit\nPROPERTY P\n",
    )
    .expect("config");
    let report = dive_api::dfid_file(&DfidRequest {
        file,
        compat_tlc: true,
        max_level: 6,
        seed: Some(7),
        ..DfidRequest::default()
    })
    .expect("report");
    let json = report.to_json();
    assert_eq!(report.exit_status(), 3, "{json}");
    assert_eq!(json["result"], "unsupported", "{json}");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|m| m.contains("initial properties with state constraints")),
        "{json}"
    );
    assert_eq!(json["statistics"]["generated"], 0, "{json}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}
