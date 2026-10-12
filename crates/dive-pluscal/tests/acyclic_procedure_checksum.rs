//! Actual call edges are distinct from the reference LAST-header alias.
use dive_pluscal::{
    Code, TranslationOptions, TranslationStage, algorithm_checksum_at, parse_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::{fmt::Write, path::Path};

#[test]
fn all_new_acyclic_rows_match_original_header_bytes_and_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/acyclic-procedure-checksum");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    let mut admitted = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        if !id.starts_with("AcyclicHeader_")
            && !id.ends_with("_forward_call")
            && !id.ends_with("_reverse_call")
        {
            continue;
        }
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let checksum = algorithm_checksum_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &TranslationOptions::default(),
        )?;
        assert_eq!(row["translated"], true, "{id}");
        assert_eq!(row["reference_errors"], false, "{id}");
        assert_eq!(row["sany"]["accepted"], true, "{id}");
        assert_eq!(
            checksum.serialized_ast,
            std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?,
            "{id}"
        );
        assert_eq!(
            checksum.checksum, row["stages"]["translated"]["crc"],
            "{id}"
        );
        admitted += 1;
    }
    assert_eq!(rows.len(), 130);
    assert_eq!(admitted, 36);
    Ok(())
}

#[test]
fn former_recursive_boundary_now_matches_exact_primary_header()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/acyclic-procedure-checksum");
    for syntax in ["C", "P"] {
        for shape in ["self_recursion", "mutual_recursion"] {
            let id = format!("MultipleOwner_{syntax}_{shape}");
            let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
            let selected = select_rewrite(&source, &RewriteOptions::default())?;
            let actual = algorithm_checksum_at(
                &source,
                selected.algorithm.start..selected.algorithm.end,
                &TranslationOptions::default(),
            )?;
            let expected =
                std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?;
            assert_eq!(actual.serialized_ast, expected, "{id}");
        }
    }
    Ok(())
}

#[test]
fn graph_and_expansion_share_one_unchanged_budget() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from("--algorithm A variable x=0; ");
    for i in 0..16 {
        write!(source, "procedure P{i}(a{i}=0) begin L{i}:")?;
        if i < 15 {
            write!(source, "call P{}(a{i}); R{i}:", i + 1)?;
        }
        source.push_str("return; end procedure; ");
    }
    source.push_str("begin M:call P0(1); E:skip; end algorithm;");
    let mut options = TranslationOptions::default();
    options.parser.max_nodes = 128;
    assert!(parse_at(&source, 0..source.len(), &options.parser).is_ok());
    let error = algorithm_checksum_at(&source, 0..source.len(), &options)
        .err()
        .ok_or("call graph escaped shared expansion reservation")?;
    assert_eq!(error.stage, TranslationStage::Normalization);
    assert_eq!(error.diagnostic.code, Code::Resource);
    assert!(error.diagnostic.message.contains("header expansion limit"));
    Ok(())
}

#[test]
fn acyclic_checksum_preserves_exact_output_budget() -> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/acyclic-procedure-checksum");
    for shape in [
        "forward",
        "reverse",
        "chain",
        "if",
        "elsif",
        "while_true_tail",
    ] {
        let source =
            std::fs::read_to_string(base.join(format!("cases/AcyclicHeader_P_{shape}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let region = selected.algorithm.start..selected.algorithm.end;
        let expected =
            algorithm_checksum_at(&source, region.clone(), &TranslationOptions::default())?;
        for max_output_bytes in [0, 1, expected.serialized_ast.len() - 1] {
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
            max_output_bytes: expected.serialized_ast.len(),
            ..TranslationOptions::default()
        };
        assert_eq!(
            algorithm_checksum_at(&source, region, &options)?.serialized_ast,
            expected.serialized_ast
        );
    }
    Ok(())
}
