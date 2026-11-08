//! Definition cache identities include the active INSTANCE substitution context.
#![allow(clippy::expect_used, reason = "pinned regression fixtures")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

const OPTS: &[&str] = &[
    "",
    "-memo",
    "-step-memo",
    "-eager",
    "-fold,-memo,-step-memo",
    "-hashcons",
];

fn request(file: PathBuf, compat_tlc: bool, workers: usize) -> CheckRequest {
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    // The ordinary harness exposes counts only when TLC prints them
    // (dive-conformance/src/dive.rs). Initial invariant failures omit them.
    let stats = if doc["statistics"]["reported_by_tlc"] == true {
        doc["statistics"].clone()
    } else {
        Value::Null
    };
    if doc["tlc"]["code"] == 2107 {
        // Retain an independent native assertion; these numbers are not
        // promoted to counts observed from TLC's initial-failure stream.
        assert_eq!(doc["statistics"]["distinct"], 1);
        assert_eq!(doc["statistics"]["generated"], 1);
        assert_eq!(doc["statistics"]["depth"], 1);
        assert_eq!(doc["statistics"]["reported_by_tlc"], false);
    }
    json!({
        "verdict": doc["verdict"], "property": doc["property"],
        "code": doc["tlc"]["code"], "error_class": doc["tlc"]["name"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "distinct": stats["distinct"],
        "generated": stats["generated"], "depth": stats["depth"],
    })
}

#[test]
fn original_holding_and_missed_violation_controls_match_the_oracle() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/core-instance-scopes/specifications");
    for (name, text) in [
        (
            "ScopeHolding",
            include_str!(
                "../../../conformance/golden/tlc-2026.10.06.014338/core-instance-scopes/specifications/ScopeHolding/ScopeHolding.cfg.json"
            ),
        ),
        (
            "ScopeViolation",
            include_str!(
                "../../../conformance/golden/tlc-2026.10.06.014338/core-instance-scopes/specifications/ScopeViolation/ScopeViolation.cfg.json"
            ),
        ),
    ] {
        let golden: Value = serde_json::from_str(text).expect("official golden");
        let expected = &golden["result"];
        for compat in [false, true] {
            for workers in [1, 4, 8] {
                let req = request(
                    fixtures.join(name).join(format!("{name}.tla")),
                    compat,
                    workers,
                );
                let check = |report: &dive_api::CheckReport| {
                    let actual = observed(report);
                    assert_eq!(report.to_json()["engine"], "core");
                    assert_eq!(
                        actual["verdict"],
                        if name == "ScopeHolding" {
                            "success"
                        } else {
                            "invariant-violation"
                        }
                    );
                    for (key, oracle_key) in [
                        ("code", "verdict_code"),
                        ("error_class", "error_class"),
                        ("exit", "exit_code"),
                        ("property", "violated_property"),
                    ] {
                        assert_eq!(
                            actual[key], expected[oracle_key],
                            "{name}/{compat}/{workers}/{key}"
                        );
                    }
                    for (key, oracle_key) in [
                        ("distinct", "distinct_states"),
                        ("generated", "states_generated"),
                        ("depth", "depth"),
                    ] {
                        assert_eq!(
                            actual[key], expected["counts"][oracle_key],
                            "{name}/{compat}/{workers}/{key}"
                        );
                    }
                };
                for opts in OPTS {
                    let core = check_file_core_opts(&req, opts).expect("core");
                    check(&core);
                }
                check(&dive_api::check_file(&req).expect("the default engine stays on core"));
                // Main's reference still has #515 on this exact Naturals dummy
                // body. Independent words are checked externally by the frozen
                // #515 validator, with its source/ELF identity in the receipt.
            }
        }
    }
}

fn validate(req: &CheckRequest, report: &dive_api::CheckReport, expected: &Value) {
    let doc = report.to_json();
    let payload = &doc["counterexample"]["trace"];
    if expected["verdict"] == "success" {
        assert!(payload.is_null());
        return;
    }
    assert!(!payload.is_null(), "required independent word is missing");
    let model = SpecModel::load(
        &req.file,
        req.config.as_ref().expect("config"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("fresh independent validator");
    let trace = TraceDoc::from_json(payload).expect("word decodes");
    assert!(
        matches!(model.validate(&trace).validation, Validation::Valid),
        "{}",
        doc
    );
    if !expected["underlying_class"].is_null() {
        assert_eq!(payload["ending"]["ec"], expected["underlying_class"]);
    }
    let mut mutant = payload.clone();
    mutant["states"][0]["values"]["x"] = json!(99);
    let mutant = TraceDoc::from_json(&mutant).expect("mutant decodes");
    assert!(matches!(
        model.validate(&mutant).validation,
        Validation::Invalid(_)
    ));
}

fn controls(role: &str) {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/core-instance-scopes/oracle.json"))
            .expect("compact official facts");
    let cases = oracle["cases"].as_object().expect("cases");
    assert_eq!(cases.len(), 9);
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-instance-scopes");
    for (name, expected) in cases {
        if expected["role"] != role {
            continue;
        }
        for compat in [false, true] {
            for workers in [1, 4, 8] {
                let req = request(root.join(format!("{name}.tla")), compat, workers);
                let reference = check_file_engine(&req, Engine::Core).expect("reference");
                let observed_reference = observed(&reference);
                for key in [
                    "verdict",
                    "property",
                    "code",
                    "error_class",
                    "exit",
                    "distinct",
                    "generated",
                    "depth",
                ] {
                    assert_eq!(
                        observed_reference[key], expected[key],
                        "{name}/{compat}/{workers}/{key}"
                    );
                }
                for opts in OPTS {
                    let core = check_file_core_opts(&req, opts).expect("core");
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        observed_reference,
                        "{name}/{compat}/{workers}/{opts}"
                    );
                    validate(&req, &core, expected);
                }
                let default = dive_api::check_file(&req).expect("default engine");
                assert_eq!(default.to_json()["engine"], "core");
                assert_eq!(observed(&default), observed_reference);
                validate(&req, &default, expected);
            }
        }
    }
}

#[test]
fn state_query_and_lexical_scopes_remain_distinct() {
    controls("state");
}

#[test]
fn primed_step_definitions_and_outer_actuals_remain_distinct() {
    controls("step");
}

#[test]
fn unproved_query_contexts_remain_explicit_refusals() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-m0");
    for name in ["EnabNonidentityInstance", "EnabConstInstanceInit"] {
        for compat in [false, true] {
            let req = request(fixtures.join(format!("{name}.tla")), compat, 1);
            let report = check_file_engine(&req, Engine::Core).expect("refusal");
            assert_eq!(report.to_json()["verdict"], "unsupported");
            assert_eq!(report.exit_code(), 3);
        }
    }
}
