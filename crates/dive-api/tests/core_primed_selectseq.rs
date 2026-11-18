//! Only state-independent `SelectSeq` callbacks are admitted beneath a prime.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file, check_file_core_opts};
use serde_json::{Value, json};

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/core-primed-selectseq/oracle.json"))
        .expect("compact official-harness oracle results")
}

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-primed-selectseq")
        .join(format!("{name}.tla"));
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
    json!({
        "verdict": doc["verdict"], "property": doc["property"],
        "code": doc["tlc"]["code"], "error_class": doc["tlc"]["name"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "distinct": doc["statistics"]["distinct"],
        "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"],
    })
}

fn check_oracle(actual: &Value, expected: &Value, context: &str) {
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
        let expected = if key == "verdict" && expected[key] == "semantic-error" {
            json!("rejected")
        } else {
            expected[key].clone()
        };
        assert_eq!(actual[key], expected, "{context}/{key}: {actual:#}");
    }
}

fn validate_bad_step(file: &Path, compat_tlc: bool, report: &dive_api::CheckReport) {
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
    let mut word = report.to_json()["counterexample"]["trace"].clone();
    let trace = TraceDoc::from_json(&word).expect("word decodes");
    assert!(matches!(
        model.validate(&trace).validation,
        Validation::Valid
    ));
    word["states"][1]["values"]["s"] = json!({"seq": [1]});
    let mutant = TraceDoc::from_json(&word).expect("mutant decodes");
    assert!(matches!(
        model.validate(&mutant).validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn primed_sequence_and_pure_predicates_match_oracle_and_reference() {
    let oracle = oracle();
    let cases = oracle["cases"].as_object().expect("cases");
    assert_eq!(
        cases.values().filter(|e| e["core"] == "admitted").count(),
        8
    );
    for (name, expected) in cases {
        if expected["core"] != "admitted" {
            continue;
        }
        for compat in [false, true] {
            for workers in [1, 4, 8] {
                let req = request(name, compat, workers);
                let reference = check_file(&req).expect("reference runs");
                let reference_observed = observed(&reference);
                check_oracle(&reference_observed, expected, name);
                for opts in ["", "-fold,-memo,-step-memo", "-eager", "-hashcons"] {
                    let core = check_file_core_opts(&req, opts).expect("core runs");
                    let doc = core.to_json();
                    assert_eq!(doc["engine"], "core", "{name}: {doc:#}");
                    assert_eq!(
                        observed(&core),
                        reference_observed,
                        "{name}/{compat}/{workers}/{opts}: {doc:#}"
                    );
                    if name == "PureBad" {
                        validate_bad_step(&req.file, compat, &core);
                    }
                    // Final-expansion error words have their own outstanding
                    // proof obligation (#434); report parity is not word credit.
                    if name == "SequenceErrorFirst" {
                        assert!(
                            doc["message"]
                                .as_str()
                                .expect("error")
                                .contains("first argument")
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn state_captures_effects_and_other_higher_order_operators_stay_refused() {
    let oracle = oracle();
    let cases = oracle["cases"].as_object().expect("cases");
    assert_eq!(
        cases
            .values()
            .filter(|e| e["core"] == "unsupported")
            .count(),
        9
    );
    for (name, expected) in cases {
        if expected["core"] != "unsupported" {
            continue;
        }
        for compat in [false, true] {
            for workers in [1, 4, 8] {
                let req = request(name, compat, workers);
                // The retired reference engine reproduced the pinned TLC
                // results for these (#791, #910); Core refuses each by name.
                for opts in ["", "-fold,-memo,-step-memo,-eager"] {
                    let core = check_file_core_opts(&req, opts).expect("core refuses");
                    let doc = core.to_json();
                    assert_eq!(doc["engine"], "core");
                    assert_eq!(doc["verdict"], "unsupported", "{name}/{opts}: {doc:#}");
                    assert_eq!(core.exit_code(), 3);
                    assert!(
                        doc["message"]
                            .as_str()
                            .expect("refusal reason")
                            .contains("under a prime"),
                        "{name}: {doc:#}"
                    );
                }
            }
        }
    }
}

#[test]
fn free_and_double_primes_remain_frontend_errors() {
    let oracle = oracle();
    let cases = oracle["cases"].as_object().expect("cases");
    assert_eq!(
        cases.values().filter(|e| e["core"] == "rejected").count(),
        2
    );
    for (name, expected) in cases {
        if expected["core"] != "rejected" {
            continue;
        }
        for compat in [false, true] {
            let req = request(name, compat, 1);
            {
                let report = check_file(&req).expect("frontend rejection reports");
                check_oracle(&observed(&report), expected, name);
                assert!(
                    report.to_json()["diagnostics"]
                        .as_array()
                        .expect("diagnostics")
                        .iter()
                        .any(|d| d["sany_code"] == 4205),
                    "{name}: not the expected level-error rejection"
                );
            }
        }
    }
}
