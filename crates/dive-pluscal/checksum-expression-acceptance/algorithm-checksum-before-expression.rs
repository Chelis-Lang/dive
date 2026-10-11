//! Post-translation AST byte controls, independent of generated-body CRCs.
use dive_pluscal::{
    Code, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;
#[test]
fn retained_reference_ast_bytes_are_exact_or_explicitly_unsupported()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("ast-checksum-acceptance");
    let mut total = 0;
    let mut exact = 0;
    let mut refused = 0;
    let mut rejected = 0;
    for campaign in [
        "primary",
        "token-controls",
        "control-shapes",
        "context-controls",
    ] {
        let base = base.join(campaign);
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join("manifest.json"))?)?;
        let rows = manifest["rows"].as_array().ok_or("missing rows")?;
        total += rows.len();
        for row in rows {
            let id = row["id"].as_str().ok_or("missing id")?;
            let source = std::fs::read_to_string(base.join(format!("{id}.tla")))?;
            let selection = select_rewrite(&source, &RewriteOptions::default())?;
            match algorithm_checksum_at(
                &source,
                selection.algorithm.start..selection.algorithm.end,
                &TranslationOptions::default(),
            ) {
                Ok(checksum) => {
                    assert_eq!(
                        checksum.serialized_ast,
                        std::fs::read_to_string(base.join(format!("{id}.translated.ast")))?,
                        "{id}"
                    );
                    assert_eq!(checksum.checksum, row["primary_crc"], "{id}");
                    exact += 1;
                }
                Err(error) => {
                    if row["primary_translated"] == false
                        && error.diagnostic.code != Code::Unsupported
                    {
                        assert!(
                            matches!(
                                error.diagnostic.code,
                                Code::Syntax | Code::Expression | Code::Lexical | Code::Label
                            ),
                            "{id}: {error}"
                        );
                        rejected += 1;
                    } else {
                        assert_eq!(error.diagnostic.code, Code::Unsupported, "{id}: {error}");
                        refused += 1;
                    }
                }
            }
        }
    }
    assert_eq!(total, 192);
    assert_eq!((exact, refused, rejected), (138, 32, 22));
    Ok(())
}
#[test]
fn full_checksum_bounds_and_reference_fair_weakness_stay_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let body = "variable x=0; begin L:x:=1; E:skip; end algorithm;";
    let unfair = format!("--algorithm A {body}");
    let fair = format!("--fair algorithm A {body}");
    let a = algorithm_checksum_at(&unfair, 0..unfair.len(), &TranslationOptions::default())?;
    let b = algorithm_checksum_at(&fair, 0..fair.len(), &TranslationOptions::default())?;
    assert_eq!(a.serialized_ast, b.serialized_ast);
    assert!(!a.fair_header_crc_only);
    assert!(b.fair_header_crc_only);
    assert_eq!(b.checksum, "7c28162a");
    assert_ne!(a.checksum, b.checksum);
    for max_output_bytes in [0, 1, a.serialized_ast.len() - 1] {
        let options = TranslationOptions {
            max_output_bytes,
            ..Default::default()
        };
        assert!(
            algorithm_checksum_at(&unfair, 0..unfair.len(), &options)
                .is_err_and(|e| e.diagnostic.code == Code::Resource)
        );
    }
    Ok(())
}
