//! Process header ownership is separate from indexed generated storage and LAST.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, TranslationStage, algorithm_checksum_at, parse_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::{fmt::Write, path::Path};

#[test]
fn process_original_header_bytes_and_crc_match_every_admitted_row()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("process-procedure-checksum-acceptance");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    let mut count = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        if id.contains("_boundary_") {
            continue;
        }
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        for fairness in [FairnessOption::Declared, FairnessOption::None] {
            let options = TranslationOptions {
                fairness,
                ..TranslationOptions::default()
            };
            let checksum = algorithm_checksum_at(
                &source,
                selected.algorithm.start..selected.algorithm.end,
                &options,
            )?;
            assert_eq!(row["translated"], true, "{id}");
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
        }
        count += 1;
    }
    assert_eq!(rows.len(), 72);
    assert_eq!(count, 56);
    Ok(())
}

#[test]
fn unproved_process_owners_fairness_and_scoped_names_have_no_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("process-procedure-checksum-acceptance");
    for syntax in ["C", "P"] {
        for shape in [
            "duplicate_global",
            "label_collision",
            "unknown_target",
            "arity",
            "scoped_capture",
            "fair_process",
            "mixed_blocks",
            "nonfinite_domain",
        ] {
            let id = format!("ProcessHeader_{syntax}_boundary_{shape}");
            let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
            let selected = select_rewrite(&source, &RewriteOptions::default())?;
            assert!(
                algorithm_checksum_at(
                    &source,
                    selected.algorithm.start..selected.algorithm.end,
                    &TranslationOptions::default()
                )
                .is_err(),
                "{id}"
            );
        }
    }
    let source = std::fs::read_to_string(base.join("cases/ProcessHeader_P_member_ordinary.tla"))?;
    let selected = select_rewrite(&source, &RewriteOptions::default())?;
    for fairness in [
        FairnessOption::Weak,
        FairnessOption::Strong,
        FairnessOption::Next,
    ] {
        let options = TranslationOptions {
            fairness,
            ..TranslationOptions::default()
        };
        let error = algorithm_checksum_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &options,
        )
        .err()
        .ok_or("missing fairness refusal")?;
        assert_eq!(error.diagnostic.code, Code::Unsupported);
        assert!(
            error
                .diagnostic
                .message
                .contains("fairness checksum ownership")
        );
    }
    Ok(())
}

#[test]
fn process_calls_share_the_existing_expansion_and_output_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from(
        "--algorithm A variable x=0; procedure First(a=0) begin F:return; end procedure; process Worker \\in {1,2} begin ",
    );
    for i in 0..48 {
        write!(source, "L{i}:call First({i}); ")?;
    }
    source.push_str("E:skip; end process; end algorithm;");
    let mut options = TranslationOptions::default();
    options.parser.max_nodes = 128;
    assert!(parse_at(&source, 0..source.len(), &options.parser).is_ok());
    let error = algorithm_checksum_at(&source, 0..source.len(), &options)
        .err()
        .ok_or("process calls escaped aggregate budget")?;
    assert_eq!(error.stage, TranslationStage::Normalization);
    assert_eq!(error.diagnostic.code, Code::Resource);
    assert!(error.diagnostic.message.contains("header expansion limit"));
    let expected = algorithm_checksum_at(&source, 0..source.len(), &TranslationOptions::default())?;
    for max_output_bytes in [0, 1, expected.serialized_ast.len() - 1] {
        let options = TranslationOptions {
            max_output_bytes,
            ..TranslationOptions::default()
        };
        assert!(
            algorithm_checksum_at(&source, 0..source.len(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    let options = TranslationOptions {
        max_output_bytes: expected.serialized_ast.len(),
        ..TranslationOptions::default()
    };
    assert_eq!(
        algorithm_checksum_at(&source, 0..source.len(), &options)?.serialized_ast,
        expected.serialized_ast
    );
    Ok(())
}
