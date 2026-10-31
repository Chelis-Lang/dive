//! Mandatory auxiliary replay preserves the oracle's formal trace absence.

use std::path::PathBuf;
use std::time::Duration;

use dive_api::trace::{SpecModel, TraceOptions};
use dive_api::{CheckRequest, Engine, check_file_engine};
use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::dive::{CheckMapping, check_result, check_result_with_model};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::OracleResult;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn source_file(corpus: &str, name: &str) -> PathBuf {
    root().join(format!(
        "conformance/corpus/{corpus}/specifications/{name}/{name}.tla"
    ))
}
fn spec(file: &std::path::Path) -> Result<SpecModel, Box<dyn std::error::Error>> {
    Ok(SpecModel::load(
        file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: true,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )?)
}
fn outcome(report: &Value) -> ProcessOutcome {
    ProcessOutcome {
        exit_code: Some(75),
        signal: None,
        timed_out: false,
        stdout: report.to_string(),
        stderr: String::new(),
        elapsed: Duration::ZERO,
    }
}
fn actual(file: &std::path::Path) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(check_file_engine(
        &CheckRequest {
            file: file.to_path_buf(),
            config: Some(file.with_extension("cfg")),
            compat_tlc: true,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        Engine::Reference,
    )?
    .to_json())
}
fn mapped(
    report: &Value,
    source: &SpecModel,
) -> Result<Box<OracleResult>, Box<dyn std::error::Error>> {
    match check_result_with_model(&outcome(report), true, source) {
        CheckMapping::Result(result) => Ok(result),
        CheckMapping::Refused(reason) => Err(reason.into()),
    }
}

#[test]
fn all_eight_actual_auxiliary_words_match_without_inventing_an_oracle_trace() -> TestResult {
    for name in [
        "InitialLiteral",
        "Issue620",
        "InitialStateLeft",
        "InitialStateRight",
        "InitialTupleNested",
        "RecordLiteral",
        "LetTuple",
        "InstanceTuple",
    ] {
        let file = source_file("comparison-receiver-primary", name);
        let report = actual(&file)?;
        assert!(report["counterexample"].is_null());
        assert_eq!(report["diagnostic_witness"]["trace"]["version"], 2);
        assert_eq!(
            report["diagnostic_witness"]["trace"]["ending"]["ec"],
            "GENERAL"
        );
        assert!(
            matches!(
                check_result(&outcome(&report), true),
                CheckMapping::Refused(_)
            ),
            "a source-free header cannot authorize this route"
        );
        let source = spec(&file)?;
        let result = mapped(&report, &source)?;
        assert!(result.trace.is_none());
        let pin:Value=serde_json::from_str(&std::fs::read_to_string(root().join(format!("conformance/golden/tlc-2026.10.06.014338/comparison-receiver-primary/specifications/{name}/{name}.cfg.json")))?)?;
        let pin: OracleResult = serde_json::from_value(pin["result"].clone())?;
        assert!(pin.trace.is_none());
        assert!(
            diff_results(
                &pin,
                &result,
                CompareContext {
                    workers: 1,
                    counts_comparable: true
                }
            )
            .is_empty(),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn missing_or_mutated_auxiliary_and_header_retagging_cannot_evade_replay() -> TestResult {
    let file = source_file("comparison-receiver-primary", "InitialLiteral");
    let report = actual(&file)?;
    let source = spec(&file)?;
    for (pointer, value) in [
        ("/diagnostic_witness", Value::Null),
        ("/diagnostic_witness/trace/ending/name", json!("Missing")),
        (
            "/diagnostic_witness/trace/ending/ec",
            json!("TLC_FINGERPRINT_EXCEPTION"),
        ),
        ("/diagnostic_witness/trace/states/0/values/x", json!(1)),
        ("/diagnostic_witness/trace/version", json!(1)),
        ("/tlc/reported", json!(1000)),
        ("/error_site", json!("init")),
        (
            "/error_witness",
            json!({"trace":report["diagnostic_witness"]["trace"].clone()}),
        ),
        (
            "/counterexample",
            json!({"trace":report["diagnostic_witness"]["trace"].clone()}),
        ),
    ] {
        let mut wrong = report.clone();
        if pointer == "/error_witness" {
            wrong["error_witness"] = value;
        } else {
            *wrong.pointer_mut(pointer).ok_or("field")? = value;
        }
        assert!(
            matches!(
                check_result_with_model(&outcome(&wrong), true, &source),
                CheckMapping::Refused(_)
            ),
            "{pointer}"
        );
    }
    for site in [
        Value::Null,
        json!("init"),
        json!("initial-states"),
        json!("invariant"),
    ] {
        let mut wrong = report.clone();
        wrong["diagnostic_witness"] = Value::Null;
        wrong["tlc"]["reported"] = json!(1000);
        wrong["error_site"] = site;
        assert!(
            matches!(
                check_result_with_model(&outcome(&wrong), true, &source),
                CheckMapping::Refused(_)
            ),
            "retagging cannot turn an actual emitted invariant error into Init generation"
        );
    }
    let mut omitted = report.clone();
    omitted
        .as_object_mut()
        .ok_or("object")?
        .remove("diagnostic_witness");
    omitted
        .as_object_mut()
        .ok_or("object")?
        .remove("error_site");
    omitted["tlc"]["reported"] = json!(1000);
    assert!(matches!(
        check_result_with_model(&outcome(&omitted), true, &source),
        CheckMapping::Refused(_)
    ));
    let foreign = spec(&source_file("comparison-receiver-primary", "NextDivZero"))?;
    assert!(matches!(
        check_result_with_model(&outcome(&report), true, &foreign),
        CheckMapping::Refused(_)
    ));
    Ok(())
}

#[test]
fn genuine_no_word_generation_and_function_routes_keep_their_old_results() -> TestResult {
    for (corpus, name) in [
        ("comparison-receiver-primary", "InitGeneration"),
        ("constant-retry-lazy", "WholeFunctionError"),
    ] {
        let file = source_file(corpus, name);
        let report = actual(&file)?;
        assert!(report["counterexample"].is_null());
        assert!(report["diagnostic_witness"].is_null());
        assert_eq!(report["tlc"]["reported"], 1000);
        let result = mapped(&report, &spec(&file)?)?;
        assert_eq!(result.verdict_code, Some(2147));
        assert!(result.trace.is_none());
    }
    Ok(())
}
