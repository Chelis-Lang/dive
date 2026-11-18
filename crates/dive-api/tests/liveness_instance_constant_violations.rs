//! Pinned constant-fallback liveness violations keep their counts and
//! independently validated words on Core. They ran on the retired reference
//! final analysis until #791. Exact storage with temporal properties is a
//! named Core refusal.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "pinned primary fixture assertions"
)]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use dive_api::CheckRequest;
use dive_api::temporal::Outcome;
use dive_runtime::CheckOptions;
use dive_trace::Validation;
use std::path::Path;

#[test]
fn absent_constants_keep_primary_violations_counts_and_valid_words() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-trace/tests/fixtures/instance-constants");
    let facts: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-trace/tests/fixtures/instance-constants/primary-violations.json"
    ))
    .expect("primary facts");
    let cases = facts["cases"].as_array().expect("two cases");
    assert_eq!(cases.len(), 2);
    for defaults in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for store in [
            dive_store::StoreMode::Exact,
            dive_store::StoreMode::Fingerprint,
        ] {
            let options = CheckOptions {
                store,
                ..defaults.clone()
            };
            for case in cases {
                let id = case["id"].as_str().expect("id");
                let path = fixtures.join(id);
                let req = CheckRequest {
                    file: path.join("Constant.tla"),
                    config: Some(path.join("Constant.cfg")),
                    ..core_final::request(&options)
                };
                let report = dive_api::check_file(&req).expect("Core");
                if store == dive_store::StoreMode::Exact {
                    refused::assert_refused(
                        &report.to_json(),
                        report.exit_code(),
                        refused::TEMPORAL_EXACT_STORE,
                    );
                    continue;
                }
                let temporal = report
                    .temporal_result()
                    .unwrap_or_else(|| panic!("{id}: {}", report.to_json()));
                let Outcome::NativeViolation(violation) = temporal.outcome() else {
                    panic!("{id}: {:?}", temporal.outcome());
                };
                assert_eq!(violation.word().properties, ["Bad"], "{id}");
                let stats = violation.evidence().stats();
                assert_eq!(
                    serde_json::json!(stats.generated),
                    case["generated"],
                    "{id}"
                );
                assert_eq!(serde_json::json!(stats.distinct), case["distinct"], "{id}");
                assert_eq!(serde_json::json!(stats.depth), case["depth"], "{id}");
                assert_eq!(violation.validation().validation, Validation::Valid, "{id}");
                assert_eq!(
                    report.exit_code_for(dive_api::ExitConvention::Tlc),
                    13,
                    "{id}"
                );
            }
        }
    }
}
