//! Pinned Java set conversion demand, receiver and alias controls.
#![allow(clippy::expect_used, reason = "pinned fixtures and model results")]

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
            for engine in [Engine::Reference, Engine::Core] {
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
        let reference = check_file_engine(&req, Engine::Reference).expect("Reference receiver");
        let expected = reference.to_json();
        assert_eq!(expected["verdict"], "evaluation-error");
        assert_eq!(reference.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(expected["tlc"]["verified"], false);
        assert!(
            expected["message"]
                .as_str()
                .is_some_and(|s| s.contains("non-tuple"))
        );
        let core = check_file_engine(&req, Engine::Core).expect("Core receiver");
        let d = core.to_json();
        if compat {
            assert_eq!(d["verdict"], "unsupported");
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
            assert_eq!(d["message"], MIXED_COMPAT);
        } else {
            assert_eq!(d["verdict"], expected["verdict"]);
            assert_eq!(d["message"], expected["message"]);
            assert_eq!(d["tlc"], expected["tlc"]);
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
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
            let reference = check_file_engine(&req, Engine::Reference).expect("demanded error");
            let d = reference.to_json();
            if !compat && matches!(name, "AliasSizeAfterProjection" | "NonemptyMixedProjection") {
                let expected = json!({"verdict":"success", "code":2193, "class":"TLC_SUCCESS",
                    "distinct":3, "generated":3, "depth":3, "exit":0});
                assert_eq!(facts(&reference), expected, "{name}");
                let core = check_file_engine(&req, Engine::Core).expect("canonical Core check");
                assert_eq!(facts(&core), expected, "{name}");
                continue;
            }
            assert_eq!(d["verdict"], "evaluation-error", "{name}/{compat}");
            assert_eq!(reference.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(d["tlc"]["verified"], false);
            let message = d["message"].as_str().expect("error message");
            assert!(message.contains(cause), "{name}/{message}");
            if name == "AliasSizeAfterProjection" {
                assert!(!message.contains("SequencesExt.SetToSeq"));
            }
            let core = check_file_engine(&req, Engine::Core).expect("guarded Core error");
            assert_eq!(core.to_json()["verdict"], "unsupported");
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
            assert_eq!(
                core.to_json()["message"],
                if name == "UnknownFirstProjection" {
                    "Cardinality/IsFiniteSet of a Product with an unproved factor shape or size"
                } else {
                    MIXED_COMPAT
                }
            );
        }
        for engine in [Engine::Reference, Engine::Core] {
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
    for compat in [false, true] {
        let report = check_file_engine(&request("AntiFunctionRawEmpty", compat), Engine::Reference)
            .expect("normalization-first error");
        let d = report.to_json();
        assert_eq!(d["verdict"], "evaluation-error");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(d["tlc"]["verified"], false);
        let message = d["message"].as_str().expect("normalization message");
        if compat {
            assert!(message.contains("Functions.antiFunction"));
            assert!(message.contains("Attempted to compare boolean TRUE with non-boolean"));
            assert!(!message.contains("The argument of AntiFunction"));
        } else {
            assert!(message.contains("STARTMSG 2283:"));
            assert!(message.contains("The argument of AntiFunction"));
            assert!(message.contains("\n{}\n"));
            assert!(!message.contains("Attempted to compare boolean"));
        }

        let report = check_file_engine(&request("PrefixRawEmpty", compat), Engine::Reference)
            .expect("conversion-first error");
        let d = report.to_json();
        assert_eq!(d["verdict"], "evaluation-error");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(d["tlc"]["verified"], false);
        let message = d["message"].as_str().expect("argument message");
        assert!(message.contains("The argument of LongestCommonPrefix"));
        assert!(!message.contains("Attempted to compare boolean"));

        let name = "OtherConvertedConsumers";
        let report = check_file_engine(&request(name, compat), Engine::Reference)
            .expect("conversion-first consumers");
        let path = root().join(format!("conformance/golden/tlc-2026.10.06.014338/set-conversion-demand/specifications/{name}/{name}.cfg.json"));
        let golden: Value = serde_json::from_slice(&std::fs::read(path).expect("primary golden"))
            .expect("golden JSON");
        let r = &golden["result"];
        assert_eq!(r["verdict"], "success");
        assert_eq!(
            facts(&report),
            json!({"verdict":r["verdict"],"code":r["verdict_code"],"class":r["error_class"],
            "distinct":r["counts"]["distinct_states"],"generated":r["counts"]["states_generated"],
            "depth":r["counts"]["depth"],"exit":r["exit_code"]})
        );
        for name in [
            "OtherConvertedConsumers",
            "AntiFunctionRawEmpty",
            "PrefixRawEmpty",
        ] {
            let core = check_file_engine(&request(name, compat), Engine::Core)
                .expect("retained Core guard");
            assert_eq!(core.to_json()["verdict"], "unsupported");
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
            let reason = if compat {
                MIXED_COMPAT
            } else {
                match name {
                    "AntiFunctionRawEmpty" => {
                        "Functions!AntiFunction consuming an unproved product receiver"
                    }
                    "PrefixRawEmpty" => {
                        "SequencesExt!LongestCommonPrefix consuming an unproved product receiver"
                    }
                    _ => "a standard operator consuming an unproved product receiver",
                }
            };
            assert_eq!(core.to_json()["message"], reason);
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
            let report = check_file_engine(&request, Engine::Reference)
                .expect("empty projection argument error");
            let d = report.to_json();
            assert_eq!(d["verdict"], "evaluation-error", "{name}/{compat}");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(d["tlc"]["verified"], false);
            assert_eq!(
                d["message"],
                "@!@!@STARTMSG 2283:0 @!@!@\nThe argument of LongestCommonPrefix should be a non-empty set, but instead it is:\n{}\n@!@!@ENDMSG 2283 @!@!@",
                "{name}/{compat}"
            );
        }

        let report = check_file_engine(
            &diagnostic_request("PrefixReverse", compat),
            Engine::Reference,
        )
        .expect("original fallback comparison");
        let d = report.to_json();
        assert_eq!(d["verdict"], "evaluation-error");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(d["tlc"]["verified"], false);
        let message = d["message"].as_str().expect("comparison error");
        if compat {
            assert!(message.contains("SequencesExt.longestCommonPrefix"));
            assert!(message.contains("Attempted to compare boolean TRUE with non-boolean:\n0"));
            assert!(!message.contains("The argument of LongestCommonPrefix"));
        } else {
            assert!(message.contains("STARTMSG 2283:"));
            assert!(message.contains("The argument of LongestCommonPrefix"));
            assert!(message.contains("\n{}\n"));
            assert!(!message.contains("Attempted to compare boolean"));
        }
        assert!(!message.contains("<unprintable"));

        let report = check_file_engine(
            &diagnostic_request("PrefixUnknownFirst", compat),
            Engine::Reference,
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
            Engine::Reference,
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

        for name in [
            "PrefixReverse",
            "PrefixNestedReverse",
            "PrefixNestedUnknown",
            "PrefixUnknownAfterEmpty",
            "PrefixUnknownFirst",
            "PrefixInfiniteAfterEmpty",
            "PrefixInfiniteFirst",
        ] {
            let report = check_file_engine(&diagnostic_request(name, compat), Engine::Core)
                .expect("retained Core prefix refusal");
            assert_eq!(
                report.to_json()["verdict"],
                "unsupported",
                "{name}/{compat}"
            );
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        }
    }
}

fn ordinary_prefix_matches_golden(compat: bool) {
    let path = root().join("conformance/golden/tlc-2026.10.06.014338/set-diagnostic-demand/specifications/PrefixOrdinary/PrefixOrdinary.cfg.json");
    let golden: Value = serde_json::from_slice(&std::fs::read(path).expect("ordinary golden"))
        .expect("golden JSON");
    let r = &golden["result"];
    for engine in [Engine::Reference, Engine::Core] {
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
