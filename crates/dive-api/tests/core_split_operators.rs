//! Split roles retain uncaptured operator substitutions without running them.
use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn split_operator_substitutions_preserve_verdicts_counts_and_words()
-> Result<(), Box<dyn std::error::Error>> {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-split-operators");
    // Pinned TLC 2026.10.06.014338 confirms holding 4/3/3, the step
    // violation 2/2/2 (code 2110), and the initial violation's code 2107.
    for compat_tlc in [false, true] {
        for (config, verdict, generated, distinct, depth, code) in [
            ("Holding", "success", 4, 3, 3, 2193),
            ("Violation", "invariant-violation", 2, 2, 2, 2110),
            ("InitialViolation", "invariant-violation", 1, 1, 1, 2107),
        ] {
            let request = CheckRequest {
                file: root.join("SplitOperators.tla"),
                config: Some(root.join(format!("{config}.cfg"))),
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                compat_tlc,
                ..Default::default()
            };
            let reference = check_file_engine(&request, Engine::Reference)?;
            let report = check_file_engine(&request, Engine::Core)?;
            let doc = report.to_json();
            assert_eq!(doc["engine"], "core");
            assert_eq!(doc["verdict"], verdict, "{config}: {doc}");
            assert_eq!(doc["statistics"]["generated"], generated);
            assert_eq!(doc["statistics"]["distinct"], distinct);
            assert_eq!(doc["statistics"]["depth"], depth);
            assert_eq!(doc["tlc"]["code"], code);
            assert_eq!(
                report.exit_code_for(ExitConvention::Tlc),
                if code == 2193 { 0 } else { 12 }
            );
            for field in ["verdict", "statistics", "property", "tlc"] {
                assert_eq!(doc[field], reference.to_json()[field], "{config}/{field}");
            }
            assert_eq!(doc["output"], json!([]), "unused callbacks stay unused");
            if code != 2193 {
                let model = SpecModel::load(
                    &request.file,
                    request.config.as_ref().ok_or("config")?,
                    &TraceOptions {
                        compat_tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    },
                )?;
                let trace = &doc["counterexample"]["trace"];
                assert_eq!(
                    model.validate(&TraceDoc::from_json(trace)?).validation,
                    Validation::Valid
                );
                let mut wrong_initial = trace.clone();
                wrong_initial["states"][0]["values"]["x"] = json!(9);
                assert!(matches!(
                    model.validate(&TraceDoc::from_json(&wrong_initial)?).validation,
                    Validation::Invalid(ref error) if error.check == Check::Init
                ));
                if code == 2110 {
                    let mut wrong_next = trace.clone();
                    wrong_next["states"][1]["values"]["x"] = json!(2);
                    assert!(matches!(
                        model.validate(&TraceDoc::from_json(&wrong_next)?).validation,
                        Validation::Invalid(ref error) if error.check == Check::Next
                    ));
                }
                let mut false_ending = trace.clone();
                false_ending["ending"]["name"] = json!("Inv");
                let holding = SpecModel::load(
                    &request.file,
                    &root.join("Holding.cfg"),
                    &TraceOptions {
                        compat_tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    },
                )?;
                assert!(
                    matches!(holding.validate(&TraceDoc::from_json(&false_ending)?).validation,
                    Validation::Invalid(ref error) if error.check == Check::Ending)
                );
            }
        }
    }
    Ok(())
}

#[test]
fn captured_operator_substitutions_keep_their_named_refusal()
-> Result<(), Box<dyn std::error::Error>> {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-split-operators");
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            file: root.join("CapturedOperators.tla"),
            config: Some(root.join("Captured.cfg")),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            compat_tlc,
            ..Default::default()
        };
        let reference = check_file_engine(&request, Engine::Reference)?;
        assert_eq!(reference.verdict(), "success");
        let core = check_file_engine(&request, Engine::Core)?;
        let report = core.to_json();
        assert_eq!(core.verdict(), "unsupported", "{report}");
        assert_eq!(
            report["message"],
            "an INSTANCE substitution of an operator by a non-top-level operator (M1 core)"
        );
        assert!(report["counterexample"].is_null());
        assert!(report["tlc"].is_null());
    }
    Ok(())
}
