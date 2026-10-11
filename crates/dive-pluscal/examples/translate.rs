//! Development-only native sequential artifact dumper; no external calls.
use std::io::{Read, Write};
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
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    if std::env::args().nth(1).as_deref() == Some("--tokens") {
        let tokens: Vec<_> = dive_syntax::tokens::tokenize_spec(&source)
            .into_iter()
            .filter(|token| !token.kind.is_trivia())
            .map(|token| {
                source[usize::from(token.range.start())..usize::from(token.range.end())].to_owned()
            })
            .collect();
        writeln!(
            std::io::stdout(),
            "{}",
            serde_json::json!({"tokens":tokens})
        )?;
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--cst") {
        let parsed = dive_syntax::parse_module(&source);
        let document = serde_json::json!({"accepted": parsed.is_ok(), "structure": structure(&parsed.syntax())});
        writeln!(std::io::stdout(), "{document}")?;
        return Ok(());
    }
    let mut options = dive_pluscal::TranslationOptions::default();
    let mut flags = std::env::args().skip(1);
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "-label" | "-reportLabels" => options.insert_missing_labels = true,
            "-labelRoot" => options.label_root = flags.next().ok_or("missing label root")?,
            "-nof" => options.fairness = dive_pluscal::FairnessOption::None,
            "-wf" => options.fairness = dive_pluscal::FairnessOption::Weak,
            "-sf" => options.fairness = dive_pluscal::FairnessOption::Strong,
            "-wfNext" => options.fairness = dive_pluscal::FairnessOption::Next,
            "-termination" => {
                let error = dive_pluscal::Diagnostic {
                    code: dive_pluscal::Code::Unsupported,
                    span: dive_pluscal::ast::Span {
                        start: 0,
                        end: source.len(),
                    },
                    message: "unsupported PlusCal translation option: -termination".into(),
                };
                let document = serde_json::json!({"stage":"Normalization","error":error,"code":error.code.as_str(),"model_proved":false,"exhaustive":false});
                writeln!(std::io::stdout(), "{document}")?;
                return Ok(());
            }
            _ => return Err("unknown development option".into()),
        }
    }
    let marker = source
        .find("--algorithm")
        .or_else(|| source.find("--fair algorithm"));
    let result = if let Some(start) = marker {
        dive_pluscal::translate_at(
            &source,
            start..source.rfind("*)").unwrap_or(source.len()),
            &options,
        )
    } else {
        dive_pluscal::translate(&source, &options)
    };
    let document = match result {
        Ok(artifact) => {
            let tokens: Vec<_> = dive_syntax::tokens::tokenize_spec(&artifact.tla)
                .into_iter()
                .filter(|token| !token.kind.is_trivia())
                .map(|token| {
                    artifact.tla[usize::from(token.range.start())..usize::from(token.range.end())]
                        .to_owned()
                })
                .collect();
            serde_json::json!({"stage":"artifact", "artifact":artifact,"tokens":tokens, "model_proved":false,"exhaustive":false,"accepted":true,"exit_code":0})
        }
        Err(error) => {
            serde_json::json!({"stage":error.stage,"error":error.diagnostic,"code":error.diagnostic.code.as_str(),"model_proved":false,"exhaustive":false})
        }
    };
    writeln!(std::io::stdout(), "{document}")?;
    Ok(())
}
