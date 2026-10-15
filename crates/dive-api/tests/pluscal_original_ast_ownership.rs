//! Public reports consume genuine original-stage and debug translation controls.
use dive_api::{RewriteRequest, TranslationRequest, original_ast_source, rewrite_source};
use dive_pluscal::TranslationOptions;
use std::path::Path;

#[test]
fn measured_original_and_header_ownership_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/original-ast-ownership");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json"))?)?;
    for compat_tlc in [false, true] {
        for id in manifest["controls"].as_array().ok_or("controls")? {
            let id = id.as_str().ok_or("id")?;
            let row: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(root.join(format!("{id}.json")))?)?;
            let source = row["source"].as_str().ok_or("source")?;
            let request = TranslationRequest {
                compat_tlc,
                limits: TranslationOptions {
                    debug: row["debug"].as_bool().ok_or("debug")?,
                    insert_missing_labels: row["insert_labels"].as_bool().ok_or("label")?,
                    ..TranslationOptions::default()
                },
                ..TranslationRequest::default()
            };
            let original = original_ast_source("Case.tla", source, &request);
            if row["stage"] == "source-error" {
                assert!(
                    !original.artifact_available(),
                    "{id}: {}",
                    original.to_json()
                );
                continue;
            }
            assert!(
                original.artifact_available(),
                "{id}: {}",
                original.to_json()
            );
            let doc = original.to_json();
            for field in ["accepted", "generated_module_accepted", "writes_performed"] {
                assert_eq!(doc[field], false, "{id}: {field}");
            }
            assert!(doc.get("analysis").is_none());
            if row["stage"] == "pre-generation WriteAST" {
                assert_eq!(
                    doc["ast_artifact"]["module_source"], row["original_module"],
                    "{id}"
                );
            } else {
                let rewritten = rewrite_source(
                    "Case.tla",
                    source,
                    &RewriteRequest {
                        translation: request,
                        ..RewriteRequest::default()
                    },
                );
                assert!(rewritten.accepted(), "{id}: {}", rewritten.to_json());
                let doc = rewritten.to_json();
                let selected = dive_pluscal::rewrite::select_rewrite(
                    source,
                    &dive_pluscal::rewrite::RewriteOptions::default(),
                )?;
                let parsed = dive_pluscal::parse_at(
                    source,
                    selected.algorithm.start..selected.algorithm.end,
                    &dive_pluscal::Options::default(),
                )?;
                if parsed.algorithm.procedures.is_empty() {
                    assert_eq!(
                        doc["replacement"]["algorithm_checksum"], row["algorithm_crc"],
                        "{id}"
                    );
                } else {
                    assert_eq!(
                        doc["replacement"]["algorithm_checksum"],
                        serde_json::Value::Null
                    );
                }
                assert_eq!(
                    doc["replacement"]["translation_checksum"], row["body_crc"],
                    "{id}"
                );
                assert_eq!(doc["filesystem_plan"]["writes_performed"], false);
            }
        }
    }
    Ok(())
}
