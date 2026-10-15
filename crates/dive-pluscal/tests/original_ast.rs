//! Exact retained original-stage artifacts and debug header checksums.
use dive_pluscal::{Code, TranslationOptions, algorithm_checksum_at, original_ast_at};
use serde::Deserialize;

#[derive(Deserialize)]
struct Controls {
    controls: Vec<Control>,
}
#[derive(Deserialize)]
struct Control {
    id: String,
    source: String,
    debug: bool,
    original_module: Option<String>,
    original_ast: Option<String>,
    primary_exit: Option<u16>,
    algorithm_crc: Option<String>,
    body_crc: Option<String>,
}

#[test]
fn retained_primary_original_ast_and_debug_header() -> Result<(), Box<dyn std::error::Error>> {
    let controls: Controls =
        serde_json::from_str(include_str!("fixtures/original-ast/primary.json"))?;
    for control in controls.controls {
        let selection = dive_pluscal::rewrite::select_rewrite(
            &control.source,
            &dive_pluscal::rewrite::RewriteOptions::default(),
        )?;
        let region = selection.algorithm.start..selection.algorithm.end;
        let options = TranslationOptions {
            debug: control.debug,
            ..TranslationOptions::default()
        };
        if let Some(expected) = control.original_module {
            let original = original_ast_at(&control.source, region.clone(), &options)?;
            assert_eq!(original.module_source, expected, "{}", control.id);
            assert_eq!(control.primary_exit, Some(255));
        }
        if let Some(expected) = control.original_ast {
            let original = original_ast_at(&control.source, region.clone(), &options)?;
            assert_eq!(original.serialized_ast, expected, "{}", control.id);
        }
        if let Some(expected) = control.algorithm_crc {
            let header = algorithm_checksum_at(&control.source, region.clone(), &options)?;
            assert_eq!(header.checksum, expected, "{}", control.id);
            let translated = dive_pluscal::translate_at(&control.source, region.clone(), &options)?;
            assert_eq!(
                Some(dive_pluscal::rewrite::translation_checksum(&translated.tla)),
                control.body_crc
            );
            let plain = dive_pluscal::translate_at(
                &control.source,
                region,
                &TranslationOptions::default(),
            )?;
            assert_eq!(translated.tla, plain.tla);
        }
    }
    Ok(())
}

#[test]
fn original_stage_does_not_require_identifier_fixing_or_generation()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable Init=0; begin L: Init:=1; end algorithm";
    let original = original_ast_at(source, 0..source.len(), &TranslationOptions::default())?;
    assert!(original.serialized_ast.contains("[var |-> \"Init\""));
    assert!(
        original
            .serialized_ast
            .contains("[lhs |-> [var |-> \"Init\"")
    );
    assert!(
        algorithm_checksum_at(source, 0..source.len(), &TranslationOptions::default()).is_err()
    );
    let bound = TranslationOptions {
        max_output_bytes: original.module_source.len() - 1,
        ..TranslationOptions::default()
    };
    assert_eq!(
        original_ast_at(source, 0..source.len(), &bound)
            .err()
            .ok_or("must bound wrapper")?
            .diagnostic
            .code,
        Code::Resource
    );
    Ok(())
}

#[test]
fn pending_original_and_debug_stages_are_named() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A procedure P() begin L:return; end procedure; begin E:call P(); end algorithm";
    assert!(original_ast_at(source, 0..source.len(), &TranslationOptions::default()).is_ok());
    let historical = TranslationOptions {
        version: Some("1.4".into()),
        ..TranslationOptions::default()
    };
    let failure = original_ast_at(source, 0..source.len(), &historical)
        .err()
        .ok_or("historical remains pending")?;
    assert_eq!(failure.diagnostic.code, Code::Unsupported);
    assert!(failure.diagnostic.message.contains("pre-1.5"));
    for (source, feature) in [
        (
            "--algorithm A procedure P() begin L:if TRUE then skip; end if; R:return; end procedure; begin E:call P(); end algorithm",
            "procedure AST checksum is unavailable",
        ),
        (
            "--algorithm A variable x=0; begin x:skip; end algorithm",
            "identifier renaming",
        ),
        (
            "--algorithm A variable Init=0; begin L:Init:=1; end algorithm",
            "also defined",
        ),
    ] {
        let options = TranslationOptions {
            debug: true,
            ..TranslationOptions::default()
        };
        assert!(original_ast_at(source, 0..source.len(), &options).is_ok());
        let failure = algorithm_checksum_at(source, 0..source.len(), &options)
            .err()
            .ok_or("debug header boundary")?;
        assert_eq!(failure.diagnostic.code, Code::Unsupported);
        assert!(failure.diagnostic.message.contains(feature), "{failure}");
        if feature == "also defined" {
            assert!(dive_pluscal::translate(source, &options).is_err());
        } else {
            assert!(dive_pluscal::translate(source, &options).is_ok());
        }
    }
    Ok(())
}

#[test]
fn sequential_source_fairness_precedes_original_wrapper_options()
-> Result<(), Box<dyn std::error::Error>> {
    for source in [
        "--fair algorithm A begin L:skip; end algorithm",
        "--algorithm A fair begin L:skip; end algorithm",
    ] {
        let options = TranslationOptions {
            version: Some("1.5".into()),
            fairness: dive_pluscal::FairnessOption::None,
            ..TranslationOptions::default()
        };
        let original = original_ast_at(source, 0..source.len(), &options)?;
        assert!(original.module_source.contains("fairness == \"wf\"\n"));
        assert_eq!(
            original.option_context.effective.fairness,
            dive_pluscal::FairnessOption::None
        );
    }
    Ok(())
}
