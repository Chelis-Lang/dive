//! Offline pinned grammar and AST controls; no Java process in the gate.
//! `tests/fixtures/parser-oracle/` holds the inputs and the pinned TLC source-grammar
//! AST projections; its `compare.py` projects dive's AST and compares.
// Test fixtures and subprocess protocol errors should fail the control visibly.
#![allow(clippy::expect_used)]
use dive_pluscal::{Options, parse};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn pinned_source_grammar_and_ast_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parser-oracle");
    let mut cases = BTreeMap::new();
    for inputs in ["curated", "edges", "imported"] {
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(root.join(inputs).join("manifest.json")).expect("manifest"),
        )
        .expect("JSON");
        for row in manifest["rows"].as_array().expect("selection") {
            let name = row["input"].as_str().expect("input");
            let id = row["id"].as_str().expect("id");
            let source = std::fs::read_to_string(root.join(inputs).join(name)).expect("fixture");
            let result = match parse(&source, &Options::default()) {
                Ok(ast) => {
                    json!({"stage": "source-grammar", "translation": "unsupported: pending", "ast": ast})
                }
                Err(error) => {
                    json!({"stage": "source-grammar", "code": error.code.as_str(), "error": error})
                }
            };
            assert!(
                cases.insert(id.to_owned(), result).is_none(),
                "no duplicate case"
            );
        }
    }
    let mut child = Command::new("python3")
        .arg("-I")
        .arg(root.join("compare.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python oracle comparison");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&serde_json::to_vec(&cases).expect("serialized ASTs"))
        .expect("request");
    let output = child.wait_with_output().expect("comparison");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("control report");
    assert_eq!(report["summary"]["source-ast-match"], 200);
    assert_eq!(report["summary"]["grammar-rejection-match"], 13);
    assert_eq!(report["summary"]["reference-stage-pending"], 4);
    assert_eq!(report["summary"]["oracle-error"], 1);
    assert_eq!(report["false_agreement_mutations"], 10);
}
