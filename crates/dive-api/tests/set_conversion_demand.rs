//! Canonical conversions, real argument errors and nominal compatibility gaps.
#![allow(clippy::expect_used, reason = "pinned fixtures and model results")]
#[path = "support/refused.rs"]
mod refused;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckReport, CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

const MIXED_COMPAT: &str = "a set of values of different shapes under --compat=tlc, where TLC's checked comparison may fail or answer by its asymmetric rules (C1–C5; the exact port is not built)";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request(name: &str, compat_tlc: bool) -> CheckRequest {
    request_in("set-conversion-demand", name, compat_tlc)
}

fn diagnostic_request(name: &str, compat_tlc: bool) -> CheckRequest {
    request_in("set-diagnostic-demand", name, compat_tlc)
}

fn request_in(corpus: &str, name: &str, compat_tlc: bool) -> CheckRequest {
    let file = root().join(format!(
        "conformance/corpus/{corpus}/specifications/{name}/{name}.tla"
    ));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        library_paths: vec![root().join("crates/dive-eval/tests/fixtures/community-modules")],
        compat_tlc,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn facts(report: &CheckReport) -> Value {
    let d = report.to_json();
    json!({"verdict":d["verdict"],"code":d["tlc"]["code"],"class":d["tlc"]["name"],
        "distinct":d["statistics"]["distinct"],"generated":d["statistics"]["generated"],
        "depth":d["statistics"]["depth"],"exit":report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn concrete_conversions_preserve_raw_axes_and_nonempty_order_in_both_profiles() {
    for name in [
        "NormalizedProjectionSkipRaw",
        "ReversedRawProjection",
        "NestedIntervalProjection",
        "EmptyProjectionOrderings",
        "NonemptyProjection",
    ] {
        let path = root().join(format!("conformance/golden/tlc-2026.10.06.014338/set-conversion-demand/specifications/{name}/{name}.cfg.json"));
        let d: Value = serde_json::from_slice(&std::fs::read(path).expect("primary golden"))
            .expect("golden JSON");
        let r = &d["result"];
        assert_eq!(r["verdict"], "success");
        let expected = json!({"verdict":r["verdict"],"code":r["verdict_code"],"class":r["error_class"],
            "distinct":r["counts"]["distinct_states"],"generated":r["counts"]["states_generated"],
            "depth":r["counts"]["depth"],"exit":r["exit_code"]});
        for compat in [false, true] {
            {
                let engine = Engine::Core;
                let report =
                    check_file_engine(&request(name, compat), engine).expect("model check");
                if compat
                    && engine == Engine::Core
                    && matches!(
                        name,
                        "NormalizedProjectionSkipRaw"
                            | "ReversedRawProjection"
                            | "NestedIntervalProjection"
                            | "EmptyProjectionOrderings"
                    )
                {
                    assert_eq!(report.verdict(), "unsupported");
                    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
                    assert_eq!(report.to_json()["message"], MIXED_COMPAT);
                } else {
                    assert_eq!(facts(&report), expected, "{name}/{compat}/{engine:?}");
                }
            }
        }
    }
}

#[test]
fn conversion_does_not_erase_receiver_or_later_error_demand() {
    for compat in [false, true] {
        let req = request("ReceiverAfterProjection", compat);
        let core = check_file_engine(&req, Engine::Core).expect("Core receiver");
        let d = core.to_json();
        if compat {
            refused::assert_refused(&d, core.exit_code(), MIXED_COMPAT);
        } else {
            assert_eq!(d["verdict"], "evaluation-error");
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(d["tlc"]["verified"], false);
            assert!(
                d["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("non-tuple"))
            );
        }

        for (name, cause) in [
            ("AliasSizeAfterProjection", "FiniteSets.Cardinality"),
            ("NonemptyMixedProjection", "SequencesExt.SetToSeq"),
            (
                "UnknownFirstProjection",
                "Shouldn't call isEmpty() on value 0",
            ),
        ] {
            assert!(!root().join(format!("conformance/golden/tlc-2026.10.06.014338/set-conversion-demand/specifications/{name}/{name}.cfg.json")).exists());
            let req = request(name, compat);
            let core = check_file_engine(&req, Engine::Core).expect("demanded error");
            let d = core.to_json();
            if name != "UnknownFirstProjection" {
                if compat {
                    // TLC's unverified demanded error; Core refuses the
                    // mixed-shape set under --compat=tlc.
                    refused::assert_refused(&d, core.exit_code(), MIXED_COMPAT);
                } else {
                    assert_eq!(
                        facts(&core),
                        json!({"verdict":"success", "code":2193, "class":"TLC_SUCCESS",
                            "distinct":3, "generated":3, "depth":3, "exit":0}),
                        "{name}"
                    );
                }
                continue;
            }
            assert_eq!(d["verdict"], "evaluation-error", "{name}/{compat}");
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(d["tlc"]["verified"], false);
            let message = d["message"].as_str().expect("error message");
            assert!(message.contains(cause), "{name}/{message}");
            same_argument_error(&req, &core, "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE");
        }
        {
            let engine = Engine::Core;
            let req = request("ProjectionOperandError", compat);
            let report = check_file_engine(&req, engine).expect("operand error");
            let d = report.to_json();
            assert_eq!(d["verdict"], "evaluation-error");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
            assert!(
                d["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("second argument of \\div is 0"))
            );
            let req = request("ProviderProjectionRefusal", compat);
            let report = check_file_engine(&req, engine).expect("provider refusal");
            assert_eq!(report.to_json()["verdict"], "unsupported");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        }
    }
}

#[test]
fn caller_order_distinguishes_receiver_normalization_from_conversion() {
    let name = "OtherConvertedConsumers";
    let path = root().join(format!("conformance/golden/tlc-2026.10.06.014338/set-conversion-demand/specifications/{name}/{name}.cfg.json"));
    let golden: Value =
        serde_json::from_slice(&std::fs::read(path).expect("primary golden")).expect("golden JSON");
    let r = &golden["result"];
    assert_eq!(r["verdict"], "success");
    let pinned = json!({"verdict":r["verdict"],"code":r["verdict_code"],"class":r["error_class"],
        "distinct":r["counts"]["distinct_states"],"generated":r["counts"]["states_generated"],
        "depth":r["counts"]["depth"],"exit":r["exit_code"]});
    for compat in [false, true] {
        for name in [
            "OtherConvertedConsumers",
            "AntiFunctionRawEmpty",
            "PrefixRawEmpty",
        ] {
            let req = request(name, compat);
            let core = check_file_engine(&req, Engine::Core).expect("Core conversion consumer");
            if compat {
                // TLC's normalization-first comparison error (AntiFunction)
                // and its conversion-first argument error (prefix) were
                // reported only by the retired reference engine (#791, #910).
                refused::assert_refused(&core.to_json(), core.exit_code(), MIXED_COMPAT);
                continue;
            }
            if name == "OtherConvertedConsumers" {
                assert_eq!(facts(&core), pinned);
                rejects_false_claims(&req);
                continue;
            }
            let d = core.to_json();
            let message = d["message"].as_str().expect("argument message");
            assert!(message.contains("STARTMSG 2283:"), "{message}");
            assert!(!message.contains("Attempted to compare boolean"));
            if name == "AntiFunctionRawEmpty" {
                assert!(message.contains("The argument of AntiFunction"));
                assert!(message.contains("\n{}\n"));
            } else {
                assert!(message.contains("The argument of LongestCommonPrefix"));
            }
            same_argument_error(&req, &core, "TLC_MODULE_ONE_ARGUMENT_ERROR");
        }
    }
}

#[test]
fn prefix_diagnostic_preserves_selective_original_product_demand() {
    for compat in [false, true] {
        for name in [
            "PrefixRawEmpty",
            "PrefixNestedReverse",
            "PrefixNestedUnknown",
            "PrefixUnknownAfterEmpty",
            "PrefixInfiniteAfterEmpty",
        ] {
            let request = if name == "PrefixRawEmpty" {
                request(name, compat)
            } else {
                diagnostic_request(name, compat)
            };
            let report =
                check_file_engine(&request, Engine::Core).expect("empty projection argument error");
            let d = report.to_json();
            if compat
                && matches!(
                    name,
                    "PrefixRawEmpty" | "PrefixNestedReverse" | "PrefixNestedUnknown"
                )
            {
                refused::assert_refused(&d, report.exit_code(), MIXED_COMPAT);
                continue;
            }
            assert_eq!(d["verdict"], "evaluation-error", "{name}/{compat}");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(d["tlc"]["verified"], false);
            assert_eq!(
                d["message"],
                "@!@!@STARTMSG 2283:0 @!@!@\nThe argument of LongestCommonPrefix should be a non-empty set, but instead it is:\n{}\n@!@!@ENDMSG 2283 @!@!@",
                "{name}/{compat}"
            );
        }

        // PrefixReverse is covered by `core_prefix_results`: TLC's fallback
        // comparison error under --compat=tlc refuses by name on Core.

        let report = check_file_engine(
            &diagnostic_request("PrefixUnknownFirst", compat),
            Engine::Core,
        )
        .expect("earlier unknown operand error");
        let d = report.to_json();
        assert_eq!(d["verdict"], "evaluation-error");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(d["tlc"]["verified"], false);
        assert!(
            d["message"]
                .as_str()
                .is_some_and(|s| s.contains("Shouldn't call isEmpty() on value 0"))
        );

        let report = check_file_engine(
            &diagnostic_request("PrefixInfiniteFirst", compat),
            Engine::Core,
        )
        .expect("retained infinite conversion refusal");
        let d = report.to_json();
        assert_eq!(d["verdict"], "unsupported");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        assert!(
            d["message"]
                .as_str()
                .is_some_and(|s| s.contains("infinite component Nat"))
        );

        ordinary_prefix_matches_golden(compat);

        core_prefix_results(compat);
    }
}

fn core_prefix_results(compat: bool) {
    for name in [
        "PrefixReverse",
        "PrefixNestedReverse",
        "PrefixNestedUnknown",
        "PrefixUnknownAfterEmpty",
        "PrefixUnknownFirst",
        "PrefixInfiniteAfterEmpty",
        "PrefixInfiniteFirst",
    ] {
        let req = diagnostic_request(name, compat);
        let report = check_file_engine(&req, Engine::Core).expect("Core prefix result");
        if name == "PrefixInfiniteFirst" {
            assert_eq!(report.verdict(), "unsupported");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
            assert!(
                report.to_json()["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("infinite component Nat"))
            );
        } else if compat
            && matches!(
                name,
                "PrefixReverse" | "PrefixNestedReverse" | "PrefixNestedUnknown"
            )
        {
            assert_eq!(report.verdict(), "unsupported");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
            assert_eq!(report.to_json()["message"], MIXED_COMPAT);
        } else {
            let ec = if name == "PrefixUnknownFirst" {
                "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE"
            } else {
                "TLC_MODULE_ONE_ARGUMENT_ERROR"
            };
            same_argument_error(&req, &report, ec);
        }
    }
}

fn ordinary_prefix_matches_golden(compat: bool) {
    let path = root().join("conformance/golden/tlc-2026.10.06.014338/set-diagnostic-demand/specifications/PrefixOrdinary/PrefixOrdinary.cfg.json");
    let golden: Value = serde_json::from_slice(&std::fs::read(path).expect("ordinary golden"))
        .expect("golden JSON");
    let r = &golden["result"];
    {
        let engine = Engine::Core;
        let report = check_file_engine(&diagnostic_request("PrefixOrdinary", compat), engine)
            .expect("ordinary prefix");
        assert_eq!(
            facts(&report),
            json!({"verdict":r["verdict"],"code":r["verdict_code"],"class":r["error_class"],
                "distinct":r["counts"]["distinct_states"],"generated":r["counts"]["states_generated"],
                "depth":r["counts"]["depth"],"exit":r["exit_code"]}),
            "{compat}/{engine:?}"
        );
    }
}

// These initial diagnostics intentionally remain unclassified in the report;
// independent replay proves their underlying error, not a TLC receiver class.
fn same_argument_error(req: &CheckRequest, core: &CheckReport, ec: &str) {
    let d = core.to_json();
    assert_eq!(core.verdict(), "evaluation-error");
    assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
    assert_eq!(d["tlc"]["code"], 2103);
    assert_eq!(d["tlc"]["verified"], false);
    assert_eq!(d["statistics"]["generated"], 1);
    assert_eq!(d["statistics"]["distinct"], 1);
    assert_eq!(d["statistics"]["depth"], 1);
    assert!(d["counterexample"].is_null());
    let model = trace_model(req);
    let wire = initial_word(2, &json!({"kind":"invariant-error", "name":"Inv", "ec":ec}));
    assert_eq!(
        model
            .validate(&TraceDoc::from_json(&wire).expect("error word"))
            .validation,
        Validation::Valid
    );
    for (pointer, value) in [
        ("/ending/ec", json!("TLC_DIVISION_BY_ZERO")),
        ("/states/0/values/x", json!(9)),
    ] {
        let mut bad = wire.clone();
        *bad.pointer_mut(pointer).expect("word field") = value;
        assert!(matches!(
            model
                .validate(&TraceDoc::from_json(&bad).expect("forged word"))
                .validation,
            Validation::Invalid(_)
        ));
    }
}

fn trace_model(req: &CheckRequest) -> SpecModel {
    SpecModel::load(
        &req.file,
        req.config.as_ref().expect("config"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            library_paths: req.library_paths.clone(),
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model")
}

fn initial_word(version: u8, ending: &Value) -> Value {
    json!({"format":"dive-trace", "version":version, "variables":["x"],
        "states":[{"action":null,"values":{"x":0}}], "length":1, "ending":ending})
}

fn rejects_false_claims(req: &CheckRequest) {
    let model = trace_model(req);
    for (version, ending) in [
        (1, json!({"kind":"invariant","name":"Inv"})),
        (
            2,
            json!({"kind":"invariant-error","name":"Inv","ec":"TLC_MODULE_ONE_ARGUMENT_ERROR"}),
        ),
    ] {
        let word = TraceDoc::from_json(&initial_word(version, &ending)).expect("false word");
        assert!(matches!(
            model.validate(&word).validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn empty_product_consumers_cannot_turn_a_false_invariant_into_success() {
    // Pinned TLC: TRUE checks 3 generated / 3 distinct / depth 3; FALSE
    // violates Inv initially (2107, exit 12), without reporting counts.
    let file = root()
        .join("crates/dive-api/tests/fixtures/core-receiver-consumers/ConvertedEmptyProducts.tla");
    for compat_tlc in [false, true] {
        for holds in [true, false] {
            let config = file.with_file_name(if holds {
                "ConvertedEmptyProducts.cfg"
            } else {
                "ConvertedEmptyProducts-false.cfg"
            });
            let req = CheckRequest {
                file: file.clone(),
                config: Some(config),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let core = check_file_engine(&req, Engine::Core).expect("paired conversion check");
            if compat_tlc {
                refused::assert_refused(&core.to_json(), core.exit_code(), MIXED_COMPAT);
                continue;
            }
            if holds {
                assert_eq!(
                    facts(&core),
                    json!({"verdict":"success", "code":2193, "class":"TLC_SUCCESS", "distinct":3, "generated":3, "depth":3, "exit":0})
                );
                rejects_false_claims(&req);
            } else {
                assert_eq!(core.to_json()["property"], "Inv");
                assert_eq!(
                    facts(&core),
                    json!({"verdict":"invariant-violation", "code":2107, "class":"TLC_INVARIANT_VIOLATED_INITIAL", "distinct":1, "generated":1, "depth":1, "exit":12})
                );
                let wire = &core.to_json()["counterexample"]["trace"];
                let model = trace_model(&req);
                assert_eq!(
                    model
                        .validate(&TraceDoc::from_json(wire).expect("violation word"))
                        .validation,
                    Validation::Valid
                );
                let mut forged = wire.clone();
                forged["states"][0]["values"]["x"] = json!(1);
                assert!(matches!(
                    model
                        .validate(&TraceDoc::from_json(&forged).expect("changed initial state"))
                        .validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
