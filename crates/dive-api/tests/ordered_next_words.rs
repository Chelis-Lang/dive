//! Ordered Next replay keeps primary facts and proves its diagnostic context.
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/ordered_next_words.json");
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn native_ordered_next_families_reproduce_actual_source_and_diagnostic_error() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = [
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterChoose/ErNextAfterChoose.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterDomain/ErNextAfterDomain.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterNonBool/ErNextAfterNonBool.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterNonBoolIf/ErNextAfterNonBoolIf.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterNonEnum/ErNextAfterNonEnum.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterTypeErr/ErNextAfterTypeErr.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextChoose/ErNextChoose.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextDomain/ErNextDomain.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextNonBool/ErNextNonBool.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextNonBoolIf/ErNextNonBoolIf.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextNonEnum/ErNextNonEnum.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsChoose/ErNextRhsChoose.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsDomain/ErNextRhsDomain.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsNonBool/ErNextRhsNonBool.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsNonBoolIf/ErNextRhsNonBoolIf.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsNonEnum/ErNextRhsNonEnum.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsTypeErr/ErNextRhsTypeErr.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextTypeErr/ErNextTypeErr.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextUnassigned/ErNextUnassigned.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErWrapFcnGuard/ErWrapFcnGuard.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErWrapFcnRhs/ErWrapFcnRhs.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErWrapFilter/ErWrapFilter.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErWrapLambdaBody/ErWrapLambdaBody.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErWrapLambdaDomain/ErWrapLambdaDomain.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErWrapRecursive/ErWrapRecursive.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextAfterAssert/ErNextAfterAssert.tla",
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsAssert/ErNextRhsAssert.tla",
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
            {
                let engine = Engine::Core;
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
                for field in ["tlc", "statistics", "counterexample"] {
                    agreed::check(AGREED, "cross/0", &(actual[field]));
                }
            }
        }
    }
    Ok(())
}
