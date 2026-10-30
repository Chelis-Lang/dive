//! Ordered Next replay keeps primary facts and proves its diagnostic context.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn native_ordered_next_families_reproduce_actual_source_and_diagnostic_error() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = [
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterChoose/ErNextAfterChoose.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterDomain/ErNextAfterDomain.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterNonBool/ErNextAfterNonBool.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterNonBoolIf/ErNextAfterNonBoolIf.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterNonEnum/ErNextAfterNonEnum.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterTypeErr/ErNextAfterTypeErr.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextChoose/ErNextChoose.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextDomain/ErNextDomain.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextNonBool/ErNextNonBool.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextNonBoolIf/ErNextNonBoolIf.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextNonEnum/ErNextNonEnum.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsChoose/ErNextRhsChoose.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsDomain/ErNextRhsDomain.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsNonBool/ErNextRhsNonBool.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsNonBoolIf/ErNextRhsNonBoolIf.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsNonEnum/ErNextRhsNonEnum.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsTypeErr/ErNextRhsTypeErr.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextTypeErr/ErNextTypeErr.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextUnassigned/ErNextUnassigned.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErWrapFcnGuard/ErWrapFcnGuard.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErWrapFcnRhs/ErWrapFcnRhs.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErWrapFilter/ErWrapFilter.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErWrapLambdaBody/ErWrapLambdaBody.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErWrapLambdaDomain/ErWrapLambdaDomain.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErWrapRecursive/ErWrapRecursive.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextAfterAssert/ErNextAfterAssert.tla",
        "crates/dive-check/tests/fixtures/errors/specs/ErNextRhsAssert/ErNextRhsAssert.tla",
        "crates/dive-api/tests/fixtures/core-lib/specs/ErAssertNext/ErAssertNext.tla",
    ];
    for relative in cases {
        let file = root.join(relative);
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
            let mut reference: Option<serde_json::Value> = None;
            for engine in [Engine::Reference, Engine::Core] {
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
                    engine,
                )?
                .to_json();
                assert_eq!(actual["error_site"], "next-state", "{relative}: {actual}");
                let wire = &actual["counterexample"]["trace"];
                assert_eq!(
                    wire["ending"]["endpoint"]["stage"], "next-ordered-terminal",
                    "{relative}: {actual}"
                );
                let doc = TraceDoc::from_json(wire)?;
                let validation = spec.try_validate(&doc)?;
                assert_eq!(
                    validation.validation,
                    Validation::Valid,
                    "{relative} {engine:?} {compat_tlc}: {validation:?}"
                );
                if let Some(reference) = &reference {
                    for field in ["tlc", "statistics", "counterexample"] {
                        assert_eq!(actual[field], reference[field], "{relative}: {field}");
                    }
                } else {
                    reference = Some(actual);
                }
            }
        }
    }
    Ok(())
}
