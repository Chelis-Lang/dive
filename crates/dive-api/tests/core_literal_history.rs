//! Consumed local literal-set history; oracle facts predate native changes.
#![allow(clippy::expect_used, reason = "pinned test fixtures")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file_core_opts};
use serde_json::{Value, json};

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/literal-set-history-primary/specifications");
    let file = root.join(name).join(format!("{name}.tla"));
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

fn primary(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/golden/tlc-2026.10.06.014338/literal-set-history-primary/specifications").join(name).join(format!("{name}.cfg.json"));
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("official generated golden"))
            .expect("golden JSON");
    assert!(
        golden["provenance"]["reason"]
            .as_str()
            .expect("reason")
            .contains("#623")
    );
    golden["result"].clone()
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict":doc["verdict"], "code":doc["tlc"]["code"], "exit":report.exit_code_for(ExitConvention::Tlc), "distinct":doc["statistics"]["distinct"], "generated":doc["statistics"]["generated"], "depth":doc["statistics"]["depth"], "property":doc["property"]})
}

fn assert_primary(report: &dive_api::CheckReport, oracle: &Value) {
    let actual = observed(report);
    assert_eq!(actual["code"], oracle["verdict_code"]);
    assert_eq!(actual["exit"], oracle["exit_code"]);
    assert_eq!(actual["distinct"], oracle["counts"]["distinct_states"]);
    assert_eq!(actual["generated"], oracle["counts"]["states_generated"]);
    assert_eq!(actual["depth"], oracle["counts"]["depth"]);
    assert_eq!(actual["property"], oracle["violated_property"]);
}

const OPTS: &[&str] = &[
    "",
    "-fold",
    "-memo",
    "-hashcons",
    "-fold,-memo,-hashcons,-eager",
];

#[test]
fn selected_literal_producers_and_local_aliases_reach_checked_consumers() {
    for name in [
        "AliasDemand",
        "IndependentDemand",
        "SharedAliasDemand",
        "DuplicateFirstRepresentative",
        "RawScalarEarlyHit",
        "RawTupleEarlyHit",
        "TupleFirstFieldDemand",
        "TupleLengthDemand",
        "TuplePermutationForward",
        "TuplePermutationReverse",
        "TypedHoleUncovered",
        "UncoveredUnionProducer",
    ] {
        let oracle = primary(name);
        assert_eq!(oracle["verdict"], "success");
        for compat in [false, true] {
            for workers in [1, 4] {
                let req = request(name, compat, workers);
                for opts in OPTS {
                    let report = check_file_core_opts(&req, opts).expect("core invocation");
                    assert_eq!(report.to_json()["engine"], "core");
                    assert_eq!(
                        report.to_json()["verdict"],
                        "success",
                        "{name}/{compat}/{workers}/{opts}: {}",
                        report.to_json()
                    );
                    assert_primary(&report, &oracle);
                    if name == "AliasDemand" {
                        assert_eq!(
                            report.to_json()["core"]["metrics"]["plan"]["folded"],
                            0,
                            "the selected constructor/consumer must remain live"
                        );
                        assert!(
                            report.to_json()["core"]["metrics"]["memo"]
                                .as_array()
                                .expect("memo site observations")
                                .iter()
                                .all(|site| !site["site"]
                                    .as_str()
                                    .expect("site name")
                                    .starts_with("Inv")),
                            "history-sensitive consumers are not keyed memo sites"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn raw_prefix_failures_stay_unclaimed_and_default_d009_is_unchanged() {
    for name in ["RawScalarPrefixError", "RawTuplePrefixError"] {
        let oracle = primary(name);
        assert_eq!(oracle["verdict"], "evaluation-error");
        for compat in [false, true] {
            for opts in OPTS {
                let report =
                    check_file_core_opts(&request(name, compat, 1), opts).expect("core invocation");
                let doc = report.to_json();
                if compat {
                    assert_eq!(doc["verdict"], "unsupported", "{name}/{opts}: {doc}");
                    assert!(
                        doc["message"]
                            .as_str()
                            .expect("reason")
                            .contains("provenance")
                    );
                } else {
                    // D-009 may find the later structural hit. This is an
                    // approved default outcome, not parity with TLC's error.
                    assert_eq!(doc["verdict"], "success", "{name}/{opts}: {doc}");
                    assert_eq!(doc["statistics"]["distinct"], 3);
                    assert_eq!(doc["statistics"]["generated"], 3);
                }
            }
        }
    }
    for name in [
        "LiteralNormalizationError",
        "TupleLaterFieldError",
        "UncoveredRecordProducer",
    ] {
        for compat in [false, true] {
            for opts in OPTS {
                let report =
                    check_file_core_opts(&request(name, compat, 1), opts).expect("core invocation");
                assert_eq!(
                    report.to_json()["verdict"],
                    "unsupported",
                    "{name}/{compat}/{opts}: {}",
                    report.to_json()
                );
            }
        }
    }
}

#[test]
fn covered_literal_violation_words_validate_independently() {
    let name = "LiteralViolationWord";
    let oracle = primary(name);
    assert_eq!(oracle["verdict"], "safety-violation");
    for compat in [false, true] {
        let req = request(name, compat, 1);
        let options = TraceOptions {
            compat_tlc: compat,
            run: Some((0, 0)),
            ..TraceOptions::default()
        };
        let model = SpecModel::load(&req.file, &req.file.with_extension("cfg"), &options)
            .expect("independent validator binds");
        for opts in OPTS {
            let report = check_file_core_opts(&req, opts).expect("core invocation");
            assert_eq!(report.to_json()["verdict"], "invariant-violation");
            assert_primary(&report, &oracle);
            let doc = report.to_json();
            let word =
                TraceDoc::from_json(&doc["counterexample"]["trace"]).expect("native word decodes");
            assert!(matches!(
                model.validate(&word).validation,
                Validation::Valid
            ));
            let roundtrip: Value =
                serde_json::from_str(&doc["counterexample"]["trace"].to_string())
                    .expect("serialized native word");
            let roundtrip = TraceDoc::from_json(&roundtrip).expect("serialized word decodes");
            assert!(matches!(
                model.validate(&roundtrip).validation,
                Validation::Valid
            ));
            let mut bad = doc["counterexample"]["trace"].clone();
            bad["states"][1]["values"]["x"] = json!(99);
            let bad = TraceDoc::from_json(&bad).expect("mutant word decodes");
            assert!(matches!(
                model.validate(&bad).validation,
                Validation::Invalid(_)
            ));
        }
    }
}

#[test]
fn larger_literal_collections_use_actual_work_budgets_without_a_new_count_limit() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "dive-literal-eligibility-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary spec directory");
    let file = dir.join("LargerLiteral.tla");
    let values = (0..64).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    std::fs::write(&file, format!("---- MODULE LargerLiteral ----\nEXTENDS Integers, FiniteSets\nVARIABLE x\nInit == x = 0\nNext == x < 2 /\\ x' = x+1\nInv == x=0 \\/ Cardinality({{{values}}}) = 64\n====\n")).expect("spec");
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )
    .expect("config");
    for compat in [false, true] {
        let req = CheckRequest {
            file: file.clone(),
            config: Some(file.with_extension("cfg")),
            compat_tlc: compat,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        for opts in OPTS {
            let report = check_file_core_opts(&req, opts).expect("existing large-literal route");
            assert_eq!(report.to_json()["verdict"], "success");
            assert_eq!(report.to_json()["statistics"]["distinct"], 3);
        }
    }
    std::fs::remove_dir_all(dir).expect("remove temporary inputs");
}
