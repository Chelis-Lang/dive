//! Reference-backed multiline admission is distinct from AST/checksum evidence.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
use std::path::Path;
#[test]
fn complete_selected_multiline_artifacts_match_primary_trees()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-pluscal/multiline-acceptance");
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("full-controls/report.json"),
    )?)?;
    let mut matched = 0;
    let rows = report["rows"]
        .as_array()
        .ok_or("missing complete selection")?;
    assert_eq!(rows.len(), 208);
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        let (folder, name) = id.split_once("__").unwrap_or(("primary", id));
        let directory = base.join(folder);
        let source = std::fs::read_to_string(directory.join(format!("{name}.tla")))?;
        let request = RewriteRequest::default();
        let module_name = name.replace('-', "_");
        let actual = rewrite_source(&format!("{module_name}.tla"), &source, &request);
        let document = actual.to_json();
        assert_eq!(document["model_proved"], false, "{id}");
        assert_eq!(document["exhaustive"], false, "{id}");
        let outcome = row["dive"]["outcome"]
            .as_str()
            .ok_or("missing disposition")?;
        if outcome == "full-derivation-and-admission" {
            assert!(actual.accepted(), "{id}: {document}");
            assert_eq!(document["algorithm_checksum_verified"], true);
            let module = document["replacement"]["module_source"]
                .as_str()
                .ok_or("missing module")?;
            let primary =
                std::fs::read_to_string(directory.join(format!("{name}.translated.tla")))?;
            let native = dive_syntax::parse_module(module);
            let oracle = dive_syntax::parse_module(&primary);
            assert!(native.is_ok() && oracle.is_ok(), "{id}");
            assert_eq!(
                structure(&native.syntax()),
                structure(&oracle.syntax()),
                "{id}"
            );
            let expected =
                std::fs::read_to_string(directory.join(format!("{name}.translated.ast")))?;
            assert_eq!(
                document["replacement"]["algorithm_checksum"]["serialized_ast"], expected,
                "{id}"
            );
            matched += 1;
        } else {
            assert!(!actual.accepted(), "{id}: {document}");
            assert_eq!(
                actual.exit_code(),
                if outcome == "named-refusal" { 3 } else { 1 },
                "{id}"
            );
        }
    }
    assert_eq!(matched, 130);
    Ok(())
}
#[test]
fn multiline_scope_and_lowered_budgets_never_admit_partial_output() {
    let source = "---- MODULE B ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1+\n 2; E:skip; end algorithm; *)\n====\n";
    for partial in [false, true] {
        let mut request = RewriteRequest::default();
        if partial {
            request.editing.checksums = ChecksumPolicy::TranslationOnly;
        }
        assert!(rewrite_source("B.tla", source, &request).accepted());
        for limit in [0, 1, 8] {
            let mut bounded = request.clone();
            bounded.translation.limits.max_output_bytes = limit;
            let report = rewrite_source("B.tla", source, &bounded);
            assert_eq!(report.exit_code(), 3);
            assert!(!report.accepted());
            assert_eq!(report.to_json()["model_proved"], false);
            let mut bounded = request.clone();
            bounded.editing.max_bytes = limit;
            assert_eq!(rewrite_source("B.tla", source, &bounded).exit_code(), 3);
        }
    }
    let cr = source.replace("+\n", "+\r");
    assert_eq!(
        rewrite_source("B.tla", &cr, &RewriteRequest::default()).exit_code(),
        3
    );
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
