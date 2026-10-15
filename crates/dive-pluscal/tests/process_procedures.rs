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
fn contains_words(source: &str, expression: &str) -> bool {
    let expected = words(expression);
    words(source)
        .windows(expected.len())
        .any(|window| window == expected)
}

fn retained(
    directory: &str,
    selected: usize,
    artifacts: usize,
    invalid: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/process-procedure")
        .join(directory);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("primary/manifest.json"))?)?;
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
                    base.join("primary")
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
fn all_single_process_procedure_artifacts_match_the_pin() -> Result<(), Box<dyn std::error::Error>>
{
    retained("", 112, 84, 8)?;
    retained("extra", 12, 12, 0)
}
#[test]
fn process_frames_are_owned_by_id_and_self_tail_keeps_the_frame()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure Recur(a=0,c=0) variable u=a+c; begin F: call Recur(a-1,a); return; end procedure; process Worker=1 begin L: call Recur(2,3); E: skip; end process; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert!(
        artifact
            .tla
            .contains("stack = [self \\in ProcSet |-> << >>]")
    );
    assert!(contains_words(&artifact.tla, "a' = [a EXCEPT ![1] = 2]"));
    let action = artifact
        .tla
        .split("F(self) ==")
        .nth(1)
        .ok_or("F")?
        .split("Recur(self) ==")
        .next()
        .ok_or("wrapper")?;
    assert!(contains_words(
        action,
        "a' = [a EXCEPT ![self] = a[self] - 1]"
    ));
    assert!(contains_words(action, "c' = [c EXCEPT ![self] = a[self]]"));
    assert!(contains_words(
        action,
        "u' = [u EXCEPT ![self] = a'[self] + c'[self]]"
    ));
    assert!(!action.contains("Head(stack"));
    assert!(!action.contains("Tail(stack"));
    assert!(contains_words(&artifact.tla, "Recur(self)"));
    Ok(())
}
#[test]
fn process_procedure_normalization_is_globally_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure P(a=0) begin F:if a>0 then call P(a-1); else x:=a; end if; R:return; end procedure; process Worker \\in {1,2} begin L:call P(1000000); E:skip; end process; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert_eq!(artifact.actions, ["F", "R", "L", "E"]);
    let options = TranslationOptions {
        max_actions: 3,
        ..TranslationOptions::default()
    };
    assert_eq!(
        translate(source, &options)
            .err()
            .ok_or("action bound")?
            .diagnostic
            .code,
        Code::Resource
    );
    let options = TranslationOptions {
        max_output_bytes: 16,
        ..TranslationOptions::default()
    };
    assert_eq!(
        translate(source, &options)
            .err()
            .ok_or("output bound")?
            .diagnostic
            .code,
        Code::Resource
    );
    Ok(())
}
#[test]
fn explicit_stack_expression_is_not_implicitly_indexed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure P(a=0) begin F:x:=Len(stack[self]); R:return; end procedure; process Worker=1 begin L:call P(1); E:skip; end process; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert!(contains_words(&artifact.tla, "x' = Len ( stack [ self ] )"));
    assert!(!artifact.tla.contains("stack[self] [ self ]"));
    Ok(())
}
