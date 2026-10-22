//! Best-effort algorithm CRCs retain real body integrity and whole-module analysis.
use dive_api::{RewriteRequest, rewrite_source};
use std::path::Path;

#[test]
fn genuine_source_population_preserves_body_crc_and_artifact_acceptance()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/p-elsif-checksum");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    for row in manifest["rows"].as_array().ok_or("rows")? {
        let id = row["id"].as_str().ok_or("id")?;
        let source = std::fs::read_to_string(base.join(format!("cases/{id}.tla")))?;
        if row["flags"][0] == "-fair" {
            // Invalid primary CLI flag; typed generation is not that CLI operation.
            assert_eq!(row["translated"], false);
            assert_eq!(row["reference_errors"], true);
            continue;
        }
        for compat_tlc in [false, true] {
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            request.translation.limits.insert_missing_labels = row["flags"][0] == "-label";
            request.translation.limits.fairness = match row["flags"][0].as_str() {
                Some("-wf") => dive_api::TranslationFairness::Weak,
                Some("-sf") => dive_api::TranslationFairness::Strong,
                Some("-nof") => dive_api::TranslationFairness::None,
                Some("-wfNext") => dive_api::TranslationFairness::Next,
                _ => dive_api::TranslationFairness::Declared,
            };
            let report = rewrite_source(&format!("{id}.tla"), &source, &request);
            let doc = report.to_json();
            assert_eq!(doc["schema_version"], 3);
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("exhaustive").is_none());
            assert!(doc.get("pending").is_none());
            if row["translated"] == true
                && row["reference_errors"] == false
                && row["sany"]["accepted"] == true
            {
                assert!(report.accepted(), "{id}: {doc}");
                assert_eq!(doc["analysis"]["accepted"], true);
                assert_eq!(
                    doc["replacement"]["translation_checksum"], row["header"][1],
                    "{id}: primary body CRC"
                );
                assert_eq!(
                    doc["replacement"]["algorithm_checksum"],
                    serde_json::Value::Null
                );
                let replacement = doc["replacement"]["module_source"]
                    .as_str()
                    .ok_or("source")?;
                assert!(replacement.contains(r"chksum(pcal) \in STRING /\ chksum(tla) = "));
                let repeated = rewrite_source(&format!("{id}.tla"), replacement, &request);
                assert!(repeated.accepted(), "{id}: {}", repeated.to_json());
                assert_eq!(
                    repeated.to_json()["replacement"]["module_source"],
                    replacement
                );
            } else {
                assert!(
                    !report.accepted(),
                    "{id}: no artifact credit for actual primary failure: {doc}"
                );
                assert!(doc.get("replacement").is_none());
            }
        }
    }
    Ok(())
}
