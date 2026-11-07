//! Error wrappers follow actual diagnostic receiver construction, in both engines.
#![allow(
    clippy::expect_used,
    reason = "source-bound oracle regression assertions"
)]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, Store, check_file_engine};
use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn whole_function_failure_matches_the_generated_oracle_without_changing_alias_demand() {
    let root = root();
    for name in ["WholeFunctionError", "Named", "Inline", "NamedAction"] {
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "conformance/golden/tlc-2026.10.06.014338/constant-retry-lazy/specifications/{name}/{name}.cfg.json"
            )))
            .expect("official generated golden"),
        )
        .expect("golden JSON");
        let file = root.join(format!(
            "conformance/corpus/constant-retry-lazy/specifications/{name}/{name}.tla"
        ));
        for compat_tlc in [false, true] {
            for engine in [Engine::Reference, Engine::Core] {
                for store in [Store::Exact, Store::Fingerprint] {
                    for workers in [1, 4] {
                        let report = check_file_engine(
                            &CheckRequest {
                                file: file.clone(),
                                config: Some(file.with_extension("cfg")),
                                compat_tlc,
                                workers: Some(workers),
                                store,
                                seed: Some(0),
                                fp: Some(0),
                                ..CheckRequest::default()
                            },
                            engine,
                        )
                        .expect("check")
                        .to_json();
                        // Native exact storage is a safety capability. This
                        // fixture has a genuine temporal property and keeps
                        // the graph-storage refusal until that is implemented.
                        if engine == Engine::Core && store == Store::Exact && name == "NamedAction"
                        {
                            assert_eq!(report["verdict"], "unsupported");
                            assert!(
                                report["message"]
                                    .as_str()
                                    .expect("temporal storage refusal")
                                    .contains("exact or spilled native temporal storage")
                            );
                            assert_eq!(report["options"]["workers"]["used"], 0);
                            assert!(report["tlc"].is_null());
                            assert!(report["counterexample"].is_null());
                            continue;
                        }
                        assert_eq!(
                            report["tlc"]["code"], golden["result"]["verdict_code"],
                            "{name}/{compat_tlc}/{engine:?}/{store:?}/{workers}: {report}"
                        );
                        assert_eq!(report["tlc"]["verified"], true, "{name}: {report}");
                        assert_eq!(report["tlc"]["exit_status"], golden["result"]["exit_code"]);
                        for (field, expected) in [
                            ("generated", "states_generated"),
                            ("distinct", "distinct_states"),
                            ("depth", "depth"),
                        ] {
                            assert_eq!(
                                report["statistics"][field], golden["result"]["counts"][expected],
                                "{name}/{engine:?}/{field}"
                            );
                        }
                        if name == "WholeFunctionError" {
                            assert_eq!(report["statistics"]["left_on_queue"], 1);
                            assert_eq!(report["tlc"]["reported"], 1000);
                            assert_eq!(report["property"], Value::Null);
                            assert_eq!(report["counterexample"], Value::Null);
                        }
                    }
                }
            }
        }
    }
}
