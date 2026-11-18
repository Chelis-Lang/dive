//! Original constant-declaration fallbacks survive unused INSTANCE imports:
//! Core's final truth and counts match TLC's primary observations. This ran
//! on the retired reference final analysis until #791. Exact seen storage
//! with temporal properties is a named Core refusal.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "original primary fixtures"
)]

#[path = "support/refused.rs"]
mod refused;

use dive_api::{CheckRequest, Store};
use std::path::Path;

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
    for compat_tlc in [true, false] {
        for store in [Store::Exact, Store::Fingerprint] {
            for case in cases {
                let id = case["id"].as_str().expect("id");
                let file = fixtures.join(id).join("Constant.tla");
                let request = CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    compat_tlc,
                    store,
                    seed: Some(0),
                    fp: Some(0),
                    workers: Some(1),
                    ..CheckRequest::default()
                };
                let report = dive_api::check_file(&request).expect("Core");
                let json = report.to_json();
                if store == Store::Exact {
                    refused::assert_refused(
                        &json,
                        report.exit_code(),
                        refused::TEMPORAL_EXACT_STORE,
                    );
                    continue;
                }
                let observed = (
                    json["verdict"].clone(),
                    json["statistics"]["generated"].clone(),
                    json["statistics"]["distinct"].clone(),
                    json["statistics"]["depth"].clone(),
                );
                let expected = (
                    serde_json::json!("success"),
                    case["generated"].clone(),
                    case["distinct"].clone(),
                    case["depth"].clone(),
                );
                if observed != expected {
                    failures.push(format!(
                        "{id}/tlc={compat_tlc}: {observed:?} != {expected:?}: {json}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
