//! Oracle-first Reference `IsFiniteSet` compound receiver demand (#771).
#![allow(
    clippy::expect_used,
    reason = "pinned primary fixtures and model checks"
)]
#[path = "support/refused.rs"]
mod refused;

const MIXED_COMPAT: &str = "a set of values of different shapes under --compat=tlc, where TLC's checked comparison may fail or answer by its asymmetric rules (C1–C5; the exact port is not built)";
/// Core refuses a Product factor whose shape or size it has not proved; only
/// the retired reference engine reported TLC's demanded error (#791, #910).
const UNPROVED_FACTOR: &str =
    "Cardinality/IsFiniteSet of a Product with an unproved factor shape or size";

use dive_api::{CheckReport, CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request(name: &str, compat_tlc: bool) -> CheckRequest {
    let file = root().join(format!(
        "conformance/corpus/reference-finiteness-demand/specifications/{name}/{name}.tla"
    ));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn golden_path(name: &str) -> PathBuf {
    root().join(format!("conformance/golden/tlc-2026.10.06.014338/reference-finiteness-demand/specifications/{name}/{name}.cfg.json"))
}

fn golden(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(golden_path(name)).expect("generated oracle fact"))
        .expect("golden JSON")
}

fn facts(report: &CheckReport) -> Value {
    let d = report.to_json();
    json!({"verdict": d["verdict"], "code": d["tlc"]["code"],
        "verified": d["tlc"]["verified"],
        "distinct": d["statistics"]["distinct"], "generated": d["statistics"]["generated"],
        "depth": d["statistics"]["depth"], "property": d["violated_property"],
        "exit": report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn nested_empty_receivers_match_pinned_successes_without_normalizing_raw_aliases() {
    for name in [
        "EmptyBeforeScalar",
        "EmptyRecordBeforeScalar",
        "NestedProductEmpty",
        "OrderedRecordEmpty",
        "EmptyFunctionAndSequence",
        "RawAxesStayRaw",
    ] {
        let g = golden(name);
        let r = &g["result"];
        assert_eq!(r["verdict"], "success");
        let expected = json!({"verdict": "success", "code": r["verdict_code"], "verified": true,
            "distinct": r["counts"]["distinct_states"], "generated": r["counts"]["states_generated"],
            "depth": r["counts"]["depth"], "property": r["violated_property"], "exit": r["exit_code"]});
        for compat in [false, true] {
            let report =
                check_file_engine(&request(name, compat), Engine::Core).expect("Core check");
            if compat && name == "RawAxesStayRaw" {
                refused::assert_refused(&report.to_json(), report.exit_code(), MIXED_COMPAT);
            } else if !compat && matches!(name, "EmptyRecordBeforeScalar" | "OrderedRecordEmpty") {
                // Canonical field order is not TLC's, so the default profile
                // proves every record field before an empty one decides
                // (#919, #910).
                refused::assert_refused(&report.to_json(), report.exit_code(), UNPROVED_FACTOR);
            } else {
                assert_eq!(facts(&report), expected, "{name}/{compat}");
            }
        }
    }
}

#[test]
fn original_finite_and_empty_method_order_keeps_demanded_errors_and_receivers() {
    for compat in [false, true] {
        for (name, cause) in [
            ("ScalarBeforeEmpty", "integer 0 is a finite set"),
            ("NestedScalarBeforeEmpty", "integer 0 is a finite set"),
            ("RecordScalarBeforeEmpty", "integer 0 is a finite set"),
            ("InfiniteBeforeScalar", "integer 0 is a finite set"),
            (
                "OperandBeforeFiniteness",
                "The second argument of \\div is 0",
            ),
            ("AliasSizeAfterFiniteness", "FiniteSets.Cardinality"),
        ] {
            assert!(
                !golden_path(name).exists(),
                "context-only primary observation"
            );
            let report = check_file_engine(&request(name, compat), Engine::Core)
                .expect("Core demanded error");
            let d = report.to_json();
            if matches!(
                name,
                "ScalarBeforeEmpty"
                    | "NestedScalarBeforeEmpty"
                    | "RecordScalarBeforeEmpty"
                    | "InfiniteBeforeScalar"
            ) {
                // TLC reports this unverified demanded error (75).
                refused::assert_refused(&d, report.exit_code(), UNPROVED_FACTOR);
                continue;
            }
            if compat && name == "AliasSizeAfterFiniteness" {
                refused::assert_refused(&d, report.exit_code(), MIXED_COMPAT);
                continue;
            }
            if !compat && name == "AliasSizeAfterFiniteness" {
                assert_eq!(
                    facts(&report),
                    json!({"verdict":"success", "code":2193, "verified":true,
                    "distinct":3, "generated":3, "depth":3, "property":null, "exit":0})
                );
                continue;
            }
            assert_eq!(d["verdict"], "evaluation-error", "{name}/{compat}");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
            if name == "OperandBeforeFiniteness" {
                // Preserve the existing native Init-context route without
                // deriving a deciding class from its nested division message.
                assert_eq!(d["tlc"]["verified"], true);
                assert_eq!(d["tlc"]["code"], 2103);
                assert_eq!(d["tlc"]["reported"], 1000);
            } else {
                assert_eq!(d["tlc"]["verified"], false);
            }
            let message = d["message"].as_str().expect("demanded cause");
            assert!(message.contains(cause), "{name}/{compat}: {message}");
        }

        let g = golden("ReceiverAfterFiniteness");
        assert_eq!(g["result"]["verdict_code"], 2147);
        let report = check_file_engine(&request("ReceiverAfterFiniteness", compat), Engine::Core)
            .expect("Core receiver error");
        let d = report.to_json();
        assert_eq!(d["verdict"], "evaluation-error");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(d["tlc"]["verified"], false);
        assert!(
            d["message"]
                .as_str()
                .is_some_and(|s| s.contains("non-tuple"))
        );
    }
}

#[test]
fn an_unproved_provider_finiteness_method_still_refuses_before_a_later_empty_axis() {
    let g = golden("ProviderBeforeEmpty");
    assert_eq!(g["result"]["verdict"], "success");
    for compat in [false, true] {
        let report = check_file_engine(&request("ProviderBeforeEmpty", compat), Engine::Core)
            .expect("Core provider refusal");
        let d = report.to_json();
        assert_eq!(d["verdict"], "unsupported");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        assert!(
            d["message"]
                .as_str()
                .is_some_and(|s| s.contains("difference of an infinite set")),
            "{d}"
        );
    }
}

#[test]
fn nonlexical_record_tokens_keep_source_success_and_error_order_in_both_profiles() {
    let corpus = "reference-finiteness-token-order";
    let positive = "EncounterTokenEmptyFirst";
    let negative = "EncounterTokenScalarFirst";
    let path = root().join(format!("conformance/golden/tlc-2026.10.06.014338/{corpus}/specifications/{positive}/{positive}.cfg.json"));
    let g: Value = serde_json::from_slice(&std::fs::read(path).expect("generated paired success"))
        .expect("golden JSON");
    let r = &g["result"];
    assert_eq!(r["verdict"], "success");
    let expected = json!({"verdict": "success", "code": r["verdict_code"], "verified": true,
        "distinct": r["counts"]["distinct_states"], "generated": r["counts"]["states_generated"],
        "depth": r["counts"]["depth"], "property": r["violated_property"], "exit": r["exit_code"]});
    assert!(!root().join(format!("conformance/golden/tlc-2026.10.06.014338/{corpus}/specifications/{negative}/{negative}.cfg.json")).exists());
    for compat in [false, true] {
        for name in [positive, negative] {
            let mut req = request(name, compat);
            req.file = root().join(format!(
                "conformance/corpus/{corpus}/specifications/{name}/{name}.tla"
            ));
            req.config = Some(req.file.with_extension("cfg"));
            let report = check_file_engine(&req, Engine::Core).expect("paired token-order check");
            if name == positive && compat {
                assert_eq!(facts(&report), expected);
            } else {
                // TLC answers by its own field order: success with the empty
                // field first, otherwise its unverified 2154 error. Core
                // follows that order only under --compat=tlc; the default
                // profile proves every field first (#919). Either way the
                // unproved factor refuses rather than guessing.
                refused::assert_refused(&report.to_json(), report.exit_code(), UNPROVED_FACTOR);
            }
        }
    }
}
