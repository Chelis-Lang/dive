//! Development-only source AST dumper; no translator or external oracle calls.
use std::io::{Read, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let result = dive_pluscal::parse(&source, &dive_pluscal::Options::default());
    let document = match result {
        Ok(ast) => {
            serde_json::json!({"stage": "source-grammar", "translation": "unsupported: pending", "ast": ast})
        }
        Err(error) => {
            serde_json::json!({"stage": "source-grammar", "error": error, "code": error.code.as_str()})
        }
    };
    writeln!(std::io::stdout(), "{document}")?;
    Ok(())
}
