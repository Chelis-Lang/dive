//! Whole-module grouping remains distinct from the original algorithm CRC.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions,
    rewrite::{RewriteOptions, select_rewrite},
    translate_at,
};
use std::path::Path;

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
fn all_original_closure_modules_keep_pinned_next_grouping() -> Result<(), Box<dyn std::error::Error>>
{
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/next-procedure-grouping");
    let selection: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("selection.json"))?)?;
    let rows = selection["rows"]
        .as_array()
        .ok_or("missing original selection")?;
    assert_eq!(rows.len(), 16);
    let mut accepted = 0;
    let mut refused = 0;
    let mut grouping_mutants = 0;
    for row in rows {
        let id = row["id"].as_str().ok_or("missing original source id")?;
        let source = std::fs::read_to_string(base.join(format!("{id}.tla")))?;
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        for fairness in [FairnessOption::Declared, FairnessOption::None] {
            let options = TranslationOptions {
                fairness,
                ..TranslationOptions::default()
            };
            let result = translate_at(
                &source,
                selected.algorithm.start..selected.algorithm.end,
                &options,
            );
            if row["primary_module_sha256"].is_null() {
                let failure = result.err().ok_or("invalid primary received an artifact")?;
                assert_ne!(failure.diagnostic.code, Code::Internal, "{id}");
                assert!(!failure.diagnostic.message.is_empty(), "{id}");
                assert!(
                    source
                        .get(failure.diagnostic.span.start..failure.diagnostic.span.end)
                        .is_some(),
                    "{id}"
                );
                refused += 1;
                continue;
            }
            let artifact = result?;
            let end = source.rfind("====").ok_or("missing module ending")?;
            let generated = format!("{}\n{}\n====\n", &source[..end], artifact.tla);
            let primary = std::fs::read_to_string(base.join(format!("{id}.primary.tla")))?;
            let left = dive_syntax::parse_module(&generated);
            let right = dive_syntax::parse_module(&primary);
            assert!(left.is_ok(), "{id}");
            assert!(right.is_ok(), "{id}");
            assert_eq!(
                structure(&left.syntax()),
                structure(&right.syntax()),
                "{id}"
            );
            if id.contains("diamond") {
                // This reproduces the old missing DISJ_LIST marker. Token-only
                // equality/algorithm CRC cannot establish this module contract.
                let begin = generated
                    .find("(\\E self \\in ProcSet: ")
                    .ok_or("missing procedure quantifier")?;
                let end = begin
                    + generated[begin..]
                        .find("))")
                        .ok_or("missing quantifier ending")?
                    + 2;
                let original = &generated[begin..end];
                let missing_list = original.replacen(":  \\/ ", ": ", 1);
                assert_ne!(missing_list, original, "{id}");
                let collapsed = missing_list
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ");
                let altered = generated.replacen(original, &collapsed, 1);
                let mutant = dive_syntax::parse_module(&altered);
                assert!(mutant.is_ok(), "{id}");
                assert_ne!(
                    structure(&mutant.syntax()),
                    structure(&right.syntax()),
                    "{id}"
                );
                grouping_mutants += 1;
            }
            accepted += 1;
        }
    }
    assert_eq!(accepted, 28);
    assert_eq!(refused, 4);
    assert_eq!(grouping_mutants, 8);
    Ok(())
}
