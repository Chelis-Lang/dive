//! A partial terminal binding is never a fabricated complete state.

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn partial_next_words_preserve_the_pinned_bfs_terminal_and_logical_length() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in [
        "crates/dive-check/tests/fixtures/errors/specs/ErNextIncomplete/ErNextIncomplete.tla",
        "crates/dive-check/tests/fixtures/dfid/DfidIncomplete.tla",
        "crates/dive-check/tests/fixtures/dfid/DfidActionPrecedence.tla",
        "crates/dive-check/tests/fixtures/dfid/DfidNextPrecedence.tla",
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
            let mut before = None;
            for engine in [Engine::Reference, Engine::Core] {
                let result = check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc,
                        workers: Some(1),
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    },
                    engine,
                )?
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
                if path.contains("RoActions") || path.contains("SplitThrows") {
                    assert!(
                        matches!(validation.validation, Validation::Unsupported(_)),
                        "{path}: {validation:?}"
                    );
                } else {
                    assert_eq!(
                        validation.validation,
                        Validation::Valid,
                        "{path}: {validation:?}"
                    );
                }
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
                if let Some(reference) = &before {
                    assert_eq!(wire, reference, "{path}");
                } else {
                    before = Some(wire.clone());
                }
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
            "crates/dive-check/tests/fixtures/errors/specs/ErNextIncomplete/ErNextIncomplete.tla",
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
        let actual = check_file_engine(
            &CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            },
            Engine::Core,
        )?
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
