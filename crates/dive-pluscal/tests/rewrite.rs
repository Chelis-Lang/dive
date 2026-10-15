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
    assert!(plan.algorithm_checksum.is_some());
    assert_eq!(
        &source[..plan.span.start],
        &plan.module_source[..plan.span.start]
    );
    assert!(plan.module_source.ends_with("\\* footer λ🙂\n====\n"));
    assert!(plan.module_source.contains("chksum(pcal) = "));
    let again =
        rewrite_document(&plan.module_source, "x == 2\n", &options()).map_err(|e| e.message)?;
    assert!(
        again
            .selection
            .checksum
            .as_ref()
            .is_some_and(|c| c.matched && !c.algorithm_disabled)
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
    assert!(plan.algorithm_checksum.is_some());
    let start = supported.find("--algorithm").ok_or("algorithm")?;
    let end = supported.find("*)").ok_or("comment")?;
    let checksum = dive_pluscal::algorithm_checksum_at(
        &supported,
        start..end,
        &dive_pluscal::TranslationOptions::default(),
    )?;
    assert_eq!(
        plan.algorithm_checksum.as_deref(),
        Some(checksum.checksum.as_str())
    );
    assert_eq!(
        checksum.serialized_ast,
        include_str!(
            "fixtures/checksum-expression/rewrite-boundary-controls/P_elsif-stale.translated.ast"
        )
    );
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/checksum-expression/rewrite-boundary-controls/manifest.json"
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
        "fixtures/checksum-expression/rewrite-boundary-controls/P_elsif-stale.translated.tla"
    ));
    assert!(native.is_ok() && primary.is_ok());
    assert_eq!(structure(&native.syntax()), structure(&primary.syntax()));
    let unused_macro = source.replace("begin L:", "macro M() begin skip; end macro; begin L:");
    let macro_plan = rewrite_document(&unused_macro, "x == 1", &RewriteOptions::default())?;
    let selection = select_rewrite(&unused_macro, &RewriteOptions::default())?;
    let macro_checksum = dive_pluscal::algorithm_checksum_at(
        &unused_macro,
        selection.algorithm.start..selection.algorithm.end,
        &dive_pluscal::TranslationOptions::default(),
    )?;
    assert_eq!(
        macro_plan.algorithm_checksum.as_deref(),
        Some(macro_checksum.checksum.as_str())
    );
    assert_eq!(macro_checksum.checksum, "dbedd2a2");
    assert_eq!(
        macro_checksum.serialized_ast,
        include_str!(
            "fixtures/macro-checksum/regression/primary/UnusedMacroRewrite.translated.ast"
        )
    );
    verify_define_boundary(&source)?;
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

fn verify_define_boundary(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let supported_define = source.replace("begin L:", "define F == 1 end define; begin L:");
    assert_eq!(
        supported_define,
        include_str!("fixtures/define/promoted/cases/DefineBlockRewrite.tla")
    );
    let define_plan = rewrite_document(&supported_define, "x == 1", &RewriteOptions::default())?;
    let selection = select_rewrite(&supported_define, &RewriteOptions::default())?;
    let define_checksum = dive_pluscal::algorithm_checksum_at(
        &supported_define,
        selection.algorithm.start..selection.algorithm.end,
        &dive_pluscal::TranslationOptions::default(),
    )?;
    assert_eq!(
        define_plan.algorithm_checksum.as_deref(),
        Some(define_checksum.checksum.as_str())
    );
    assert_eq!(
        define_checksum.serialized_ast,
        include_str!("fixtures/define/promoted/primary/DefineBlockRewrite.translated.ast")
    );
    let define_manifest: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/define/promoted/primary/manifest.json"
    ))?;
    let primary_define = define_manifest["rows"]
        .as_array()
        .ok_or("missing DEFINE rows")?
        .iter()
        .find(|row| row["id"] == "DefineBlockRewrite")
        .ok_or("missing DEFINE reference")?;
    assert_eq!(
        define_checksum.checksum,
        primary_define["stages"]["translated"]["crc"]
    );
    let selected = select_rewrite(&supported_define, &RewriteOptions::default())?;
    let define_artifact = dive_pluscal::translate_at(
        &supported_define,
        selected.algorithm.start..selected.algorithm.end,
        &dive_pluscal::TranslationOptions::default(),
    )?;
    let generated_define = rewrite_document(
        &supported_define,
        &define_artifact.tla,
        &RewriteOptions::default(),
    )?;
    let native_define = dive_syntax::parse_module(&generated_define.module_source);
    let reference_define = dive_syntax::parse_module(include_str!(
        "fixtures/define/promoted/primary/DefineBlockRewrite.translated.tla"
    ));
    assert!(native_define.is_ok() && reference_define.is_ok());
    assert_eq!(
        structure(&native_define.syntax()),
        structure(&reference_define.syntax())
    );
    let unsupported = source.replace("begin L:", "define F == x' end define; begin L:");
    let rewritten = rewrite_document(&unsupported, "x == 1", &RewriteOptions::default())?;
    assert!(rewritten.algorithm_checksum.is_none());
    assert!(rewritten.module_source.contains("chksum(pcal) \\in STRING"));
    assert!(
        rewritten
            .warnings
            .iter()
            .any(|warning| warning.code == Code::Unsupported)
    );
    Ok(())
}

#[test]
fn resolved_legacy_grammar_bounds_the_rewrite_prefix_without_relaxing_standalone_eof()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE R ----\nEXTENDS Naturals,TLC\n\\* PlusCal options (version 1.5)\n(* --algorithm A variable x=0; fair begin L:x:=1; E:skip; end algorithm; --algorithm B begin L:skip; end algorithm; *)\n====\n";
    let translation = dive_pluscal::TranslationOptions::default();
    let context = dive_pluscal::resolve_translation_options(source, &translation)?;
    let selection = dive_pluscal::rewrite::select_rewrite_with_parser(
        source,
        &RewriteOptions::default(),
        &context.effective.parser,
    )?;
    assert!(source[selection.algorithm.start..selection.algorithm.end].ends_with("end algorithm;"));
    assert!(!source[selection.algorithm.start..selection.algorithm.end].contains("--algorithm B"));
    let full_candidate = dive_pluscal::rewrite::algorithm_comment_region(source, selection.comment)
        .ok_or("missing comment region")?;
    assert!(
        dive_pluscal::parse_at(
            source,
            full_candidate.start..full_candidate.end,
            &context.effective.parser
        )
        .is_err()
    );
    for checksums in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
        let plan = dive_pluscal::rewrite::rewrite_document_with_translation_options(
            source,
            "SuppliedBody == TRUE\n",
            &RewriteOptions {
                checksums,
                ..RewriteOptions::default()
            },
            &translation,
        )?;
        assert_eq!(plan.selection.algorithm, selection.algorithm);
        assert!(plan.module_source.contains("--algorithm B"));
    }
    Ok(())
}
