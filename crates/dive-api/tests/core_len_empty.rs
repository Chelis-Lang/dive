//! Empty-domain functions are sequences for Len (#527).
#![allow(clippy::expect_used, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_len_empty.json");

use dive_api::{CheckRequest, ExitConvention, check_file, check_file_core_opts};
use serde_json::{Value, json};
use std::path::PathBuf;

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict":doc["verdict"],"property":doc["property"],"code":doc["tlc"]["code"],
        "error_class":doc["tlc"]["name"],"exit":report.exit_code_for(ExitConvention::Tlc),
        "distinct":doc["statistics"]["distinct"],"generated":doc["statistics"]["generated"],
        "depth":doc["statistics"]["depth"]})
}

#[test]
fn empty_functions_match_pinned_len_results_without_fallback() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-len-empty");
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/core-len-empty/oracle.json")).expect("oracle");
    for (name, expected) in oracle["cases"].as_object().expect("cases") {
        for compat_tlc in [false, true] {
            for workers in [1, 4] {
                let file = root.join(format!("{name}.tla"));
                let req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file,
                    compat_tlc,
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                {
                    let report = check_file(&req).expect("check");
                    assert_eq!(
                        observed(&report),
                        *expected,
                        "{name}/{compat_tlc}/{workers}"
                    );
                    assert_eq!(report.to_json()["engine"], "core");
                }
                for opts in ["-fold,-memo", "-hashcons"] {
                    let report = check_file_core_opts(&req, opts).expect("ablation");
                    assert_eq!(
                        observed(&report),
                        *expected,
                        "{name}/{compat_tlc}/{workers}/{opts}"
                    );
                }
            }
        }
    }
}

#[test]
fn empty_nonfunctions_and_nonsequence_domains_do_not_become_sequences() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-len-empty");
    for name in ["EmptySet", "NonSequenceDomain", "Scalar"] {
        for compat_tlc in [false, true] {
            let file = root.join(format!("{name}.tla"));
            let req = CheckRequest {
                config: Some(file.with_extension("cfg")),
                file,
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let reference = check_file(&req).expect("reference");
            assert_ne!(reference.to_json()["verdict"], "success");
            let core = check_file(&req).expect("core");
            assert_eq!(core.to_json()["verdict"], "evaluation-error", "{name}");
            agreed::check_eq(
                AGREED,
                "empty_nonfunctions_and_nonsequence_domains_do_not_become_sequences/0",
                &(observed(&core)),
                &(observed(&reference)),
            );
            assert_eq!(core.to_json()["tlc"]["verified"], false);
        }
    }
}
