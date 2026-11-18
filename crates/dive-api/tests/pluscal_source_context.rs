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
        assert_eq!(doc["requested_translation_options"]["fairness"], "Strong");
        assert_eq!(doc["translation_options"]["fairness"], "Weak");
        assert!(doc.get("model_proved").is_none());
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
        assert!(doc["source_options"].is_object());
        assert!(doc["replacement"]["algorithm_checksum"].is_string());
        assert!(doc.get("model_proved").is_none());
    }
}
#[test]
fn deciding_source_context_inside_previous_translation_is_not_masked() {
    let source = SOURCE.replace("\\* PlusCal options (-wf)\n", "").replace(
        "====",
        "\\* BEGIN TRANSLATION\n\\* PlusCal options (-wf)\n\\* END TRANSLATION\n====",
    );
    for compat_tlc in [false, true] {
        {
            let checksums = ChecksumPolicy::BestEffort;
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
            assert!(doc.get("model_proved").is_none());
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

#[test]
fn legacy_input_version_selects_first_algorithm_before_trailing_comment_content() {
    let single = "---- MODULE MultiVersion ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable x=0; fair begin L:x:=1; E:skip; end algorithm; *)\n====\n";
    for source_options in [false, true] {
        let single = if source_options {
            single.replace(
                "(* --algorithm",
                "\\* PlusCal options (version 1.5)\n(* --algorithm",
            )
        } else {
            single.to_owned()
        };
        for suffix in [
            "ordinary prose λ",
            "--algorithm B begin L:skip; end algorithm;",
        ] {
            let multiple = single.replace(" *)", &format!(" {suffix} *)"));
            for compat_tlc in [false, true] {
                let request = TranslationRequest {
                    limits: TranslationOptions {
                        // A source declaration overrides the requested modern grammar.
                        version: Some(if source_options { "1.9" } else { "1.5" }.into()),
                        ..TranslationOptions::default()
                    },
                    compat_tlc,
                    ..TranslationRequest::default()
                };
                let baseline = dive_api::translate_source("MultiVersion.tla", &single, &request);
                let translated =
                    dive_api::translate_source("MultiVersion.tla", &multiple, &request);
                assert!(baseline.accepted(), "{}", baseline.to_json());
                assert!(translated.accepted(), "{}", translated.to_json());
                assert_eq!(
                    translated.to_json()["artifact"]["tla"],
                    baseline.to_json()["artifact"]["tla"]
                );
                assert_eq!(
                    translated.to_json()["translation_options"]["version"],
                    "1.5"
                );
                {
                    let checksums = ChecksumPolicy::BestEffort;
                    let rewrite = RewriteRequest {
                        translation: request.clone(),
                        editing: RewriteOptions {
                            checksums,
                            ..RewriteOptions::default()
                        },
                    };
                    let baseline = dive_api::rewrite_source("MultiVersion.tla", &single, &rewrite);
                    let replaced =
                        dive_api::rewrite_source("MultiVersion.tla", &multiple, &rewrite);
                    assert!(baseline.accepted(), "{}", baseline.to_json());
                    assert!(replaced.accepted(), "{}", replaced.to_json());
                    assert_eq!(
                        replaced.to_json()["replacement"]["algorithm_checksum"],
                        baseline.to_json()["replacement"]["algorithm_checksum"]
                    );
                    assert!(
                        replaced.to_json()["replacement"]["module_source"]
                            .as_str()
                            .is_some_and(|text| text.contains(suffix))
                    );
                    assert!(replaced.to_json().get("model_proved").is_none());
                }
            }
        }
    }
}

#[test]
fn legacy_prefix_selection_never_admits_an_invalid_first_algorithm_or_ignores_bounds() {
    let source = "---- MODULE MultiVersion ----\nEXTENDS Naturals,TLC\n\\* PlusCal options (version 1.5)\n(* --algorithm A variable x=0; fair begin L:x:=; end algorithm; --algorithm B begin L:skip; end algorithm; *)\n====\n";
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            compat_tlc,
            ..TranslationRequest::default()
        };
        let report = dive_api::translate_source("MultiVersion.tla", source, &request);
        assert!(!report.accepted());
        assert!(report.to_json().get("artifact").is_none());
        let valid = source.replace("x:=;", "x:=1;");
        let bounded = TranslationRequest {
            limits: TranslationOptions {
                parser: dive_pluscal::Options {
                    max_bytes: 1,
                    ..dive_pluscal::Options::default()
                },
                ..TranslationOptions::default()
            },
            ..request
        };
        let report = dive_api::translate_source("MultiVersion.tla", &valid, &bounded);
        assert_eq!(report.exit_code(), 3);
        assert!(report.to_json().get("artifact").is_none());
    }
}
