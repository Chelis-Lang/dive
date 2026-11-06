//! Applied LET-recursive functions retain their actual lexical activations.

use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
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
                            Validation::Invalid(ref failure) if failure.check == Check::Next
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
                            Validation::Invalid(ref failure) if failure.check == Check::Ending
                                && failure.detail.contains("holds at the end")
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

#[test]
fn parameterized_instance_substitutions_never_reuse_caller_frame_slots()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for config in ["DynamicCorrect", "DynamicOpposite"] {
            let mut oracle_req = request(config, compat_tlc, 1);
            oracle_req.file.set_file_name("DynamicInstance.tla");
            let reference = check_file_engine(&oracle_req, Engine::Reference)?;
            let expected = observed(&reference);
            // Pinned TLC: Correct holds on all three states; Opposite fails
            // initially. Neither answer may be borrowed from a caller slot.
            let correct = config == "DynamicCorrect";
            assert_eq!(
                expected["verdict"],
                if correct {
                    "success"
                } else {
                    "invariant-violation"
                }
            );
            assert_eq!(expected["code"], if correct { 2193 } else { 2107 });
            assert_eq!(expected["exit"], if correct { 0 } else { 12 });
            for workers in [1, 4] {
                let mut req = request(config, compat_tlc, workers);
                req.file.set_file_name("DynamicInstance.tla");
                for switches in ["", "-fold,-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    let doc = core.to_json();
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{config}/{compat_tlc}/{workers}/{switches}: {doc}"
                    );
                    if correct {
                        assert!(doc["counterexample"].is_null());
                    } else {
                        let options = TraceOptions {
                            compat_tlc,
                            run: Some((0, 0)),
                            ..Default::default()
                        };
                        let model = SpecModel::load(
                            &req.file,
                            req.config.as_ref().ok_or("config")?,
                            &options,
                        )?;
                        let payload = doc["counterexample"]["trace"].clone();
                        assert_eq!(
                            model.validate(&TraceDoc::from_json(&payload)?).validation,
                            Validation::Valid
                        );
                        let mut holding = request("DynamicCorrect", compat_tlc, 1);
                        holding.file.set_file_name("DynamicInstance.tla");
                        let model = SpecModel::load(
                            &holding.file,
                            holding.config.as_ref().ok_or("config")?,
                            &options,
                        )?;
                        let mut bad = payload;
                        bad["ending"]["name"] = json!("Correct");
                        assert!(
                            matches!(model.validate(&TraceDoc::from_json(&bad)?).validation,
                            Validation::Invalid(ref failure) if failure.check == Check::Ending
                                && failure.detail.contains("holds at the end"))
                        );
                    }
                }
            }
        }
        let mut req = request("DynamicConstant", compat_tlc, 1);
        req.file.set_file_name("DynamicInstance.tla");
        let reference = check_file_engine(&req, Engine::Reference)?;
        assert_eq!(reference.verdict(), "success");
        for switches in ["", "-fold,-memo,-hashcons,-eager"] {
            let core = check_file_core_opts(&req, switches)?;
            assert_eq!(observed(&core), observed(&reference));
        }
        let mut req = request("DynamicEffect", compat_tlc, 1);
        req.file.set_file_name("DynamicInstance.tla");
        let core = check_file_core_opts(&req, "")?;
        assert_eq!(core.verdict(), "unsupported");
        assert_eq!(
            core.to_json()["message"],
            "an effectful or recursive INSTANCE substitution inside a recursive function (M1 core)"
        );
        assert!(core.to_json()["counterexample"].is_null());
        assert!(core.to_json()["tlc"].is_null());
    }
    Ok(())
}

#[test]
fn recursive_function_action_witness_multiplicity_is_explicitly_refused()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        let mut req = request("ActionCount", compat_tlc, 1);
        req.file.set_file_name("ActionCount.tla");
        let reference = check_file_engine(&req, Engine::Reference)?;
        let doc = reference.to_json();
        // Pinned TLC counts both existential witnesses, although their
        // resulting successor is the same. A value guard would emit only one.
        assert_eq!(reference.verdict(), "success");
        assert_eq!(doc["statistics"]["generated"], 3);
        assert_eq!(doc["statistics"]["distinct"], 1);
        assert_eq!(doc["statistics"]["depth"], 1);
        for workers in [1, 4] {
            req.workers = Some(workers);
            for switches in ["", "-fold,-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&req, switches)?;
                let doc = core.to_json();
                assert_eq!(core.verdict(), "unsupported");
                assert_eq!(
                    doc["message"],
                    "a recursive function used as an action with unproved successor multiplicity (M1 core)"
                );
                assert!(doc["counterexample"].is_null());
                assert!(doc["tlc"].is_null());
            }
        }
    }
    Ok(())
}
