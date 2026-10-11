//! Original-source settings survive report/replacement admission; no masking bypass.
use dive_api::{ChecksumPolicy, RewriteOptions, RewriteRequest, TranslationRequest};
use dive_pluscal::{FairnessOption, TranslationOptions};

const SOURCE: &str = "---- MODULE SourceContext ----\nEXTENDS Integers,TLC,Sequences\n\\* PlusCal options (-wf)\n(* --algorithm A variable g=0; process P\\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)\n====\n";
#[test]
fn source_settings_override_request_in_both_report_surfaces() {
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            limits: TranslationOptions {
                fairness: FairnessOption::Strong,
                ..TranslationOptions::default()
            },
            compat_tlc,
            ..TranslationRequest::default()
        };
        let translated = dive_api::translate_source("SourceContext.tla", SOURCE, &request);
        assert!(translated.accepted());
        let doc = translated.to_json();
        assert_eq!(doc["option_context"]["requested"]["fairness"], "Strong");
        assert_eq!(doc["translation_options"]["fairness"], "Weak");
        assert_eq!(doc["model_proved"], false);
        let rewrite = dive_api::rewrite_source(
            "SourceContext.tla",
            SOURCE,
            &RewriteRequest {
                translation: request,
                ..RewriteRequest::default()
            },
        );
        assert!(rewrite.accepted());
        let doc = rewrite.to_json();
        assert_eq!(doc["translation_options"]["fairness"], "Weak");
        assert_eq!(
            doc["replacement"]["algorithm_checksum"]["option_context"]["effective"]["fairness"],
            "Weak"
        );
        assert_eq!(doc["model_proved"], false);
    }
}
#[test]
fn deciding_source_context_inside_previous_translation_is_not_masked() {
    let source = SOURCE.replace("\\* PlusCal options (-wf)\n", "").replace(
        "====",
        "\\* BEGIN TRANSLATION\n\\* PlusCal options (-wf)\n\\* END TRANSLATION\n====",
    );
    for compat_tlc in [false, true] {
        for checksums in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
            let request = RewriteRequest {
                translation: TranslationRequest {
                    compat_tlc,
                    ..TranslationRequest::default()
                },
                editing: RewriteOptions {
                    checksums,
                    replace_modified: true,
                    ..RewriteOptions::default()
                },
            };
            let report = dive_api::rewrite_source("SourceContext.tla", &source, &request);
            assert!(!report.accepted());
            let doc = report.to_json();
            assert_eq!(doc["result"], "unsupported");
            assert_eq!(doc["stage"], "source-options");
            assert!(doc.get("replacement").is_none());
            assert_eq!(doc["model_proved"], false);
        }
    }
}
#[test]
fn erroneous_source_options_return_no_artifact() {
    for argument in ["-unknown", "-wf,-sf", "-reportLabels", "-labelRoot 9Bad"] {
        let source = SOURCE.replace("-wf", argument);
        let report = dive_api::translate_source(
            "SourceContext.tla",
            &source,
            &TranslationRequest::default(),
        );
        assert!(!report.accepted());
        assert_eq!(report.exit_code(), 3);
        assert!(report.to_json().get("artifact").is_none());
    }
}
