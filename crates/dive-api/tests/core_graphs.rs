//! Actual Core execution of the directed and undirected graph overrides.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = root
        .join("tests/fixtures/core-graphs")
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        library_paths: vec![root.join("../dive-eval/tests/fixtures/community-modules")],
        compat_tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict":doc["verdict"], "property":doc["property"],
        "distinct":doc["statistics"]["distinct"], "generated":doc["statistics"]["generated"],
        "depth":doc["statistics"]["depth"], "code":doc["tlc"]["code"],
        "exit":report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn graph_operators_execute_in_state_dependent_core_checks() {
    let oracle: Value = serde_json::from_str(include_str!("fixtures/core-graphs/oracle.json"))
        .expect("pinned observations decode");
    for compat in [false, true] {
        for workers in [1, 4, 8] {
            for name in ["GraphsStep", "GraphViolation"] {
                let req = request(name, compat, workers);
                let reference = check_file_engine(&req, Engine::Reference).expect("reference runs");
                let expected = observed(&reference);
                let pinned = &oracle["models"][name];
                assert_eq!(expected["generated"], pinned["counts"][0]);
                assert_eq!(expected["distinct"], pinned["counts"][1]);
                assert_eq!(expected["depth"], pinned["depth"]);
                assert_eq!(expected["exit"], pinned["physical_exit"]);
                assert_eq!(expected["distinct"], 3, "{name}: {expected}");
                assert_eq!(expected["generated"], 3, "{name}: {expected}");
                assert_eq!(expected["depth"], 3, "{name}: {expected}");
                let violation = name == "GraphViolation";
                assert_eq!(
                    expected["verdict"],
                    if violation {
                        "invariant-violation"
                    } else {
                        "success"
                    }
                );
                assert_eq!(expected["code"], if violation { 2110 } else { 2193 });
                assert_eq!(expected["exit"], if violation { 12 } else { 0 });
                for opts in ["", "-fold,-memo", "-hashcons"] {
                    let core = check_file_core_opts(&req, opts).expect("core runs");
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{name}/{compat}/{workers}/{opts}"
                    );
                    if violation {
                        let options = TraceOptions {
                            compat_tlc: compat,
                            library_paths: req.library_paths.clone(),
                            run: Some((0, 0)),
                            ..TraceOptions::default()
                        };
                        let model =
                            SpecModel::load(&req.file, &req.file.with_extension("cfg"), &options)
                                .expect("independent source");
                        let word = &core.to_json()["counterexample"]["trace"];
                        let trace = TraceDoc::from_json(word).expect("word decodes");
                        assert!(matches!(
                            model.validate(&trace).validation,
                            Validation::Valid
                        ));
                        let mut bad = word.clone();
                        bad["states"][1]["values"]["x"] = json!(7);
                        let bad = TraceDoc::from_json(&bad).expect("mutant decodes");
                        assert!(matches!(
                            model.validate(&bad).validation,
                            Validation::Invalid(_)
                        ));
                    }
                }
            }
        }
    }
}
