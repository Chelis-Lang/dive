//! Applied LET-recursive functions retain their actual lexical activations.

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

fn request(config: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-let-recursive");
    CheckRequest {
        file: root.join("LetRecCalls.tla"),
        config: Some(root.join(format!("{config}.cfg"))),
        workers: Some(workers),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict": doc["verdict"], "statistics": doc["statistics"],
        "property": doc["property"], "class": doc["tlc"]["name"],
        "code": doc["tlc"]["code"], "exit": report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn lexical_captures_repeated_calls_and_substitutions_match_reference()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, verdict, generated, distinct, depth) in [
            ("Holding", "success", 4, 3, 3),
            ("Violation", "invariant-violation", 2, 2, 2),
        ] {
            let reference = check_file_engine(&request(config, compat_tlc, 1), Engine::Reference)?;
            let expected = observed(&reference);
            assert_eq!(expected["verdict"], verdict);
            assert_eq!(expected["statistics"]["generated"], generated);
            assert_eq!(expected["statistics"]["distinct"], distinct);
            assert_eq!(expected["statistics"]["depth"], depth);
            // The pinned TLC oracle confirms these complete holding/violation
            // controls, including exact counts, depth, property and exit.
            if config == "Holding" {
                assert_eq!(expected["code"], 2193);
                assert_eq!(expected["exit"], 0);
                assert!(expected["property"].is_null());
            } else {
                assert_eq!(expected["code"], 2110);
                assert_eq!(expected["exit"], 12);
                assert_eq!(expected["property"], "Wrong");
            }
            for workers in [1, 4] {
                let req = request(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{config}/{compat_tlc}/{switches}"
                    );
                    if config == "Violation" {
                        let model = SpecModel::load(
                            &req.file,
                            req.config.as_ref().ok_or("config")?,
                            &TraceOptions {
                                compat_tlc,
                                run: Some((0, 0)),
                                ..Default::default()
                            },
                        )?;
                        let payload = core.to_json()["counterexample"]["trace"].clone();
                        let word = TraceDoc::from_json(&payload)?;
                        assert_eq!(model.validate(&word).validation, Validation::Valid);
                        let mut bad = payload.clone();
                        bad["states"][1]["values"]["x"] = json!(2);
                        assert!(matches!(
                            model.validate(&TraceDoc::from_json(&bad)?).validation,
                            Validation::Invalid(_)
                        ));
                        let mut bad = payload;
                        // Inv holds throughout this actual word; naming it
                        // cannot turn a valid Wrong violation into another one.
                        bad["ending"]["name"] = json!("Inv");
                        let holding = request("Holding", compat_tlc, 1);
                        let holding_model = SpecModel::load(
                            &holding.file,
                            holding.config.as_ref().ok_or("holding config")?,
                            &TraceOptions {
                                compat_tlc,
                                run: Some((0, 0)),
                                ..Default::default()
                            },
                        )?;
                        assert!(matches!(
                            holding_model
                                .validate(&TraceDoc::from_json(&bad)?)
                                .validation,
                            Validation::Invalid(_)
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn escape_prime_operator_and_nested_recursion_keep_named_refusals()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, message) in [
            ("Escape", "used other than applied"),
            ("Prime", "under a prime"),
            ("Operator", "operator parameter"),
            ("Nested", "nested recursive"),
        ] {
            let core = check_file_core_opts(&request(config, compat_tlc, 1), "")?;
            let doc = core.to_json();
            assert_eq!(core.verdict(), "unsupported", "{config}: {doc}");
            assert!(
                doc["message"].as_str().is_some_and(|m| m.contains(message)),
                "{config}: {doc}"
            );
            assert!(doc["tlc"].is_null());
            assert!(doc["counterexample"].is_null());
        }
    }
    Ok(())
}
