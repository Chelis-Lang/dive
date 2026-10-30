//! Public JSON, human and Rust views name exactly their validated word's properties.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "small pinned attribution fixtures"
)]

use std::path::PathBuf;

use dive_api::temporal::Outcome;
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::json;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn actual_word_names_agree_in_rust_json_and_human_reports() {
    for (name, properties, formatted) in [
        ("Fair", vec!["Q"], "Q"),
        ("Identical", vec!["P", "Q"], "P and Q"),
        ("Mutual", vec!["P", "Q"], "P and Q"),
        (
            "Liveness",
            vec!["XReachesN", "AlwaysEvXN", "QuantProp"],
            "XReachesN, AlwaysEvXN, and QuantProp",
        ),
    ] {
        let file = if name == "Fair" {
            root().join("crates/dive-api/tests/fixtures/core-liveness/Fair.tla")
        } else {
            root().join(format!(
                "conformance/corpus/word-relative-liveness/specifications/{name}/{name}.tla"
            ))
        };
        for compat_tlc in [false, true] {
            for engine in [Engine::Reference, Engine::Core] {
                for workers in [1, 4] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            seed: Some(0),
                            fp: Some(0),
                            workers: Some(workers),
                            ..CheckRequest::default()
                        },
                        engine,
                    )
                    .expect("check");
                    let temporal = report.temporal_result().expect("temporal report");
                    let word = match temporal.outcome() {
                        Outcome::Violation(w) => w.document(),
                        Outcome::PrefixViolation(w) => w.document(),
                        Outcome::NativeViolation(w) => w.document(),
                        Outcome::NativePrefixViolation(w) => w.document(),
                        other => panic!("expected violation, got {other:?}"),
                    };
                    assert_eq!(
                        word.properties, properties,
                        "{name}/{engine:?}/{compat_tlc}/{workers}"
                    );
                    let doc = report.to_json();
                    assert_eq!(doc["verdict"], "liveness-violation");
                    assert_eq!(doc["property"], formatted);
                    assert_eq!(doc["properties"], json!(properties));
                    assert_eq!(
                        doc["counterexample"]["trace"]["properties"],
                        json!(properties)
                    );
                    assert_eq!(doc["counterexample"]["validation"], "valid");
                    let headline = if properties.len() == 1 {
                        format!("Temporal property {formatted} was violated.")
                    } else {
                        format!("Temporal properties {formatted} were violated.")
                    };
                    assert!(report.render_human().lines().any(|line| line == headline));
                }
            }
        }
    }
}
