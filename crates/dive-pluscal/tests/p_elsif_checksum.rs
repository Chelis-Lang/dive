//! Exact header AST controls, distinct from expanded call/return ownership.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;
#[test]
fn all_declared_elsif_rows_match_original_header_bytes_and_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("p-elsif-checksum-acceptance");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    let mut census = [0usize; 4];
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
        match algorithm_checksum_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &options,
        ) {
            Ok(checksum) => {
                assert_eq!(row["translated"], true, "{id}");
                assert_eq!(row["reference_errors"], false, "{id}");
                assert_eq!(
                    checksum.serialized_ast,
                    std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?,
                    "{id}"
                );
                assert_eq!(
                    checksum.checksum, row["stages"]["translated"]["crc"],
                    "{id}"
                );
                // Invalid primary modules earn byte equality only, never artifact credit.
                census[usize::from(row["sany"]["accepted"] != true)] += 1;
            }
            Err(error) if error.diagnostic.code == Code::Unsupported => {
                assert!(
                    error.diagnostic.message.starts_with("unsupported"),
                    "{id}: {error}"
                );
                census[2] += 1;
            }
            Err(error) => {
                assert!(
                    row["translated"] == false || row["reference_errors"] == true,
                    "{id}: {error}"
                );
                assert!(
                    matches!(
                        error.diagnostic.code,
                        Code::Syntax | Code::Expression | Code::Lexical | Code::Label
                    ),
                    "{id}: {error}"
                );
                census[3] += 1;
            }
        }
    }
    assert_eq!(rows.len(), 32);
    assert_eq!(census, [32, 0, 0, 0]);
    Ok(())
}

#[test]
fn virtual_branch_expansion_preserves_resource_refusal_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("p-elsif-checksum-acceptance");
    for shape in ["all_call", "nested_call", "either_return", "while_return"] {
        let source =
            std::fs::read_to_string(base.join(format!("cases/ElsifProcedure_P_{shape}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let region = selected.algorithm.start..selected.algorithm.end;
        let reference =
            algorithm_checksum_at(&source, region.clone(), &TranslationOptions::default())?;
        for max_output_bytes in [0, 1, reference.serialized_ast.len() - 1] {
            let options = TranslationOptions {
                max_output_bytes,
                ..TranslationOptions::default()
            };
            assert!(
                algorithm_checksum_at(&source, region.clone(), &options)
                    .is_err_and(|e| e.diagnostic.code == Code::Resource)
            );
        }
        let mut options = TranslationOptions {
            max_output_bytes: reference.serialized_ast.len(),
            ..TranslationOptions::default()
        };
        assert_eq!(
            algorithm_checksum_at(&source, region.clone(), &options)?.serialized_ast,
            reference.serialized_ast
        );
        options.parser.max_depth = 1;
        assert!(
            algorithm_checksum_at(&source, region.clone(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
        options.parser.max_depth = 128;
        options.parser.max_nodes = 1;
        assert!(
            algorithm_checksum_at(&source, region, &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    Ok(())
}
