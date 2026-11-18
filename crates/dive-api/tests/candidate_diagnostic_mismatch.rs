//! Shared physical exits do not certify a candidate's diagnostic exception.
#![allow(clippy::expect_used, reason = "pinned diagnostic lifetime controls")]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file};
use serde_json::{Value, json};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/candidate-diagnostic-mismatch")
        .join(format!("{name}.tla"))
}

#[test]
fn a_different_diagnostic_cause_refuses_without_retagging_or_publishing_an_invalid_word() {
    for (name, cause) in [
        ("LaterDivDiagnosticOverflow", "TLC_MODULE_DIVISION_BY_ZERO"),
        ("LaterOverflowDiagnosticDiv", "TLC_MODULE_OVERFLOW"),
    ] {
        let file = fixture(name);
        for workers in [1, 4] {
            let report = check_file(&CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc: true,
                workers: Some(workers),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })
            .expect("actual file check");
            let result = report.to_json();
            assert_eq!(
                result["verdict"], "unsupported",
                "{name}/{workers}: {result}"
            );
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
            assert!(result["tlc"].is_null());
            assert!(result["counterexample"].is_null());
            assert!(
                result["notes"]
                    .as_array()
                    .expect("actual primary note")
                    .iter()
                    .any(|note| note.as_str().is_some_and(|note| note.contains(cause)))
            );
            assert_eq!(result["statistics"]["initial_generated"], 1);
            if workers == 1 {
                assert_eq!(result["statistics"]["distinct"], 3);
                assert_eq!(result["statistics"]["generated"], 4);
                assert_eq!(result["statistics"]["depth"], 2);
                assert_eq!(result["statistics"]["left_on_queue"], 1);
            }
        }
    }
}

#[test]
fn default_unbounded_arithmetic_does_not_inherit_the_tlc_overflow_boundary() {
    for name in ["LaterDivDiagnosticOverflow", "LaterOverflowDiagnosticDiv"] {
        let file = fixture(name);
        {
            let report = check_file(&CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })
            .expect("default check");
            let result = report.to_json();
            if name == "LaterDivDiagnosticOverflow" {
                assert_eq!(result["tlc"]["code"], 2179, "{result}");
                let model = SpecModel::load(
                    &file,
                    &file.with_extension("cfg"),
                    &TraceOptions {
                        run: Some((0, 0)),
                        ..TraceOptions::default()
                    },
                )
                .expect("fresh source model");
                assert_eq!(
                    model
                        .try_validate(
                            &TraceDoc::from_json(&result["counterexample"]["trace"])
                                .expect("actual word")
                        )
                        .expect("actual replay")
                        .validation,
                    Validation::Valid
                );
            } else {
                assert_eq!(result["verdict"], "action-property-violation", "{result}");
                assert_eq!(result["tlc"]["code"], 2112);
                assert_ne!(result["counterexample"]["trace"]["version"], json!(4));
            }
            assert_ne!(result["tlc"]["code"], Value::from(2178));
        }
    }
}
