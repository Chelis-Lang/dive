//! Exact pinned process AST bytes and conservative public checksum contracts.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;
#[test]
fn retained_process_serialization_matches_each_primary_stage()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("process-checksum-acceptance");
    let mut outcomes = [0_usize; 3];
    for folder in [
        "primary",
        "corrected-c",
        "corrected-modifiers",
        "context-controls",
        "owner-controls",
        "width-controls",
    ] {
        let base = root.join(folder);
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join("manifest.json"))?)?;
        for row in manifest["rows"].as_array().ok_or("missing primary rows")? {
            let id = row["id"].as_str().ok_or("missing primary identifier")?;
            let source = std::fs::read_to_string(base.join(format!("{id}.tla")))?;
            let selection = select_rewrite(&source, &RewriteOptions::default())?;
            match algorithm_checksum_at(
                &source,
                selection.algorithm.start..selection.algorithm.end,
                &TranslationOptions::default(),
            ) {
                Ok(actual) => {
                    assert_eq!(row["primary_translated"], true, "{folder}/{id}");
                    assert_eq!(
                        actual.serialized_ast,
                        std::fs::read_to_string(base.join(format!("{id}.translated.ast")))?,
                        "{folder}/{id}"
                    );
                    assert_eq!(actual.checksum, row["primary_crc"], "{folder}/{id}");
                    assert_eq!(actual.fair_header_crc_only, id.contains("_fair_"));
                    outcomes[0] += 1;
                }
                Err(error) if error.diagnostic.code == Code::Unsupported => outcomes[1] += 1,
                Err(error) => {
                    assert_eq!(row["primary_translated"], false, "{folder}/{id}: {error}");
                    assert!(
                        matches!(
                            error.diagnostic.code,
                            Code::Syntax | Code::Expression | Code::Lexical | Code::Label
                        ),
                        "{folder}/{id}: {error}"
                    );
                    outcomes[2] += 1;
                }
            }
        }
    }
    assert_eq!(outcomes, [160, 28, 36]);
    Ok(())
}
#[test]
fn process_option_checksum_captures_match_bytes_and_declared_precedence()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("process-option-checksum-acceptance");
    let mut outcomes = [0_usize; 4];
    for folder in ["primary", "context", "label-options"] {
        let base = root.join(folder);
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join("manifest.json"))?)?;
        for row in manifest["rows"].as_array().ok_or("missing rows")? {
            let id = row["id"].as_str().ok_or("missing identifier")?;
            let flags = row["flags"].as_array().ok_or("missing options")?;
            let has = |flag: &str| flags.iter().any(|actual| actual == flag);
            if has("-wf") && has("-sf") {
                assert_eq!(row["primary_translated"], false);
                outcomes[3] += 1;
                continue;
            }
            let mut options = TranslationOptions::default();
            options.insert_missing_labels = has("-label") || has("-reportLabels");
            if let Some(root) = flags.iter().position(|flag| flag == "-labelRoot") {
                options.label_root = flags[root + 1].as_str().ok_or("missing label root")?.into();
            }
            options.fairness = if has("-nof") {
                FairnessOption::None
            } else if has("-wf") {
                FairnessOption::Weak
            } else if has("-sf") {
                FairnessOption::Strong
            } else if has("-wfNext") {
                FairnessOption::Next
            } else {
                FairnessOption::Declared
            };
            let source = std::fs::read_to_string(base.join(format!("{id}.tla")))?;
            let selection = select_rewrite(&source, &RewriteOptions::default())?;
            match algorithm_checksum_at(
                &source,
                selection.algorithm.start..selection.algorithm.end,
                &options,
            ) {
                Ok(actual) => {
                    assert_eq!(row["primary_translated"], true, "{folder}/{id}");
                    assert_eq!(
                        actual.serialized_ast,
                        std::fs::read_to_string(base.join(format!("{id}.translated.ast")))?,
                        "{folder}/{id}"
                    );
                    assert_eq!(actual.checksum, row["primary_crc"], "{folder}/{id}");
                    assert_eq!(actual.fair_header_crc_only, id.contains("_fair_"));
                    outcomes[0] += 1;
                }
                Err(error) if error.diagnostic.code == Code::Unsupported => outcomes[1] += 1,
                Err(error) => {
                    assert_eq!(row["primary_translated"], false, "{folder}/{id}: {error}");
                    assert!(matches!(
                        error.diagnostic.code,
                        Code::Label | Code::Syntax | Code::Expression | Code::Lexical
                    ));
                    outcomes[2] += 1;
                }
            }
        }
    }
    assert_eq!(outcomes, [320, 48, 128, 28]);
    Ok(())
}
#[test]
fn process_ast_budgets_are_global_and_never_admit_partial_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable g=0; process P=1 begin L:g:=1; E:skip; end process; process Q=2 begin M:g:=2; F:skip; end process; end algorithm;";
    let exact = algorithm_checksum_at(source, 0..source.len(), &TranslationOptions::default())?;
    for max_output_bytes in [0, 1, exact.serialized_ast.len() - 1] {
        let options = TranslationOptions {
            max_output_bytes,
            ..Default::default()
        };
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    for max_actions in [0, 1, 3] {
        let options = TranslationOptions {
            max_actions,
            ..Default::default()
        };
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    for parameter in [0, 1, 2] {
        let mut options = TranslationOptions::default();
        match parameter {
            0 => options.parser.max_tokens = 1,
            1 => options.parser.max_nodes = 1,
            _ => options.parser.max_bytes = 1,
        }
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    Ok(())
}

#[test]
fn source_option_comments_have_no_artifact_or_full_crc() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("process-option-checksum-acceptance/validator-context");
    for name in ["weak", "strong", "labels", "root"] {
        let source = std::fs::read_to_string(root.join(format!("SourceOptions_{name}.tla")))?;
        let selection = select_rewrite(&source, &RewriteOptions::default())?;
        let region = selection.algorithm.start..selection.algorithm.end;
        let checksum =
            algorithm_checksum_at(&source, region.clone(), &TranslationOptions::default());
        assert!(checksum.is_err_and(|e| e.diagnostic.code == Code::Unsupported));
        let artifact = dive_pluscal::translate_at(&source, region, &TranslationOptions::default());
        assert!(artifact.is_err_and(|e| e.diagnostic.code == Code::Unsupported));
    }
    Ok(())
}
#[test]
fn option_normalization_budgets_are_global() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable g=0; process P=1 begin g:=1; g:=2; end process; process Q=2 begin g:=1; g:=2; end process; end algorithm;";
    let options = TranslationOptions {
        insert_missing_labels: true,
        fairness: FairnessOption::Strong,
        ..Default::default()
    };
    let actual = algorithm_checksum_at(source, 0..source.len(), &options)?;
    for max_actions in [0, 1, 2, 3] {
        let limited = TranslationOptions {
            max_actions,
            ..options.clone()
        };
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &limited)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    for max_output_bytes in [0, 1, actual.serialized_ast.len() - 1] {
        let limited = TranslationOptions {
            max_output_bytes,
            ..options.clone()
        };
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &limited)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    let invalid = TranslationOptions {
        label_root: "x".repeat(257),
        ..options
    };
    assert!(
        algorithm_checksum_at(source, 0..source.len(), &invalid)
            .is_err_and(|e| e.diagnostic.code == Code::Resource)
    );
    Ok(())
}
