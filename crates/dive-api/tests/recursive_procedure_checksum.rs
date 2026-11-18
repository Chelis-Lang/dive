//! Best-effort algorithm CRCs retain real body integrity and whole-module analysis.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
use std::path::Path;

#[test]
fn genuine_source_population_preserves_body_crc_and_artifact_acceptance()
-> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/recursive-procedure-checksum");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("primary/manifest.json"),
    )?)?;
    let fixture_root = base.parent().ok_or("fixture root")?;
    let lineage: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        fixture_root.join("procedure-input-lineage.json"),
    )?)?;
    let declared = &lineage["families"]["recursive-procedure-checksum"];
    let folders = declared["input_folders"]
        .as_array()
        .ok_or("input folders")?;
    let unavailable = lineage["unavailable_ids"]
        .as_array()
        .ok_or("unavailable ids")?;
    let mut exercised = 0usize;
    let mut unexercised = 0usize;
    let mut historical_successes = 0usize;
    for row in manifest["rows"].as_array().ok_or("rows")? {
        let id = row["id"].as_str().ok_or("id")?;
        let candidates = folders
            .iter()
            .map(|folder| {
                folder
                    .as_str()
                    .map(|name| fixture_root.join(name).join(format!("cases/{id}.tla")))
                    .ok_or("invalid fixture folder")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if unavailable
            .iter()
            .any(|missing| missing.as_str() == Some(id))
        {
            assert!(
                candidates.iter().all(|path| !path.is_file()),
                "{id}: recovered inputs must be exercised"
            );
            unexercised += 1;
            historical_successes +=
                usize::from(row["translated"] == true && row["reference_errors"] == false);
            continue; // Historical primary fact only; no native pass/refusal/coverage.
        }
        let source_path = candidates
            .iter()
            .find(|path| path.is_file())
            .ok_or("missing genuine source")?;
        let source = std::fs::read_to_string(source_path)?;
        exercised += 1;
        check_primary_rewrite(row, id, &source)?;
    }
    assert_eq!(
        exercised,
        usize::try_from(declared["available"].as_u64().ok_or("available count")?)?
    );
    assert_eq!(
        unexercised,
        usize::try_from(
            declared["unexercised"]
                .as_u64()
                .ok_or("unexercised count")?
        )?
    );
    assert_eq!(historical_successes, 14);
    Ok(())
}

fn check_primary_rewrite(
    row: &serde_json::Value,
    id: &str,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if row["flags"][0] == "-fair" {
        // Invalid primary CLI flag; typed generation is not that CLI operation.
        assert_eq!(row["translated"], false);
        assert_eq!(row["reference_errors"], true);
        return Ok(());
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
        let report = rewrite_source(&format!("{id}.tla"), source, &request);
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
    Ok(())
}

#[test]
fn the_best_effort_policy_round_trips_and_generation_limits_still_refuse()
-> Result<(), Box<dyn std::error::Error>> {
    let source =
        include_str!("../../dive-pluscal/tests/fixtures/procedure/cases/Procedure_P_simple.tla");
    {
        let spelling = "BestEffort";
        let policy: ChecksumPolicy = serde_json::from_value(serde_json::json!(spelling))?;
        assert_eq!(policy, ChecksumPolicy::BestEffort);
        for compat_tlc in [false, true] {
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            request.editing.checksums = policy;
            assert!(rewrite_source("Procedure_P_simple.tla", source, &request).accepted());
            for limit in [0, 1, 16, 256] {
                request.translation.limits.max_output_bytes = limit;
                let report = rewrite_source("Procedure_P_simple.tla", source, &request);
                assert!(!report.accepted());
                assert!(report.to_json().get("replacement").is_none());
            }
        }
    }
    Ok(())
}
