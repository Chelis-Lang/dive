//! Full procedure editing and resource/unsupported boundaries never claim proof.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
const SOURCE: &str =
    include_str!("../../dive-pluscal/procedure-checksum-acceptance/cases/ProcedureCrc_P_basic.tla");
#[test]
fn procedure_full_and_body_only_admission_are_explicit_and_idempotent()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for policy in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            request.editing.checksums = policy;
            let report = rewrite_source("ProcedureCrc_P_basic.tla", SOURCE, &request);
            assert!(report.accepted(), "{}", report.to_json());
            let doc = report.to_json();
            assert_eq!(doc["model_proved"], false);
            assert_eq!(doc["exhaustive"], false);
            assert_eq!(
                doc["algorithm_checksum_verified"],
                policy == ChecksumPolicy::Full
            );
            assert_eq!(doc["body_checksum_verified"], true);
            assert_eq!(
                doc["profile"],
                if compat_tlc {
                    if policy == ChecksumPolicy::Full {
                        "tlc"
                    } else {
                        "tlc+overrides"
                    }
                } else {
                    "dive"
                }
            );
            let source = doc["replacement"]["module_source"]
                .as_str()
                .ok_or("accepted replacement lacks source")?;
            let again = rewrite_source("ProcedureCrc_P_basic.tla", source, &request);
            assert!(again.accepted(), "{}", again.to_json());
            assert_eq!(again.to_json()["replacement"]["module_source"], source);
        }
    }
    Ok(())
}
#[test]
fn nested_shared_mutation_and_resource_limits_have_no_replacement() {
    let nested = include_str!(
        "../../dive-pluscal/procedure-checksum-acceptance/cases/ProcedureCrc_P_if_call.tla"
    );
    for compat_tlc in [false, true] {
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat_tlc;
        let report = rewrite_source("ProcedureCrc_P_if_call.tla", nested, &request);
        assert!(!report.accepted());
        let doc = report.to_json();
        assert_eq!(doc["result"], "unsupported");
        assert!(doc.get("replacement").is_none());
        assert_eq!(doc["model_proved"], false);
        for limit in [0, 1, 16, 256] {
            request.translation.limits.max_output_bytes = limit;
            let report = rewrite_source("ProcedureCrc_P_basic.tla", SOURCE, &request);
            assert!(!report.accepted(), "{}", report.to_json());
            assert!(report.to_json().get("replacement").is_none());
            assert_eq!(report.to_json()["model_proved"], false);
        }
    }
}
