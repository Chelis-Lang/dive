//! Complete reference syntax trees keep generated CASE assignment scope exact.
use dive_pluscal::{TranslationOptions, translate_at};
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
fn case_arrows_preserve_assignment_scope_in_both_syntaxes() -> Result<(), Box<dyn std::error::Error>>
{
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("ast-checksum-acceptance/expression-controls");
    for id in ["C_expression_case", "P_expression_case"] {
        let source = std::fs::read_to_string(base.join(format!("{id}.tla")))?;
        let start = source.find("--algorithm").ok_or("missing algorithm")?;
        let end = source.rfind("*)").ok_or("missing algorithm comment end")?;
        let artifact = translate_at(&source, start..end, &TranslationOptions::default())?;
        let terminator = source.rfind("====").ok_or("missing module terminator")?;
        let native = format!(
            "{}{}{}",
            &source[..terminator],
            artifact.tla,
            &source[terminator..]
        );
        let primary = std::fs::read_to_string(base.join(format!("{id}.translated.tla")))?;
        let native = dive_syntax::parse_module(&native);
        let primary = dive_syntax::parse_module(&primary);
        assert!(native.is_ok() && primary.is_ok(), "{id}");
        assert_eq!(
            structure(&native.syntax()),
            structure(&primary.syntax()),
            "{id}: CASE must not absorb following action conjuncts"
        );
    }
    Ok(())
}
