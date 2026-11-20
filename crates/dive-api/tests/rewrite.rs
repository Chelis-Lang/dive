//! Best-effort algorithm checksums do not gate real generated-body integrity.
use dive_api::{ChecksumPolicy, RewriteOptions, RewriteRequest, rewrite_source};
const SOURCE: &str = "---- MODULE R ----\nEXTENDS Integers, TLC\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n\\* footer\n====\n";

#[test]
fn the_best_effort_policy_has_one_exact_procedure_free_crc_and_schema()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        {
            let checksums = ChecksumPolicy::BestEffort;
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            request.editing.checksums = checksums;
            let report = rewrite_source("R.tla", SOURCE, &request);
            assert!(report.accepted(), "{}", report.to_json());
            let doc = report.to_json();
            assert_eq!(doc["schema_version"], 4);
            assert_eq!(doc["profile"], if compat_tlc { "tlc" } else { "dive" });
            for field in [
                "model_proved",
                "exhaustive",
                "pending",
                "algorithm_checksum_verified",
                "body_checksum_verified",
                "exact_primary_generated_bytes",
                "option_context",
                "overrides",
            ] {
                assert!(doc.get(field).is_none(), "{field}");
            }
            assert!(doc["replacement"]["algorithm_checksum"].is_string());
            assert_eq!(doc["analysis"]["accepted"], true);
            assert!(doc["artifact"].get("option_context").is_none());
            let source = doc["replacement"]["module_source"]
                .as_str()
                .ok_or("missing string")?;
            assert!(source.contains("chksum(pcal) = "));
            let repeated = rewrite_source("R.tla", source, &request);
            assert!(repeated.accepted());
            assert_eq!(repeated.to_json()["replacement"]["module_source"], source);
        }
    }
    Ok(())
}

#[test]
fn changed_algorithm_regenerates_and_body_overwrite_resolves_by_profile()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat_tlc;
        let initial = rewrite_source("R.tla", SOURCE, &request).to_json();
        let source = initial["replacement"]["module_source"]
            .as_str()
            .ok_or("missing string")?;
        let changed_algorithm = source.replace("x:=1", "x:=2");
        assert_ne!(source, changed_algorithm);
        let regenerated = rewrite_source("R.tla", &changed_algorithm, &request);
        assert!(regenerated.accepted(), "{}", regenerated.to_json());
        assert!(
            regenerated.to_json()["artifact"]["tla"]
                .as_str()
                .ok_or("missing string")?
                .contains("x' = 2")
        );
        let changed_body = source.replace("x' = 1", "x' = 2");
        let refused = rewrite_source("R.tla", &changed_body, &request);
        if compat_tlc {
            assert!(refused.accepted());
            assert_eq!(refused.to_json()["editing"]["replace_modified"], true);
            assert_eq!(
                refused.to_json()["requested_editing"]["replace_modified"],
                false
            );
            assert_eq!(
                refused.to_json()["replacement"]["selection"]["checksum"]["matched"],
                false
            );
        } else {
            assert!(!refused.accepted());
            assert!(refused.to_json().get("replacement").is_none());
        }
        request.editing.replace_modified = true;
        let granted = rewrite_source("R.tla", &changed_body, &request);
        assert!(granted.accepted());
        assert!(
            granted.to_json()["artifact"]["tla"]
                .as_str()
                .ok_or("missing string")?
                .contains("x' = 1")
        );
    }
    Ok(())
}

#[test]
fn disabled_old_algorithm_value_does_not_disable_body_protection()
-> Result<(), Box<dyn std::error::Error>> {
    let request = RewriteRequest::default();
    let initial = rewrite_source("R.tla", SOURCE, &request).to_json();
    let source = initial["replacement"]["module_source"]
        .as_str()
        .ok_or("missing string")?;
    let crc = initial["replacement"]["algorithm_checksum"]
        .as_str()
        .ok_or("missing string")?;
    let disabled = source.replace(
        &format!("chksum(pcal) = \"{crc}\""),
        "chksum(pcal) \\in STRING",
    );
    assert!(rewrite_source("R.tla", &disabled, &request).accepted());
    let damaged = disabled.replace("x' = 1", "x' = 2");
    assert!(!rewrite_source("R.tla", &damaged, &request).accepted());
    Ok(())
}

#[test]
fn admitted_action_helper_keeps_real_body_when_algorithm_crc_is_unavailable()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE.replace("begin L:", "define F == x' end define; begin L:");
    for compat_tlc in [false, true] {
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat_tlc;
        let generated = dive_api::translate_source("R.tla", &source, &request.translation);
        assert!(generated.accepted());
        let rewritten = rewrite_source("R.tla", &source, &request);
        assert!(rewritten.accepted(), "{}", rewritten.to_json());
        let doc = rewritten.to_json();
        assert_eq!(
            doc["artifact"]["tla"],
            generated.to_json()["artifact"]["tla"]
        );
        assert_eq!(
            doc["replacement"]["algorithm_checksum"],
            serde_json::Value::Null
        );
        assert!(
            doc["replacement"]["module_source"]
                .as_str()
                .ok_or("missing string")?
                .contains("chksum(pcal) \\in STRING /\\ chksum(tla) = ")
        );
        assert_ne!(
            doc["replacement"]["warnings"]
                .as_array()
                .ok_or("missing array")?
                .as_slice(),
            &[] as &[serde_json::Value]
        );
        assert!(doc["replacement"]["translation_checksum"].is_string());
    }
    Ok(())
}

#[test]
fn checksum_only_resource_exhaustion_does_not_gate_valid_generation_or_rewrite()
-> Result<(), Box<dyn std::error::Error>> {
    let selection = dive_pluscal::rewrite::select_rewrite(SOURCE, &RewriteOptions::default())?;
    let region = selection.algorithm.start..selection.algorithm.end;
    let defaults = dive_pluscal::TranslationOptions::default();
    let body = dive_pluscal::translate_at(SOURCE, region.clone(), &defaults)?;
    let exact = dive_pluscal::algorithm_checksum_at(SOURCE, region.clone(), &defaults)?;
    assert!(exact.serialized_ast.len() > body.tla.len());
    let limited = dive_pluscal::TranslationOptions {
        max_output_bytes: body.tla.len(),
        ..defaults
    };
    let failure = dive_pluscal::algorithm_checksum_at(SOURCE, region, &limited)
        .err()
        .ok_or("exact checksum must exhaust this deliberately smaller quota")?;
    assert_eq!(failure.diagnostic.code, dive_pluscal::Code::Resource);
    for compat_tlc in [false, true] {
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat_tlc;
        request.translation.limits = limited.clone();
        let generated = dive_api::translate_source("R.tla", SOURCE, &request.translation);
        assert!(generated.accepted(), "{}", generated.to_json());
        let rewritten = rewrite_source("R.tla", SOURCE, &request);
        assert!(rewritten.accepted(), "{}", rewritten.to_json());
        let doc = rewritten.to_json();
        assert_eq!(doc["artifact"]["tla"], body.tla);
        assert!(doc["replacement"]["algorithm_checksum"].is_null());
        assert_eq!(
            doc["replacement"]["translation_checksum"],
            dive_pluscal::rewrite::translation_checksum(&body.tla)
        );
        let warnings: Vec<dive_pluscal::Diagnostic> =
            serde_json::from_value(doc["replacement"]["warnings"].clone())?;
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == dive_pluscal::Code::Resource)
        );
    }
    Ok(())
}

#[test]
fn failed_generation_and_rewrite_limits_never_return_replacement() {
    let request = RewriteRequest::default();
    let bad = SOURCE.replace("x:=1", "undefined:=1");
    let failed = rewrite_source("R.tla", &bad, &request);
    assert!(!failed.accepted());
    assert!(failed.to_json().get("replacement").is_none());
    for max_bytes in [0, 1, 16] {
        let request = RewriteRequest {
            editing: RewriteOptions {
                max_bytes,
                ..RewriteOptions::default()
            },
            ..RewriteRequest::default()
        };
        let refused = rewrite_source("R.tla", SOURCE, &request);
        assert!(!refused.accepted());
        assert!(refused.to_json().get("replacement").is_none());
    }
}

#[test]
fn original_source_options_refuse_before_masking_previous_translation() {
    let source = include_str!("fixtures/pluscal-source-option-masking/MaskedOptions.tla");
    let declaration = dive_pluscal::source_option_declaration_span(source)
        .unwrap_or_else(|| unreachable!("retained reference-recognized declaration"));
    assert_eq!(
        &source[declaration.start..declaration.end],
        "PlusCal options"
    );
    for compat_tlc in [false, true] {
        {
            let checksums = ChecksumPolicy::BestEffort;
            for replace_modified in [false, true] {
                let request = RewriteRequest {
                    translation: dive_api::TranslationRequest {
                        compat_tlc,
                        ..Default::default()
                    },
                    editing: RewriteOptions {
                        checksums,
                        replace_modified,
                        ..Default::default()
                    },
                };
                let report = rewrite_source("MaskedOptions.tla", source, &request);
                let document = report.to_json();
                assert!(!report.accepted());
                assert_eq!(report.exit_code(), 3);
                assert_eq!(document["result"], "unsupported");
                assert_eq!(document["stage"], "source-options");
                assert!(document.get("model_proved").is_none());
                assert!(document.get("exhaustive").is_none());
                assert_eq!(document["diagnostic"]["code"], "Unsupported");
                assert_eq!(document["diagnostic"]["span"]["start"], declaration.start);
                assert_eq!(document["diagnostic"]["span"]["end"], declaration.end);
                assert!(document.get("artifact").is_none());
                assert!(document.get("replacement").is_none());
                assert!(document.get("algorithm_checksum_verified").is_none());
            }
        }
    }
}

#[test]
fn physical_replacement_newline_metadata_describes_actual_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let lf = "---- MODULE Case ----\nEXTENDS Naturals, TLC\n\n(* --algorithm A variables x = 0; begin A: x := 1; B: skip; end algorithm; *)\n====\n";
    let controls = [
        (lf.to_owned(), "lf", "\n"),
        (lf.replace('\n', "\r\n"), "crlf", "\r\n"),
        (lf.replace('\n', "\r"), "cr", "\r"),
        (lf.replacen('\n', "\r", 1), "cr", "\r"),
        (lf.replacen('\n', "\r\n", 1), "crlf", "\r\n"),
        (lf.replacen("; *)\n", "; *)\r", 1), "lf", "\n"),
    ];
    let baseline = rewrite_source("Case.tla", lf, &RewriteRequest::default()).to_json();
    for (source, native_kind, native_separator) in controls {
        for (normalize, compat_tlc) in [(false, false), (true, false), (false, true)] {
            let mut request = RewriteRequest::default();
            request.editing.normalize_newlines = normalize;
            request.translation.compat_tlc = compat_tlc;
            let report = rewrite_source("Case.tla", &source, &request);
            assert!(report.accepted(), "{}", report.to_json());
            let doc = report.to_json();
            let (kind, separator) = if normalize || compat_tlc {
                ("lf", "\n")
            } else {
                (native_kind, native_separator)
            };
            assert_eq!(doc["filesystem_plan"]["body_newlines"], kind);
            let replacement = doc["replacement"]["replacement"]
                .as_str()
                .ok_or("replacement bytes")?;
            assert!(replacement.split(separator).count() > 3);
            assert!(!replacement.replace(separator, "").contains(['\r', '\n']));
            assert_eq!(doc["artifact"]["tla"], baseline["artifact"]["tla"]);
            assert_eq!(
                doc["replacement"]["translation_checksum"],
                baseline["replacement"]["translation_checksum"]
            );
            let mappings = doc["artifact"]["mappings"].as_array().ok_or("mappings")?;
            assert_eq!(mappings.len(), 2);
            for (mapping, statement) in mappings.iter().zip(["A: x := 1;", "B: skip;"]) {
                let at = source.find(statement).ok_or("literal statement")?;
                assert_eq!(mapping["original"]["start"], at);
                assert_eq!(mapping["original"]["end"], at + statement.len());
            }
        }
    }
    Ok(())
}
