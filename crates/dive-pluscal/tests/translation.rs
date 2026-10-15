//! Offline pinned translator controls; all selected rows remain accounted for.
use dive_pluscal::{Code, TranslationOptions, translate, translate_at};
use std::path::Path;
fn words(source: &str) -> Vec<String> {
    dive_syntax::tokens::tokenize_spec(source)
        .into_iter()
        .filter(|token| !token.kind.is_trivia())
        .map(|token| {
            source[usize::from(token.range.start())..usize::from(token.range.end())].into()
        })
        .collect()
}
fn structure(node: &dive_syntax::SyntaxNode) -> serde_json::Value {
    let children: Vec<_> = node
        .children_with_tokens()
        .filter_map(|child| {
            if let Some(node) = child.clone().into_node() {
                Some(structure(&node))
            } else {
                child
                    .into_token()
                    .filter(|token| !token.kind().is_trivia())
                    .map(|token| serde_json::json!([format!("{:?}", token.kind()), token.text()]))
            }
        })
        .collect();
    serde_json::json!([format!("{:?}", node.kind()), children])
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "keeps the complete pinned selection and source/token/CST checks together"
)]
fn retained_primary_token_streams() -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/translation");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let rows = manifest["rows"]
        .as_array()
        .ok_or("missing retained selection")?;
    assert_eq!(rows.len(), 160);
    let mut artifacts = 0;
    let mut c_artifacts = 0;
    let mut raw_invalid = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing case id")?;
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("missing input")?),
        )?;
        let start = source
            .find("--algorithm")
            .or_else(|| source.find("--fair algorithm"))
            .ok_or("missing marker")?;
        let end = source.rfind("*)").ok_or("missing comment end")?;
        let result = translate_at(&source, start..end, &TranslationOptions::default());
        match result {
            Err(error) if error.diagnostic.code == Code::Unsupported => {}
            Err(error) => {
                assert_ne!(error.diagnostic.code, Code::Internal, "{id}");
                assert!(
                    row["primary"]["status"] == "rejected" || row["sany"]["status"] == "rejected",
                    "{id}: {error}"
                );
            }
            Ok(artifact) => {
                assert_eq!(row["primary"]["status"], "translated", "{id}");
                let primary = std::fs::read_to_string(
                    base.join("fixtures").join(
                        row["translated"]["path"]
                            .as_str()
                            .ok_or("missing artifact")?,
                    ),
                )?;
                let lines: Vec<_> = primary
                    .lines()
                    .skip_while(|line| !line.contains("BEGIN TRANSLATION"))
                    .skip(1)
                    .take_while(|line| !line.contains("END TRANSLATION"))
                    .collect();
                assert_eq!(words(&artifact.tla), words(&lines.join("\n")), "{id}");
                let terminator = source.rfind("====").ok_or("missing module end")?;
                let native = format!(
                    "{}{}{}",
                    &source[..terminator],
                    artifact.tla,
                    &source[terminator..]
                );
                let native = dive_syntax::parse_module(&native);
                let reference = dive_syntax::parse_module(&primary);
                assert!(native.is_ok() && reference.is_ok(), "{id}");
                assert_eq!(
                    structure(&native.syntax()),
                    structure(&reference.syntax()),
                    "operator grouping/layout: {id}"
                );

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
                    artifacts += 1;
                    if id.contains("c_current_") || id.contains("c_complete_") {
                        c_artifacts += 1;
                    }
                } else {
                    assert_eq!(row["sany"]["status"], "rejected", "{id}");
                    raw_invalid += 1;
                }
            }
        }
    }
    assert_eq!(
        raw_invalid, 3,
        "primary translations rejected by SANY earn no credit"
    );
    assert_eq!(artifacts, 92);
    assert!(
        artifacts >= 40,
        "both surface syntaxes need positive retained controls"
    );
    assert!(
        c_artifacts >= 14,
        "C coverage cannot come only from rejection controls"
    );
    Ok(())
}
#[test]
fn atomic_reads_and_parallel_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variables x=0,y=0; begin L: x:=1; y:=x; E: skip; end algorithm";
    let result = translate(source, &TranslationOptions::default())?;
    assert!(
        words(&result.tla)
            .windows(5)
            .any(|window| window == ["y", "'", "=", "x", "'"])
    );
    let source = "--algorithm A variables x=1,y=2; begin L: x:=y || y:=x; E: skip; end algorithm";
    let result = translate(source, &TranslationOptions::default())?;
    assert!(result.tla.contains("y' = x"));
    assert!(!result.tla.contains("y' = x'"));
    Ok(())
}
#[test]
fn resource_failures_return_no_artifact() {
    let source = "--algorithm A variable x=0; begin L: x:=1; E: skip; end algorithm";
    for options in [
        TranslationOptions {
            max_actions: 0,
            ..TranslationOptions::default()
        },
        TranslationOptions {
            max_output_bytes: 1,
            ..TranslationOptions::default()
        },
    ] {
        let result = translate(source, &options);
        assert!(matches!(result,Err(error) if error.diagnostic.code==Code::Resource));
    }
}
#[test]
fn reserved_names_and_bound_expressions_follow_the_primary()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable Next=0; begin L: Next:=1; end algorithm";
    assert!(
        matches!(translate(source, &TranslationOptions::default()),Err(error) if error.diagnostic.code == Code::Unsupported)
    );
    for source in [
        "--algorithm A variable x=0; begin L: with pc=1 do skip; end with; end algorithm",
        "--algorithm A variable x=0; begin L: print CHOOSE k \\in {1}: TRUE; end algorithm",
    ] {
        let artifact = translate(source, &TranslationOptions::default())?;
        assert_eq!(artifact.actions, ["L"]);
    }
    Ok(())
}

#[test]
fn source_stack_and_invalid_ranges_are_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin L: x:=1; E: skip; end algorithm".to_owned();
    let worker = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || translate(&source, &TranslationOptions::default()))?;
    let artifact = worker.join().map_err(|_| "caller stack failed")??;
    assert_eq!(artifact.actions, ["L", "E"]);
    let source = "α --algorithm A begin L: skip; end algorithm";
    assert!(
        matches!(translate_at(source,1..source.len(),&TranslationOptions::default()),Err(error) if error.diagnostic.code==Code::Syntax)
    );
    Ok(())
}

#[test]
fn flat_elsif_cannot_create_unbounded_normalized_depth() {
    let source = format!(
        "--algorithm A variable x=0; begin L: if TRUE then skip; {} end if; E: skip; end algorithm",
        "elsif FALSE then skip; ".repeat(150)
    );
    assert!(
        matches!(translate(&source,&TranslationOptions::default()),Err(error) if error.diagnostic.code==Code::Resource && error.diagnostic.message.contains("normalized control-flow depth"))
    );
}

#[test]
fn labelled_assertion_uses_keyword_location() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin L: assert x=0; end algorithm";
    let result = translate(source, &TranslationOptions::default())?;
    assert!(result.tla.contains("line 1, column 38."));
    Ok(())
}
#[test]
fn unicode_binders_preserve_bound_names_and_prime_state() -> Result<(), Box<dyn std::error::Error>>
{
    for expression in ["∀ k ∈ {1}: k=x", "∃ k ∈ {1}: k=x", "[k ∈ {1} ↦ k+x]"] {
        let source =
            format!("--algorithm A variable x=0; begin L: x:=1; print {expression}; end algorithm");
        let artifact = translate(&source, &TranslationOptions::default())?;
        let tokens = words(&artifact.tla);
        assert!(tokens.windows(2).any(|w| w == ["x", "'"]));
        assert!(!tokens.windows(2).any(|w| w == ["k", "'"]));
    }
    Ok(())
}

#[test]
fn quoted_done_is_the_terminal_label() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin L: x:=1; goto \"Done\"; end algorithm";
    let result = translate(source, &TranslationOptions::default())?;
    assert!(result.tla.contains("pc' = \"Done\""));
    assert!(!result.tla.contains("\\\"Done"));
    Ok(())
}

#[test]
fn sequential_label_modifiers_force_pc_without_inventing_fairness()
-> Result<(), Box<dyn std::error::Error>> {
    for modifier in ["+", "-"] {
        let source = format!(
            "--algorithm A variable x=0; begin L:{modifier} while TRUE do x:=1; end while; end algorithm"
        );
        let result = translate(&source, &TranslationOptions::default())?;
        assert!(result.program_counter);
        assert!(!result.tla.contains("WF_vars") && !result.tla.contains("SF_vars"));
    }
    Ok(())
}
