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
fn modified_missing_full_and_ambiguous_contracts_refuse() {
    let source = source();
    assert!(rewrite_document(&source, "x == 1", &RewriteOptions::default()).is_err());
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
