//! Missing next-state actions have a different TLC ending from an action
//! that is defined but disabled, and from an empty initial frontier.
#![allow(clippy::expect_used, reason = "test fixture failures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_no_next.json");

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::json;

#[test]
fn missing_next_actions_keep_tlcs_model_error_and_precedence() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cases = [
        (
            "tests/fixtures/checker/dfid/DfidInitialOnly.tla",
            "model-error",
            2115,
            75,
        ),
        (
            "tests/fixtures/core-no-next/EmptyNoNext.tla",
            "success",
            2193,
            0,
        ),
        (
            "tests/fixtures/core-no-next/ExcludedNoNext.tla",
            "success",
            2193,
            0,
        ),
        (
            "tests/fixtures/core-no-next/DisabledNext.tla",
            "deadlock",
            2114,
            11,
        ),
        (
            "tests/fixtures/core-no-next/MissingNextInvariant.tla",
            "model-error",
            2232,
            151,
        ),
    ];
    for (fixture, verdict, code, exit) in cases {
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                for no_deadlock in [false, true] {
                    let disabled = code == 2114 && no_deadlock;
                    let expected_verdict = if disabled { "success" } else { verdict };
                    let expected_code = if disabled { 2193 } else { code };
                    let expected_exit = if disabled { 0 } else { exit };
                    let file = root.join(fixture);
                    let request = CheckRequest {
                        config: Some(file.with_extension("cfg")),
                        file,
                        compat_tlc,
                        workers: Some(workers),
                        no_deadlock,
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    };
                    {
                        let engine = Engine::Core;
                        let report = check_file_engine(&request, engine).expect("check runs");
                        assert_eq!(report.exit_code_for(ExitConvention::Tlc), expected_exit);
                        let document = report.to_json();
                        // These are pinned TLC BFS results, not DFID or
                        // simulation expectations from the original fixtures.
                        assert_eq!(
                            document["verdict"], expected_verdict,
                            "{fixture}: {document:#}"
                        );
                        assert_eq!(document["tlc"]["code"], expected_code);
                        if code == 2115 {
                            assert!(document["counterexample"].is_null());
                            assert_eq!(document["statistics"]["reported_by_tlc"], false);
                        }
                        let observed = json!({
                            "statistics": document["statistics"],
                            "tlc": document["tlc"],
                            "property": document["property"],
                            "message": document["message"],
                            "trace_length": document["counterexample"]["length"],
                            "trace_ending": document["counterexample"]["ending"],
                        });
                        agreed::check(AGREED, "cross/0", &(observed));
                        {
                            assert_eq!(document["engine"], "core");
                        }
                    }
                }
            }
        }
    }
}
