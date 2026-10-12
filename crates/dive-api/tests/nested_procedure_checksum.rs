//! Public nested header admission remains separate from model proofs.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
use std::path::Path;
#[test]
fn nested_full_and_body_policies_are_explicit_idempotent_and_bounded()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/nested-procedure-checksum");
    for syntax in ["C", "P"] {
        for shape in [
            "if_call_one",
            "with_if_call",
            "while_nested_if_call",
            "while_nested_return",
        ] {
            let id = format!("NestedProcedureCrc_{syntax}_{shape}");
            let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
            let expected =
                std::fs::read_to_string(base.join(format!("primary/{id}.translated.ast")))?;
            for compat_tlc in [false, true] {
                for policy in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
                    let mut request = RewriteRequest::default();
                    request.translation.compat_tlc = compat_tlc;
                    request.editing.checksums = policy;
                    let report = rewrite_source(&id, &source, &request);
                    assert!(report.accepted(), "{}", report.to_json());
                    let doc = report.to_json();
                    assert_eq!(doc["model_proved"], false);
                    assert_eq!(doc["exhaustive"], false);
                    assert_eq!(
                        doc["algorithm_checksum_verified"],
                        policy == ChecksumPolicy::Full
                    );
                    assert_eq!(doc["body_checksum_verified"], true);
                    if policy == ChecksumPolicy::Full {
                        assert_eq!(
                            doc["replacement"]["algorithm_checksum"]["serialized_ast"],
                            expected
                        );
                    }
                    let replacement = doc["replacement"]["module_source"]
                        .as_str()
                        .ok_or("missing replacement")?;
                    let again = rewrite_source(&id, replacement, &request);
                    assert!(again.accepted(), "{}", again.to_json());
                    assert_eq!(again.to_json()["replacement"]["module_source"], replacement);
                    for limit in [0, 1, 256] {
                        request.translation.limits.max_output_bytes = limit;
                        let refused = rewrite_source(&id, &source, &request).to_json();
                        assert_eq!(refused["accepted"], false);
                        assert_eq!(refused["model_proved"], false);
                        assert!(refused.get("replacement").is_none());
                    }
                }
            }
        }
    }
    Ok(())
}
