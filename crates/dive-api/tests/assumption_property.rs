//! Failed assumptions retain their typed source image in JSON (#504).
#![allow(clippy::expect_used, reason = "test fixtures")]

use dive_api::{CheckRequest, ExitConvention, check_file};
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn failed_assumptions_name_the_exact_oracle_property_without_parsing_messages() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    for name in [
        "FalsePlain",
        "FalseNamed",
        "LaterFalse",
        "AssumeError",
        "AllTrue",
    ] {
        let path=root.join(format!("golden/tlc-2026.10.06.014338/assumption-reporting/specifications/Assumptions/{name}.cfg.json"));
        let golden: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("golden")).expect("JSON");
        let expected = &golden["result"];
        for compat_tlc in [false, true] {
            for workers in [1, 4] {
                let file = root.join(format!(
                    "corpus/assumption-reporting/specifications/Assumptions/{name}.tla"
                ));
                let request = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file,
                    compat_tlc,
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                {
                    let report = check_file(&request).expect("check");
                    let actual = report.to_json();
                    let native_verdict = if expected["verdict_code"] == 2105 {
                        Value::String("evaluation-error".into())
                    } else {
                        expected["verdict"].clone()
                    };
                    for (actual, expected) in [
                        // The native API distinguishes evaluation errors; the
                        // harness classifies EC2105 as assumption-failure.
                        (&actual["verdict"], &native_verdict),
                        (&actual["property"], &expected["violated_property"]),
                        (&actual["tlc"]["code"], &expected["verdict_code"]),
                        (&actual["tlc"]["name"], &expected["error_class"]),
                    ] {
                        assert_eq!(actual, expected, "{name}/{compat_tlc}/{workers}");
                    }
                    assert_eq!(
                        Value::from(report.exit_code_for(ExitConvention::Tlc)),
                        expected["exit_code"]
                    );
                    assert!(actual["counterexample"].is_null());
                    if name == "AllTrue" {
                        assert_eq!(actual["statistics"]["distinct"], 1);
                    } else {
                        assert!(actual["statistics"].is_null());
                        assert_eq!(actual["options"]["workers"]["used"], 0);
                    }
                }
            }
        }
    }
}
