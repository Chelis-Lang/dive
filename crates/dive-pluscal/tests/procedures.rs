//! Native procedure call/return controls, with raw and admitted artifacts kept separate.
use dive_pluscal::{Code, FairnessOption, TranslationOptions, translate, translate_at};
use std::path::Path;
fn words(source: &str) -> Vec<String> {
    dive_syntax::tokens::tokenize_spec(source)
        .into_iter()
        .filter(|t| !t.kind.is_trivia())
        .map(|t| source[usize::from(t.range.start())..usize::from(t.range.end())].into())
        .collect()
}
fn retained(
    directory: &str,
    selected: usize,
    artifacts: usize,
    invalid: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/procedure")
        .join(directory);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let rows = manifest["rows"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), selected);
    let mut matched = 0;
    let mut raw_invalid = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("id")?;
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("source")?),
        )?;
        let mut options = TranslationOptions::default();
        for flag in row["options"].as_array().ok_or("flags")? {
            match flag.as_str().ok_or("flag")? {
                "-label" => options.insert_missing_labels = true,
                "-wf" => options.fairness = FairnessOption::Weak,
                "-nof" => options.fairness = FairnessOption::None,
                "-sf" => options.fairness = FairnessOption::Strong,
                "-wfNext" => options.fairness = FairnessOption::Next,
                _ => return Err("unrecognized retained option".into()),
            }
        }
        let start = source.find("--algorithm").ok_or("marker")?;
        let end = source.rfind("*)").ok_or("comment")?;
        match translate_at(&source, start..end, &options) {
            Err(e) if matches!(e.diagnostic.code, Code::Unsupported | Code::Resource) => {}
            Err(e) => {
                assert_ne!(e.diagnostic.code, Code::Internal, "{id}");
                assert!(
                    row["primary"]["status"] == "rejected" || row["sany"]["status"] == "rejected",
                    "{id}: {e}"
                );
                assert!(
                    source
                        .get(e.diagnostic.span.start..e.diagnostic.span.end)
                        .is_some(),
                    "{id}"
                );
            }
            Ok(artifact) => {
                assert_eq!(row["primary"]["status"], "translated", "{id}");
                let primary = std::fs::read_to_string(
                    base.join("fixtures")
                        .join(row["translated"]["path"].as_str().ok_or("artifact")?),
                )?;
                let body = primary
                    .lines()
                    .skip_while(|l| !l.contains("BEGIN TRANSLATION"))
                    .skip(1)
                    .take_while(|l| !l.contains("END TRANSLATION"))
                    .collect::<Vec<_>>()
                    .join("\n");
                assert_eq!(words(&artifact.tla), words(&body), "{id}");
                for mapping in &artifact.mappings {
                    assert!(
                        source
                            .get(mapping.original.start..mapping.original.end)
                            .is_some(),
                        "{id}"
                    );
                    assert!(
                        artifact
                            .tla
                            .get(mapping.generated.start..mapping.generated.end)
                            .is_some(),
                        "{id}"
                    );
                }
                if row["sany"]["status"] == "accepted" {
                    matched += 1;
                } else {
                    raw_invalid += 1;
                }
            }
        }
    }
    assert_eq!(matched, artifacts);
    assert_eq!(raw_invalid, invalid);
    Ok(())
}
#[test]
fn pinned_procedure_artifacts_rejections_and_uncredited_invalid_output()
-> Result<(), Box<dyn std::error::Error>> {
    retained("", 84, 52, 2)
}
#[test]
fn call_and_return_do_not_mutate_the_source_ast_or_exceed_action_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure Put(a=0) begin P: x:=a; R: return; end procedure; begin L: call Put(2); E: skip; end algorithm";
    let parsed = dive_pluscal::parse(source, &dive_pluscal::Options::default())?;
    assert_eq!(parsed.algorithm.procedures.len(), 1);
    let artifact = translate(source, &TranslationOptions::default())?;
    assert_eq!(artifact.actions, ["P", "R", "L", "E"]);
    assert!(artifact.tla.contains("pc' = Head(stack).pc"));
    let options = TranslationOptions {
        max_actions: 3,
        ..TranslationOptions::default()
    };
    let failure = translate(source, &options)
        .err()
        .ok_or("must refuse resource")?;
    assert_eq!(failure.diagnostic.code, Code::Resource);
    assert!(
        source
            .get(failure.diagnostic.span.start..failure.diagnostic.span.end)
            .is_some()
    );
    Ok(())
}
