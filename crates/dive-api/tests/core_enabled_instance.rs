//! ENABLED can cross INSTANCE scopes whose variable mappings preserve names.
#![allow(clippy::expect_used, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_enabled_instance.json");

use std::path::{Path, PathBuf};

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file, check_file_core_opts};
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
            let expected = observed(&check_file(&req).expect("reference"));
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
            agreed::check_eq(AGREED, "check_case/0", &(states), &(expected["distinct"]));
            assert_eq!(
                expected["generated"],
                if bad || error { states } else { states + 1 }
            );
            agreed::check_eq(AGREED, "check_case/1", &(states), &(expected["depth"]));
            for switches in ["", "-eager", "-memo", "-step-memo", "-memo,-step-memo"] {
                let actual = check_file_core_opts(&req, switches).expect("core");
                assert_eq!(actual.to_json()["engine"], "core");
                let actual = observed(&actual);
                agreed::check_eq(AGREED, "check_case/2", &(actual), &(expected));
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
            let report = check_file(&req).expect("check returns refusal");
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
            {
                let req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file: file.clone(),
                    compat_tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let report = check_file(&req).expect("frontend rejects");
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
fn pure_recursive_calls_in_identity_instance_queries_run_on_core() {
    // Pinned TLC tlc-2026.10.06.014338 (jar SHA-256 7beec0f0...a62fa193),
    // BFS/W1/seed0/fp0: EnabRecursive succeeds with 4 distinct/5 generated/
    // depth 4. A call whose whole plan is effect-free and reaches no nested
    // query cannot observe the instance cutoff (identity substitutions).
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-enabled-boundaries/EnabRecursive.tla");
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
            let expected = observed(&check_file(&req).expect("reference"));
            assert_eq!(expected["verdict"], "success");
            assert_eq!(
                [
                    &expected["distinct"],
                    &expected["generated"],
                    &expected["depth"]
                ],
                [4, 5, 4]
            );
            for switches in ["", "-eager", "-memo", "-step-memo", "-memo,-step-memo"] {
                let actual = check_file_core_opts(&req, switches).expect("core");
                assert_eq!(actual.to_json()["engine"], "core");
                agreed::check_eq(
                    AGREED,
                    "pure_recursive_calls_in_identity_instance_queries_run_on_core/0",
                    &(observed(&actual)),
                    &(expected),
                );
            }
        }
    }
}

#[test]
fn identity_instance_fairness_over_recursive_functions_runs_native_atoms() {
    // Pinned TLC tlc-2026.10.06.014338, BFS/W1/seed0/fp0: RecLive succeeds
    // and RecLiveBad violates Prop (2116, exit 13), each 6/6/6. Every atom,
    // including `~ENABLED Sched!Step` and the WF enabledness of an action
    // reading the recursive `Sum`, is evaluated natively by Core.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-enabled-recursive-live");
    for (name, bad) in [("RecLive", false), ("RecLiveBad", true)] {
        let file = dir.join(format!("{name}.tla"));
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
            let reference = check_file(&req).expect("reference");
            let expected = observed(&reference);
            assert_eq!(expected["exit"], if bad { 13 } else { 0 }, "{name}");
            assert_eq!(
                [
                    &expected["distinct"],
                    &expected["generated"],
                    &expected["depth"]
                ],
                [6, 6, 6]
            );
            let core = check_file(&req).expect("core");
            let value = core.to_json();
            assert_eq!(value["engine"], "core", "{name}: {value}");
            let actual = observed(&core);
            for key in [
                "verdict",
                "property",
                "distinct",
                "generated",
                "depth",
                "code",
                "exit",
            ] {
                assert_eq!(actual[key], expected[key], "{name}/{compat_tlc}/{key}");
            }
            let atoms = &value["core"]["metrics"]["liveness"]["atoms"];
            assert_eq!(
                atoms["eligible_atoms"], atoms["total_atoms"],
                "{name}: {atoms}"
            );
            assert!(
                atoms["rejections"]
                    .as_object()
                    .expect("rejections")
                    .values()
                    .all(|count| count == 0),
                "{name}: {atoms}"
            );
            assert_eq!(atoms["negated_enabled_evaluations"], 6, "{name}: {atoms}");
            assert_eq!(
                atoms["reference_evaluations"]["state"], 0,
                "{name}: {atoms}"
            );
            assert_eq!(
                atoms["reference_evaluations"]["action"], 0,
                "{name}: {atoms}"
            );
            if bad {
                let model = SpecModel::load(
                    &file,
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
        "EnabRecursivePrint",
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
                // Only the retired reference engine ran these (#791, #910).
                let core = check_file(&req).expect("refusal");
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
