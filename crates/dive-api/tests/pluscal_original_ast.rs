//! Public original artifacts and debug CRCs consume actual pinned controls.
use dive_api::{RewriteRequest, TranslationRequest, original_ast_source, rewrite_source};
use dive_pluscal::TranslationOptions;

#[test]
fn artifacts_and_debug_replacements_have_separate_acceptance()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-pluscal/tests/fixtures/original-ast/primary.json"
    ))?;
    for compat_tlc in [false, true] {
        for control in fixture["controls"].as_array().ok_or("controls")? {
            let source = control["source"].as_str().ok_or("source")?;
            let request = TranslationRequest {
                limits: TranslationOptions {
                    debug: control["debug"].as_bool().ok_or("debug")?,
                    ..TranslationOptions::default()
                },
                compat_tlc,
                ..TranslationRequest::default()
            };
            let original = original_ast_source("Case.tla", source, &request);
            assert!(original.artifact_available(), "{}", original.to_json());
            let doc = original.to_json();
            assert_eq!(doc["accepted"], false);
            assert_eq!(doc["generated_module_accepted"], false);
            assert_eq!(doc["writes_performed"], false);
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("module_source").is_none());
            assert!(doc.get("analysis").is_none());
            if control["original_module"].is_string() {
                assert_eq!(
                    doc["ast_artifact"]["module_source"],
                    control["original_module"]
                );
            }
            if control["original_ast"].is_string() {
                assert_eq!(
                    doc["ast_artifact"]["serialized_ast"],
                    control["original_ast"]
                );
            }
            if let Some(expected) = control["algorithm_crc"].as_str() {
                let rewritten = rewrite_source(
                    "Case.tla",
                    source,
                    &RewriteRequest {
                        translation: request,
                        ..RewriteRequest::default()
                    },
                );
                assert!(rewritten.accepted(), "{}", rewritten.to_json());
                assert_eq!(
                    rewritten.to_json()["replacement"]["algorithm_checksum"],
                    expected
                );
                assert_eq!(
                    rewritten.to_json()["replacement"]["translation_checksum"],
                    control["body_crc"]
                );
                assert_eq!(
                    rewritten.to_json()["filesystem_plan"]["writes_performed"],
                    false
                );
            }
        }
    }
    Ok(())
}

#[test]
fn original_ast_does_not_make_unresolved_module_a_successful_translation() {
    let source = "---- MODULE Bad ----\n(* --algorithm A variable x=UNKNOWN; begin L:skip; end algorithm *)\n====\n";
    let request = TranslationRequest::default();
    let ast = original_ast_source("Bad.tla", source, &request);
    assert!(ast.artifact_available());
    assert_eq!(ast.to_json()["generated_module_accepted"], false);
    assert!(!dive_api::translate_source("Bad.tla", source, &request).accepted());
}

#[test]
fn write_ast_is_still_a_named_filesystem_refusal() {
    let source = "---- MODULE A ----\n(* --algorithm A begin L:skip; end algorithm *)\n====\n";
    let request = TranslationRequest {
        limits: TranslationOptions {
            write_ast: true,
            ..TranslationOptions::default()
        },
        ..TranslationRequest::default()
    };
    let result = original_ast_source("A.tla", source, &request);
    assert!(!result.artifact_available());
    assert_eq!(result.exit_code(), 3);
    assert!(
        result.to_json()["diagnostic"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("writeAST"))
    );
}

#[test]
fn original_module_prefix_uses_effective_version_and_keeps_strict_region_eof()
-> Result<(), Box<dyn std::error::Error>> {
    let algorithm = "--algorithm First variable x=0; fair begin L:skip; end algorithm;";
    for tail in [
        "--algorithm Second begin E:skip; end algorithm;",
        "ordinary trailing prose",
    ] {
        for source_version in [false, true] {
            let declaration = if source_version {
                "\\* PlusCal options (-version 1.5)\n"
            } else {
                ""
            };
            let source =
                format!("---- MODULE Prefix ----\n{declaration}(* {algorithm} {tail} *)\n====\n");
            for compat_tlc in [false, true] {
                let request = TranslationRequest {
                    limits: TranslationOptions {
                        version: if source_version {
                            None
                        } else {
                            Some("1.5".into())
                        },
                        ..TranslationOptions::default()
                    },
                    compat_tlc,
                    ..TranslationRequest::default()
                };
                let report = original_ast_source("Prefix.tla", &source, &request);
                assert!(report.artifact_available(), "{}", report.to_json());
                let doc = report.to_json();
                assert!(
                    doc["ast_artifact"]["serialized_ast"]
                        .as_str()
                        .is_some_and(
                            |ast| ast.contains("name  |-> \"First\"") && !ast.contains("Second")
                        )
                );
                assert!(
                    doc["ast_artifact"]["module_source"]
                        .as_str()
                        .is_some_and(|module| module.contains("fairness == \"wf\""))
                );
                assert_eq!(doc["translation_options"]["version"], "1.5");
                let comment_start = source.find("(* ").ok_or("comment")? + 3;
                let comment_end = source.find(" *)").ok_or("end comment")?;
                assert!(
                    dive_pluscal::original_ast_at(
                        &source,
                        comment_start..comment_end,
                        &request.limits
                    )
                    .is_err()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn original_module_keeps_first_errors_resources_and_previous_source_options() {
    let malformed = "---- MODULE Prefix ----\n\\* PlusCal options (-version 1.5)\n(* --algorithm First begin L: x := ; end algorithm; --algorithm Second begin E:skip; end algorithm; *)\n====\n";
    let previous = include_str!("fixtures/pluscal-source-option-masking/MaskedOptions.tla");
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            compat_tlc,
            ..TranslationRequest::default()
        };
        let bad = original_ast_source("Prefix.tla", malformed, &request);
        assert!(!bad.artifact_available());
        assert_eq!(bad.exit_code(), 1);
        assert_eq!(bad.to_json()["code"], "DIVE-P0003");
        assert!(
            bad.to_json()["diagnostic"]["span"]["start"]
                .as_u64()
                .is_some_and(
                    |offset| usize::try_from(offset).is_ok_and(|offset| malformed
                        .get(offset..)
                        .is_some_and(|tail| tail.contains("--algorithm Second")))
                )
        );
        let lexical = original_ast_source(
            "Prefix.tla",
            "---- MODULE Prefix ----\n(* --algorithm First begin L:skip; end algorithm;\n====\n",
            &request,
        );
        assert!(!lexical.artifact_available());
        assert_eq!(lexical.to_json()["code"], "DIVE-P0001");
        let old = original_ast_source("MaskedOptions.tla", previous, &request);
        assert!(!old.artifact_available());
        assert_eq!(old.exit_code(), 3);
        assert!(
            old.to_json()["diagnostic"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("inside previous translation"))
        );
        let limits = TranslationOptions {
            parser: dive_pluscal::Options {
                max_nodes: 1,
                ..dive_pluscal::Options::default()
            },
            ..TranslationOptions::default()
        };
        let bounded = original_ast_source(
            "Prefix.tla",
            malformed,
            &TranslationRequest { limits, ..request },
        );
        assert_eq!(bounded.exit_code(), 3);
        assert_eq!(bounded.to_json()["code"], "DIVE-P0040");
    }
}

#[test]
fn original_module_selection_uses_the_requested_byte_bound()
-> Result<(), Box<dyn std::error::Error>> {
    // The earlier default selector imposed 1 MiB before the requested parser
    // could own the first algorithm. This is a native resource-contract control,
    // not an additional primary AST measurement.
    let algorithm = "--algorithm First begin L:skip; end algorithm;";
    let source = format!(
        "---- MODULE Prefix ----\n(* {} *)\n(* {algorithm} ordinary prose *)\n====\n",
        " ".repeat(1_048_576)
    );
    let start = source.find("--algorithm").ok_or("algorithm")?;
    let limits = TranslationOptions {
        parser: dive_pluscal::Options {
            max_bytes: source.len(),
            ..dive_pluscal::Options::default()
        },
        ..TranslationOptions::default()
    };
    let direct = dive_pluscal::original_ast_at(&source, start..start + algorithm.len(), &limits)?;
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            compat_tlc,
            ..TranslationRequest::default()
        };
        let bounded = original_ast_source("Prefix.tla", &source, &request);
        assert!(!bounded.artifact_available());
        assert_eq!(bounded.to_json()["code"], "DIVE-P0040");
        let original = original_ast_source(
            "Prefix.tla",
            &source,
            &TranslationRequest {
                limits: limits.clone(),
                ..request
            },
        );
        assert!(original.artifact_available(), "{}", original.to_json());
        assert_eq!(
            original.to_json()["ast_artifact"]["module_source"],
            direct.module_source
        );
        assert_eq!(original.to_json()["accepted"], false);
        assert_eq!(original.to_json()["writes_performed"], false);
    }
    Ok(())
}
