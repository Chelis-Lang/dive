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
        .join("tests/fixtures/recursive-procedure")
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
fn all_recursive_source_artifacts_match_the_pin_without_crediting_invalid_output()
-> Result<(), Box<dyn std::error::Error>> {
    retained("", 116, 86, 4)
}
#[test]
fn recursive_translation_is_source_bounded_and_keeps_runtime_frames()
-> Result<(), Box<dyn std::error::Error>> {
    let make = |depth| {
        format!(
            "--algorithm A variable x=0; procedure Recur(a=0) variable u=a+1; begin F: if a>0 then call Recur(a-1); else x:=u; end if; R: return; end procedure; begin L: call Recur({depth}); E: skip; end algorithm"
        )
    };
    let small = translate(&make(2), &TranslationOptions::default())?;
    let large = translate(&make(1_000_000), &TranslationOptions::default())?;
    assert_eq!(small.actions, ["F", "R", "L", "E"]);
    assert_eq!(small.actions, large.actions);
    assert!(large.tla.len() < small.tla.len() + 32);
    let source = make(2);
    let parsed = dive_pluscal::parse(&source, &dive_pluscal::Options::default())?;
    let before = serde_json::to_value(&parsed)?;
    let options = TranslationOptions {
        max_actions: 3,
        ..TranslationOptions::default()
    };
    let refusal = translate(&source, &options)
        .err()
        .ok_or("global budget must refuse")?;
    assert_eq!(refusal.diagnostic.code, Code::Resource);
    assert_eq!(before, serde_json::to_value(&parsed)?);
    Ok(())
}
#[test]
fn self_tail_call_keeps_frame_and_uses_simultaneous_parameter_values()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure Recur(a=0,c=0) variable u=a+c; begin F: call Recur(a-1,a); return; end procedure; begin L: call Recur(2,3); E: skip; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    let action = artifact
        .tla
        .split("F ==")
        .nth(1)
        .ok_or("F")?
        .split("Recur ==")
        .next()
        .ok_or("wrapper")?;
    assert!(action.contains("a' = a - 1"));
    assert!(action.contains("c' = a"));
    assert!(action.contains("u' = a' + c'"));
    assert!(action.contains("UNCHANGED << x, stack >>"));
    assert!(!action.contains("Head(stack)"));
    assert!(!action.contains("Tail(stack)"));
    Ok(())
}
#[test]
fn zero_parameter_self_tail_and_double_assignment_are_never_silent_repairs()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure Recur() begin F: call Recur(); return; end procedure; begin L: call Recur(); E: skip; end algorithm";
    let rejection = translate(source, &TranslationOptions::default())
        .err()
        .ok_or("invalid generated conjunction must reject")?;
    assert_eq!(rejection.diagnostic.code, Code::Expression);
    let source = "--algorithm A variable x=0; procedure Recur(a=0) begin F: a:=a-1; call Recur(a); return; end procedure; begin L: call Recur(2); E: skip; end algorithm";
    let rejection = translate(source, &TranslationOptions::default())
        .err()
        .ok_or("repeated parameter assignment must reject")?;
    assert_eq!(rejection.diagnostic.code, Code::Label);
    Ok(())
}
