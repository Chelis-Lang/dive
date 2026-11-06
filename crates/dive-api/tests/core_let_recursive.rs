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
fn escape_operator_and_nested_recursion_keep_named_refusals()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, message) in [
            ("Escape", "used other than applied"),
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
fn recursive_action_predicates_preserve_truth_without_duplicate_witnesses()
-> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/predicate-witness-counts/specifications/ActionCount");
    for compat_tlc in [false, true] {
        for (config, positive, deadlock) in [
            ("ActionCount", true, false),
            ("ActionCountOpposite", false, false),
            ("ActionCountFalse", false, true),
        ] {
            let mut req = CheckRequest {
                file: root.join(if deadlock {
                    "ActionCountFalse.tla"
                } else {
                    "ActionCount.tla"
                }),
                config: Some(root.join(format!("{config}.cfg"))),
                workers: Some(1),
                compat_tlc,
                seed: Some(0),
                fp: Some(0),
                ..Default::default()
            };
            let reference = check_file_engine(&req, Engine::Reference)?;
            let expected = observed(&reference);
            assert_eq!(
                expected["verdict"],
                if positive {
                    "success"
                } else if deadlock {
                    "deadlock"
                } else {
                    "invariant-violation"
                }
            );
            assert_eq!(
                expected["code"],
                if positive {
                    2193
                } else if deadlock {
                    2114
                } else {
                    2107
                }
            );
            assert_eq!(
                expected["exit"],
                if positive {
                    0
                } else if deadlock {
                    11
                } else {
                    12
                }
            );
            assert_eq!(
                expected["statistics"]["generated"],
                if positive { 3 } else { 1 }
            );
            assert_eq!(expected["statistics"]["distinct"], 1);
            assert_eq!(expected["statistics"]["depth"], 1);
            assert_eq!(
                expected["property"],
                if positive || deadlock {
                    Value::Null
                } else {
                    json!("Opposite")
                }
            );
            let options = TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..Default::default()
            };
            let model = SpecModel::load(&req.file, req.config.as_ref().ok_or("config")?, &options)?;
            for workers in [1, 4] {
                req.workers = Some(workers);
                for switches in ["", "-fold,-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    let doc = core.to_json();
                    let mut native = observed(&core);
                    assert_eq!(
                        native["statistics"]["generated"],
                        if positive { 2 } else { 1 }
                    );
                    // D-029 authorizes exactly this count pair. Everything
                    // else, including class, property and physical exit, agrees.
                    if positive {
                        native["statistics"]["generated"] = json!(3);
                    }
                    assert_eq!(
                        native, expected,
                        "{config}/{compat_tlc}/{workers}/{switches}"
                    );
                    assert_eq!(doc["engine"], "core");
                    validate_action_predicate_word(&req, &model, positive, &doc)?;
                }
            }
        }
    }
    Ok(())
}

fn primed_request(config: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let mut req = request(config, compat_tlc, workers);
    if config.starts_with("Queue") {
        req.file.set_file_name("PrimedQueue.tla");
    } else if config.starts_with("Top") {
        req.file.set_file_name("TopRecInstance.tla");
    } else if config != "Prime" {
        req.file.set_file_name("PrimedCalls.tla");
    }
    req
}

#[test]
fn top_level_recursive_instances_never_borrow_the_callers_frame()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        // Pinned TLC 94d0c50: both the plain and mixed opposite claims fail.
        // Reusing the caller thunk slot instead returns 1 and reverses them.
        for config in [
            "TopPlainHolding",
            "TopMixedHolding",
            "TopPrimeHolding",
            "TopPlainWrong",
            "TopMixedWrong",
        ] {
            let wrong = config.ends_with("Wrong");
            let reference =
                check_file_engine(&primed_request(config, compat_tlc, 1), Engine::Reference)?;
            let expected = observed(&reference);
            assert_eq!(
                expected["verdict"],
                if wrong {
                    "action-property-violation"
                } else {
                    "success"
                }
            );
            assert_eq!(expected["code"], if wrong { 2112 } else { 2193 });
            assert_eq!(expected["exit"], if wrong { 13 } else { 0 });
            assert_eq!(
                expected["statistics"]["generated"],
                if wrong { 3 } else { 17 }
            );
            assert_eq!(
                expected["statistics"]["distinct"],
                if wrong { 2 } else { 4 }
            );
            assert_eq!(expected["statistics"]["depth"], 2);
            for workers in [1, 4] {
                let req = primed_request(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{config}/{compat_tlc}/{workers}/{switches}"
                    );
                    if wrong {
                        validate_primed_word(&req, &core, compat_tlc, config)?;
                    } else {
                        assert!(core.to_json()["counterexample"].is_null());
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn primed_domains_bodies_and_arguments_observe_each_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        // Pinned TLC 94d0c50: the two candidates have different domains,
        // recursive bodies and arguments. OnlyOne retains just x'=1.
        for (config, generated, distinct, depth) in [
            ("Prime", 4, 3, 3),
            ("PrimedHolding", 7, 3, 2),
            ("PrimedReverse", 7, 3, 2),
            ("PrimedValue", 7, 3, 2),
            ("PrimedFiltered", 5, 2, 2),
            ("PrimedUnused", 7, 3, 2),
            ("QueueHolding", 3, 2, 2),
            ("PrimedWrong", 2, 2, 2),
            ("QueueWrong", 2, 2, 2),
        ] {
            let wrong = config.ends_with("Wrong");
            let reference =
                check_file_engine(&primed_request(config, compat_tlc, 1), Engine::Reference)?;
            let expected = observed(&reference);
            assert_eq!(
                expected["verdict"],
                if wrong {
                    "action-property-violation"
                } else {
                    "success"
                }
            );
            assert_eq!(expected["code"], if wrong { 2112 } else { 2193 });
            assert_eq!(expected["exit"], if wrong { 13 } else { 0 });
            assert_eq!(expected["statistics"]["generated"], generated);
            assert_eq!(expected["statistics"]["distinct"], distinct);
            assert_eq!(expected["statistics"]["depth"], depth);
            for workers in [1, 4] {
                let req = primed_request(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{config}/{compat_tlc}/{workers}/{switches}"
                    );
                    if wrong {
                        validate_primed_word(&req, &core, compat_tlc, config)?;
                    } else {
                        assert!(core.to_json()["counterexample"].is_null());
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_primed_word(
    req: &CheckRequest,
    core: &dive_api::CheckReport,
    compat_tlc: bool,
    config: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let options = TraceOptions {
        compat_tlc,
        run: Some((0, 0)),
        ..Default::default()
    };
    let model = SpecModel::load(&req.file, req.config.as_ref().ok_or("config")?, &options)?;
    let payload = core.to_json()["counterexample"]["trace"].clone();
    assert_eq!(
        model.validate(&TraceDoc::from_json(&payload)?).validation,
        Validation::Valid
    );
    let mut bad = payload.clone();
    bad["states"][1]["values"]["x"] = json!(if config.starts_with("Top") { 4 } else { 3 });
    assert!(matches!(
        model.validate(&TraceDoc::from_json(&bad)?).validation,
        Validation::Invalid(ref failure) if failure.check == Check::Next
    ));
    let holding = primed_request(
        if config == "QueueWrong" {
            "QueueHolding"
        } else if config.starts_with("Top") {
            "TopMixedHolding"
        } else {
            "PrimedHolding"
        },
        compat_tlc,
        1,
    );
    let holding_model = SpecModel::load(
        &holding.file,
        holding.config.as_ref().ok_or("holding config")?,
        &options,
    )?;
    let mut bad = payload;
    bad["ending"]["name"] = json!("Correct");
    assert!(matches!(
        holding_model.validate(&TraceDoc::from_json(&bad)?).validation,
        Validation::Invalid(ref failure) if failure.check == Check::Ending
            && failure.detail.contains("holds")
    ));
    Ok(())
}

#[test]
fn primed_captures_mixed_self_calls_effects_and_escapes_stay_refused()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, message) in [
            (
                "PrimedCaptured",
                "a recursive LET definition reading an enclosing local under a prime (M1 core)",
            ),
            (
                "PrimedMixed",
                "a recursive function with mixed plain and primed self applications (M1 core)",
            ),
            (
                "PrimedEffect",
                "an effectful recursive function under a prime (M1 core)",
            ),
            (
                "PrimedEscape",
                "a recursive function used other than applied (M1 core)",
            ),
            (
                "TopParameter",
                "a recursive function with a dynamic INSTANCE capture outside its lexical activation (M1 core)",
            ),
        ] {
            for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&primed_request(config, compat_tlc, 1), switches)?;
                let doc = core.to_json();
                assert_eq!(core.verdict(), "unsupported", "{config}: {doc}");
                assert_eq!(doc["message"], message);
                assert!(doc["tlc"].is_null());
                assert!(doc["counterexample"].is_null());
            }
        }
    }
    Ok(())
}

fn validate_action_predicate_word(
    req: &CheckRequest,
    model: &SpecModel,
    positive: bool,
    doc: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let options = TraceOptions {
        compat_tlc: req.compat_tlc,
        run: Some((0, 0)),
        ..Default::default()
    };
    if positive {
        assert!(doc["counterexample"].is_null());
        let mut word = json!({"format":"dive-trace", "version":1,
            "variables":["x"], "states":[{"values":{"x":0}},{"values":{"x":0}}],
            "ending":{"kind":"behavior"}});
        assert_eq!(
            model.validate(&TraceDoc::from_json(&word)?).validation,
            Validation::Valid
        );
        word["states"][1]["values"]["x"] = json!(1);
        assert!(
            matches!(model.validate(&TraceDoc::from_json(&word)?).validation,
            Validation::Invalid(ref failure) if failure.check == Check::Next)
        );
    } else {
        let mut word = doc["counterexample"]["trace"].clone();
        assert_eq!(
            model.validate(&TraceDoc::from_json(&word)?).validation,
            Validation::Valid
        );
        word["ending"] = json!({"kind":"invariant", "name":"Inv"});
        let holding = SpecModel::load(
            &req.file,
            &req.file.with_file_name("ActionCount.cfg"),
            &options,
        )?;
        assert!(
            matches!(holding.validate(&TraceDoc::from_json(&word)?).validation,
            Validation::Invalid(ref failure) if failure.check == Check::Ending)
        );
    }
    Ok(())
}
