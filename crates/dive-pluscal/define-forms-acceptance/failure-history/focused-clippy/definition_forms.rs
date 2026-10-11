//! Source-owned LOCAL/function DEFINE domains, bodies and exact primary bytes.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;
#[test]
fn every_selected_definition_form_ast_is_exact_or_named_refusal()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("define-forms-acceptance");
    let mut counts = [0usize; 4];
    for folder in [
        "",
        "../define-acceptance",
        "../define-acceptance/contexts",
        "../define-acceptance/corpus",
        "../define-acceptance/promoted",
    ] {
        let base = root.join(folder);
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            base.join("primary/manifest.json"),
        )?)?;
        for row in manifest["rows"].as_array().ok_or("missing rows")? {
            let id = row["id"].as_str().ok_or("missing id")?;
            let source = std::fs::read_to_string(base.join("cases").join(format!("{id}.tla")))?;
            let flags = row["flags"].as_array().ok_or("missing flags")?;
            let has = |flag: &str| flags.iter().any(|actual| actual == flag);
            let options = TranslationOptions {
                insert_missing_labels: has("-label"),
                fairness: if has("-wf") {
                    FairnessOption::Weak
                } else if has("-sf") {
                    FairnessOption::Strong
                } else {
                    FairnessOption::Declared
                },
                ..TranslationOptions::default()
            };
            let selected = select_rewrite(&source, &RewriteOptions::default())?;
            match algorithm_checksum_at(
                &source,
                selected.algorithm.start..selected.algorithm.end,
                &options,
            ) {
                Ok(actual) => {
                    assert_eq!(row["translated"], true, "{folder}/{id}");
                    assert_eq!(row["reference_errors"], false, "{folder}/{id}");
                    assert_eq!(
                        actual.serialized_ast,
                        std::fs::read_to_string(
                            base.join("primary").join(format!("{id}.translated.ast"))
                        )?,
                        "{folder}/{id}"
                    );
                    assert_eq!(
                        actual.checksum, row["stages"]["translated"]["crc"],
                        "{folder}/{id}"
                    );
                    counts[usize::from(row["sany"]["accepted"] != true)] += 1;
                }
                Err(error) if error.diagnostic.code == Code::Unsupported => {
                    assert_ne!(error.diagnostic.message, "");
                    counts[2] += 1;
                }
                Err(error) => {
                    assert_eq!(row["reference_errors"], true, "{folder}/{id}: {error}");
                    assert!(matches!(
                        error.diagnostic.code,
                        Code::Syntax | Code::Lexical | Code::Expression | Code::Label
                    ));
                    counts[3] += 1;
                }
            }
        }
    }
    assert_eq!(counts, [152, 28, 52, 2]);
    Ok(())
}

#[test]
fn domain_and_body_dependencies_cannot_hide_action_or_cycles()
-> Result<(), Box<dyn std::error::Error>> {
    for definition in [
        "F[a \\in {x'}] == a",
        "F[a \\in {1}] == x'",
        "LOCAL F[a \\in {1}] == <>TRUE",
        "F[a \\in {F}] == a",
        "LOCAL F[a \\in {G}] == a G == F",
        "LOCAL Init == TRUE",
        "LOCAL F == 1 F == 2",
        "LOCAL F(a) == G(a) LOCAL G(a) == F(a)",
    ] {
        let source = format!(
            "--algorithm A variable x=0; define {definition} end define; begin L:x:=1; end algorithm;"
        );
        assert!(
            algorithm_checksum_at(&source, 0..source.len(), &TranslationOptions::default())
                .is_err_and(|e| e.diagnostic.code == Code::Unsupported),
            "{definition}"
        );
    }
    for definition in [
        "LOCAL F==1",
        "LOCAL F(a)==a+1",
        "F[a \\in {1,2}] == a+1",
        "LOCAL F[<<a,b>> \\in {<<1,2>>}] == a+b",
    ] {
        let source = format!(
            "--algorithm A variable x=0; define {definition} end define; begin L:x:=1; end algorithm;"
        );
        let actual =
            algorithm_checksum_at(&source, 0..source.len(), &TranslationOptions::default())?;
        assert!(!actual.serialized_ast.is_empty());
        for budget in [0, 1, 8] {
            let options = TranslationOptions {
                max_output_bytes: budget,
                ..TranslationOptions::default()
            };
            assert!(
                algorithm_checksum_at(&source, 0..source.len(), &options)
                    .is_err_and(|e| e.diagnostic.code == Code::Resource)
            );
        }
    }
    Ok(())
}
