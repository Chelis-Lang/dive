//! Shared procedure actions and individually owned process calls.
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
        .join("multiple-procedure-acceptance")
        .join(directory);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("primary/manifest.json"))?)?;
    let rows = manifest["rows"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), selected);
    let mut matched = 0;
    let mut option_refused = 0;
    let mut raw_invalid = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("id")?;
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("source")?),
        )?;
        if row["options"]
            .as_array()
            .ok_or("flags")?
            .iter()
            .any(|v| v == "-termination")
        {
            option_refused += 1;
            continue; // The development/CLI option boundary is exercised separately in the retained comparator.
        }
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
        let start = source
            .find("--algorithm")
            .or_else(|| source.find("--fair algorithm"))
            .ok_or("marker")?;
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
    assert_eq!(option_refused, 2);
    assert_eq!(matched, artifacts);
    assert_eq!(raw_invalid, invalid);
    Ok(())
}
#[test]
fn retained_multiple_process_procedure_artifacts_match() -> Result<(), Box<dyn std::error::Error>> {
    retained("", 166, 94, 6)
}
#[test]
fn shared_actions_and_storage_are_not_multiplied_by_process_count()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; procedure P(a=0) begin F:x:=a; R:return; end procedure; process One \\in {2} begin L:call P(1); E:skip; end process; process Two=1 begin M:call P(2); N:skip; end process; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert_eq!(artifact.actions, ["F", "R", "L", "E", "M", "N"]);
    assert_eq!(artifact.tla.matches("a = [self").count(), 1);
    assert_eq!(artifact.tla.matches("P(self) ==").count(), 1);
    assert!(artifact.tla.contains(
        "Next == Two \\/ (\\E self \\in ProcSet: P(self)) \\/ (\\E self \\in { 2 }: One(self))"
    ));
    assert!(artifact.tla.contains("a' = [a EXCEPT ![1] = 2]"));
    let options = TranslationOptions {
        max_actions: 5,
        ..TranslationOptions::default()
    };
    assert_eq!(
        translate(source, &options)
            .err()
            .ok_or("bound")?
            .diagnostic
            .code,
        Code::Resource
    );
    Ok(())
}
#[test]
fn scopes_fairness_and_overlapping_domains_remain_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    for source in [
        "--algorithm A variable x=0; procedure P(a=0) begin F:x:=a; R:return; end procedure; process One=1 begin L:call P(1); E:skip; end process; process Two=1 begin M:call P(2); N:skip; end process; end algorithm",
        "--algorithm A variable x=0; procedure P(a=0) begin F:x:=a; R:return; end procedure; fair process One=1 begin L:call P(1); E:skip; end process; process Two=2 begin M:call P(2); N:skip; end process; end algorithm",
        "--algorithm A variable x=0; procedure P(a=0) begin F:x:=y; R:return; end procedure; process One=1 variable y=0; begin L:call P(1); E:skip; end process; process Two=2 begin M:call P(2); N:skip; end process; end algorithm",
    ] {
        let error = translate(source, &TranslationOptions::default())
            .err()
            .ok_or("refusal")?;
        assert_eq!(error.diagnostic.code, Code::Unsupported);
        assert!(
            source
                .get(error.diagnostic.span.start..error.diagnostic.span.end)
                .is_some()
        );
    }
    Ok(())
}
