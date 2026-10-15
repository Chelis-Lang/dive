//! Genuine procedural Elsif generation and body CRCs; header alias emulation is retired.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite, translation_checksum},
    translate_at,
};
use std::path::Path;
#[test]
fn genuine_elsif_inputs_preserve_primary_generated_body_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/p-elsif-checksum");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let fairness = match row["flags"][0].as_str() {
            Some("-wf") => FairnessOption::Weak,
            Some("-sf") => FairnessOption::Strong,
            Some("-nof") => FairnessOption::None,
            Some("-wfNext") => FairnessOption::Next,
            _ => FairnessOption::Declared,
        };
        let options = TranslationOptions {
            fairness,
            ..TranslationOptions::default()
        };
        let region = selected.algorithm.start..selected.algorithm.end;
        assert_eq!(row["translated"], true, "{id}");
        assert_eq!(row["reference_errors"], false, "{id}");
        assert_eq!(row["sany"]["accepted"], true, "{id}");
        let body = translate_at(&source, region.clone(), &options)?;
        assert_eq!(
            translation_checksum(&body.tla),
            row["header"][1],
            "{id}: real primary body CRC"
        );
        assert!(
            algorithm_checksum_at(&source, region, &options).is_err_and(|error| error
                .diagnostic
                .code
                == Code::Unsupported
                && error
                    .diagnostic
                    .message
                    .contains("procedure AST checksum is unavailable"))
        );
    }
    assert_eq!(rows.len(), 32);
    Ok(())
}

#[test]
fn elsif_generation_output_and_parser_bounds_remain_real_refusals()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/p-elsif-checksum");
    for shape in ["all_call", "nested_call", "either_return", "while_return"] {
        let source =
            std::fs::read_to_string(base.join(format!("cases/ElsifProcedure_P_{shape}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let region = selected.algorithm.start..selected.algorithm.end;
        let reference = translate_at(&source, region.clone(), &TranslationOptions::default())?;
        for max_output_bytes in [0, 1, reference.tla.len() - 1] {
            let options = TranslationOptions {
                max_output_bytes,
                ..TranslationOptions::default()
            };
            assert!(
                translate_at(&source, region.clone(), &options)
                    .is_err_and(|e| e.diagnostic.code == Code::Resource)
            );
        }
        let mut options = TranslationOptions {
            max_output_bytes: reference.tla.len(),
            ..TranslationOptions::default()
        };
        assert_eq!(
            translate_at(&source, region.clone(), &options)?.tla,
            reference.tla
        );
        options.parser.max_depth = 1;
        assert!(
            translate_at(&source, region.clone(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
        options.parser.max_depth = 128;
        options.parser.max_nodes = 1;
        assert!(
            translate_at(&source, region, &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    Ok(())
}
