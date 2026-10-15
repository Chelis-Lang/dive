//! Real generated-body checksum controls; procedure-header alias emulation is retired.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite, translation_checksum},
    translate_at,
};
use std::path::Path;

#[test]
fn genuine_primary_inputs_keep_generated_body_crc_and_source_failures()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multiple-procedure-checksum");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let rows = manifest["rows"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), 64);
    for row in rows {
        let id = row["id"].as_str().ok_or("id")?;
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let selection = select_rewrite(&source, &RewriteOptions::default())?;
        let fairness = match row["flags"][0].as_str() {
            Some("-wf") => FairnessOption::Weak,
            Some("-sf") => FairnessOption::Strong,
            Some("-nof") => FairnessOption::None,
            Some("-wfNext") => FairnessOption::Next,
            _ => FairnessOption::Declared,
        };
        if row["flags"][0] == "-fair" {
            // This retained primary is an invalid CLI flag, not a source failure.
            assert_eq!(row["translated"], false);
            assert_eq!(row["reference_errors"], true);
            continue;
        }
        let options = TranslationOptions {
            fairness,
            insert_missing_labels: row["flags"][0] == "-label",
            ..TranslationOptions::default()
        };
        let translated = translate_at(
            &source,
            selection.algorithm.start..selection.algorithm.end,
            &options,
        );
        if row["translated"] == true && row["reference_errors"] == false {
            match translated {
                Ok(body) => assert_eq!(
                    translation_checksum(&body.tla),
                    row["header"][1],
                    "{id}: real body CRC remains exact even when primary SANY rejects",
                ),
                Err(error) => {
                    assert_ne!(
                        row["sany"]["accepted"], true,
                        "{id}: a valid primary artifact must produce its exact body: {error}"
                    );
                    assert!(
                        matches!(
                            error.diagnostic.code,
                            Code::Unsupported
                                | Code::Lexical
                                | Code::Syntax
                                | Code::Expression
                                | Code::Label
                        ),
                        "{id}: invalid primary must have a named source refusal, not a bug or quota failure: {error}"
                    );
                    // Invalid primary module; no native body/AST/artifact agreement credit.
                }
            }
        } else {
            assert!(
                translated.is_err(),
                "{id}: preserve actual primary source failure"
            );
        }
    }
    Ok(())
}

#[test]
fn exact_procedure_crc_is_unavailable_without_guessing() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        include_str!("fixtures/multiple-procedure-checksum/cases/MultipleHeader_C_both.tla");
    let selection = select_rewrite(source, &RewriteOptions::default())?;
    for debug in [false, true] {
        let options = TranslationOptions {
            debug,
            ..TranslationOptions::default()
        };
        let error = algorithm_checksum_at(
            source,
            selection.algorithm.start..selection.algorithm.end,
            &options,
        )
        .err()
        .ok_or("procedure CRC must not be invented")?;
        assert_eq!(error.diagnostic.code, Code::Unsupported);
        assert!(
            error
                .diagnostic
                .message
                .contains("procedure AST checksum is unavailable")
        );
        assert!(
            translate_at(
                source,
                selection.algorithm.start..selection.algorithm.end,
                &options
            )
            .is_ok()
        );
    }
    Ok(())
}

#[test]
fn generation_output_limits_remain_errors_without_partial_body()
-> Result<(), Box<dyn std::error::Error>> {
    let source =
        include_str!("fixtures/multiple-procedure-checksum/cases/MultipleHeader_C_both.tla");
    let selection = select_rewrite(source, &RewriteOptions::default())?;
    let region = selection.algorithm.start..selection.algorithm.end;
    let baseline = translate_at(source, region.clone(), &TranslationOptions::default())?;
    for max_output_bytes in [0, 1, baseline.tla.len() - 1] {
        let options = TranslationOptions {
            max_output_bytes,
            ..TranslationOptions::default()
        };
        assert!(
            translate_at(source, region.clone(), &options)
                .is_err_and(|error| error.diagnostic.code == Code::Resource)
        );
    }
    Ok(())
}
