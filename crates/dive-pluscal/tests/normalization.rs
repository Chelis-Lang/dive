//! Primary option/artifact evidence; generated labels are original statement associations.
use dive_pluscal::{Code, TranslationOptions, translate, translate_at};
use std::path::Path;
fn words(source: &str) -> Vec<String> {
    dive_syntax::tokens::tokenize_spec(source)
        .into_iter()
        .filter(|t| !t.kind.is_trivia())
        .map(|t| source[usize::from(t.range.start())..usize::from(t.range.end())].into())
        .collect()
}
fn retained_selection(
    directory: &str,
    selected: usize,
    expected_artifacts: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/normalization")
        .join(directory);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let rows = manifest["rows"].as_array().ok_or("missing rows")?;
    assert_eq!(rows.len(), selected);
    let mut artifacts = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("input")?),
        )?;
        let mut options = TranslationOptions::default();
        let flags = row["options"].as_array().ok_or("flags")?;
        let mut flags = flags.iter();
        while let Some(flag) = flags.next() {
            match flag.as_str().ok_or("flag")? {
                "-label" | "-reportLabels" => options.insert_missing_labels = true,
                "-labelRoot" => {
                    options.label_root = flags
                        .next()
                        .and_then(serde_json::Value::as_str)
                        .ok_or("root")?
                        .into();
                }
                _ => return Err("unknown retained option".into()),
            }
        }
        let start = source
            .find("--algorithm")
            .or_else(|| source.find("--fair algorithm"))
            .ok_or("marker")?;
        let end = source.rfind("*)").ok_or("comment")?;
        match translate_at(&source, start..end, &options) {
            Ok(artifact) => {
                assert_eq!(row["primary"]["status"], "translated", "{id}");
                assert_eq!(row["sany"]["status"], "accepted", "{id}");
                let primary = std::fs::read_to_string(
                    base.join("fixtures")
                        .join(row["translated"]["path"].as_str().ok_or("artifact")?),
                )?;
                let body: Vec<_> = primary
                    .lines()
                    .skip_while(|l| !l.contains("BEGIN TRANSLATION"))
                    .skip(1)
                    .take_while(|l| !l.contains("END TRANSLATION"))
                    .collect();
                assert_eq!(words(&artifact.tla), words(&body.join("\n")), "{id}");
                let normalized: serde_json::Value = serde_json::from_slice(&std::fs::read(
                    base.join("fixtures").join(
                        row["normalized_reference_adapter"]["document"]["path"]
                            .as_str()
                            .ok_or("normalized AST")?,
                    ),
                )?)?;
                let names: Vec<_> = artifact
                    .inserted_labels
                    .iter()
                    .map(|l| l.name.as_str())
                    .collect();
                assert_eq!(serde_json::json!(names), normalized["inserted"], "{id}");
                let positions: Vec<_> = artifact
                    .inserted_labels
                    .iter()
                    .map(|l| format!("line {}, column {}", l.position.line, l.position.column))
                    .collect();
                assert_eq!(
                    serde_json::json!(positions),
                    normalized["locations"],
                    "{id}"
                );
                assert_eq!(
                    !artifact.configuration.is_empty(),
                    normalized["default_initialization"]
                        .as_bool()
                        .ok_or("default init")?,
                    "{id}"
                );
                artifacts += 1;
            }
            Err(e) if e.diagnostic.code == Code::Unsupported => {}
            Err(e) => {
                assert_ne!(e.diagnostic.code, Code::Internal, "{id}");
                assert!(
                    row["primary"]["status"] == "rejected" || row["sany"]["status"] == "rejected",
                    "{id}: {e}"
                );
            }
        }
    }
    assert_eq!(artifacts, expected_artifacts);
    Ok(())
}
#[test]
fn retained_normalization_artifacts_and_errors() -> Result<(), Box<dyn std::error::Error>> {
    retained_selection("", 110, 86)?;
    retained_selection("interactions", 16, 11)
}

#[test]
fn source_ast_stays_original_and_generated_labels_are_bounded()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin x:=1; x:=2; end algorithm";
    let parsed = dive_pluscal::parse(source, &dive_pluscal::Options::default())?;
    assert!(parsed.statements.iter().all(|s| s.label.is_none()));
    let artifact = translate(source, &TranslationOptions::default())?;
    assert_eq!(artifact.actions, ["Lbl_1", "Lbl_2"]);
    for label in artifact.inserted_labels {
        assert!(source.get(label.source.start..label.source.end).is_some());
    }
    for options in [
        TranslationOptions {
            max_actions: 1,
            ..TranslationOptions::default()
        },
        TranslationOptions {
            max_output_bytes: 1,
            ..TranslationOptions::default()
        },
    ] {
        assert!(matches!(translate(source,&options),Err(e) if e.diagnostic.code==Code::Resource));
    }
    Ok(())
}
#[test]
fn prefix_does_not_enable_insertion_and_typed_names_are_skipped()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin Lbl_1: x:=1; x:=2; end algorithm";
    assert!(
        matches!(translate(source,&TranslationOptions::default()),Err(e) if e.diagnostic.code==Code::Label)
    );
    let result = translate(
        source,
        &TranslationOptions {
            insert_missing_labels: true,
            ..TranslationOptions::default()
        },
    )?;
    assert_eq!(result.actions, ["Lbl_1", "Lbl_2"]);
    let source = "--algorithm A variable x=0; begin x:=1; E: skip; end algorithm";
    assert!(
        matches!(translate(source,&TranslationOptions{label_root:"Step".into(),..TranslationOptions::default()}),Err(e) if e.diagnostic.code==Code::Label)
    );
    Ok(())
}
