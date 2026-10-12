//! Full process header admission is distinct from indexed runtime state and proofs.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
use std::path::Path;
fn admitted(syntax: &str, owner: &str) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/process-procedure-checksum");
    for shape in [
        "ordinary",
        "parameter",
        "two_parameters",
        "swap_parameters",
        "local",
        "prior_initializer",
        "call_goto",
        "self_tail",
        "recursive_base",
        "nested_if",
        "nested_with",
        "nested_either",
        "while_false",
        "unused",
    ] {
        let id = format!("ProcessHeader_{syntax}_{owner}_{shape}");
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        let expected = std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?;
        for compat_tlc in [false, true] {
            for policy in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
                let mut request = RewriteRequest::default();
                request.translation.compat_tlc = compat_tlc;
                request.editing.checksums = policy;
                let doc = rewrite_source(&id, &source, &request).to_json();
                assert_eq!(doc["accepted"], true, "{id}: {doc}");
                assert_eq!(doc["model_proved"], false, "{id}");
                assert_eq!(doc["exhaustive"], false, "{id}");
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
                let generated = doc["replacement"]["module_source"]
                    .as_str()
                    .ok_or("missing replacement")?;
                assert_eq!(
                    rewrite_source(&id, generated, &request).to_json()["replacement"]["module_source"],
                    generated,
                    "{id}"
                );
            }
        }
    }
    Ok(())
}
#[test]
fn c_constant_full_body_and_profiles() -> Result<(), Box<dyn std::error::Error>> {
    admitted("C", "constant")
}
#[test]
fn c_member_full_body_and_profiles() -> Result<(), Box<dyn std::error::Error>> {
    admitted("C", "member")
}
#[test]
fn p_constant_full_body_and_profiles() -> Result<(), Box<dyn std::error::Error>> {
    admitted("P", "constant")
}
#[test]
fn p_member_full_body_and_profiles() -> Result<(), Box<dyn std::error::Error>> {
    admitted("P", "member")
}
#[test]
fn boundary_and_resource_refusals_never_emit_a_crc_or_replacement()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/process-procedure-checksum");
    for syntax in ["C", "P"] {
        for shape in [
            "duplicate_global",
            "label_collision",
            "unknown_target",
            "arity",
            "scoped_capture",
            "fair_process",
            "mixed_blocks",
            "nonfinite_domain",
        ] {
            let id = format!("ProcessHeader_{syntax}_boundary_{shape}");
            let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
            for compat_tlc in [false, true] {
                let mut request = RewriteRequest::default();
                request.translation.compat_tlc = compat_tlc;
                let doc = rewrite_source(&id, &source, &request).to_json();
                assert_eq!(doc["accepted"], false, "{id}: {doc}");
                assert_eq!(doc["model_proved"], false);
                assert_ne!(doc["result"], "internal-error");
                assert!(doc.get("replacement").is_none());
            }
        }
    }
    let source = std::fs::read_to_string(base.join("cases/ProcessHeader_P_member_nested_if.tla"))?;
    for limit in [0, 1, 256] {
        let mut request = RewriteRequest::default();
        request.translation.limits.max_output_bytes = limit;
        let doc = rewrite_source("Bound", &source, &request).to_json();
        assert_eq!(doc["accepted"], false);
        assert_eq!(doc["model_proved"], false);
        assert!(doc.get("replacement").is_none());
    }
    Ok(())
}
