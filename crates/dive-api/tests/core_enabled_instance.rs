//! ENABLED can cross INSTANCE scopes whose variable mappings preserve names.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-m0")
        .join(format!("{name}.tla"))
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let d = report.to_json();
    json!({
        "verdict": d["verdict"], "property": d["property"],
        "distinct": d["statistics"]["distinct"],
        "generated": d["statistics"]["generated"],
        "depth": d["statistics"]["depth"],
        "code": d["tlc"]["code"], "class": d["tlc"]["name"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "trace": d["counterexample"]["trace"],
    })
}

fn validate_word(file: &Path, compat_tlc: bool, actual: &Value, error: bool) {
    let model = SpecModel::load(
        file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent validator binds");
    let word = TraceDoc::from_json(&actual["trace"]).expect("word decodes");
    assert!(matches!(
        model.validate(&word).validation,
        Validation::Valid
    ));
    let mut mutant = actual["trace"].clone();
    if error {
        mutant["ending"]["ec"] = json!("TLC_VALUE_ASSERT_FAILED");
    } else {
        mutant["states"][1]["values"]["x"] = json!(0);
    }
    let mutant = TraceDoc::from_json(&mutant).expect("mutant decodes");
    assert!(matches!(
        model.validate(&mutant).validation,
        Validation::Invalid(_)
    ));
}

fn check_case(name: &str, bad: bool, error: bool, states: u64) {
    let file = fixture(name);
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
            let expected =
                observed(&check_file_engine(&req, Engine::Reference).expect("reference"));
            assert_eq!(
                expected["code"],
                if error {
                    2111
                } else if bad {
                    2110
                } else {
                    2193
                }
            );
            assert_eq!(
                expected["exit"],
                if error {
                    76
                } else if bad {
                    12
                } else {
                    0
                }
            );
            assert_eq!(expected["distinct"], states);
            assert_eq!(
                expected["generated"],
                if bad || error { states } else { states + 1 }
            );
            assert_eq!(expected["depth"], states);
            for switches in ["", "-eager", "-memo", "-step-memo", "-memo,-step-memo"] {
                let actual = check_file_core_opts(&req, switches).expect("core");
                assert_eq!(actual.to_json()["engine"], "core");
                let actual = observed(&actual);
                assert_eq!(actual, expected, "{name}/{compat_tlc}/{workers}/{switches}");
                if bad || error {
                    validate_word(&file, compat_tlc, &actual, error);
                }
            }
        }
    }
}

#[test]
fn parameter_instances_preserve_query_results_and_demand_order() {
    // Pinned TLC tlc-2026.10.06.014338, jar SHA-256
    // 7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d,
    // BFS/W1/seed0/fp0/final: holding controls 4 distinct/5 generated/depth4;
    // Bad is Inv/2110/exit12 and Error is 2111/2179/exit76, each 2/2/2.
    // Lazy's unused 1\div0 is never demanded. Chain retains the same root
    // variable through two instance scopes; Op substitutes a constant operator.
    for name in [
        "EnabConstInstance",
        "EnabConstInstanceLazy",
        "EnabConstInstanceChain",
        "EnabConstInstanceOp",
    ] {
        check_case(name, false, false, 4);
    }
    check_case("EnabConstInstanceGuard", false, false, 3);
    check_case("EnabConstInstanceBad", true, false, 2);
    check_case("EnabConstInstanceError", false, true, 2);
}

#[test]
fn nonidentity_and_hidden_query_contexts_remain_explicitly_refused() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![
        fixture("EnabNonidentityInstance"),
        fixture("EnabConstInstanceInit"),
    ];
    for corpus in [
        "lazy-enabled-capture",
        "lazy-enabled-cache",
        "lazy-query-lifetimes",
    ] {
        let dir = root
            .join("conformance/corpus")
            .join(corpus)
            .join("specifications");
        for entry in std::fs::read_dir(dir).expect("corpus") {
            let entry = entry.expect("entry");
            if entry.file_name() != "root_remove_two" {
                files.push(entry.path().join("CallbackEnabled.tla"));
            }
        }
    }
    files.sort();
    for file in files {
        for compat_tlc in [false, true] {
            let req = CheckRequest {
                config: Some(file.with_extension("cfg")),
                file: file.clone(),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let report = check_file_engine(&req, Engine::Core).expect("check returns refusal");
            let value = report.to_json();
            assert_eq!(
                value["verdict"],
                "unsupported",
                "{}: {value}",
                file.display()
            );
            assert!(value.to_string().contains("INSTANCE"));
            if file
                .file_stem()
                .is_some_and(|s| s == "EnabConstInstanceInit")
            {
                assert!(value.to_string().contains("initial predicate"));
                let reference = check_file_engine(&req, Engine::Reference).expect("reference");
                let reference = reference.to_json();
                assert_eq!(reference["verdict"], "success");
                assert_eq!(reference["statistics"]["distinct"], 4);
            }
        }
    }
}

#[test]
fn constant_parameters_cannot_capture_state_across_the_query_boundary() {
    // Pinned SANY/TLC rejects all seven sources: code 3002, exit 150.
    // Constant/operator substitutions must be level zero. Formal forwarding
    // preserves that maximum-level constraint at the instance application.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-enabled-levels");
    for name in [
        "EnabConstState",
        "EnabConstPrime",
        "EnabOperatorState",
        "EnabOperatorPrime",
        "EnabHiddenOperatorState",
        "EnabConstFormal",
        "EnabOperatorFormal",
    ] {
        let file = dir.join(format!("{name}.tla"));
        for compat_tlc in [false, true] {
            for engine in [Engine::Core, Engine::Reference] {
                let req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file: file.clone(),
                    compat_tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let report = check_file_engine(&req, engine).expect("frontend rejects");
                assert_eq!(report.exit_code_for(ExitConvention::Tlc), 150);
                let value = report.to_json();
                assert_eq!(value["verdict"], "rejected", "{name}/{compat_tlc}");
                assert_eq!(value["tlc"]["code"], 3002);
                assert!(
                    value["diagnostics"]
                        .as_array()
                        .expect("diagnostics")
                        .iter()
                        .any(|d| d["code"]
                            == if name.ends_with("Formal") {
                                "DIVE-E0600"
                            } else {
                                "DIVE-E0602"
                            })
                );
            }
        }
    }
}

#[test]
fn unproved_configured_captures_effects_and_calls_stay_static_refusals() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-enabled-boundaries");
    for name in [
        "EnabConfiguredState",
        "EnabConfiguredPrime",
        "EnabConfiguredHiddenState",
        "EnabConfiguredHiddenPrime",
        "EnabConfiguredRedirect",
        "EnabPrint",
        "EnabRandom",
        "EnabRegister",
        "EnabRecursive",
    ] {
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                let file = dir.join(format!("{name}.tla"));
                let req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file,
                    compat_tlc,
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let reference = check_file_engine(&req, Engine::Reference).expect("reference");
                let reference = reference.to_json();
                assert_eq!(reference["verdict"], "success", "{name}");
                let states = if matches!(name, "EnabPrint" | "EnabRandom" | "EnabRegister") {
                    3
                } else {
                    4
                };
                assert_eq!(reference["statistics"]["distinct"], states);
                assert_eq!(reference["statistics"]["generated"], states + 1);
                assert_eq!(reference["statistics"]["depth"], states);
                let core = check_file_engine(&req, Engine::Core).expect("refusal");
                let core = core.to_json();
                assert_eq!(core["verdict"], "unsupported", "{name}: {core}");
                let reason = if name.starts_with("EnabConfigured") {
                    "state-dependent configured operator"
                } else {
                    "effects or recursive/nested calls"
                };
                assert!(
                    core["message"].as_str().expect("reason").contains(reason),
                    "{name}: {core}"
                );
            }
        }
    }
}
