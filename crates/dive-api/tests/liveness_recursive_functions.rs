//! Primary `RecFcn` fairness outcomes, counts and independently validated
//! words on Core in both profiles. They ran on the retired reference final
//! analysis until #791.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "tiny generated oracle controls"
)]

use dive_api::temporal::Outcome;
use dive_api::{CheckRequest, Engine, Store};
use dive_trace::Validation;
use serde_json::{Value, json};
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/recursive-function")
}

#[test]
fn seven_actual_primary_outcomes_counts_and_native_words_match() {
    let oracle: Value = serde_json::from_slice(
        &std::fs::read(fixture().join("oracle.json")).expect("generated primary"),
    )
    .expect("JSON");
    assert_eq!(oracle["primary_before_production_implementation"], true);
    let rows = oracle["observations"].as_array().expect("rows");
    assert_eq!(rows.len(), 7);
    for row in rows {
        let module = row["module"].as_str().expect("module");
        for tlc in [true, false] {
            let req = CheckRequest {
                file: fixture().join(format!("{module}.tla")),
                config: Some(fixture().join(row["config"].as_str().expect("config"))),
                compat_tlc: tlc,
                store: Store::Fingerprint,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                ..CheckRequest::default()
            };
            let report = dive_api::check_file_engine(&req, Engine::Core).expect("Core");
            let what = format!("{row} tlc={tlc}: {}", report.to_json());
            let temporal = report.temporal_result().unwrap_or_else(|| panic!("{what}"));
            let (stats, violation) = match temporal.outcome() {
                Outcome::NativeHolds(complete) => (complete.stats(), None),
                Outcome::NativeViolation(violation) => {
                    (violation.evidence().stats(), Some(violation))
                }
                other => panic!("{what}: {other:?}"),
            };
            assert_eq!(json!(stats.generated), row["generated"], "{what}");
            assert_eq!(json!(stats.distinct), row["distinct"], "{what}");
            assert_eq!(json!(stats.depth), row["depth"], "{what}");
            assert_eq!(
                json!(report.exit_code_for(dive_api::ExitConvention::Tlc)),
                row["physical_exit"],
                "{what}"
            );
            if let Some(violation) = violation {
                assert_eq!(row["error_codes"][0], 2116, "{what}");
                assert_eq!(
                    json!(violation.word().properties),
                    json!([row["violated_property"]]),
                    "{what}"
                );
                assert_eq!(
                    violation.validation().validation,
                    Validation::Valid,
                    "{what}"
                );
            }
        }
    }
}
