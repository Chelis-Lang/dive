//! Consumed pinned option controls: output bytes and both checksum authorities.
use dive_pluscal::{Code, FairnessOption, TranslationOptions, algorithm_checksum_at, translate_at};
use serde_json::Value;

fn options(args: &Value) -> Result<TranslationOptions, Box<dyn std::error::Error>> {
    let mut options = TranslationOptions::default();
    let mut args = args.as_array().ok_or("args")?.iter();
    while let Some(arg) = args.next() {
        match arg.as_str().ok_or("flag")? {
            "-termination" => options.check_termination = true,
            "-noDoneDisjunct" => options.no_done_disjunct = true,
            "-nof" => options.fairness = FairnessOption::None,
            "-sf" => options.fairness = FairnessOption::Strong,
            "-wfNext" => options.fairness = FairnessOption::Next,
            "-label" => options.insert_missing_labels = true,
            "-reportLabels" => options.report_labels = true,
            "-labelRoot" => {
                options.label_root = args.next().and_then(Value::as_str).ok_or("root")?.into();
            }
            "-lineWidth" => {
                options.line_width = args
                    .next()
                    .and_then(Value::as_str)
                    .ok_or("width")?
                    .parse()?;
            }
            "-version" => {
                options.version =
                    Some(args.next().and_then(Value::as_str).ok_or("version")?.into());
            }
            _ => return Err("fixture flag unconsumed".into()),
        }
    }
    Ok(options)
}

#[test]
fn primary_option_bodies_and_checksums() -> Result<(), Box<dyn std::error::Error>> {
    let primary: Value =
        serde_json::from_str(include_str!("fixtures/remaining-options/primary.json"))?;
    let rows = primary["rows"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), 28);
    for row in rows {
        let id = row["id"].as_str().ok_or("id")?;
        let source = row["source"].as_str().ok_or("source")?;
        let start = source.find("--algorithm").ok_or("algorithm")?;
        let end = source.rfind("*)").ok_or("comment")?;
        let options = options(&row["args"])?;
        let native = translate_at(source, start..end, &options)?;
        assert_eq!(native.tla, row["body"].as_str().ok_or("body")?, "{id}");
        assert_eq!(
            dive_pluscal::rewrite::translation_checksum(&native.tla),
            row["body_crc"],
            "{id}"
        );
        let checksum = algorithm_checksum_at(source, start..end, &options)?;
        assert_eq!(checksum.checksum, row["algorithm_crc"], "{id}");
        assert_eq!(
            serde_json::to_value(&native.option_context.effective)?,
            serde_json::to_value(
                &dive_pluscal::resolve_translation_options(source, &options)?.effective
            )?,
            "{id}"
        );
        assert_eq!(
            native.configuration.contains("PROPERTY Termination"),
            row["termination_required"].as_bool().ok_or("termination")?,
            "{id}"
        );
        if options.report_labels {
            let observed: Vec<_> = native.inserted_labels.iter().map(|label| serde_json::json!({"name":label.name,"line":label.position.line,"column":label.position.column})).collect();
            assert_eq!(serde_json::to_value(observed)?, row["labels"], "{id}");
        }
    }
    Ok(())
}

#[test]
fn unsupported_options_are_named_and_atomic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin L:skip; end algorithm";
    // Retain the predecessor debug input as supported, with exact source locations.
    let debug = TranslationOptions {
        debug: true,
        ..TranslationOptions::default()
    };
    let translated = translate_at(source, 0..source.len(), &debug)?;
    assert!(translated.option_context.effective.debug);
    let header = algorithm_checksum_at(source, 0..source.len(), &debug)?;
    assert!(header.serialized_ast.starts_with("(*0, 0*)"));
    assert!(header.serialized_ast.contains("[type |-> \"skip\"]"));
    for (flag, options) in [
        (
            "writeAST",
            TranslationOptions {
                write_ast: true,
                ..TranslationOptions::default()
            },
        ),
        (
            "myspec",
            TranslationOptions {
                specification_backend: Some(("myspec".into(), "LocalSpec".into())),
                ..TranslationOptions::default()
            },
        ),
    ] {
        let error = translate_at(source, 0..source.len(), &options)
            .err()
            .ok_or("must refuse")?;
        assert_eq!(error.diagnostic.code, Code::Unsupported);
        assert!(error.diagnostic.message.contains(flag));
        assert!(algorithm_checksum_at(source, 0..source.len(), &options).is_err());
    }
    for source in [
        "PlusCal options (lineWidth 60)",
        "PlusCal options (-reportLabels)",
        "PlusCal options (-unixEOL)",
        "PlusCal options (nocfg)",
        "PlusCal options (-spec2 S)",
    ] {
        assert!(
            dive_pluscal::resolve_translation_options(source, &TranslationOptions::default())
                .is_err(),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn version_and_width_boundaries_remain_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin L:skip; end algorithm";
    for version in ["abc", "1.904", "2.0", "1.1234"] {
        let options = TranslationOptions {
            version: Some(version.into()),
            ..TranslationOptions::default()
        };
        assert!(
            translate_at(source, 0..source.len(), &options).is_err(),
            "{version}"
        );
    }
    let options = TranslationOptions {
        version: Some("1.4".into()),
        ..TranslationOptions::default()
    };
    let processes = "--algorithm A process P=1 begin L:skip; end process; end algorithm";
    let error = translate_at(processes, 0..processes.len(), &options)
        .err()
        .ok_or("unmeasured old process AST")?;
    assert_eq!(error.diagnostic.code, Code::Unsupported);
    assert!(
        error
            .diagnostic
            .message
            .contains("pre-1.5 process/procedure")
    );
    let options = TranslationOptions {
        line_width: i32::MAX,
        ..TranslationOptions::default()
    };
    let artifact = translate_at(source, 0..source.len(), &options)?;
    assert!(artifact.tla.len() < 1024);
    let tiny = TranslationOptions {
        max_output_bytes: artifact.tla.len() - 1,
        ..options
    };
    assert_eq!(
        translate_at(source, 0..source.len(), &tiny)
            .err()
            .ok_or("budget")?
            .diagnostic
            .code,
        Code::Resource
    );
    Ok(())
}

#[test]
fn version_15_process_fairness_is_not_sequential_legacy_fair()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; fair + process P=1 begin L:skip; E:skip; end process; end algorithm";
    let options = TranslationOptions {
        version: Some("1.5".into()),
        ..TranslationOptions::default()
    };
    let artifact = translate_at(source, 0..source.len(), &options)?;
    assert!(artifact.tla.contains("SF_vars"));
    assert!(!artifact.tla.contains("WF_vars(Next)"));
    let parser = dive_pluscal::Options {
        input_version: 1500,
        ..dive_pluscal::Options::default()
    };
    assert!(!dive_pluscal::parse(source, &parser)?.algorithm.legacy_fair);
    Ok(())
}
