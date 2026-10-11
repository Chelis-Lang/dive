//! Native syntax-tree evidence adapter; ignores trivia/spans and retains grouping.
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected module path")?;
    let source = std::fs::read_to_string(path)?;
    let parsed = dive_syntax::parse_module(&source);
    println!(
        "{}",
        serde_json::json!({"accepted":parsed.is_ok(),"tree":structure(&parsed.syntax()),"model_proved":false})
    );
    Ok(())
}
