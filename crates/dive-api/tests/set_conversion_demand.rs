//! Pinned Java set conversion demand, receiver and alias controls.
#![allow(clippy::expect_used, reason = "pinned fixtures and model results")]

use dive_api::{CheckReport, CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request(name: &str, compat_tlc: bool) -> CheckRequest {
    let file = root().join(format!(
        "conformance/corpus/set-conversion-demand/specifications/{name}/{name}.tla"
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
                assert_eq!(facts(&report), expected, "{name}/{compat}/{engine:?}");
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
        assert_eq!(d["verdict"], expected["verdict"]);
        assert_eq!(d["message"], expected["message"]);
        assert_eq!(d["tlc"], expected["tlc"]);
        assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);

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
