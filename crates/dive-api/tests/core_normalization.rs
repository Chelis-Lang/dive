//! Canonical default construction and nominal compat against unchanged TLC facts.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, ExitConvention, check_file, check_file_core_opts};
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
fn canonical_default_accepts_shapes_while_nominal_compat_names_its_gaps() {
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
                let reference = check_file(&req).expect("reference runs");
                let reference_observed = observed(&reference);
                // Under --compat=tlc Core refuses every case but the three
                // controls; the retired reference engine reproduced the
                // pinned TLC results for them (#791, #910).
                let refused = compat_tlc
                    && !matches!(
                        name.as_str(),
                        "HomogeneousControl" | "LazyControl" | "RecordDomainControl"
                    );
                if compat_tlc && !refused {
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
                if !compat_tlc {
                    // Every invariant in this matrix is a true mathematical
                    // cardinality/equality statement over exactly three states.
                    assert_eq!(reference_observed["verdict"], "success");
                    assert_eq!(reference_observed["distinct"], 3);
                    assert_eq!(reference_observed["generated"], 3);
                    assert_eq!(reference_observed["depth"], 3);
                }
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
            }
        }
    }
}
