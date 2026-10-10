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
        .join("procedure-graph-acceptance")
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
fn pinned_nonrecursive_graph_artifacts_and_invalid_output() -> Result<(), Box<dyn std::error::Error>>
{
    retained("", 120, 68, 4)
}
#[test]
fn all_call_graph_cycles_are_refused_even_when_unused() -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("procedure-graph-acceptance/cases");
    for kind in [
        "self_recursive",
        "self_tail",
        "mutual",
        "unused_cycle",
        "conditional_cycle",
    ] {
        for language in ["C", "P"] {
            let source = std::fs::read_to_string(
                base.join(format!("ProcedureGraph_{language}_{kind}.tla")),
            )?;
            let start = source.find("--algorithm").ok_or("marker")?;
            let end = source.rfind("*)").ok_or("comment")?;
            let err = translate_at(&source, start..end, &TranslationOptions::default())
                .err()
                .ok_or("cycle must refuse")?;
            assert_eq!(err.diagnostic.code, Code::Unsupported);
            assert!(
                err.diagnostic
                    .message
                    .contains("recursive procedure call graph")
            );
            assert!(
                source
                    .get(err.diagnostic.span.start..err.diagnostic.span.end)
                    .is_some()
            );
        }
    }
    Ok(())
}
#[test]
fn graph_scope_actions_are_globally_bounded_and_source_is_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure Outer(a=0) variable u=1; begin O: call Inner(a+u); R: return; end procedure; procedure Inner(b=0) variable v=2; begin I: x:=b+v; S: return; end procedure; begin L: call Outer(3); E: skip; end algorithm";
    let parsed = dive_pluscal::parse(source, &dive_pluscal::Options::default())?;
    let before = serde_json::to_value(&parsed)?;
    let artifact = translate(source, &TranslationOptions::default())?;
    assert_eq!(artifact.actions, ["O", "R", "I", "S", "L", "E"]);
    assert!(artifact.tla.contains("procedure |-> \"Inner\""));
    let options = TranslationOptions {
        max_actions: 5,
        ..TranslationOptions::default()
    };
    let err = translate(source, &options)
        .err()
        .ok_or("global action quota must refuse")?;
    assert_eq!(err.diagnostic.code, Code::Resource);
    assert_eq!(before, serde_json::to_value(&parsed)?);
    Ok(())
}
#[test]
fn cross_procedure_call_return_keeps_its_reference_frame_owner()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure Outer(a=0) variable u=1; begin O: call Inner(a+u); return; end procedure; procedure Inner(b=0) variable v=2; begin I: x:=b+v; S: return; end procedure; begin L: call Outer(3); E: skip; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert!(artifact.tla.contains("Tail(stack)"));
    assert!(artifact.tla.contains("u' = Head(stack).u"));
    assert!(!artifact.tla.contains("a' = Head(stack).a /\\"));
    assert_eq!(artifact.actions, ["O", "I", "S", "L", "E"]);
    Ok(())
}
