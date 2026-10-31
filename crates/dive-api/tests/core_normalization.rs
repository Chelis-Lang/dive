//! C2 normalization errors remain failures in both semantic profiles (#619).
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({
        "verdict": doc["verdict"],
        "property": doc["property"],
        "code": doc["tlc"]["code"],
        "error_class": doc["tlc"]["name"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "distinct": doc["statistics"]["distinct"],
        "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"],
    })
}

#[test]
fn compound_normalization_requires_comparable_shapes_in_both_profiles() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-normalization");
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/core-normalization/oracle.json"))
            .expect("compact pinned oracle summary");
    for (name, expected) in oracle["cases"].as_object().expect("oracle cases") {
        for compat_tlc in [false, true] {
            for workers in [1, 4] {
                let file = root.join(format!("{name}.tla"));
                let req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file,
                    workers: Some(workers),
                    compat_tlc,
                    seed: Some(19),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let reference = check_file_engine(&req, Engine::Reference).expect("reference runs");
                let reference_observed = observed(&reference);
                // The direct equality fixture deliberately exercises D-009's
                // approved default answer; its TLC error is not default parity.
                if name != "DefaultEqualityControl" {
                    for key in [
                        "verdict",
                        "error_class",
                        "exit",
                        "code",
                        "distinct",
                        "generated",
                        "depth",
                    ] {
                        assert_eq!(
                            reference_observed[key], expected[key],
                            "{name}/{compat_tlc}/{workers}/{key}: {reference_observed:#}"
                        );
                    }
                }
                // These two pinned successful controls now have the actual
                // constructor/demand path; all failing C2 fixtures still refuse.
                let covered = matches!(name.as_str(), "OrderShortCircuit" | "SizeShortCircuit");
                let refused = (expected["core"] == "unsupported" && !covered)
                    || (name == "DefaultEqualityControl" && compat_tlc);
                for opts in ["", "-fold,-memo", "-hashcons"] {
                    let core = check_file_core_opts(&req, opts).expect("core runs");
                    let doc = core.to_json();
                    assert_eq!(doc["engine"], "core", "{name}: {doc:#}");
                    if refused {
                        assert_eq!(
                            doc["verdict"], "unsupported",
                            "{name}/{compat_tlc}/{workers}/{opts}: {doc:#}"
                        );
                        assert_eq!(core.exit_code(), 3);
                        let reason = doc["message"].as_str().expect("reason");
                        assert!(
                            reason.contains("provenance")
                                || reason.contains("normalizes")
                                || reason.contains("shapes"),
                            "{name}: {doc:#}"
                        );
                    } else {
                        assert_eq!(doc["verdict"], "success", "{name}: {doc:#}");
                        assert_eq!(
                            observed(&core),
                            reference_observed,
                            "{name}/{compat_tlc}/{workers}/{opts}"
                        );
                    }
                }
                if refused {
                    let auto = check_file_engine(&req, Engine::Auto).expect("auto runs");
                    assert_eq!(auto.to_json()["engine"], "reference");
                    assert_eq!(observed(&auto), reference_observed, "{name}");
                }
            }
        }
    }
}
