//! A partial terminal binding is never a fabricated complete state.
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/partial_terminal_words.json");

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn partial_next_words_preserve_the_pinned_bfs_terminal_and_logical_length() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in [
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextIncomplete/ErNextIncomplete.tla",
        "crates/dive-api/tests/fixtures/checker/dfid/DfidIncomplete.tla",
        "crates/dive-api/tests/fixtures/checker/dfid/DfidActionPrecedence.tla",
        "crates/dive-api/tests/fixtures/checker/dfid/DfidNextPrecedence.tla",
        "crates/dive-eval/tests/fixtures/next/specs/NxUnassigned/NxUnassigned.tla",
        "crates/dive-model/tests/fixtures/action-inventory/DirectTrue.tla",
        "crates/dive-model/tests/fixtures/action-inventory/SharedTrue.tla",
        "crates/dive-sim/tests/fixtures/SimIncomplete.tla",
        "crates/dive-model/tests/fixtures/specs/RoActions/RoActions.tla",
        "crates/dive-ir/tests/fixtures/actions/specs/SplitThrows/SplitThrows.tla",
    ] {
        let file = root.join(path);
        for compat_tlc in [false, true] {
            let model = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            {
                let result = check_file(&CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    compat_tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                })?
                .to_json();
                assert_eq!(result["tlc"]["code"], 2109, "{path}: {result}");
                assert_eq!(result["tlc"]["exit_status"], 75, "{path}: {result}");
                let wire = &result["counterexample"]["trace"];
                assert_eq!(wire["version"], 3, "{path}: {result}");
                let doc = TraceDoc::from_json(wire)?;
                assert_eq!(
                    result["counterexample"]["length"].as_u64(),
                    Some(u64::try_from(doc.logical_length())?)
                );
                assert_eq!(doc.logical_length(), doc.states.len() + 1);
                assert!(matches!(
                    model.validate(&doc).validation,
                    Validation::Unsupported(_)
                ));
                let validation = model.try_validate(&doc)?;
                assert_eq!(validation.length, doc.logical_length());
                assert_eq!(
                    validation.validation,
                    Validation::Valid,
                    "{path}: {validation:?}"
                );
                if path.contains("ErNextIncomplete") {
                    assert_eq!(doc.states.len(), 3);
                    assert_eq!(
                        wire["ending"]["endpoint"]["assignments"],
                        serde_json::json!({"x":3})
                    );
                    assert_eq!(
                        wire["ending"]["endpoint"]["unassigned"],
                        serde_json::json!(["y"])
                    );
                    for (name, value) in [("generated", 8), ("distinct", 5), ("depth", 3)] {
                        assert_eq!(result["statistics"][name], value);
                    }
                }
                agreed::check(AGREED, "cross/0", &(wire));
            }
        }
    }
    Ok(())
}

#[test]
fn complete_prefix_must_pass_configured_safety_before_partial_replay() -> TestResult {
    use dive_api::trace::Check;
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(
            "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextIncomplete/ErNextIncomplete.tla",
        );
    let original = std::fs::read_to_string(&file)?;
    let additions =
        "\nBadInit == x#0\nActionProperty == [][x'=x]_<<x,y>>\nBadInvariant == 1 \\div x=0\n";
    let marker = original.rfind("====").ok_or("module end")?;
    let source = format!(
        "{}{}{}",
        &original[..marker],
        additions,
        &original[marker..]
    );
    for compat_tlc in [false, true] {
        let actual = check_file(&CheckRequest {
            file: file.clone(),
            config: Some(file.with_extension("cfg")),
            compat_tlc,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        })?
        .to_json();
        let doc = TraceDoc::from_json(&actual["counterexample"]["trace"])?;
        for (config, source, expected, state) in [
            ("INVARIANT Init", original.as_str(), Check::Invariant, 1),
            ("CONSTRAINT Init", original.as_str(), Check::Constraint, 1),
            (
                "ACTION_CONSTRAINT Init",
                original.as_str(),
                Check::ActionConstraint,
                2,
            ),
            ("PROPERTY BadInit", source.as_str(), Check::ImpliedInit, 0),
            (
                "PROPERTY ActionProperty",
                source.as_str(),
                Check::ImpliedAction,
                1,
            ),
            (
                "INVARIANT BadInvariant",
                source.as_str(),
                Check::Invariant,
                0,
            ),
            // Constraint failure must retain priority over the invariant.
            (
                "CONSTRAINT Init\nINVARIANT Init",
                original.as_str(),
                Check::Constraint,
                1,
            ),
        ] {
            let model = SpecModel::from_source(
                "ErNextIncomplete.tla",
                source,
                &format!("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n{config}\n"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            let report = model.try_validate(&doc)?;
            let Validation::Invalid(failure) = report.validation else {
                return Err(format!("{config}: {report:?}").into());
            };
            assert_eq!(failure.check, expected, "{config}: {failure:?}");
            assert_eq!(failure.state, state, "{config}: {failure:?}");
            assert_eq!(report.length, 4);
            // The original public Behavior contract remains plain membership.
            let mut behavior = doc.clone().with_ending(dive_api::trace::Ending::Behavior);
            behavior.length = Some(behavior.states.len());
            assert_eq!(model.try_validate(&behavior)?.validation, Validation::Valid);
        }
    }
    Ok(())
}

#[test]
fn init_partial_error_witness_preserves_the_absent_formal_counterexample() -> TestResult {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(
            "crates/dive-api/tests/fixtures/checker/errors/specs/ErInitIncomplete/ErInitIncomplete.tla",
        );
    for compat_tlc in [false, true] {
        let spec = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )?;
        {
            let actual = check_file(&CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })?
            .to_json();
            assert_eq!(actual["tlc"]["code"], 2102);
            assert_eq!(actual["tlc"]["exit_status"], 255);
            assert!(actual["counterexample"].is_null());
            assert_eq!(actual["statistics"]["reported_by_tlc"], false);
            let wire = &actual["error_witness"];
            assert_eq!(wire["version"], 3, "{actual}");
            assert_eq!(wire["length"], 1);
            assert_eq!(wire["states"], serde_json::json!([]));
            assert_eq!(wire["ending"]["endpoint"]["stage"], "init-partial-state");
            assert_eq!(
                wire["ending"]["endpoint"]["assignments"],
                serde_json::json!({"x":0})
            );
            assert_eq!(
                wire["ending"]["endpoint"]["unassigned"],
                serde_json::json!(["y"])
            );
            let doc = TraceDoc::from_json(&actual)?;
            let report = spec.try_validate(&doc)?;
            assert_eq!(report.validation, Validation::Valid);
            assert_eq!(report.length, 1);
            assert!(matches!(
                spec.validate(&doc).validation,
                Validation::Unsupported(_)
            ));
            for (pointer, value) in [
                ("/ending/endpoint/assignments/x", serde_json::json!(1)),
                ("/ending/endpoint/source/module", serde_json::json!("Wrong")),
                ("/length", serde_json::json!(0)),
                ("/variables", serde_json::json!([])),
                (
                    "/states",
                    serde_json::json!([{"action":null,"values":{"x":0,"y":0}}]),
                ),
            ] {
                let mut bad = wire.clone();
                *bad.pointer_mut(pointer).ok_or("field")? = value;
                assert!(
                    matches!(
                        spec.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
                        Validation::Invalid(_)
                    ),
                    "{bad}"
                );
            }
            for (pointer, value) in [
                ("/version", serde_json::json!(2)),
                (
                    "/ending/ec",
                    serde_json::json!("TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT"),
                ),
                (
                    "/ending/endpoint/stage",
                    serde_json::json!("next-partial-state"),
                ),
            ] {
                let mut bad = wire.clone();
                *bad.pointer_mut(pointer).ok_or("field")? = value;
                assert!(TraceDoc::from_json(&bad).is_err());
            }
            for field in ["tlc", "statistics", "counterexample", "error_witness"] {
                agreed::check(AGREED, "cross/1", &(actual[field]));
            }
        }
    }
    Ok(())
}

#[test]
fn initial_candidate_level_proves_earlier_property_priority() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal-level");
    let good = root.join("one/InitLevel.tla");
    let bad = root.join("zero/InitLevel.tla");
    for compat_tlc in [false, true] {
        let options = TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        };
        let correct = SpecModel::load(&good, &good.with_extension("cfg"), &options)?;
        let forged = SpecModel::load(&bad, &bad.with_extension("cfg"), &options)?;
        {
            let request = |file: &PathBuf| CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let genuine = check_file(&request(&good))?.to_json();
            assert_eq!(genuine["tlc"]["code"], 2102);
            assert_eq!(genuine["tlc"]["exit_status"], 255);
            assert!(genuine["counterexample"].is_null());
            let witness = TraceDoc::from_json(&genuine["error_witness"])?;
            let earlier = check_file(&request(&bad))?.to_json();
            assert_eq!(earlier["tlc"]["code"], 2108);
            assert_eq!(earlier["tlc"]["exit_status"], 13);
            assert_eq!(earlier["property"], "P");
            assert!(earlier["error_witness"].is_null());
            assert!(matches!(forged.try_validate(&witness)?.validation,
                Validation::Invalid(ref failure) if failure.check == dive_api::trace::Check::ImpliedInit));
            assert_eq!(
                correct.try_validate(&witness)?.validation,
                Validation::Valid
            );
        }
    }
    Ok(())
}
