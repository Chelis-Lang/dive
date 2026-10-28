//! Initial-state generation sets TLC's depth even when constraints exclude
//! every candidate; an initial predicate emitting nothing leaves depth zero.
#![allow(clippy::expect_used, reason = "test fixture failures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::{Value, json};

#[test]
fn initial_depth_counts_emitted_states_instead_of_retained_states() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cases = [
        ("../dive-sim/tests/fixtures/SimExcludedInitial.tla", 1, 1),
        (
            "tests/fixtures/core-initial-depth/ExcludedInitial.tla",
            3,
            1,
        ),
        ("tests/fixtures/core-initial-depth/EmptyInitial.tla", 0, 0),
    ];
    for (fixture, generated, depth) in cases {
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                let file = root.join(fixture);
                let request = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file,
                    compat_tlc,
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let mut reference = Value::Null;
                for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                    let report = check_file_engine(&request, engine).expect("check runs");
                    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 0);
                    let document = report.to_json();
                    // Pinned TLC BFS (tlc-2026.10.06.014338, fp/seed 0):
                    // these complete runs have no retained state or trace.
                    assert_eq!(document["verdict"], "success", "{fixture}: {document:#}");
                    assert_eq!(document["statistics"]["generated"], generated);
                    assert_eq!(document["statistics"]["distinct"], 0);
                    assert_eq!(document["statistics"]["depth"], depth);
                    assert_eq!(document["statistics"]["left_on_queue"], 0);
                    assert!(document["counterexample"].is_null());
                    let observed = json!({
                        "statistics": document["statistics"],
                        "tlc": document["tlc"],
                        "property": document["property"],
                    });
                    if engine == Engine::Reference {
                        reference = observed;
                    } else {
                        assert_eq!(observed, reference, "{fixture}: {document:#}");
                        assert_eq!(document["engine"], "core");
                    }
                }
            }
        }
    }
}
