//! Captured SelectSeq/concatenation values keep TLC's primary counts and
//! verdicts on Core, with independently validated words. These ran on the
//! retired reference final analysis until #791.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "primary reference assertions"
)]

#[path = "support/refused.rs"]
mod refused;

use dive_api::CheckRequest;
use dive_trace::Validation;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn spec(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/selectseq-concat-type-capability/specifications")
        .join(name)
        .join(format!("{name}.tla"))
}

fn primary(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/golden/tlc-2026.10.06.014338/selectseq-concat-type-capability/specifications")
        .join(name)
        .join(format!("{name}.cfg.json"));
    serde_json::from_slice(&std::fs::read(path).expect("generated reference")).expect("JSON")
}

fn check(name: &str, compat_tlc: bool) -> dive_api::CheckReport {
    let file = spec(name);
    dive_api::check_file(&CheckRequest {
        file: file.clone(),
        config: Some(file.with_extension("cfg")),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..CheckRequest::default()
    })
    .expect("Core")
}

#[test]
fn captured_hold_fail_and_empty_callback_keep_primary_counts() {
    for compat_tlc in [true, false] {
        for name in [
            "CapturedConcatHolds",
            "CapturedConcatFails",
            "EmptySkipsCallbackError",
        ] {
            let expected = primary(name);
            let report = check(name, compat_tlc);
            let json = report.to_json();
            let what = format!("{name}/tlc={compat_tlc}: {json}");
            let counts = &expected["result"]["counts"];
            assert_eq!(json["verdict"], expected["result"]["verdict"], "{what}");
            assert_eq!(
                json["statistics"]["distinct"], counts["distinct_states"],
                "{what}"
            );
            assert_eq!(
                json["statistics"]["generated"], counts["states_generated"],
                "{what}"
            );
            assert_eq!(
                json["statistics"]["left_on_queue"], counts["states_left_on_queue"],
                "{what}"
            );
            assert_eq!(json["statistics"]["depth"], counts["depth"], "{what}");
            if expected["result"]["verdict"] == "liveness-violation" {
                let temporal = report.temporal_result().expect("temporal result");
                let dive_api::temporal::Outcome::NativeViolation(v) = temporal.outcome() else {
                    panic!("{what}")
                };
                assert_eq!(v.word().properties, vec!["P".to_owned()], "{what}");
                assert_eq!(v.validation().validation, Validation::Valid, "{what}");
            }
        }
    }
}

/// TLC reports success; Core, like the retired reference analysis, does not
/// plan the property's runtime string creation and refuses by name.
#[test]
fn string_only_native_name_creation_stays_a_named_refusal() {
    assert_eq!(primary("StringConcatNames")["result"]["verdict"], "success");
    for compat_tlc in [true, false] {
        let report = check("StringConcatNames", compat_tlc);
        refused::assert_refused(
            &report.to_json(),
            report.exit_code(),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
    }
}
