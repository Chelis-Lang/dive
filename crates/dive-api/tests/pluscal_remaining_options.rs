//! Supported flag contexts are consistent through both complete-module surfaces.
use dive_api::{ChecksumPolicy, RewriteOptions, RewriteRequest, TranslationRequest};
use dive_pluscal::{FairnessOption, TranslationOptions};

const SOURCE: &str = "---- MODULE FlagOptions ----\nEXTENDS Naturals,TLC\n\\* PlusCal options (termination, noDoneDisjunct, -lineWidth 60, -noold, -nocfg)\n(* --algorithm A variable x=0; process P\\in {1,2} begin L:x:=x+1; E:skip; end process; end algorithm; *)\n====\n";

#[test]
fn supported_source_flags_flow_through_all_artifacts() {
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            limits: TranslationOptions {
                line_width: 100,
                ..TranslationOptions::default()
            },
            compat_tlc,
            ..TranslationRequest::default()
        };
        let translated = dive_api::translate_source("FlagOptions.tla", SOURCE, &request);
        assert!(translated.accepted(), "{}", translated.to_json());
        let doc = translated.to_json();
        assert_eq!(doc["requested_translation_options"]["line_width"], 100);
        assert_eq!(doc["translation_options"]["line_width"], 60);
        assert_eq!(doc["translation_options"]["fairness"], "Weak");
        assert!(
            doc["artifact"]["tla"]
                .as_str()
                .is_some_and(|body| body.contains("WF_vars") && !body.contains("Terminating =="))
        );
        assert_eq!(doc["artifact"]["configuration"], "PROPERTY Termination\n");
        assert_eq!(doc["filesystem_plan"]["writes_performed"], false);
        assert_eq!(doc["filesystem_plan"]["backup_requested"], false);
        assert_eq!(doc["filesystem_plan"]["configuration_requested"], false);
        assert_eq!(
            doc["filesystem_plan"]["configuration_requirements"],
            "PROPERTY Termination\n"
        );
        assert!(doc.get("model_proved").is_none());
        assert!(doc.get("exhaustive").is_none());
        for checksums in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
            let report = dive_api::rewrite_source(
                "FlagOptions.tla",
                SOURCE,
                &RewriteRequest {
                    translation: request.clone(),
                    editing: RewriteOptions {
                        checksums,
                        ..RewriteOptions::default()
                    },
                },
            );
            assert!(report.accepted(), "{}", report.to_json());
            let rewritten = report.to_json();
            assert_eq!(rewritten["translation_options"], doc["translation_options"]);
            for field in [
                "backup_requested",
                "configuration_requested",
                "configuration_requirements",
                "unix_eol_requested",
                "writes_performed",
                "body_newlines",
            ] {
                assert_eq!(
                    rewritten["filesystem_plan"][field],
                    doc["filesystem_plan"][field]
                );
            }
            assert_eq!(
                rewritten["filesystem_plan"]["source_newlines"],
                if compat_tlc {
                    "normalized-lf"
                } else {
                    "preserved"
                }
            );
            if checksums == ChecksumPolicy::Full {
                assert_eq!(rewritten["translation_options"], doc["translation_options"]);
            }
            assert!(rewritten.get("model_proved").is_none());
        }
    }
}

#[test]
fn nof_remains_explicit_and_unix_eol_is_not_a_false_lf_promise() {
    let source = SOURCE
        .replace(
            "termination, noDoneDisjunct, -lineWidth 60, -noold, -nocfg",
            "termination, nof",
        )
        .replace('\n', "\r\n");
    let report = dive_api::translate_source(
        "FlagOptions.tla",
        &source,
        &TranslationRequest {
            limits: TranslationOptions {
                unix_eol: true,
                fairness: FairnessOption::Strong,
                ..TranslationOptions::default()
            },
            ..TranslationRequest::default()
        },
    );
    assert!(report.accepted());
    let doc = report.to_json();
    assert_eq!(doc["translation_options"]["fairness"], "None");
    assert_eq!(doc["filesystem_plan"]["unix_eol_requested"], true);
    assert_eq!(doc["filesystem_plan"]["writes_performed"], false);
    assert!(
        doc["artifact"]["tla"]
            .as_str()
            .is_some_and(|body| !body.contains("WF_vars"))
    );
    let report = dive_api::rewrite_source(
        "FlagOptions.tla",
        &source,
        &RewriteRequest {
            translation: TranslationRequest {
                limits: TranslationOptions {
                    unix_eol: true,
                    ..TranslationOptions::default()
                },
                ..TranslationRequest::default()
            },
            ..RewriteRequest::default()
        },
    );
    assert!(report.accepted(), "{}", report.to_json());
    assert_eq!(
        report.to_json()["replacement"]["newlines_normalized"],
        false
    );
    assert_eq!(report.to_json()["filesystem_plan"]["body_newlines"], "crlf");
    assert!(
        report.to_json()["replacement"]["module_source"]
            .as_str()
            .is_some_and(|module| module.contains("\r\n"))
    );
}

#[test]
fn unsupported_artifact_modes_produce_no_module_or_replacement() {
    for compat_tlc in [false, true] {
        // Retain the predecessor debug/source-options input under its supported behavior.
        let debug_request = TranslationRequest {
            limits: TranslationOptions {
                debug: true,
                ..TranslationOptions::default()
            },
            compat_tlc,
            ..TranslationRequest::default()
        };
        let debug_report = dive_api::translate_source("FlagOptions.tla", SOURCE, &debug_request);
        assert!(debug_report.accepted(), "{}", debug_report.to_json());
        assert_eq!(debug_report.to_json()["translation_options"]["debug"], true);
        let debug_rewrite = dive_api::rewrite_source(
            "FlagOptions.tla",
            SOURCE,
            &RewriteRequest {
                translation: debug_request,
                ..RewriteRequest::default()
            },
        );
        assert!(debug_rewrite.accepted(), "{}", debug_rewrite.to_json());
        assert!(debug_rewrite.to_json()["replacement"]["algorithm_checksum"].is_string());
        let exact = dive_pluscal::rewrite::select_rewrite(
            SOURCE,
            &dive_pluscal::rewrite::RewriteOptions::default(),
        )
        .map_err(|error| error.to_string())
        .and_then(|selection| {
            dive_pluscal::algorithm_checksum_at(
                SOURCE,
                selection.algorithm.start..selection.algorithm.end,
                &TranslationOptions {
                    debug: true,
                    ..TranslationOptions::default()
                },
            )
            .map_err(|error| error.to_string())
        });
        assert!(exact.is_ok_and(|checksum| checksum.serialized_ast.starts_with("(*0, 0*)")));
        for limits in [
            TranslationOptions {
                write_ast: true,
                ..TranslationOptions::default()
            },
            TranslationOptions {
                specification_backend: Some(("myspec2".into(), "LocalSpec".into())),
                ..TranslationOptions::default()
            },
        ] {
            let request = TranslationRequest {
                limits,
                compat_tlc,
                ..TranslationRequest::default()
            };
            let report = dive_api::translate_source("FlagOptions.tla", SOURCE, &request);
            assert_eq!(report.exit_code(), 3);
            let doc = report.to_json();
            assert!(doc.get("artifact").is_none());
            assert!(doc.get("module_source").is_none());
            let report = dive_api::rewrite_source(
                "FlagOptions.tla",
                SOURCE,
                &RewriteRequest {
                    translation: request,
                    ..RewriteRequest::default()
                },
            );
            assert_eq!(report.exit_code(), 3);
            assert!(report.to_json().get("replacement").is_none());
            assert!(report.to_json().get("model_proved").is_none());
        }
    }
}
