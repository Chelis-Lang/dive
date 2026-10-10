//! Retained process storage/action contracts and bounds, independent of native enumeration.
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
        .join("process-acceptance")
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
fn retained_process_artifacts_and_errors() -> Result<(), Box<dyn std::error::Error>> {
    retained_selection("", 208, 92)?;
    retained_selection("interactions", 16, 2)?;
    retained_selection("subscripts", 12, 8)?;
    retained_selection("names", 6, 0)
}
#[test]
fn process_labels_require_explicit_insertion_and_remain_source_mapped()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A process P \\in{1,2} variables x=0; begin x:=1; x:=2; end process; end algorithm";
    let parsed = dive_pluscal::parse(source, &dive_pluscal::Options::default())?;
    assert!(parsed.statements.iter().all(|s| s.label.is_none()));
    assert!(
        matches!(translate(source,&TranslationOptions::default()),Err(e) if e.diagnostic.code==Code::Label)
    );
    let options = TranslationOptions {
        insert_missing_labels: true,
        ..TranslationOptions::default()
    };
    let artifact = translate(source, &options)?;
    assert_eq!(artifact.actions, ["Lbl_1", "Lbl_2"]);
    assert_eq!(artifact.processes.len(), 1);
    for label in artifact.inserted_labels {
        assert!(source.get(label.source.start..label.source.end).is_some());
    }
    for options in [
        TranslationOptions {
            max_actions: 1,
            ..options.clone()
        },
        TranslationOptions {
            max_output_bytes: 1,
            ..options
        },
    ] {
        assert!(matches!(translate(source,&options),Err(e) if e.diagnostic.code==Code::Resource));
    }
    Ok(())
}
#[test]
fn process_name_shadowing_and_fairness_refuse_without_artifacts() {
    for source in [
        "--algorithm A variable x=0; process P =1 variables x=1; begin L: x:=2; end process; end algorithm",
        "--algorithm A fair process P \\in{1,2} begin L: skip; end process; end algorithm",
        "--algorithm A process P \\in{1,2} begin L:+ skip; end process; end algorithm",
        "--algorithm A process P \\inS begin L: skip; end process; end algorithm",
        "--algorithm A process P =1 variables x=self; begin L: skip; end process; end algorithm",
    ] {
        assert!(
            matches!(translate(source,&TranslationOptions::default()),Err(e) if e.diagnostic.code==Code::Unsupported),
            "{source}"
        );
    }
}
