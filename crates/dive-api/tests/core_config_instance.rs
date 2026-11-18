//! Configured definitions retain their owned binding through imported declarations.
#![allow(clippy::expect_used, reason = "pinned regression fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_config_instance.json");

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file_core_opts};
use serde_json::{Value, json};

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-config-instance")
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(workers),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn check_word(req: &CheckRequest, doc: &Value) {
    let model = SpecModel::load(
        &req.file,
        req.config.as_ref().expect("config"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model");
    let payload = if doc["counterexample"]["trace"].is_null() {
        &doc["diagnostic_witness"]["trace"]
    } else {
        &doc["counterexample"]["trace"]
    };
    assert!(!payload.is_null(), "required named word: {doc}");
    let mut word = payload.clone();
    let trace = TraceDoc::from_json(&word).expect("actual word");
    assert!(matches!(
        model.validate(&trace).validation,
        Validation::Valid
    ));
    word["states"][0]["values"]["x"] = json!(99);
    let trace = TraceDoc::from_json(&word).expect("forged state");
    assert!(matches!(
        model.validate(&trace).validation,
        Validation::Invalid(_)
    ));
    word = payload.clone();
    word["ending"]["name"] = json!("AbsentProperty");
    let trace = TraceDoc::from_json(&word).expect("forged property");
    assert!(matches!(
        model.validate(&trace).validation,
        Validation::Invalid(_)
    ));
    if payload["ending"]["kind"] == "invariant-error" {
        word = payload.clone();
        assert_eq!(word["ending"]["ec"], "TLC_MODULE_DIVISION_BY_ZERO");
        word["ending"]["ec"] = json!("TLC_VALUE_ASSERT_FAILED");
        let trace = TraceDoc::from_json(&word).expect("forged error class");
        assert!(matches!(
            model.validate(&trace).validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "pinned oracle table and complete result checks"
)]
fn configured_imported_constants_match_pinned_results_in_both_profiles() {
    // Pinned TLC tlc-2026.10.06.014338, sha256
    // 7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d,
    // -workers 1 -seed 0 -fp 0 -tool. Initial violations print no count/depth.
    for (name, verdict, code, class, exit, property, counts) in [
        (
            "BindingHolding",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingCapture",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingOwner",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingOverride",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingStep",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingOpposite",
            "invariant-violation",
            2107,
            "TLC_INVARIANT_VIOLATED_INITIAL",
            12,
            Some("Inv"),
            None,
        ),
        (
            "BindingStepOpposite",
            "action-property-violation",
            2112,
            "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR",
            13,
            Some("Prop"),
            Some([2, 2, 2]),
        ),
        (
            "BindingLazyErrorDirect",
            "evaluation-error",
            2111,
            "TLC_INVARIANT_EVALUATION_FAILED",
            76,
            None,
            Some([2, 2, 2]),
        ),
        (
            "BindingUnusedErrorDirect",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([3, 2, 2]),
        ),
        (
            "BindingDemandedError",
            "evaluation-error",
            2111,
            "TLC_INVARIANT_EVALUATION_FAILED",
            76,
            None,
            Some([2, 2, 2]),
        ),
        (
            "BindingSkippedError",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([3, 2, 2]),
        ),
        (
            "BindingEffects",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingEffectsDirect",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingIndirectEffects",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "BindingEffectOverride",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "AppliedRngBefore",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([2, 1, 1]),
        ),
        (
            "AppliedRngAfter",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([2, 1, 1]),
        ),
        (
            "AppliedUnusedError",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([2, 1, 1]),
        ),
        (
            "AppliedPrimeHolding",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([3, 2, 2]),
        ),
        (
            "AppliedPrimeOpposite",
            "action-property-violation",
            2112,
            "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR",
            13,
            Some("Prop"),
            Some([2, 2, 2]),
        ),
        (
            "AppliedOverride",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([2, 1, 1]),
        ),
        (
            "BoolHelperAsValue",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([3, 2, 2]),
        ),
        (
            "BoolHelperAsPredicate",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([3, 2, 2]),
        ),
        (
            "DirectPrintHelperOrdered",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
        (
            "DirectPrintConfigHelperOrdered",
            "success",
            2193,
            "TLC_SUCCESS",
            0,
            None,
            Some([4, 3, 3]),
        ),
    ] {
        for compat in [false, true] {
            for workers in [1, 4] {
                let req = request(name, compat, workers);
                for opts in ["", "-memo,-step-memo", "-fold,-hashcons"] {
                    let report = check_file_core_opts(&req, opts).expect("Core report");
                    let doc = report.to_json();
                    let context = format!("{name}/{compat}/{workers}/{opts}: {doc}");
                    assert_eq!(doc["engine"], "core", "{context}");
                    assert_eq!(doc["verdict"], verdict, "{context}");
                    assert_eq!(doc["tlc"]["code"], code, "{context}");
                    assert_eq!(doc["tlc"]["name"], class, "{context}");
                    assert_eq!(report.exit_code_for(ExitConvention::Tlc), exit, "{context}");
                    assert_eq!(doc["property"], json!(property), "{context}");
                    if let Some([generated, distinct, depth]) = counts {
                        assert_eq!(doc["statistics"]["generated"], generated, "{context}");
                        assert_eq!(doc["statistics"]["distinct"], distinct, "{context}");
                        assert_eq!(doc["statistics"]["depth"], depth, "{context}");
                    } else {
                        // Native accounting, not counts observed from TLC.
                        for field in ["generated", "distinct", "depth"] {
                            assert_eq!(doc["statistics"][field], 1, "{context}");
                        }
                        assert_eq!(doc["statistics"]["reported_by_tlc"], false);
                    }
                    if verdict != "success" {
                        check_word(&req, &doc);
                    }
                    if name.starts_with("AppliedRng") {
                        let reference = dive_api::check_file(&req)
                            .expect("reference startup argument")
                            .to_json();
                        agreed::check_eq(
                            AGREED,
                            "configured_imported_constants_match_pinned_results_in_both_profiles/0",
                            &(doc["output"]),
                            &(reference["output"]),
                        );
                        if compat {
                            assert_eq!(doc["output"], json!(["<<731, 731>>"]), "{context}");
                        }
                    }
                    if let Some(count) = match name {
                        "BindingEffects" => Some(8),
                        "BindingEffectsDirect" => Some(7),
                        "BindingIndirectEffects" => Some(9),
                        "BindingEffectOverride" => Some(0),
                        "BoolHelperAsValue" | "BoolHelperAsPredicate" => Some(1),
                        "DirectPrintHelperOrdered" => Some(2),
                        "DirectPrintConfigHelperOrdered" => Some(3),
                        _ => None,
                    } {
                        let output = doc["output"].as_array().expect("printed output");
                        assert_eq!(output.len(), count, "{context}");
                        for message in output {
                            assert_eq!(
                                message,
                                if name.starts_with("BoolHelper") {
                                    "\"bool-helper\""
                                } else if name.starts_with("DirectPrint") {
                                    "\"review-effect\""
                                } else {
                                    "\"bound-target\""
                                },
                                "{context}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn unproved_lazy_configured_instance_values_refuse_instead_of_claiming_errors() {
    // Direct C[x] is covered above. Core cannot yet keep this configured
    // function's undemanded entries lazy through A!Read. Independent evaluation
    // prevents that implementation limitation from claiming a specification error.
    for compat in [false, true] {
        for name in ["BindingLazyError", "BindingUnusedError"] {
            let req = request(name, compat, 1);
            let doc = check_file_core_opts(&req, "").expect("report").to_json();
            assert_eq!(doc["engine"], "core");
            assert_eq!(doc["verdict"], "unsupported", "{doc}");
            assert!(doc["counterexample"].is_null());
            let message = doc["message"].as_str().expect("named refusal");
            assert!(
                message
                    .contains("an evaluation error that independent validation does not reproduce"),
                "{message}"
            );
        }
    }
}

#[test]
fn indirect_register_targets_keep_writes_and_never_turn_a_violation_into_success() {
    // Pinned TLC: Holding succeeds with 4 generated / 3 distinct / depth 3;
    // Opposite violates Inv in the initial state (2107 / exit 12).
    for compat in [false, true] {
        for workers in [1, 4] {
            for opts in ["", "-memo,-step-memo", "-fold,-hashcons"] {
                let req = request("BindingRegisterHolding", compat, workers);
                let doc = check_file_core_opts(&req, opts).expect("holding").to_json();
                assert_eq!(doc["verdict"], "success", "{doc}");
                assert_eq!(doc["tlc"]["code"], 2193);
                assert_eq!(doc["tlc"]["name"], "TLC_SUCCESS");
                assert_eq!(doc["tlc"]["exit_status"], 0);
                for (field, count) in [("generated", 4), ("distinct", 3), ("depth", 3)] {
                    assert_eq!(doc["statistics"][field], count);
                }
                let req = request("BindingRegisterOpposite", compat, workers);
                let doc = check_file_core_opts(&req, opts)
                    .expect("opposite")
                    .to_json();
                // Non-success register witnesses are already unsupported by Core;
                // do not turn missing runtime writes into a false successful run.
                assert_eq!(doc["verdict"], "unsupported", "{doc}");
                assert_eq!(
                    doc["message"],
                    "TLCSet or TLCGet in a run that does not end in success (M1 core)"
                );
                assert!(doc["counterexample"].is_null());
            }
        }
    }
}

#[test]
fn enclosing_helpers_cannot_hide_configured_register_effects() {
    // The five exact review repros are pinned to a successor Inv violation:
    // TLC 2110 / exit 12, 2 generated / 2 distinct / depth 2. Their paired
    // Holding controls succeed with 3 generated / 2 distinct / depth 2.
    for name in [
        "ImportedRegisterImmediate",
        "DirectRegisterHelperOrdered",
        "DirectRegisterConfigHelperOrdered",
        "ImportedRegisterHelperOrdered",
        "ImportedRegisterConfigHelperOrdered",
        "AppliedHelper",
        "NumericRegisterHelper",
        "BoolRegisterHelperAsValue",
    ] {
        for compat in [false, true] {
            for workers in [1, 4] {
                for timed in [false, true] {
                    for opts in ["", "-memo,-step-memo", "-fold,-hashcons"] {
                        let mut req = request(&format!("{name}Holding"), compat, workers);
                        if timed {
                            req.time_limit = dive_api::BudgetSetting::Explicit(
                                std::time::Duration::from_secs(10),
                            );
                        }
                        let doc = check_file_core_opts(&req, opts).expect("holding").to_json();
                        assert_eq!(doc["engine"], "core");
                        assert_eq!(doc["verdict"], "success", "{name}/{timed}/{opts}: {doc}");
                        assert_eq!(doc["tlc"]["code"], 2193);
                        assert_eq!(doc["tlc"]["exit_status"], 0);
                        for (field, count) in [("generated", 3), ("distinct", 2), ("depth", 2)] {
                            assert_eq!(doc["statistics"][field], count, "{name}/{timed}/{opts}");
                        }
                        req.file = request(name, compat, workers).file;
                        req.config = Some(req.file.with_extension("cfg"));
                        let doc = check_file_core_opts(&req, opts)
                            .expect("violation")
                            .to_json();
                        // Register-dependent words remain outside independent
                        // replay support. A precise refusal is required here;
                        // erasing the writes must never become a false success.
                        assert_eq!(
                            doc["verdict"], "unsupported",
                            "{name}/{timed}/{opts}: {doc}"
                        );
                        assert_eq!(
                            doc["message"],
                            "TLCSet or TLCGet in a run that does not end in success (M1 core)",
                            "{name}/{timed}/{opts}"
                        );
                        assert!(doc["counterexample"].is_null());
                    }
                }
            }
        }
    }
}

#[test]
fn unresolved_calls_in_configured_targets_refuse_without_reusing_a_foreign_body() {
    // TLC succeeds with 2 generated / 1 distinct / depth 1. Function-body
    // cache identities do not yet include configured-target compilation context.
    for compat in [false, true] {
        for opts in ["", "-memo,-step-memo", "-fold,-hashcons"] {
            let req = request("BindingUnprovedCall", compat, 1);
            let doc = check_file_core_opts(&req, opts).expect("report").to_json();
            assert_eq!(doc["engine"], "core");
            assert_eq!(doc["verdict"], "unsupported", "{doc}");
            assert_eq!(
                doc["message"],
                "a configured constant target with unresolved recursive or ENABLED calls (core)"
            );
            assert!(doc["counterexample"].is_null());
        }
    }
}

#[test]
fn demanded_initial_argument_errors_keep_the_context_report_and_validate_the_cause() {
    // Pinned TLC: division by zero inside the first invariant, contextual2103,
    // physical exit75, 1 generated / 1 distinct / depth1, no formal behavior.
    for compat in [false, true] {
        for opts in ["", "-memo,-step-memo", "-fold,-hashcons"] {
            let req = request("AppliedUsedError", compat, 1);
            let report = check_file_core_opts(&req, opts).expect("initial error");
            let doc = report.to_json();
            assert_eq!(doc["verdict"], "evaluation-error", "{doc}");
            assert_eq!(doc["tlc"]["code"], 2103);
            assert_eq!(doc["tlc"]["name"], "TLC_NESTED_EXPRESSION");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
            assert!(doc["counterexample"].is_null());
            for field in ["generated", "distinct", "depth"] {
                assert_eq!(doc["statistics"][field], 1);
            }
            assert!(doc["message"].as_str().expect("cause").contains("2179"));
            // Authored cause witness, not a producer-emitted formal trace. This
            // independently proves the demanded error and rejects forged causes.
            let cause = json!({"counterexample":{"trace":{
                "format":"dive-trace", "version":2, "variables":["x"],
                "states":[{"action":null,"values":{"x":0}}], "length":1,
                "ending":{"kind":"invariant-error","name":"Inv", "ec":"TLC_MODULE_DIVISION_BY_ZERO"}
            }}});
            check_word(&req, &cause);
        }
    }
}

#[test]
fn lazy_field_and_materialized_forces_never_cache_startup_register_success() {
    // The exact field control has TLC's incidental STRING-cardinality error;
    // default semantics need not emulate it. MaterializedApply has TLC's real
    // x=0 -> x=1 violation. Both native forms may precisely refuse, never hold.
    for (name, feature) in [
        ("LazyField", "STRING (M0 core)"),
        (
            "MaterializedApply",
            "Print, PrintT, RandomElement, TLCSet or TLCGet inside a function constructor, set filter, set map or operator argument, which TLC may evaluate lazily or in its own order (M1 core)",
        ),
    ] {
        for compat in [false, true] {
            let mut req = request(name, compat, 1);
            req.time_limit = dive_api::BudgetSetting::Explicit(std::time::Duration::from_secs(10));
            let doc = check_file_core_opts(&req, "")
                .expect("force control")
                .to_json();
            assert_eq!(doc["engine"], "core");
            assert_eq!(doc["verdict"], "unsupported", "{name}/{compat}: {doc}");
            assert_eq!(doc["message"], feature);
            assert!(doc["counterexample"].is_null());
        }
    }
}
