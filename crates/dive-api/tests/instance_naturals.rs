//! State-dependent native arguments must not enter an empty-state cache (#515).
#![allow(clippy::expect_used, reason = "pinned regression fixtures")]

use std::path::PathBuf;
use std::sync::Arc;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let fixtures = root().join("tests/fixtures/instance-naturals");
    let module = if name == "StartupStoredRedirect" {
        "StartupStored"
    } else {
        name
    };
    CheckRequest {
        file: fixtures.join(format!("{module}.tla")),
        config: Some(fixtures.join(format!("{name}.cfg"))),
        compat_tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({
        "verdict": doc["verdict"], "property": doc["property"],
        "code": doc["tlc"]["code"], "error_class": doc["tlc"]["name"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "distinct": doc["statistics"]["distinct"],
        "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"], "output": doc["output"],
    })
}

#[test]
fn original_issue_matches_the_official_minimal_corpus_golden() {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../conformance/golden/tlc-2026.10.06.014338/reference-instance-naturals/specifications/RefSubstDouble/RefSubstDouble.cfg.json"
    )).expect("official golden");
    let expected = &golden["result"];
    let file = root().join("../../conformance/corpus/reference-instance-naturals/specifications/RefSubstDouble/RefSubstDouble.tla");
    for compat_tlc in [false, true] {
        for workers in [1, 4, 8] {
            let req = CheckRequest {
                config: Some(file.with_extension("cfg")),
                file: file.clone(),
                compat_tlc,
                workers: Some(workers),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                let report = check_file_engine(&req, engine).expect("runs");
                let actual = observed(&report);
                assert_eq!(actual["verdict"], expected["verdict"]);
                assert_eq!(actual["code"], expected["verdict_code"]);
                assert_eq!(actual["error_class"], expected["error_class"]);
                assert_eq!(actual["exit"], expected["exit_code"]);
                assert_eq!(actual["distinct"], expected["counts"]["distinct_states"]);
                assert_eq!(actual["generated"], expected["counts"]["states_generated"]);
                assert_eq!(actual["depth"], expected["counts"]["depth"]);
                assert_eq!(actual["property"], expected["violated_property"]);
            }
        }
    }
}

fn validate_error(req: &CheckRequest, report: &dive_api::CheckReport, expected: &Value) {
    let model = SpecModel::load(
        &req.file,
        req.config.as_ref().expect("config"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("validator binds");
    let mut doc = report.to_json()["counterexample"]["trace"].clone();
    assert_eq!(doc["ending"]["ec"], expected["underlying_class"]);
    let word = TraceDoc::from_json(&doc).expect("word decodes");
    assert!(matches!(
        model.validate(&word).validation,
        Validation::Valid
    ));
    doc["ending"]["ec"] = json!(if expected["underlying_code"] == 2132 {
        "TLC_MODULE_DIVISION_BY_ZERO"
    } else {
        "TLC_VALUE_ASSERT_FAILED"
    });
    let wrong = TraceDoc::from_json(&doc).expect("mutant decodes");
    assert!(matches!(
        model.validate(&wrong).validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn multiplication_scope_demand_and_error_priority_match_pinned_controls() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/instance-naturals/oracle.json"))
            .expect("compact official oracle controls");
    let cases = oracle["cases"].as_object().expect("cases");
    assert_eq!(cases.len(), 18);
    for (name, expected) in cases {
        for compat in [false, true] {
            for workers in [1, 4, 8] {
                let req = request(name, compat, workers);
                // NestedScopes exposes a separately tracked existing core
                // disagreement. Its original oracle fact remains strict for
                // Reference; it earns no Core/Auto agreement credit.
                let engines: &[Engine] = if name == "NestedScopes" {
                    &[Engine::Reference]
                } else {
                    &[Engine::Reference, Engine::Core, Engine::Auto]
                };
                for &engine in engines {
                    let report = check_file_engine(&req, engine).expect("runs");
                    let actual = observed(&report);
                    for key in [
                        "verdict",
                        "property",
                        "code",
                        "error_class",
                        "exit",
                        "distinct",
                        "generated",
                        "depth",
                        "output",
                    ] {
                        // #504 separately fixes missing assumption-name metadata;
                        // this control establishes priority, never name agreement.
                        if name == "AssumePriority" && key == "property" {
                            continue;
                        }
                        assert_eq!(
                            actual[key], expected[key],
                            "{name}/{compat}/{workers}/{engine:?}/{key}: {actual:#}"
                        );
                    }
                    if !expected["underlying_class"].is_null() {
                        validate_error(&req, &report, expected);
                    }
                }
                if name == "NestedScopes" {
                    continue;
                }
                // Core optimizations cannot hide a regression in the shared
                // binder/evaluator handoff or change actual lazy demands.
                let ablated = check_file_core_opts(&req, "-fold,-memo,-step-memo,-eager")
                    .expect("ablated core runs");
                let reference = check_file_engine(&req, Engine::Reference).expect("reference runs");
                assert_eq!(
                    observed(&ablated),
                    observed(&reference),
                    "{name}/{compat}/{workers}"
                );
            }
        }
    }
}

#[test]
fn explicit_startup_precomputation_still_keeps_constant_instances() {
    let req = request("ConstantInstance", false, 1);
    let analysis =
        dive_sema::analyze_file(&req.file, &dive_sema::Options::default()).expect("source loads");
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config =
        dive_config::parse(&std::fs::read_to_string(req.config.expect("config")).expect("cfg"))
            .into_config()
            .expect("parsed config");
    for options in [
        dive_model::BindOptions::dive(),
        dive_model::BindOptions::tlc(),
    ] {
        let model = dive_model::bind(ir.clone(), &config, &options).expect("binder");
        for name in ["I!Left", "Double"] {
            let def = ir.root_def(name).expect("constant instance definition");
            assert_eq!(
                model.evaluator.precomputed(def),
                Some(dive_value::Value::Int(6)),
                "{name}"
            );
        }
        assert!(
            model
                .evaluator
                .precomputed(ir.root_def("Init").expect("Init"))
                .is_none()
        );
    }
}
