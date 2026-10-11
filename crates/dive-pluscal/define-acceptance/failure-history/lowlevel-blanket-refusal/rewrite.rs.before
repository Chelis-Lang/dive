//! Body checksum integrity is independent of source editing and model proofs.
use dive_pluscal::{
    Code,
    rewrite::{
        ChecksumPolicy, RewriteOptions, rewrite_document, select_rewrite, translation_checksum,
    },
};
fn source() -> String {
    "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; end algorithm; *) \\* tail\n\\* footer λ🙂\n====\n".to_owned()
}
fn options() -> RewriteOptions {
    RewriteOptions {
        checksums: ChecksumPolicy::TranslationOnly,
        ..RewriteOptions::default()
    }
}
#[test]
fn exact_line_checksum_and_prefix_suffix_replacement() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(translation_checksum("123456789"), "cbf43926");
    assert_eq!(
        translation_checksum("a\nbc\n"),
        translation_checksum("ab\r\nc")
    );
    let source = source();
    let plan = rewrite_document(&source, "x == 1\n", &options()).map_err(|e| e.message)?;
    assert!(!plan.algorithm_checksum_verified);
    assert_eq!(
        &source[..plan.span.start],
        &plan.module_source[..plan.span.start]
    );
    assert!(plan.module_source.ends_with("\\* footer λ🙂\n====\n"));
    assert!(plan.module_source.contains("chksum(pcal) \\in STRING"));
    let again =
        rewrite_document(&plan.module_source, "x == 2\n", &options()).map_err(|e| e.message)?;
    assert!(
        again
            .selection
            .checksum
            .as_ref()
            .is_some_and(|c| c.matched && c.algorithm_disabled)
    );
    Ok(())
}
#[test]
fn modified_missing_full_and_ambiguous_contracts_refuse() -> Result<(), Box<dyn std::error::Error>>
{
    let source = source();
    let supported = source.replace(
        "L:x:=1;",
        "L:if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if;",
    );
    let plan = rewrite_document(&supported, "x == 1", &RewriteOptions::default())
        .map_err(|error| error.message)?;
    assert!(plan.algorithm_checksum_verified);
    let checksum = plan.algorithm_checksum.as_ref().ok_or("missing AST CRC")?;
    assert_eq!(
        checksum.serialized_ast,
        include_str!(
            "../checksum-expression-acceptance/rewrite-boundary-controls/P_elsif-stale.translated.ast"
        )
    );
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../checksum-expression-acceptance/rewrite-boundary-controls/manifest.json"
    ))?;
    assert_eq!(checksum.checksum, manifest["rows"][0]["primary_crc"]);
    let start = supported.find("--algorithm").ok_or("missing algorithm")?;
    let end = supported.find("*)").ok_or("missing comment close")?;
    let artifact = dive_pluscal::translate_at(
        &supported,
        start..end,
        &dive_pluscal::TranslationOptions::default(),
    )?;
    let generated = rewrite_document(&supported, &artifact.tla, &RewriteOptions::default())
        .map_err(|error| error.message)?;
    let native = dive_syntax::parse_module(&generated.module_source);
    let primary = dive_syntax::parse_module(include_str!(
        "../checksum-expression-acceptance/rewrite-boundary-controls/P_elsif-stale.translated.tla"
    ));
    assert!(native.is_ok() && primary.is_ok());
    assert_eq!(structure(&native.syntax()), structure(&primary.syntax()));
    let unused_macro = source.replace("begin L:", "macro M() begin skip; end macro; begin L:");
    let macro_plan = rewrite_document(&unused_macro, "x == 1", &RewriteOptions::default())?;
    let macro_checksum = macro_plan
        .algorithm_checksum
        .ok_or("missing macro checksum")?;
    assert_eq!(macro_checksum.checksum, "dbedd2a2");
    assert_eq!(
        macro_checksum.serialized_ast,
        include_str!(
            "../macro-checksum-acceptance/regression/primary/UnusedMacroRewrite.translated.ast"
        )
    );
    let unsupported = source.replace("begin L:", "define F == 1 end define; begin L:");
    assert!(
        rewrite_document(&unsupported, "x == 1", &RewriteOptions::default())
            .is_err_and(|error| error.code == Code::Unsupported)
    );
    let initial = rewrite_document(&source, "x == 1", &options());
    assert!(initial.is_ok());
    if let Ok(initial) = initial {
        let modified = initial.module_source.replace("x == 1", "x == 2");
        assert!(rewrite_document(&modified, "x == 3", &options()).is_err());
        let forced = RewriteOptions {
            replace_modified: true,
            ..options()
        };
        assert!(rewrite_document(&modified, "x == 3", &forced).is_ok());
        let duplicate = initial.module_source.replace(
            "\\* END TRANSLATION",
            "\\* BEGIN TRANSLATION\n\\* END TRANSLATION",
        );
        assert!(select_rewrite(&duplicate, &options()).is_err());
    }
    let misleading = source.replace(
        "EXTENDS Integers",
        "EXTENDS Integers\nString == \"BEGIN TRANSLATION\"",
    );
    assert!(select_rewrite(&misleading, &options()).is_err_and(|e| e.code == Code::Unsupported));
    let invalid = source.replace(" *) ", " *) Z == 1 ");
    assert!(select_rewrite(&invalid, &options()).is_err());
    Ok(())
}
#[test]
fn source_body_normalization_and_output_limits_are_explicit() {
    let source = source().replace('\n', "\r\n");
    let result = rewrite_document(&source, "x == 1\n", &options());
    assert!(result.is_ok());
    if let Ok(result) = result {
        assert!(result.module_source.contains("\r\n"));
        let normalized = rewrite_document(
            &source,
            "x == 1\n",
            &RewriteOptions {
                normalize_newlines: true,
                ..options()
            },
        );
        assert!(normalized.is_ok());
        if let Ok(normalized) = normalized {
            assert!(!normalized.module_source.contains('\r'));
        }
        for max_bytes in [0, source.len(), result.module_source.len() - 1] {
            let tiny = RewriteOptions {
                max_bytes,
                ..options()
            };
            assert!(
                rewrite_document(&source, "x == 1\n", &tiny)
                    .is_err_and(|e| e.code == Code::Resource)
            );
        }
    }
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
