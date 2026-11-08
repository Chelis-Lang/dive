//! Sampling must retain the source state's actual TLC evaluation level.
#![allow(clippy::expect_used, reason = "test fixture failures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_sample_level.json");

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::{Value, json};

fn observed(document: &Value) -> Value {
    json!({
        "verdict": document["verdict"],
        "property": document["property"],
        "statistics": document["statistics"],
        "output": document["output"],
        "counterexample": document["counterexample"],
    })
}

#[test]
fn sampled_level_operators_match_actual_search_at_every_depth() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = [
        "tests/fixtures/core-sample/LevelChain.tla",
        "tests/fixtures/core-lib/specs/RgLevel/RgLevel.tla",
        "tests/fixtures/core-lib/specs/RgLevelViol/RgLevelViol.tla",
        "tests/fixtures/checker/dfid/DfidLiveGet.tla",
    ];
    for fixture in fixtures {
        for compat_tlc in [false, true] {
            let file = root.join(fixture);
            let mut request = CheckRequest {
                config: Some(file.with_extension("cfg")),
                file,
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let reference = check_file_engine(&request, Engine::Core).expect("reference runs");
            let plain = check_file_engine(&request, Engine::Core).expect("core runs");
            let plain_json = plain.to_json();
            assert_eq!(plain_json["engine"], "core", "{fixture}: {plain_json:#}");
            agreed::check_eq(
                AGREED,
                "sampled_level_operators_match_actual_search_at_every_depth/0",
                &(observed(&plain_json)),
                &(observed(&reference.to_json())),
            );
            agreed::check_eq(
                AGREED,
                "sampled_level_operators_match_actual_search_at_every_depth/1",
                &(plain.exit_code_for(ExitConvention::Tlc)),
                &(reference.exit_code_for(ExitConvention::Tlc)),
            );
            if fixture.ends_with("LevelChain.tla") {
                // Pinned TLC BFS, seed/fp 0: y'=the expanded state's level
                // gives y=x at every depth, 5 distinct / 6 generated / depth 5.
                assert_eq!(plain_json["verdict"], "success");
                assert_eq!(plain_json["statistics"]["distinct"], 5);
                assert_eq!(plain_json["statistics"]["generated"], 6);
                assert_eq!(plain_json["statistics"]["depth"], 5);
            }
            for count in [1, 32] {
                let mut expected = Value::Null;
                for workers in [1, 4, 8] {
                    request.workers = Some(workers);
                    request.core_sample = Some(count);
                    let report = check_file_engine(&request, Engine::Core).expect("sample runs");
                    let document = report.to_json();
                    assert_eq!(observed(&document), observed(&plain_json), "{fixture}");
                    let sample = &document["core"]["sample"];
                    assert_eq!(sample["mismatches"], 0, "{fixture}: {sample:#}");
                    assert_eq!(sample["skipped"], 0, "{fixture}: {sample:#}");
                    assert!(sample["compared"].as_u64().is_some_and(|n| n > 0));
                    if workers == 1 {
                        expected = sample.clone();
                    } else {
                        assert_eq!(*sample, expected, "{fixture}: workers={workers}");
                    }
                }
            }
        }
    }
}
