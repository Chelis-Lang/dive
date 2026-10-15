//! Genuine pre-generation artifacts and independently captured debug headers.
use dive_pluscal::{
    TranslationOptions, algorithm_checksum_at, original_ast_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one consumed control loop keeps original, header and failed-source stages distinct"
)]
fn measured_original_ownership_locations_and_header_stages()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/original-ast-ownership");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json"))?)?;
    assert_eq!(
        manifest["upstream_commit"],
        "94d0c5070852b37b801bb8998c436383ab7ee37b"
    );
    assert_eq!(
        manifest["jar_sha256"],
        "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
    );
    let mut counts = [0usize; 3];
    for id in manifest["controls"].as_array().ok_or("controls")? {
        let id = id.as_str().ok_or("id")?;
        let row: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(format!("{id}.json")))?)?;
        assert_eq!(row["id"], id);
        let source = row["source"].as_str().ok_or("source")?;
        let options = TranslationOptions {
            debug: row["debug"].as_bool().ok_or("debug")?,
            insert_missing_labels: row["insert_labels"].as_bool().ok_or("label")?,
            ..TranslationOptions::default()
        };
        let selected = select_rewrite(source, &RewriteOptions::default());
        if row["stage"] == "source-error" {
            assert_eq!(row["primary_exit"], 255);
            assert_eq!(row["artifact_exists"], false);
            let actual = selected
                .map_err(|error| error.to_string())
                .and_then(|selection| {
                    original_ast_at(
                        source,
                        selection.algorithm.start..selection.algorithm.end,
                        &options,
                    )
                    .map_err(|error| error.to_string())
                });
            assert!(
                actual.is_err(),
                "{id}: must not turn primary source failure into an artifact"
            );
            counts[2] += 1;
            continue;
        }
        let selected = selected?;
        let region = selected.algorithm.start..selected.algorithm.end;
        if row["stage"] == "pre-generation WriteAST" {
            assert_eq!(row["primary_exit"], 255);
            let original = original_ast_at(source, region.clone(), &options)?;
            assert_eq!(
                original.module_source,
                row["original_module"].as_str().ok_or("module")?,
                "{id}"
            );
            let mut limited = options.clone();
            limited.max_output_bytes = original.module_source.len() - 1;
            assert!(
                original_ast_at(source, region.clone(), &limited)
                    .is_err_and(|error| error.diagnostic.code == dive_pluscal::Code::Resource),
                "{id}: wrapper limit"
            );
            counts[0] += 1;
        } else {
            assert_eq!(row["stage"], "post-generation header");
            assert_eq!(row["primary_exit"], 0);
            assert_eq!(row["sany_exit"], 0);
            assert_eq!(row["sany_semantic_accepted"], true);
            assert_eq!(row["sany_warning_count"], 1);
            assert_eq!(row["sany_original_checksum_validated"], false);
            assert_eq!(
                row["sany_embedded_validator_error"],
                id == "corrected_macro_nested_translation_debug"
            );
            let parsed = dive_pluscal::parse_at(source, region.clone(), &options.parser)?;
            let header = if parsed.algorithm.procedures.is_empty() {
                Some(algorithm_checksum_at(source, region.clone(), &options)?)
            } else {
                assert!(
                    algorithm_checksum_at(source, region.clone(), &options).is_err_and(|error| {
                        error
                            .diagnostic
                            .message
                            .contains("procedure AST checksum is unavailable")
                    })
                );
                None
            };
            if let Some(header) = &header {
                assert_eq!(
                    header.checksum,
                    row["algorithm_crc"].as_str().ok_or("CRC")?,
                    "{id}"
                );
                let original = original_ast_at(source, region.clone(), &options)?;
                if [
                    "while_translation_debug",
                    "owners_sequences_translation_debug",
                ]
                .contains(&id)
                {
                    assert_eq!(
                        header.serialized_ast, original.serialized_ast,
                        "the measured copied-vector header is a no-mutation positive"
                    );
                } else if [
                    "label_if_translation_debug",
                    "label_either_translation_debug",
                ]
                .contains(&id)
                {
                    assert_ne!(
                        header.serialized_ast, original.serialized_ast,
                        "the shared unlabelled vectors mutate only the generated header"
                    );
                }
            }
            let result = dive_pluscal::translate_at(source, region.clone(), &options)?;
            assert_eq!(
                result.tla.trim_end_matches('\n'),
                row["generated_body"]
                    .as_str()
                    .ok_or("body")?
                    .trim_end_matches('\n'),
                "{id}"
            );
            assert_eq!(
                dive_pluscal::rewrite::translation_checksum(&result.tla),
                row["body_crc"].as_str().ok_or("body CRC")?,
                "{id}"
            );
            let plain = dive_pluscal::translate_at(
                source,
                region,
                &TranslationOptions {
                    debug: false,
                    ..options
                },
            )?;
            assert_eq!(plain.tla, result.tla, "{id}: debug preserves body");
            counts[1] += 1;
        }
    }
    assert_eq!(counts, [32, 12, 4]);
    Ok(())
}
