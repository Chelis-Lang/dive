//! Source-bound terminal Next errors use the existing optional endpoint wire.
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn real_next_errors_retain_source_cause_and_independently_validate() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = [
        (
            "EnabledIntervalFirstError",
            "conformance/corpus/interval-streaming/specifications/EnabledIntervalFirstError/EnabledIntervalFirstError.tla",
            2179,
            255,
            1,
            1,
            1,
            "TLC_MODULE_DIVISION_BY_ZERO",
        ),
        (
            "ErWrapFcnGuard",
            "crates/dive-check/tests/fixtures/errors/specs/ErWrapFcnGuard/ErWrapFcnGuard.tla",
            2103,
            75,
            5,
            7,
            3,
            "GENERAL",
        ),
    ];
    for (name, relative, code, exit, distinct, generated, depth, ec) in cases {
        let file = root.join(relative);
        let source = std::fs::read_to_string(&file)?;
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
            for engine in [Engine::Reference, Engine::Core] {
                for workers in [1, 4] {
                    let result = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            workers: Some(workers),
                            seed: Some(0),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?
                    .to_json();
                    assert_eq!(result["error_site"], "next-state", "{result}");
                    assert_eq!(result["tlc"]["code"], code);
                    assert_eq!(result["tlc"]["exit_status"], exit);
                    if workers == 1 {
                        for (field, expected) in [
                            ("distinct", distinct),
                            ("generated", generated),
                            ("depth", depth),
                        ] {
                            assert_eq!(result["statistics"][field], expected, "{result}");
                        }
                    }
                    let wire = &result["counterexample"]["trace"];
                    assert_eq!(wire["version"], 1);
                    assert_eq!(wire["ending"]["kind"], "error");
                    assert_eq!(wire["ending"]["endpoint"]["source"]["module"], name);
                    let start = wire["ending"]["endpoint"]["source"]["start"]
                        .as_u64()
                        .ok_or("start")?;
                    let end = wire["ending"]["endpoint"]["source"]["end"]
                        .as_u64()
                        .ok_or("end")?;
                    let shown = source
                        .get(usize::try_from(start)?..usize::try_from(end)?)
                        .ok_or("source range")?;
                    assert_ne!(shown, "");
                    let doc = TraceDoc::from_json(wire)?;
                    assert!(
                        matches!(&doc.ending, Some(Ending::NextStateError { ec: actual,.. }) if actual==ec)
                    );
                    assert!(matches!(
                        model.validate(&doc).validation,
                        Validation::Unsupported(_)
                    ));
                    assert_eq!(
                        model.try_validate(&doc)?.validation,
                        Validation::Valid,
                        "{name} {engine:?} {compat_tlc} {workers}: {result}"
                    );
                }
            }
        }
    }
    Ok(())
}
