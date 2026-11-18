//! Applied LET-recursive functions retain their actual lexical activations.
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_let_recursive.json");

use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file, check_file_core_opts};
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
            let reference = check_file(&request(config, compat_tlc, 1))?;
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
fn escape_and_nested_recursion_keep_named_refusals() -> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, message) in [
            ("Escape", "used other than applied"),
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

fn op_request(config: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let mut req = request(config, compat_tlc, workers);
    if config != "Operator" {
        req.file.set_file_name("RecOpCalls.tla");
    }
    req
}

fn validate_op_word(
    req: &CheckRequest,
    core: &dive_api::CheckReport,
    compat_tlc: bool,
    prime: bool,
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
    bad["states"][1]["values"]["x"] = json!(3);
    assert!(
        matches!(model.validate(&TraceDoc::from_json(&bad)?).validation,
        Validation::Invalid(ref failure) if failure.check == Check::Next)
    );
    let holding = op_request(
        if prime { "OpPrimeHolding" } else { "OpHolding" },
        compat_tlc,
        1,
    );
    let model = SpecModel::load(
        &holding.file,
        holding.config.as_ref().ok_or("holding config")?,
        &options,
    )?;
    let mut bad = payload;
    bad["ending"]["name"] = json!(if prime { "PrimeCorrect" } else { "Good" });
    assert!(
        matches!(model.validate(&TraceDoc::from_json(&bad)?).validation,
        Validation::Invalid(ref failure) if failure.check == Check::Ending
            && failure.detail.contains("holds at the end"))
    );
    Ok(())
}

#[test]
fn enclosing_operator_bindings_inline_in_their_actual_lexical_context()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for config in [
            "Operator",
            "OpHolding",
            "OpWrong",
            "OpPrimeHolding",
            "OpPrimeWrong",
        ] {
            let wrong = config.ends_with("Wrong");
            let prime = config.starts_with("OpPrime");
            let req = op_request(config, compat_tlc, 1);
            let expected = observed(&check_file(&req)?);
            // Pinned TLC 94d0c50 confirms the complete positive/opposite
            // counts, property, error class and exit for these inputs.
            assert_eq!(
                expected["verdict"],
                if wrong {
                    if prime {
                        "action-property-violation"
                    } else {
                        "invariant-violation"
                    }
                } else {
                    "success"
                }
            );
            assert_eq!(
                expected["code"],
                if wrong {
                    if prime { 2112 } else { 2110 }
                } else {
                    2193
                }
            );
            assert_eq!(
                expected["exit"],
                if wrong {
                    if prime { 13 } else { 12 }
                } else {
                    0
                }
            );
            assert_eq!(
                expected["property"],
                if wrong {
                    json!(if prime { "PrimeWrong" } else { "Wrong" })
                } else {
                    Value::Null
                }
            );
            assert_eq!(
                expected["statistics"]["generated"],
                if wrong { 2 } else { 4 }
            );
            assert_eq!(
                expected["statistics"]["distinct"],
                if wrong { 2 } else { 3 }
            );
            assert_eq!(expected["statistics"]["depth"], if wrong { 2 } else { 3 });
            for workers in [1, 4] {
                let req = op_request(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    agreed::check_eq(
                        AGREED,
                        "enclosing_operator_bindings_inline_in_their_actual_lexical_context/0",
                        &(observed(&core)),
                        &(expected),
                    );
                    if !wrong {
                        assert!(core.to_json()["counterexample"].is_null());
                        continue;
                    }
                    validate_op_word(&req, &core, compat_tlc, prime)?;
                }
            }
        }
    }
    Ok(())
}

#[test]
fn enclosing_callbacks_keep_capture_effect_and_recursive_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, message) in [
            (
                "OpOutsideCapture",
                "a recursive function with a dynamic INSTANCE capture outside its lexical activation (M1 core)",
            ),
            (
                "OpEffect",
                "an enclosing operator argument with effects or recursive calls inside a recursive function (M1 core)",
            ),
            (
                "OpCapturedEffect",
                "Print, PrintT, RandomElement, TLCSet or TLCGet inside a function constructor, set filter, set map or operator argument, which TLC may evaluate lazily or in its own order (M1 core)",
            ),
            (
                "OpCapturedCall",
                "a recursive definition capturing a value with unproved recursive calls (M1 core)",
            ),
            (
                "OpRecursiveCallback",
                "an enclosing operator argument with effects or recursive calls inside a recursive function (M1 core)",
            ),
            (
                "OpActionBoundary",
                "an enclosing operator parameter used as an action inside a recursive function (M1 core)",
            ),
            (
                "OpRecursiveBoundary",
                "an enclosing operator parameter inside a recursive LET definition (M1 core)",
            ),
        ] {
            for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&op_request(config, compat_tlc, 1), switches)?;
                let doc = core.to_json();
                assert_eq!(core.verdict(), "unsupported", "{config}: {doc}");
                assert_eq!(doc["message"], message, "{config}");
                assert!(doc["tlc"].is_null());
                assert!(doc["counterexample"].is_null());
                assert_eq!(doc["output"], json!([]));
            }
        }
    }
    Ok(())
}

fn mutual_request(config: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let mut req = request(config, compat_tlc, workers);
    req.file.set_file_name(if config.starts_with("Instance") {
        "InstanceOps.tla"
    } else {
        "MutualTop.tla"
    });
    req
}

fn validate_mutual_word(
    req: &CheckRequest,
    core: &dive_api::CheckReport,
    compat_tlc: bool,
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
    let config = req
        .config
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .ok_or("config")?;
    let initial = config.ends_with("Opposite");
    bad["states"][usize::from(!initial)]["values"]["x"] = json!(3);
    assert!(
        matches!(model.validate(&TraceDoc::from_json(&bad)?).validation,
        Validation::Invalid(ref failure) if failure.check == if initial { Check::Init } else { Check::Next })
    );
    let (holding, property) = if config.starts_with("InstanceSelf") {
        ("InstanceSelfHolding", "SelfGood")
    } else if config.starts_with("InstanceMutual") {
        ("InstanceMutualHolding", "MutualGood")
    } else {
        ("MutualHolding", "Good")
    };
    let holding = mutual_request(holding, compat_tlc, 1);
    let model = SpecModel::load(
        &holding.file,
        holding.config.as_ref().ok_or("holding config")?,
        &options,
    )?;
    let mut bad = payload;
    bad["ending"]["name"] = json!(property);
    assert!(
        matches!(model.validate(&TraceDoc::from_json(&bad)?).validation,
        Validation::Invalid(ref failure) if failure.check == Check::Ending
            && failure.detail.contains("holds at the end"))
    );
    Ok(())
}

#[test]
fn mutually_recursive_top_level_operators_keep_isolated_frames()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, verdict, generated, distinct, depth, code, exit) in [
            ("MutualHolding", "success", 4, 3, 3, 2193, 0),
            ("MutualWrong", "invariant-violation", 2, 2, 2, 2110, 12),
        ] {
            let expected = observed(&check_file(&mutual_request(config, compat_tlc, 1))?);
            // Pinned TLC 94d0c50 confirms both complete verdicts and counts.
            agreed::check_eq(
                AGREED,
                "mutually_recursive_top_level_operators_keep_isolated_frames/0",
                &(verdict),
                &(expected["verdict"]),
            );
            agreed::check_eq(
                AGREED,
                "mutually_recursive_top_level_operators_keep_isolated_frames/1",
                &(generated),
                &(expected["statistics"]["generated"]),
            );
            agreed::check_eq(
                AGREED,
                "mutually_recursive_top_level_operators_keep_isolated_frames/2",
                &(distinct),
                &(expected["statistics"]["distinct"]),
            );
            agreed::check_eq(
                AGREED,
                "mutually_recursive_top_level_operators_keep_isolated_frames/3",
                &(depth),
                &(expected["statistics"]["depth"]),
            );
            agreed::check_eq(
                AGREED,
                "mutually_recursive_top_level_operators_keep_isolated_frames/4",
                &(code),
                &(expected["code"]),
            );
            agreed::check_eq(
                AGREED,
                "mutually_recursive_top_level_operators_keep_isolated_frames/5",
                &(exit),
                &(expected["exit"]),
            );
            let wrong = config == "MutualWrong";
            assert_eq!(
                expected["property"],
                if wrong { json!("Wrong") } else { Value::Null }
            );
            for workers in [1, 4] {
                let req = mutual_request(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    agreed::check_eq(
                        AGREED,
                        "mutually_recursive_top_level_operators_keep_isolated_frames/6",
                        &(observed(&core)),
                        &(expected),
                    );
                    if wrong {
                        validate_mutual_word(&req, &core, compat_tlc)?;
                    } else {
                        assert!(core.to_json()["counterexample"].is_null());
                    }
                }
            }
        }
        for (config, message) in [
            (
                "MutualNested",
                "mutually recursive nested definitions (M1 core)",
            ),
            ("MutualPrimedEnabled", "a primed ENABLED (M1 core)"),
        ] {
            for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&mutual_request(config, compat_tlc, 1), switches)?;
                let doc = core.to_json();
                assert_eq!(core.verdict(), "unsupported", "{config}: {doc}");
                assert_eq!(doc["message"], message);
                assert!(doc["counterexample"].is_null());
                assert!(doc["tlc"].is_null());
            }
        }
    }
    Ok(())
}

#[test]
fn nonterminating_mutual_calls_stop_with_a_named_bound() -> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for config in ["MutualLoopBody", "MutualLoopArgument"] {
            for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&mutual_request(config, compat_tlc, 1), switches)?;
                let doc = core.to_json();
                assert_eq!(core.verdict(), "unsupported", "{config}: {doc}");
                assert_eq!(
                    doc["message"],
                    "Core recursive-call depth limit (64 activations)"
                );
                assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
                assert!(doc["counterexample"].is_null());
                assert!(doc["tlc"].is_null());
            }
        }
    }
    Ok(())
}

#[test]
fn recursive_operator_instances_keep_their_substitutions() -> Result<(), Box<dyn std::error::Error>>
{
    for compat_tlc in [false, true] {
        for (config, property, generated, distinct, depth, code) in [
            ("InstanceSelfHolding", "", 4, 3, 3, 2193),
            ("InstanceSelfOpposite", "SelfOpposite", 1, 1, 1, 2107),
            ("InstanceSelfWrong", "SelfWrong", 2, 2, 2, 2110),
            ("InstanceMutualHolding", "", 4, 3, 3, 2193),
            ("InstanceMutualOpposite", "MutualOpposite", 1, 1, 1, 2107),
            ("InstanceMutualWrong", "MutualWrong", 2, 2, 2, 2110),
            ("InstanceOperatorConstant", "", 4, 3, 3, 2193),
        ] {
            let expected = observed(&check_file(&mutual_request(config, compat_tlc, 1))?);
            // Pinned TLC 94d0c50: each Good holds, each Opposite fails
            // initially, and each Wrong first fails after one transition.
            let holds = code == 2193;
            assert_eq!(
                expected["verdict"],
                if holds {
                    "success"
                } else {
                    "invariant-violation"
                }
            );
            agreed::check_eq(
                AGREED,
                "recursive_operator_instances_keep_their_substitutions/0",
                &(generated),
                &(expected["statistics"]["generated"]),
            );
            agreed::check_eq(
                AGREED,
                "recursive_operator_instances_keep_their_substitutions/1",
                &(distinct),
                &(expected["statistics"]["distinct"]),
            );
            agreed::check_eq(
                AGREED,
                "recursive_operator_instances_keep_their_substitutions/2",
                &(depth),
                &(expected["statistics"]["depth"]),
            );
            agreed::check_eq(
                AGREED,
                "recursive_operator_instances_keep_their_substitutions/3",
                &(code),
                &(expected["code"]),
            );
            assert_eq!(expected["exit"], if holds { 0 } else { 12 });
            assert_eq!(
                expected["property"],
                if holds { Value::Null } else { json!(property) }
            );
            for workers in [1, 4] {
                let req = mutual_request(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    agreed::check_eq(
                        AGREED,
                        "recursive_operator_instances_keep_their_substitutions/4",
                        &(observed(&core)),
                        &(expected),
                    );
                    if holds {
                        assert!(core.to_json()["counterexample"].is_null());
                    } else {
                        validate_mutual_word(&req, &core, compat_tlc)?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn recursive_operator_instances_keep_dynamic_capture_and_effect_refusals()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, message) in [
            (
                "InstanceOperatorDynamic",
                "a recursive function with a dynamic INSTANCE capture outside its lexical activation (M1 core)",
            ),
            (
                "InstanceOperatorEffect",
                "an effectful or recursive INSTANCE substitution inside a recursive function (M1 core)",
            ),
        ] {
            for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&mutual_request(config, compat_tlc, 1), switches)?;
                let doc = core.to_json();
                assert_eq!(core.verdict(), "unsupported", "{config}: {doc}");
                assert_eq!(doc["message"], message);
                assert!(doc["counterexample"].is_null());
                assert!(doc["tlc"].is_null());
                assert_eq!(doc["output"], json!([]));
            }
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
            let reference = check_file(&oracle_req)?;
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
        let reference = check_file(&req)?;
        assert_eq!(reference.verdict(), "success");
        for switches in ["", "-fold,-memo,-hashcons,-eager"] {
            let core = check_file_core_opts(&req, switches)?;
            agreed::check_eq(
                AGREED,
                "parameterized_instance_substitutions_never_reuse_caller_frame_slots/0",
                &(observed(&core)),
                &(observed(&reference)),
            );
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
            let reference = check_file(&req)?;
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
            // D-029: Core counts emitted successors, two where pinned TLC
            // generates three for the positive configuration. Every other
            // observable here is TLC's.
            assert_eq!(
                expected["statistics"]["generated"],
                if positive { 2 } else { 1 }
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
                    let native = observed(&core);
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
            let reference = check_file(&primed_request(config, compat_tlc, 1))?;
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
            let reference = check_file(&primed_request(config, compat_tlc, 1))?;
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
                "PrimedCapturedInside",
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

#[test]
fn recursive_action_predicates_refuse_direct_and_indirect_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/predicate-witness-counts/specifications/ActionCount");
    for compat_tlc in [false, true] {
        for name in ["RegisterWitness", "IndirectEffectCall"] {
            for workers in [1, 4] {
                let mut req = request(name, compat_tlc, workers);
                req.file.set_file_name(format!("{name}.tla"));
                if name == "RegisterWitness" {
                    req.file = corpus.join("RegisterWitness.tla");
                    req.config = Some(corpus.join("RegisterWitness.cfg"));
                }
                for switches in ["", "-fold,-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    let doc = core.to_json();
                    assert_eq!(doc["verdict"], "unsupported", "{name}: {doc}");
                    assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
                    assert_eq!(
                        doc["message"],
                        "recursive Boolean action predicates in a compiled plan with effects or unproved calls"
                    );
                    assert!(doc["counterexample"].is_null());
                    assert!(doc["tlc"].is_null());
                }
            }
        }
    }
    Ok(())
}

#[test]
fn recursive_boolean_or_does_not_demand_a_proven_unnecessary_error()
-> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/predicate-witness-counts/specifications/ActionCount");
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            let req = CheckRequest {
                file: root.join("OrError.tla"),
                config: Some(root.join("OrError.cfg")),
                workers: Some(workers),
                compat_tlc,
                seed: Some(0),
                fp: Some(0),
                ..Default::default()
            };
            for switches in ["", "-fold,-memo,-hashcons,-eager"] {
                let core = check_file_core_opts(&req, switches)?;
                let doc = core.to_json();
                assert_eq!(doc["verdict"], "success");
                assert_eq!(core.exit_code_for(ExitConvention::Tlc), 0);
                assert_eq!(doc["statistics"]["generated"], 2);
                assert_eq!(doc["statistics"]["distinct"], 1);
                assert_eq!(doc["statistics"]["depth"], 1);
                assert!(doc["property"].is_null());
                let model = SpecModel::load(
                    &req.file,
                    req.config.as_ref().ok_or("config")?,
                    &TraceOptions {
                        compat_tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    },
                )?;
                validate_action_predicate_word(&req, &model, true, &doc)?;
            }
        }
    }
    Ok(())
}

/// The request for a next-state variant fixture, with the holding
/// configuration and property its wrong variant is checked against.
fn next_state_variant(config: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    if config.starts_with("Mutual") {
        mutual_request(config, compat_tlc, workers)
    } else if config.starts_with("Op") {
        op_request(config, compat_tlc, workers)
    } else {
        primed_request(config, compat_tlc, workers)
    }
}

#[test]
fn recursive_definitions_under_a_prime_evaluate_in_the_next_state()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        // Pinned TLC 2026.10.06.014338 confirms each verdict and count. The
        // holding variants read the recursive definition (a top-level
        // mutually recursive operator, a LET function capturing an operator
        // parameter, one capturing an operator argument) primed; the wrong
        // ones compare it with the current state.
        for (config, holding, property, generated, distinct, depth) in [
            ("MutualPrime", "", "", 4, 3, 3),
            ("MutualPrimeWrong", "MutualPrime", "PrimeCorrect", 2, 2, 2),
            ("PrimedCaptured", "", "", 7, 3, 2),
            ("PrimedCapturedHolding", "", "", 7, 3, 2),
            (
                "PrimedCapturedWrong",
                "PrimedCapturedHolding",
                "CapturedCorrect",
                2,
                2,
                2,
            ),
            ("OpPrimeCapture", "", "", 4, 3, 3),
            ("OpPrimeCaptureHolding", "", "", 4, 3, 3),
            (
                "OpPrimeCaptureWrong",
                "OpPrimeCaptureHolding",
                "PrimeCaptureCorrect",
                2,
                2,
                2,
            ),
        ] {
            let wrong = config.ends_with("Wrong");
            let reference = check_file(&next_state_variant(config, compat_tlc, 1))?;
            let expected = observed(&reference);
            assert_eq!(
                expected["verdict"],
                if wrong {
                    "action-property-violation"
                } else {
                    "success"
                },
                "{config}"
            );
            assert_eq!(expected["code"], if wrong { 2112 } else { 2193 });
            assert_eq!(expected["exit"], if wrong { 13 } else { 0 });
            assert_eq!(expected["statistics"]["generated"], generated);
            assert_eq!(expected["statistics"]["distinct"], distinct);
            assert_eq!(expected["statistics"]["depth"], depth);
            for workers in [1, 4] {
                let req = next_state_variant(config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{config}/{compat_tlc}/{workers}/{switches}"
                    );
                    if !wrong {
                        assert!(core.to_json()["counterexample"].is_null());
                        continue;
                    }
                    let options = TraceOptions {
                        compat_tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    };
                    let model =
                        SpecModel::load(&req.file, req.config.as_ref().ok_or("config")?, &options)?;
                    let payload = core.to_json()["counterexample"]["trace"].clone();
                    assert_eq!(
                        model.validate(&TraceDoc::from_json(&payload)?).validation,
                        Validation::Valid
                    );
                    let mut bad = payload.clone();
                    bad["states"][1]["values"]["x"] = json!(3);
                    assert!(matches!(
                        model.validate(&TraceDoc::from_json(&bad)?).validation,
                        Validation::Invalid(ref failure) if failure.check == Check::Next
                    ));
                    let holding = next_state_variant(holding, compat_tlc, 1);
                    let holding_model = SpecModel::load(
                        &holding.file,
                        holding.config.as_ref().ok_or("holding config")?,
                        &options,
                    )?;
                    let mut bad = payload;
                    bad["ending"]["name"] = json!(property);
                    assert!(matches!(
                        holding_model.validate(&TraceDoc::from_json(&bad)?).validation,
                        Validation::Invalid(ref failure) if failure.check == Check::Ending
                            && failure.detail.contains("holds")
                    ));
                }
            }
        }
    }
    Ok(())
}

fn next_read_request(file: &str, config: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-let-recursive");
    CheckRequest {
        file: root.join(format!("{file}.tla")),
        config: Some(root.join(format!("{config}.cfg"))),
        workers: Some(workers),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    }
}

#[test]
fn recursive_calls_reading_the_next_state_are_not_shared_across_disjuncts()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        // An operator argument whose recursive callee reads the state being
        // built (through a primed recursive call, a direct prime, a
        // recursive function, a primed INSTANCE substitution, or a state
        // read in an initial predicate) is evaluated in each disjunct's
        // state, not shared between them. Pinned TLC 2026.10.06.014338
        // confirms each count.
        for (file, config, generated, distinct, depth) in [
            ("NextReadCalls", "NextReadPrimed", 5, 2, 2),
            ("NextReadCalls", "NextReadDirect", 5, 2, 2),
            ("NextReadCalls", "NextReadFcn", 5, 2, 2),
            ("NextReadCalls", "NextReadInit", 4, 2, 1),
            ("NextReadSub", "NextReadSub", 5, 2, 2),
        ] {
            let reference = check_file(&next_read_request(file, config, compat_tlc, 1))?;
            let expected = observed(&reference);
            assert_eq!(expected["verdict"], "success", "{config}");
            assert_eq!(expected["statistics"]["generated"], generated);
            assert_eq!(expected["statistics"]["distinct"], distinct);
            assert_eq!(expected["statistics"]["depth"], depth);
            for workers in [1, 4] {
                let req = next_read_request(file, config, compat_tlc, workers);
                for switches in ["", "-fold,-memo,-step-memo,-hashcons,-eager"] {
                    let core = check_file_core_opts(&req, switches)?;
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        observed(&core),
                        expected,
                        "{config}/{compat_tlc}/{workers}/{switches}"
                    );
                }
            }
        }
    }
    Ok(())
}
