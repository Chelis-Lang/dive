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

fn canonical_initial_success(value: &Value, exit: Option<i32>) {
    assert_eq!(value["verdict"], "success", "{value:#}");
    assert_eq!(exit, Some(0));
    for field in ["distinct", "generated", "depth"] {
        assert_eq!(value["statistics"][field], 3, "{field}/{value:#}");
    }
    assert_eq!(value["statistics"]["left_on_queue"], 0);
    for field in ["counterexample", "error_witness", "diagnostic_witness"] {
        assert!(value[field].is_null(), "{field}/{value:#}");
    }
}

#[test]
fn context_only_initial_errors_keep_unclassified_oracles_and_refuse_native_class_claims()
-> TestResult {
    use dive_conformance::tlc::{messages, result};
    for (name, pinned, sha256) in [
        (
            "InitialTool",
            include_str!("fixtures/initial-context-unclassified/InitialTool.stdout"),
            "77eb050796d1cb1fc30b433a666553fc898cc9152eeb5e00fac5aeb0a35a6936",
        ),
        (
            "InitialOverride",
            include_str!("fixtures/initial-context-unclassified/InitialOverride.stdout"),
            "a7ab40e040f98033631201bf9899be1e619b349cf8453f564e2c570e8018b695",
        ),
    ] {
        assert_eq!(
            dive_conformance::hash::sha256_bytes(pinned.as_bytes()),
            sha256
        );
        let transcript = messages::parse(pinned);
        assert_eq!(transcript.anomalies, [] as [String; 0]);
        let oracle = result::classify(
            &transcript,
            &result::RunFacts {
                exit_code: Some(75),
                ..result::RunFacts::default()
            },
        );
        assert_eq!(oracle.verdict, result::Verdict::Unclassified);
        assert_eq!(oracle.error_codes, [2102, 2103]);
        assert_eq!(oracle.exit_code, Some(75));
        let file = root().join(format!(
            "crates/dive-api/tests/fixtures/comparison-receiver-unclassified/{name}.tla"
        ));
        for compat in [false, true] {
            let source = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc: compat,
                    run: Some((19, 0)),
                    ..TraceOptions::default()
                },
            )?;
            for (engine, workers) in [Engine::Reference, Engine::Core, Engine::Auto]
                .into_iter()
                .flat_map(|engine| [1, 4].map(|workers| (engine, workers)))
            {
                let report = check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc: compat,
                        workers: Some(workers),
                        seed: Some(19),
                        fp: Some(0),
                        ..CheckRequest::default()
                    },
                    engine,
                )?;
                let value = report.to_json();
                let mut actual_outcome = outcome(&value);
                actual_outcome.exit_code = Some(i32::from(
                    report.exit_code_for(dive_api::ExitConvention::Tlc),
                ));
                if name == "InitialOverride" && !compat {
                    // Canonical construction (D-028) avoids the legacy sorting
                    // error. The pinned primary observation above remains
                    // unclassified; it supplies no oracle agreement credit.
                    canonical_initial_success(&value, actual_outcome.exit_code);
                    continue;
                }
                if name == "InitialOverride" && engine == Engine::Core {
                    assert_eq!(value["verdict"], "unsupported");
                    assert_eq!(report.exit_code_for(dive_api::ExitConvention::Tlc), 3);
                    assert!(matches!(
                        check_result(&actual_outcome, compat),
                        CheckMapping::Refused(_)
                    ));
                    continue;
                }

                assert_eq!(report.exit_code_for(dive_api::ExitConvention::Tlc), 75);
                assert_eq!(value["tlc"]["verified"], false);
                assert_eq!(value["tlc"]["code"], 2103);
                assert_eq!(value["tlc"]["reported"], 2102);
                assert!(value["counterexample"].is_null());
                assert!(value["error_witness"].is_null());
                assert!(value["diagnostic_witness"].is_null());
                assert!(matches!(
                    check_result(&actual_outcome, compat),
                    CheckMapping::Refused(_)
                ));
                assert!(matches!(
                    check_result_with_model(&actual_outcome, compat, &source),
                    CheckMapping::Refused(_)
                ));
                for field in ["distinct", "generated", "depth", "left_on_queue"] {
                    assert_eq!(value["statistics"][field], 1);
                }
                let human = report.render_human();
                assert!(human.contains("While working on the initial state:"));
                assert!(human.contains("no proved deciding class"));
                assert!(!human.contains("The behavior up to this point"));
            }
        }
    }
    Ok(())
}

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
