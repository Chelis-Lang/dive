//! Pinned `SelectSeq` source captures keep their TLC verdicts, counts and
//! independently validated words on Core in both profiles. They ran on the
//! retired reference final analysis until #791. The three captures with
//! effects or primed actuals, which the reference planner refused, are
//! Core's named refusals.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#[path = "support/refused.rs"]
mod refused;

use dive_api::temporal::Outcome;
use dive_api::{CheckRequest, Store};
use dive_trace::Validation;
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn complete_primary_selection_preserves_counts_truth_and_fresh_replay() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = repository.join("conformance/corpus/selectseq-captures/specifications");
    let goldens = repository
        .join("conformance/golden/tlc-2026.10.06.014338/selectseq-captures/specifications");
    let mut directories = std::fs::read_dir(&corpus)
        .expect("complete corpus")
        .map(|entry| entry.expect("entry").path())
        .collect::<Vec<_>>();
    directories.sort();
    assert_eq!(directories.len(), 19);
    for tlc in [true, false] {
        for directory in &directories {
            let name = directory.file_name().expect("name").to_str().expect("utf8");
            let primary: Value = serde_json::from_slice(
                &std::fs::read(goldens.join(name).join("CallbackCapture.cfg.json"))
                    .expect("primary"),
            )
            .expect("primary JSON");
            let req = CheckRequest {
                file: directory.join("CallbackCapture.tla"),
                config: Some(directory.join("CallbackCapture.cfg")),
                compat_tlc: tlc,
                store: Store::Fingerprint,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                ..CheckRequest::default()
            };
            let report = dive_api::check_file(&req).expect("Core");
            let json = report.to_json();
            let what = format!("{name}/tlc={tlc}: {json}");
            if matches!(
                name,
                "empty_effect_capture_holds"
                    | "configured_action_capture_holds"
                    | "unused_effect_actual_holds"
            ) {
                refused::assert_refused(
                    &json,
                    report.exit_code(),
                    refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
                );
                continue;
            }
            let temporal = report.temporal_result().unwrap_or_else(|| panic!("{what}"));
            let (stats, violation) = match temporal.outcome() {
                Outcome::NativeHolds(complete) => (complete.stats(), None),
                Outcome::NativeViolation(violation) => {
                    (violation.evidence().stats(), Some(violation))
                }
                other => panic!("{what}: {other:?}"),
            };
            let counts = &primary["result"]["counts"];
            assert_eq!(json!(stats.distinct), counts["distinct_states"], "{what}");
            assert_eq!(json!(stats.generated), counts["states_generated"], "{what}");
            assert_eq!(json!(stats.depth), counts["depth"], "{what}");
            if let Some(violation) = violation {
                assert_eq!(primary["result"]["verdict"], "liveness-violation", "{what}");
                assert_eq!(violation.word().properties, ["P"], "{what}");
                assert_eq!(
                    violation.validation().validation,
                    Validation::Valid,
                    "{what}"
                );
            } else {
                assert_eq!(primary["result"]["verdict"], "success", "{what}");
            }
        }
    }
}
