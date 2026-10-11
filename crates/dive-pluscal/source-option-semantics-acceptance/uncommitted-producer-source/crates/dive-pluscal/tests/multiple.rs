//! Source-owned multi-process storage and action controls; no model-proof claims.
use dive_pluscal::{Code, FairnessOption, TranslationOptions, translate_at};
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
        .join("multiple-acceptance")
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
                "-nof" => options.fairness = FairnessOption::None,
                "-wf" => options.fairness = FairnessOption::Weak,
                "-sf" => options.fairness = FairnessOption::Strong,
                "-wfNext" => options.fairness = FairnessOption::Next,
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
fn retained_multiple_process_artifacts_and_errors() -> Result<(), Box<dyn std::error::Error>> {
    retained_selection("", 108, 92)?;
    retained_selection("extra", 144, 106)
}
#[test]
fn limits_charge_all_process_actions_and_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A process P=1 variable x=0; begin L: x:=1; end process; process Q=2 variable y=0; begin M: y:=1; end process; end algorithm";
    let translated = dive_pluscal::translate(source, &TranslationOptions::default())?;
    assert_eq!(translated.processes.len(), 2);
    assert_eq!(translated.actions, ["L", "M"]);
    for options in [
        TranslationOptions {
            max_actions: 0,
            ..TranslationOptions::default()
        },
        TranslationOptions {
            max_actions: 1,
            ..TranslationOptions::default()
        },
        TranslationOptions {
            max_output_bytes: 1,
            ..TranslationOptions::default()
        },
    ] {
        assert!(
            matches!(dive_pluscal::translate(source,&options),Err(e) if e.diagnostic.code==Code::Resource)
        );
    }
    Ok(())
}

#[test]
fn aggregate_budget_fails_before_combining_parts() {
    let source = "--algorithm A variable g=0; process P=1 begin L: g:=1; E: g:=2; end process; process Q=2 begin M: g:=3; F: g:=4; end process; end algorithm";
    let options = TranslationOptions {
        max_actions: 3,
        ..TranslationOptions::default()
    };
    let result = dive_pluscal::translate(source, &options);
    assert!(
        matches!(result,Err(e) if e.stage==dive_pluscal::TranslationStage::Normalization && e.diagnostic.code==Code::Resource && e.diagnostic.message.contains("across all processes"))
    );
}
