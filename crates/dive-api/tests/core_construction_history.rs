//! Actual producer-to-consumer set history and state handoff, against pinned TLC.
#![allow(clippy::expect_used, reason = "official pinned test fixtures")]
use dive_api::{CheckRequest, ExitConvention, check_file_core_opts};
use serde_json::Value;
use std::path::PathBuf;
fn request(name: &str, compat: bool, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/construction-set-history/specifications");
    let file = root.join(name).join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc: compat,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn primary(name: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../conformance/golden/tlc-2026.10.06.014338/construction-set-history/specifications",
    );
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(name).join(format!("{name}.cfg.json")))
            .expect("official golden"),
    )
    .expect("golden JSON");
    assert!(
        doc["provenance"]["reason"]
            .as_str()
            .expect("reason")
            .contains("#623")
    );
    doc["result"].clone()
}
#[test]
fn literal_map_union_and_nested_except_cross_the_real_state_boundary() {
    for name in [
        "ModelMapUnion",
        "MapAliasDemand",
        "UnionEmptyAlias",
        "ModelRawPrefix",
        "ExceptSetHandoff",
        "ExcludedRawState",
        "EmptyRecordQuery",
        "EmptySetQuery",
        "RepeatedCandidateSet",
        "RepeatedCandidateNested",
    ] {
        let oracle = primary(name);
        for compat in [false, true] {
            for workers in [1, 2, 3, 8] {
                for opts in [
                    "",
                    "-fold",
                    "-memo",
                    "-member-index",
                    "-hashcons",
                    "-early-probe",
                ] {
                    let report = check_file_core_opts(&request(name, compat, workers), opts)
                        .expect("core invocation");
                    let doc = report.to_json();
                    assert_eq!(
                        doc["verdict"], "success",
                        "{name}/{compat}/{workers}/{opts}: {doc}"
                    );
                    assert_eq!(
                        Value::from(report.exit_code_for(ExitConvention::Tlc)),
                        oracle["exit_code"]
                    );
                    assert_eq!(
                        doc["statistics"]["distinct"], oracle["counts"]["distinct_states"],
                        "{name}: {doc}"
                    );
                    assert_eq!(
                        doc["statistics"]["generated"], oracle["counts"]["states_generated"],
                        "{name}: {doc}"
                    );
                    assert_eq!(
                        doc["statistics"]["depth"], oracle["counts"]["depth"],
                        "{name}: {doc}"
                    );
                }
            }
        }
    }
}
#[test]
fn bad_model_prefix_cannot_be_skipped_to_claim_tlc_hit() {
    let name = "ModelBadPrefix";
    assert_eq!(primary(name)["verdict"], "evaluation-error");
    for workers in [1, 8] {
        let report =
            check_file_core_opts(&request(name, true, workers), "").expect("core invocation");
        assert_eq!(
            report.to_json()["verdict"],
            "unsupported",
            "{}",
            report.to_json()
        );
    }
}

#[test]
fn actual_run_budget_stops_new_bulk_construction_without_a_verdict() {
    use dive_api::BudgetSetting;
    use std::time::{Duration, Instant};
    let dir = std::env::temp_dir().join(format!("dive-construction-budget-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary directory");
    for initial in [true, false] {
        let file = dir.join("Bulk.tla");
        let (init, next) = if initial {
            (
                "x=Cardinality({<<i,j>>:i \\in 1..10000,j \\in 1..10000})",
                "UNCHANGED x",
            )
        } else {
            (
                "x=0",
                "x=0 /\\ Cardinality({<<i,j>>:i \\in 1..10000,j \\in 1..10000})>0 /\\ x'=1",
            )
        };
        std::fs::write(&file,format!("---- MODULE Bulk ----\nEXTENDS Naturals, FiniteSets\nVARIABLE x\nInit == {init}\nNext == {next}\n====\n")).expect("source");
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
        )
        .expect("config");
        for compat in [false, true] {
            for workers in [1, 8] {
                let req = CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    compat_tlc: compat,
                    workers: Some(workers),
                    time_limit: BudgetSetting::Explicit(Duration::from_millis(50)),
                    max_memory: BudgetSetting::Explicit(1024 * 1024 * 1024),
                    ..CheckRequest::default()
                };
                let started = Instant::now();
                let report = check_file_core_opts(&req, "").expect("actual file route");
                let doc = report.to_json();
                assert_eq!(
                    doc["verdict"], "stopped",
                    "{initial}/{compat}/{workers}: {doc}"
                );
                assert_eq!(report.exit_code(), 5);
                assert!(
                    doc["tlc"].is_null(),
                    "a budget stop must make no TLC verdict claim: {doc}"
                );
                assert!(
                    started.elapsed() < Duration::from_secs(3),
                    "owned bulk stop must return promptly"
                );
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("temporary inputs removed");
}

#[test]
fn actual_history_handoff_words_replay_and_reject_mutated_middle_states() {
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    let name = "MapUnionViolation";
    let oracle = primary(name);
    for compat in [false, true] {
        for workers in [1, 2, 3, 8] {
            let req = request(name, compat, workers);
            let options = TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..TraceOptions::default()
            };
            let model = SpecModel::load(&req.file, req.config.as_ref().expect("config"), &options)
                .expect("independent model");
            for opts in ["", "-hashcons", "-memo,-fold,-early-probe"] {
                let report = check_file_core_opts(&req, opts).expect("core invocation");
                let doc = report.to_json();
                assert_eq!(
                    doc["verdict"], "invariant-violation",
                    "{compat}/{workers}/{opts}: {doc}"
                );
                assert_eq!(
                    Value::from(report.exit_code_for(ExitConvention::Tlc)),
                    oracle["exit_code"]
                );
                for (key, expected) in [
                    ("distinct", "distinct_states"),
                    ("generated", "states_generated"),
                    ("depth", "depth"),
                ] {
                    assert_eq!(doc["statistics"][key], oracle["counts"][expected], "{doc}");
                }
                let word =
                    TraceDoc::from_json(&doc["counterexample"]["trace"]).expect("actual word");
                assert!(matches!(
                    model.validate(&word).validation,
                    Validation::Valid
                ));
                let mut mutant = doc["counterexample"]["trace"].clone();
                mutant["states"][1]["values"]["step"] = Value::from(99);
                let mutant = TraceDoc::from_json(&mutant).expect("mutant decodes");
                assert!(matches!(
                    model.validate(&mutant).validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}

#[test]
fn printed_and_register_effects_do_not_erase_an_actual_bulk_budget_stop() {
    use dive_api::BudgetSetting;
    use std::time::Duration;
    let dir =
        std::env::temp_dir().join(format!("dive-effect-history-budget-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary directory");
    for initial in [true, false] {
        for print in [true, false] {
            let effect = if print {
                "PrintT(\"owned-budget-effect\")"
            } else {
                "TLCSet(0,42)"
            };
            let file = dir.join("Effects.tla");
            let bulk = "Cardinality({<<i,j>>:i \\in 1..10000,j \\in 1..10000})";
            let init = if initial {
                format!("{effect} /\\ x={bulk}")
            } else {
                "x=0".into()
            };
            let next = if initial {
                "UNCHANGED x".into()
            } else {
                format!("x=0 /\\ {effect} /\\ {bulk}>0 /\\ x'=1")
            };
            std::fs::write(&file,format!("---- MODULE Effects ----\nEXTENDS Naturals,FiniteSets,TLC\nVARIABLE x\nInit == {init}\nNext == {next}\n====\n")).expect("source");
            std::fs::write(
                file.with_extension("cfg"),
                "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
            )
            .expect("config");
            for compat in [false, true] {
                for workers in [1, 8] {
                    let req = CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc: compat,
                        workers: Some(workers),
                        time_limit: BudgetSetting::Explicit(Duration::from_millis(100)),
                        max_memory: BudgetSetting::Explicit(1024 * 1024 * 1024),
                        ..CheckRequest::default()
                    };
                    let report = check_file_core_opts(&req, "").expect("actual file route");
                    let doc = report.to_json();
                    assert_eq!(
                        doc["verdict"], "stopped",
                        "{initial}/{print}/{compat}/{workers}: {doc}"
                    );
                    assert_eq!(report.exit_code(), 5);
                    assert!(doc["tlc"].is_null());
                    assert_eq!(doc["stopped"]["resource"], "time", "{doc}");
                    assert_eq!(doc["stopped"]["phase"], "safety-search", "{doc}");
                    assert_eq!(doc["options"]["workers"]["requested"], workers);
                    assert_eq!(doc["options"]["workers"]["used"], 1);
                    assert_eq!(doc["statistics"]["distinct"], u64::from(!initial));
                    assert_eq!(doc["statistics"]["generated"], u64::from(!initial));
                    assert_eq!(doc["statistics"]["depth"], u64::from(!initial));
                    if print {
                        assert_eq!(doc["output"].as_array().expect("output").len(), 1);
                        assert!(
                            doc["output"][0]
                                .as_str()
                                .expect("printed text")
                                .contains("owned-budget-effect")
                        );
                    }
                }
            }
        }
    }
    for print in [false, true] {
        let effect = if print {
            "PrintT(\"effect-error\")"
        } else {
            "TLCSet(0,42)"
        };
        let file = dir.join("Effects.tla");
        std::fs::write(&file,format!("---- MODULE Effects ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x=0\nNext == {effect} /\\ x'=1 \\div 0\n====\n")).expect("source");
        for compat in [false, true] {
            for workers in [1, 8] {
                let req = CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    compat_tlc: compat,
                    workers: Some(workers),
                    ..CheckRequest::default()
                };
                let report = check_file_core_opts(&req, "").expect("effect error");
                let doc = report.to_json();
                assert_eq!(doc["verdict"], "unsupported", "{doc}");
                assert_eq!(report.exit_code(), 3);
                assert!(doc["stopped"].is_null());
                assert!(doc["tlc"].is_null());
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("remove inputs");
}
