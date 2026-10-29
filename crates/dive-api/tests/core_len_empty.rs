//! Empty-domain functions are sequences for Len (#527).
#![allow(clippy::expect_used, reason = "test fixtures")]

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
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
                for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                    let report = check_file_engine(&req, engine).expect("check");
                    assert_eq!(
                        observed(&report),
                        *expected,
                        "{name}/{compat_tlc}/{workers}/{engine:?}"
                    );
                    if engine != Engine::Reference {
                        assert_eq!(report.to_json()["engine"], "core");
                    }
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
            let reference = check_file_engine(&req, Engine::Reference).expect("reference");
            assert_ne!(reference.to_json()["verdict"], "success");
            let core = check_file_engine(&req, Engine::Core).expect("core");
            assert_eq!(core.to_json()["verdict"], "evaluation-error", "{name}");
            assert_eq!(observed(&core), observed(&reference), "{name}");
            assert_eq!(core.to_json()["tlc"]["verified"], false);
            let auto = check_file_engine(&req, Engine::Auto).expect("auto");
            assert_eq!(observed(&auto), observed(&reference), "{name}");
        }
    }
}
