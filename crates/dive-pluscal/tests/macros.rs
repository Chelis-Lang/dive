//! Native macro substitution controls, with raw and admitted artifacts kept separate.
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
        .join("tests/fixtures/macro")
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
fn pinned_macro_artifacts_rejections_and_uncredited_invalid_output()
-> Result<(), Box<dyn std::error::Error>> {
    retained("", 92, 57, 2)?;
    retained("extra", 48, 30, 3)?;
    retained("process", 32, 26, 6)?;
    retained("scope", 16, 8, 2)?;
    retained("lexical", 26, 16, 0)?;
    retained("origins", 8, 8, 0)
}
#[test]
fn amplification_is_charged_before_output_allocation() {
    let body = "x:=1;".repeat(200);
    let source = format!(
        "--algorithm A variable x=0; macro M() begin {body} end macro; begin {} end algorithm",
        "M();".repeat(200)
    );
    assert!(
        matches!(translate(&source,&TranslationOptions::default()),Err(e) if e.diagnostic.code==Code::Resource)
    );
    let expression = std::iter::repeat_n("1", 2000).collect::<Vec<_>>().join("+");
    let source = format!(
        "--algorithm A variable x=0; macro M() begin x:={expression}; end macro; begin {} end algorithm",
        "M();".repeat(1000)
    );
    assert!(
        matches!(translate(&source,&TranslationOptions::default()),Err(e) if e.diagnostic.code==Code::Resource)
    );
}
#[test]
fn original_ast_and_all_error_spans_remain_source_owned() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "--algorithm A variable x=0; macro M(v) begin v:=2; end macro; begin L: M(x); E: skip; end algorithm";
    let parsed = dive_pluscal::parse(source, &dive_pluscal::Options::default())?;
    assert_eq!(parsed.algorithm.macros.len(), 1);
    let result = translate(source, &TranslationOptions::default())?;
    assert_eq!(result.actions, ["L", "E"]);
    for bad in ["L: M(1)", "L: M(x + 2)", "L: M(x,2)"] {
        let source = source.replace("L: M(x)", bad);
        if let Err(e) = translate(&source, &TranslationOptions::default()) {
            assert!(
                source
                    .get(e.diagnostic.span.start..e.diagnostic.span.end)
                    .is_some()
            );
        }
    }
    Ok(())
}
#[test]
fn unchanged_scopes_do_not_hide_macro_free_process_names() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "--algorithm A variable x=0; macro M(v) begin v:=w; end macro; process P=1 variable z=0; begin L: M(z); end process; process Q=2 variable w=0; begin K: skip; end process; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert!(
        words(&artifact.tla)
            .windows(4)
            .any(|w| w == ["z", "'", "=", "w"])
    );
    Ok(())
}

#[test]
fn repeated_large_actual_is_bounded_before_clone_amplification() {
    let body = std::iter::repeat_n("a", 1000).collect::<Vec<_>>().join(",");
    let actual = "s".repeat(10_000);
    let source = format!(
        "--algorithm A variable x=0; macro M(a) begin x:=<<{body}>>; end macro; begin L: M(\"{actual}\"); end algorithm"
    );
    assert!(
        matches!(translate(&source,&TranslationOptions::default()),Err(e) if e.diagnostic.code==Code::Resource && e.diagnostic.message.contains("before token allocation"))
    );
}
