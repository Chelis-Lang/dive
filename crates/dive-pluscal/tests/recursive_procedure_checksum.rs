//! Recursive source call graphs are finite AST data, not execution unrolling.
use dive_pluscal::{
    Code, TranslationOptions, TranslationStage, algorithm_checksum_at, parse_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::{fmt::Write, path::Path};

#[test]
fn every_recursive_row_matches_original_header_bytes_and_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recursive-procedure-checksum");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    let mut admitted = 0;
    let mut invalid_primary = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        if !id.starts_with("RecursiveHeader_") {
            continue;
        }
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let actual = algorithm_checksum_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &TranslationOptions::default(),
        )?;
        assert_eq!(row["translated"], true, "{id}");
        assert_eq!(row["reference_errors"], false, "{id}");
        assert_eq!(
            actual.serialized_ast,
            std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?,
            "{id}"
        );
        assert_eq!(actual.checksum, row["stages"]["translated"]["crc"], "{id}");
        if row["sany"]["accepted"] != true {
            assert!(id.ends_with("_zero_param_self_tail"), "{id}");
            invalid_primary += 1;
        }
        admitted += 1;
    }
    assert_eq!(rows.len(), 162);
    assert_eq!(admitted, 32);
    assert_eq!(invalid_primary, 2);
    Ok(())
}

#[test]
fn recursive_graph_keeps_shared_expansion_and_exact_output_limits()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recursive-procedure-checksum");
    for shape in [
        "self_base",
        "mutual_forward",
        "mutual_reverse",
        "while_self",
        "unused_recursive",
    ] {
        let source =
            std::fs::read_to_string(base.join(format!("cases/RecursiveHeader_P_{shape}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let region = selected.algorithm.start..selected.algorithm.end;
        let expected =
            algorithm_checksum_at(&source, region.clone(), &TranslationOptions::default())?;
        for bytes in [0, 1, expected.serialized_ast.len() - 1] {
            let options = TranslationOptions {
                max_output_bytes: bytes,
                ..TranslationOptions::default()
            };
            let error = algorithm_checksum_at(&source, region.clone(), &options)
                .err()
                .ok_or("missing resource refusal")?;
            assert_eq!(error.diagnostic.code, Code::Resource);
        }
        let options = TranslationOptions {
            max_output_bytes: expected.serialized_ast.len(),
            ..TranslationOptions::default()
        };
        assert_eq!(
            algorithm_checksum_at(&source, region.clone(), &options)?.serialized_ast,
            expected.serialized_ast
        );
    }
    Ok(())
}

#[test]
fn parsed_cyclic_owners_do_not_escape_one_shared_budget() -> Result<(), Box<dyn std::error::Error>>
{
    let mut source = String::from("--algorithm A variable x=0; ");
    for i in 0..16 {
        write!(
            source,
            "procedure P{i}(a{i}=0) begin L{i}:call P{}(a{i}); R{i}:return; end procedure; ",
            (i + 1) % 16
        )?;
    }
    source.push_str("begin M:call P0(1); E:skip; end algorithm;");
    let mut options = TranslationOptions::default();
    options.parser.max_nodes = 128;
    assert!(parse_at(&source, 0..source.len(), &options.parser).is_ok());
    assert!(
        algorithm_checksum_at(&source, 0..source.len(), &TranslationOptions::default()).is_ok()
    );
    let error = algorithm_checksum_at(&source, 0..source.len(), &options)
        .err()
        .ok_or("cyclic graph escaped shared expansion reservation")?;
    assert_eq!(error.stage, TranslationStage::Normalization);
    assert_eq!(error.diagnostic.code, Code::Resource);
    assert!(error.diagnostic.message.contains("header expansion limit"));
    Ok(())
}

#[test]
fn tail_call_locals_parameters_frames_and_reversed_owners_match_pinned_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/recursive-procedure-checksum/tail-ownership");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    assert_eq!(rows.len(), 8);
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let actual = algorithm_checksum_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &TranslationOptions::default(),
        )?;
        assert_eq!(row["sany"]["accepted"], true, "{id}");
        assert_eq!(
            actual.serialized_ast,
            std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?,
            "{id}"
        );
        assert_eq!(actual.checksum, row["stages"]["translated"]["crc"], "{id}");
    }
    Ok(())
}
