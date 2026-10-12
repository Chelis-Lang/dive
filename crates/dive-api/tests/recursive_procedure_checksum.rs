//! Public recursive artifacts are admitted independently from model proofs.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
use std::path::Path;

#[test]
fn recursive_full_and_body_admission_does_not_claim_model_or_invalid_artifact()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/recursive-procedure-checksum");
    for syntax in ["C", "P"] {
        for shape in [
            "self_base",
            "self_tail",
            "self_local",
            "self_two_args",
            "mutual_forward",
            "mutual_reverse",
            "mutual_local",
            "mutual_tail",
            "if_self",
            "elsif_self",
            "with_self",
            "either_self",
            "while_self",
            "unused_recursive",
            "nonterminating_self",
            "zero_param_self_tail",
        ] {
            let id = format!("RecursiveHeader_{syntax}_{shape}");
            let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
            let expected =
                std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?;
            for compat_tlc in [false, true] {
                for policy in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
                    let mut request = RewriteRequest::default();
                    request.translation.compat_tlc = compat_tlc;
                    request.editing.checksums = policy;
                    let doc = rewrite_source(&id, &source, &request).to_json();
                    assert_eq!(doc["model_proved"], false, "{id}");
                    assert_eq!(doc["exhaustive"], false, "{id}");
                    if shape == "zero_param_self_tail" {
                        assert_eq!(doc["accepted"], false, "{id}");
                        assert_ne!(doc["result"], "internal-error", "{id}");
                        assert!(doc.get("replacement").is_none(), "{id}");
                        continue;
                    }
                    assert_eq!(doc["accepted"], true, "{id}: {doc}");
                    assert_eq!(
                        doc["algorithm_checksum_verified"],
                        policy == ChecksumPolicy::Full
                    );
                    assert_eq!(doc["body_checksum_verified"], true);
                    if policy == ChecksumPolicy::Full {
                        assert_eq!(
                            doc["replacement"]["algorithm_checksum"]["serialized_ast"], expected,
                            "{id}"
                        );
                    }
                    let replacement = doc["replacement"]["module_source"]
                        .as_str()
                        .ok_or("missing replacement")?;
                    assert_eq!(
                        rewrite_source(&id, replacement, &request).to_json()["replacement"]["module_source"],
                        replacement,
                        "{id}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn recursive_arity_and_resource_errors_never_emit_successful_replacement()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/recursive-procedure-checksum");
    let source = std::fs::read_to_string(base.join("cases/RecursiveHeader_P_mutual_local.tla"))?;
    for compat_tlc in [false, true] {
        for limit in [0, 1, 256] {
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            request.translation.limits.max_output_bytes = limit;
            let doc = rewrite_source("Bound", &source, &request).to_json();
            assert_eq!(doc["accepted"], false);
            assert_eq!(doc["model_proved"], false);
            assert!(doc.get("replacement").is_none());
        }
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat_tlc;
        let invalid = source.replace("call Recur(3)", "call Recur(3,4)");
        assert_ne!(invalid, source);
        let doc = rewrite_source("Arity", &invalid, &request).to_json();
        assert_eq!(doc["accepted"], false);
        assert_eq!(doc["model_proved"], false);
        assert!(doc.get("replacement").is_none());
    }
    Ok(())
}
