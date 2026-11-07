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

#[test]
fn actual_next_function_receiver_requires_its_first_source_diagnostic_lifetime() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["EmptyLazyQuery", "DefinedFunction"] {
        let file = root.join(format!(
            "crates/dive-api/tests/fixtures/next-receiver-lifetime/{name}.tla"
        ));
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
                    let report = check_file_engine(
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
                    )?;
                    let value = report.to_json();
                    assert_eq!(value["tlc"]["code"], 2147, "{value}");
                    assert_eq!(value["tlc"]["reported"], 2179);
                    assert_eq!(value["tlc"]["verified"], true);
                    assert_eq!(report.exit_code_for(dive_api::ExitConvention::Tlc), 75);
                    for (field, expected) in [
                        ("distinct", 1),
                        ("generated", 1),
                        ("depth", 1),
                        ("left_on_queue", 0),
                    ] {
                        assert_eq!(value["statistics"][field], expected, "{value}");
                    }
                    assert!(value["diagnostic_witness"].is_null());
                    assert!(value["error_witness"].is_null());
                    let wire = &value["counterexample"]["trace"];
                    assert_eq!(wire["version"], 1);
                    assert_eq!(wire["states"].as_array().ok_or("states")?.len(), 1);
                    assert_eq!(wire["ending"]["endpoint"]["stage"], "next-ordered-terminal");
                    assert_eq!(wire["ending"]["ec"], "TLC_MODULE_DIVISION_BY_ZERO");
                    let doc = TraceDoc::from_json(wire)?;
                    assert_eq!(
                        model.try_validate(&doc)?.validation,
                        Validation::Valid,
                        "{value}"
                    );
                    for (pointer, replacement) in [
                        ("/ending/ec", serde_json::json!("TLC_MODULE_OVERFLOW")),
                        (
                            "/ending/endpoint/source/module",
                            serde_json::json!("Forged"),
                        ),
                        ("/ending/endpoint/source/start", serde_json::json!(0)),
                        ("/ending/endpoint/reported", serde_json::json!(1000)),
                        ("/ending/endpoint/deciding", serde_json::json!(2103)),
                        ("/states/0/values/step", serde_json::json!(1)),
                    ] {
                        let mut bad = wire.clone();
                        *bad.pointer_mut(pointer).ok_or("mutation field")? = replacement;
                        assert_ne!(
                            model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
                            Validation::Valid,
                            "{bad}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn effectful_later_and_multiple_lifetimes_gain_no_receiver_certificate() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "MultipleInitial",
        "LaterReceiver",
        "EffectBeforeReceiver",
        "CandidateBeforeReceiver",
    ] {
        let file = root.join(format!(
            "crates/dive-api/tests/fixtures/next-receiver-lifetime/{name}.tla"
        ));
        for compat_tlc in [false, true] {
            let source = SpecModel::load(
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
                    let report = check_file_engine(
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
                    )?;
                    let value = report.to_json();
                    assert_ne!(
                        value["tlc"]["code"], 2147,
                        "{name}/{engine:?}/{workers}: {value}"
                    );
                    if value["verdict"] == "unsupported" {
                        continue;
                    }
                    assert_eq!(value["tlc"]["code"], 2179, "{name}: {value}");
                    assert_eq!(value["tlc"]["verified"], false, "{name}: {value}");
                    assert_eq!(report.exit_code_for(dive_api::ExitConvention::Tlc), 255);
                    if name == "EffectBeforeReceiver" {
                        assert!(report.render_human().contains("actual-before-receiver"));
                    }
                    let mut forged = value["counterexample"]["trace"].clone();
                    let endpoint = &mut forged["ending"]["endpoint"];
                    endpoint["stage"] = serde_json::json!("next-ordered-terminal");
                    endpoint["reported"] = serde_json::json!(2179);
                    endpoint["deciding"] = serde_json::json!(2147);
                    assert_ne!(
                        source
                            .try_validate(&TraceDoc::from_json(&forged)?)?
                            .validation,
                        Validation::Valid,
                        "{name}: {forged}"
                    );
                }
            }
        }
    }
    Ok(())
}
