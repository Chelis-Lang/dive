//! Generated names must precede user definitions; genuine unknown names still reject.
use dive_api::{RewriteRequest, TranslationRequest};
use std::path::Path;

#[test]
fn generated_declarations_precede_trailing_definitions() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-pluscal/macro-checksum-acceptance");
    for syntax in ["C", "P"] {
        for compat_tlc in [false, true] {
            for (folder, prefix) in [("insertion", "Trailing"), ("insertion-old", "OldTrailing")] {
                let file = format!("{prefix}_{syntax}.tla");
                let source = std::fs::read_to_string(root.join(folder).join("cases").join(&file))?;
                let request = RewriteRequest {
                    translation: TranslationRequest {
                        compat_tlc,
                        ..TranslationRequest::default()
                    },
                    ..RewriteRequest::default()
                };
                let report = dive_api::rewrite_source(&file, &source, &request);
                assert!(report.accepted(), "{}", report.to_json());
                let document = report.to_json();
                assert_eq!(document["model_proved"], false);
                assert_eq!(document["exhaustive"], false);
                assert_eq!(document["algorithm_checksum_verified"], true);
                let generated = document["replacement"]["module_source"]
                    .as_str()
                    .ok_or("missing replacement")?;
                let variables = generated.find("VARIABLES").ok_or("missing declarations")?;
                let invariant = generated
                    .find("Inv ==")
                    .ok_or("missing original invariant")?;
                assert!(variables < invariant);
            }
            let file = format!("UnknownTrailing_{syntax}.tla");
            let source = std::fs::read_to_string(root.join("insertion/cases").join(&file))?;
            let report = dive_api::rewrite_source(
                &file,
                &source,
                &RewriteRequest {
                    translation: TranslationRequest {
                        compat_tlc,
                        ..TranslationRequest::default()
                    },
                    ..RewriteRequest::default()
                },
            );
            assert!(!report.accepted());
            let document = report.to_json();
            assert_eq!(document["result"], "rejected");
            assert_eq!(document["model_proved"], false);
            assert!(document.get("replacement").is_none());
            assert!(document.to_string().contains("neverGenerated"));
        }
    }
    Ok(())
}

#[test]
fn closing_line_context_matches_reference_admission() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-pluscal/macro-checksum-acceptance");
    for folder in ["closing-line", "inline"] {
        let base = root.join(folder);
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            base.join("primary/manifest.json"),
        )?)?;
        for row in manifest["rows"].as_array().ok_or("missing primary rows")? {
            let name = row["id"].as_str().ok_or("missing primary id")?;
            let file = format!("{name}.tla");
            let source = std::fs::read_to_string(base.join("cases").join(&file))?;
            let expected = row["reference_errors"] == false && row["sany"]["accepted"] == true;
            for compat_tlc in [false, true] {
                let request = TranslationRequest {
                    compat_tlc,
                    ..TranslationRequest::default()
                };
                let translated = dive_api::translate_source(&file, &source, &request);
                let rewritten = dive_api::rewrite_source(
                    &file,
                    &source,
                    &RewriteRequest {
                        translation: request,
                        ..RewriteRequest::default()
                    },
                );
                for document in [translated.to_json(), rewritten.to_json()] {
                    assert_eq!(document["accepted"], expected, "{file}: {document}");
                    assert_eq!(document["model_proved"], false);
                    assert_eq!(document["exhaustive"], false);
                    if !expected {
                        assert_eq!(document["result"], "rejected");
                        assert!(document.get("replacement").is_none());
                        assert!(document.get("artifact").is_none());
                    }
                    if row["reference_errors"] == true {
                        assert_eq!(document["stage"], "selection");
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn marker_order_and_lexical_boundaries_match_primary() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-pluscal/macro-checksum-acceptance");
    for folder in ["marker", "marker-extra", "marker-single"] {
        let base = root.join(folder);
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            base.join("primary/manifest.json"),
        )?)?;
        for row in manifest["rows"].as_array().ok_or("missing rows")? {
            let id = row["id"].as_str().ok_or("missing identifier")?;
            let source = std::fs::read_to_string(base.join("cases").join(format!("{id}.tla")))?;
            for compat_tlc in [false, true] {
                let report = dive_api::rewrite_source(
                    id,
                    &source,
                    &RewriteRequest {
                        translation: TranslationRequest {
                            compat_tlc,
                            ..TranslationRequest::default()
                        },
                        ..RewriteRequest::default()
                    },
                );
                let document = report.to_json();
                assert_eq!(document["model_proved"], false);
                assert_eq!(document["exhaustive"], false);
                if row["translated"] == true && row["reference_errors"] == false {
                    assert!(report.accepted(), "{id}: {document}");
                    assert_eq!(
                        document["replacement"]["algorithm_checksum"]["serialized_ast"],
                        std::fs::read_to_string(
                            base.join("primary").join(format!("{id}.translated.ast"))
                        )?,
                        "{id}"
                    );
                    assert_eq!(
                        document["replacement"]["algorithm_checksum"]["checksum"], row["header"][0],
                        "{id}"
                    );
                } else {
                    assert!(!report.accepted(), "{id}: {document}");
                    assert!(document.get("replacement").is_none());
                }
            }
        }
    }
    Ok(())
}
