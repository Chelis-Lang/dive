//! Original constant-declaration fallbacks must survive unused INSTANCE imports.
#![allow(clippy::expect_used, reason = "original primary fixtures")]

use dive_check::CheckOptions;
use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use std::{path::Path, sync::Arc};

#[test]
fn original_constant_cases_keep_primary_final_truth_and_counts() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-trace/tests/fixtures/instance-constants");
    let facts: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-trace/tests/fixtures/instance-constants/primary.json"
    ))
    .expect("primary observations");
    let cases = facts["cases"].as_array().expect("four cases");
    assert_eq!(cases.len(), 4);
    let mut failures = Vec::new();
    for tlc in [true, false] {
        for store in [
            dive_store::StoreMode::Exact,
            dive_store::StoreMode::Fingerprint,
        ] {
            let options = CheckOptions {
                store,
                ..if tlc {
                    CheckOptions::tlc(0, 0)
                } else {
                    CheckOptions::dive(0)
                }
            };
            for case in cases {
                let id = case["id"].as_str().expect("id");
                let path = fixtures.join(id);
                let source = dive_sema::analyze_file(
                    &path.join("Constant.tla"),
                    &dive_sema::Options::default(),
                )
                .expect("original source");
                assert!(source.accepted(), "{:?}", source.diagnostics());
                let config = dive_config::parse(
                    &std::fs::read_to_string(path.join("Constant.cfg")).expect("config bytes"),
                )
                .into_config()
                .expect("config");
                let model = dive_model::bind(
                    Arc::new(dive_ir::lower(&source).expect("IR")),
                    &config,
                    &dive_model::BindOptions {
                        eval: options.eval,
                        ..dive_model::BindOptions::tlc()
                    },
                )
                .expect("model");
                for _ in 0..2 {
                    match analyze_final(&model, &options, Limits::default()) {
                        Ok(Analysis::Holds(complete)) => {
                            assert_eq!(
                                serde_json::json!(complete.stats.generated),
                                case["generated"],
                                "{id}"
                            );
                            assert_eq!(
                                serde_json::json!(complete.stats.distinct),
                                case["distinct"],
                                "{id}"
                            );
                            assert_eq!(
                                serde_json::json!(complete.stats.depth),
                                case["depth"],
                                "{id}"
                            );
                        }
                        other => failures.push(format!("{id}/tlc={tlc}/{store:?}: {other:?}")),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
