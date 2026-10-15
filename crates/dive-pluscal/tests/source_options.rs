//! Source context is applied exactly once and partial reference errors are refused.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests require explicit successful setup"
)]
use dive_pluscal::{Code, FairnessOption, TranslationOptions, resolve_translation_options};

#[test]
fn requested_and_effective_settings_are_distinct() {
    let requested = TranslationOptions {
        fairness: FairnessOption::Strong,
        ..TranslationOptions::default()
    };
    let source = "λ\n\\* PlusCal options (-wf, -label, -labelRoot Src)\n\\* PlusCal options (-sf)";
    let context = resolve_translation_options(source, &requested).unwrap();
    assert_eq!(context.requested.fairness, FairnessOption::Strong);
    assert_eq!(context.effective.fairness, FairnessOption::Weak);
    assert!(context.effective.insert_missing_labels);
    assert_eq!(context.effective.label_root, "Src");
    let declaration = context.source_declaration.unwrap();
    assert_eq!(
        &source[declaration.span.start..declaration.span.end],
        "PlusCal options (-wf, -label, -labelRoot Src)"
    );
    for argument in declaration.arguments {
        assert_eq!(
            &source[argument.span.start..argument.span.end],
            argument.text
        );
    }
}
#[test]
fn ignored_attached_tokens_and_first_empty_declaration() {
    let request = TranslationOptions {
        fairness: FairnessOption::Strong,
        ..TranslationOptions::default()
    };
    for prefix in ["(*PlusCal", "\\*PlusCal", ";PlusCal", "PlusCalX"] {
        let context =
            resolve_translation_options(&format!("{prefix} options (-wf)"), &request).unwrap();
        assert_eq!(context.effective.fairness, FairnessOption::Strong);
        assert!(context.source_declaration.is_none());
    }
    let context = resolve_translation_options(
        "\\* PlusCal options ()\n\\* PlusCal options (-wf)",
        &request,
    )
    .unwrap();
    assert_eq!(context.effective.fairness, FairnessOption::Strong);
    assert!(context.source_declaration.is_some());
}
#[test]
fn partial_reference_errors_never_return_effective_context() {
    for arguments in [
        "(-wf,-sf)",
        "(-wf,-wf)",
        "(-unknown)",
        "(-wf,-unknown)",
        "(-reportLabels)",
        "(-labelRoot)",
        "(-labelRoot 9Bad)",
        "(-wf",
        "-wf",
    ] {
        let error = resolve_translation_options(
            &format!("\\* PlusCal options {arguments}"),
            &TranslationOptions::default(),
        )
        .unwrap_err();
        assert_eq!(error.diagnostic.code, Code::Unsupported, "{arguments}");
    }
}
#[test]
fn option_allocations_are_bounded() {
    for source in [
        format!("PlusCal options ({})", "-label,".repeat(129)),
        format!("PlusCal options (-labelRoot {})", "a".repeat(257)),
    ] {
        assert_eq!(
            resolve_translation_options(&source, &TranslationOptions::default())
                .unwrap_err()
                .diagnostic
                .code,
            Code::Resource
        );
    }
}
#[test]
fn generation_and_checksum_use_identical_context() {
    let algorithm = "--algorithm A variables g=0; process P\\in {1,2} begin L:g:=1; E:skip; end process; end algorithm;";
    let source = format!("\\* PlusCal options (-wf)\n{algorithm}");
    let start = source.find("--algorithm").unwrap();
    let request = TranslationOptions {
        fairness: FairnessOption::Strong,
        ..TranslationOptions::default()
    };
    let translation = dive_pluscal::translate_at(&source, start..source.len(), &request).unwrap();
    assert!(translation.tla.contains("WF_vars"));
    assert!(!translation.tla.contains("SF_vars"));
    let checksum =
        dive_pluscal::algorithm_checksum_at(&source, start..source.len(), &request).unwrap();
    assert_eq!(
        checksum.checksum,
        dive_pluscal::algorithm_checksum_at(
            &source,
            start..source.len(),
            &TranslationOptions {
                fairness: FairnessOption::Weak,
                ..TranslationOptions::default()
            }
        )
        .unwrap()
        .checksum
    );
    assert_eq!(
        translation.option_context.effective.fairness,
        FairnessOption::Weak
    );
}
