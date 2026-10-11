//! Exact header AST controls, distinct from expanded call/return ownership.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, TranslationStage, algorithm_checksum_at, parse_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::{fmt::Write, path::Path};
#[test]
fn all_declared_multiple_procedure_rows_match_original_header_bytes_and_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("multiple-procedure-checksum-acceptance");
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
    assert_eq!(rows.len(), 64);
    assert_eq!(census, [62, 0, 0, 2]);
    Ok(())
}

#[test]
fn aggregate_expansion_budget_is_shared_across_owners() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from(
        "--algorithm A variable x=0; procedure Put(a=0) begin P:return; end procedure; procedure Get(b=0) begin G:return; end procedure; begin ",
    );
    for index in 0..48 {
        let name = if index % 2 == 0 { "Put" } else { "Get" };
        write!(source, "L{index}:call {name}({index}); ")?;
    }
    source.push_str("E:skip; end algorithm;");
    let mut options = TranslationOptions::default();
    options.parser.max_nodes = 128;
    // Source grammar fits. Target discovery, cached entries, every owner frame
    // and all nested/header work must debit one shared reservation afterwards.
    assert!(parse_at(&source, 0..source.len(), &options.parser).is_ok());
    let error = algorithm_checksum_at(&source, 0..source.len(), &options)
        .err()
        .ok_or("expansion incorrectly escaped the aggregate bound")?;
    assert_eq!(error.stage, TranslationStage::Normalization);
    assert_eq!(error.diagnostic.code, Code::Resource);
    assert!(error.diagnostic.message.contains("header expansion limit"));
    Ok(())
}

#[test]
fn multiple_owner_checksum_preserves_exact_output_budget() -> Result<(), Box<dyn std::error::Error>>
{
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("multiple-procedure-checksum-acceptance");
    for shape in [
        "both",
        "reverse_declarations",
        "if_first_else_second",
        "locals",
    ] {
        let source =
            std::fs::read_to_string(base.join(format!("cases/MultipleHeader_P_{shape}.tla")))?;
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
        let options = TranslationOptions {
            max_output_bytes: reference.serialized_ast.len(),
            ..TranslationOptions::default()
        };
        assert_eq!(
            algorithm_checksum_at(&source, region, &options)?.serialized_ast,
            reference.serialized_ast
        );
    }
    Ok(())
}
