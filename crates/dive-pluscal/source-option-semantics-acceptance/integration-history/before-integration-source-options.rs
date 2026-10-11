//! Source declarations are refused before unproved option semantics can escape.
use dive_pluscal::rewrite::{RewriteOptions, select_rewrite};
use dive_pluscal::{Code, TranslationOptions, algorithm_checksum_at, translate_at};
use std::path::Path;

fn check(source: &str, refused: bool) -> Result<(), Box<dyn std::error::Error>> {
    let selection = select_rewrite(source, &RewriteOptions::default())?;
    let region = selection.algorithm.start..selection.algorithm.end;
    let checksum = algorithm_checksum_at(source, region.clone(), &TranslationOptions::default());
    let artifact = translate_at(source, region, &TranslationOptions::default());
    if refused {
        for error in [checksum.err(), artifact.err()] {
            let error = error.ok_or("source context was admitted")?;
            assert_eq!(error.diagnostic.code, Code::Unsupported);
            assert!(error.diagnostic.message.contains("source option"));
            assert!(source.is_char_boundary(error.diagnostic.span.start));
            assert!(source.is_char_boundary(error.diagnostic.span.end));
        }
    } else {
        checksum?;
        artifact?;
    }
    Ok(())
}

#[test]
fn source_declarations_refuse_before_artifact_or_crc() -> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("source-option-acceptance/validator-context");
    for name in ["weak", "strong", "labels", "root"] {
        check(
            &std::fs::read_to_string(base.join(format!("SourceOptions_{name}.tla")))?,
            true,
        )?;
    }
    Ok(())
}

#[test]
fn reference_spacing_contexts_remain_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("source-option-acceptance/source-spacing");
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("report.json"))?)?;
    let mut checked = 0;
    for row in report["rows"].as_array().ok_or("missing spacing rows")? {
        let id = row["id"].as_str().ok_or("missing identifier")?;
        let guarded = row["native_guarded"]
            .as_bool()
            .ok_or("missing disposition")?;
        if row["primary_effective_weak_fairness"] == true {
            assert!(guarded);
        }
        check(
            &std::fs::read_to_string(base.join(format!("{id}.tla")))?,
            guarded,
        )?;
        checked += 1;
    }
    assert_eq!(checked, 20);
    Ok(())
}
