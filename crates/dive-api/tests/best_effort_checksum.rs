//! Primary-backed checksum headers preserve actual generated bodies and real edit policy.
use dive_api::{RewriteRequest, rewrite_source};
use std::path::Path;

#[test]
fn exact_and_predeclared_disabled_primary_routes_match_the_chosen_native_header()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/best-effort-checksum");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json"))?)?;
    assert_eq!(
        manifest["upstream_commit"],
        "94d0c5070852b37b801bb8998c436383ab7ee37b"
    );
    assert_eq!(
        manifest["oracle_sha256"],
        "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
    );
    let rows = manifest["rows"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), 2);
    for row in rows {
        let module = row["module"].as_str().ok_or("module")?;
        let source = std::fs::read_to_string(root.join(row["source"].as_str().ok_or("source")?))?;
        let exact =
            std::fs::read_to_string(root.join(row["exact_primary"].as_str().ok_or("exact")?))?;
        let disabled = std::fs::read_to_string(
            root.join(row["disabled_primary"].as_str().ok_or("disabled")?),
        )?;
        assert_eq!(
            row["primary_disabled_route"],
            "actual pcal.trans on predeclared disabled-header INPUT, flags[]"
        );
        assert_ne!(
            exact, disabled,
            "headers really differ; no unlike-module byte identity"
        );
        for compat_tlc in [false, true] {
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            let report = rewrite_source(&format!("{module}.tla"), &source, &request);
            assert!(report.accepted(), "{}", report.to_json());
            let doc = report.to_json();
            let chosen_primary = if row["procedure"] == true {
                &disabled
            } else {
                &exact
            };
            assert_eq!(doc["replacement"]["module_source"], *chosen_primary);
            assert_eq!(doc["analysis"]["accepted"], true);
            if row["procedure"] == true {
                assert!(doc["replacement"]["algorithm_checksum"].is_null());
                assert!(
                    doc["replacement"]["warnings"]
                        .as_array()
                        .is_some_and(|warnings| !warnings.is_empty())
                );
            } else {
                assert!(doc["replacement"]["algorithm_checksum"].is_string());
            }
            let damaged = chosen_primary
                .replace("x' = 1", "x' = 2")
                .replace("a' = 2", "a' = 3");
            assert_ne!(damaged, *chosen_primary);
            let automatic = rewrite_source(&format!("{module}.tla"), &damaged, &request);
            assert_eq!(automatic.accepted(), compat_tlc);
            if compat_tlc {
                assert_eq!(
                    automatic.to_json()["replacement"]["module_source"],
                    *chosen_primary
                );
                assert_eq!(
                    automatic.to_json()["replacement"]["selection"]["checksum"]["matched"],
                    false
                );
                assert_eq!(automatic.to_json()["editing"]["replace_modified"], true);
                assert_eq!(
                    automatic.to_json()["requested_editing"]["replace_modified"],
                    false
                );
            } else {
                assert!(automatic.to_json().get("replacement").is_none());
            }
            request.editing.replace_modified = true;
            let overwritten = rewrite_source(&format!("{module}.tla"), &damaged, &request);
            assert!(overwritten.accepted());
            assert_eq!(
                overwritten.to_json()["replacement"]["module_source"],
                *chosen_primary
            );
        }
    }
    Ok(())
}

#[test]
fn primary_noop_overwrite_is_the_effective_compatibility_policy()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/best-effort-checksum");
    let primary = std::fs::read_to_string(root.join("Case.exact-primary.tla"))?;
    let modified = std::fs::read_to_string(root.join("Case.modified-body-input.tla"))?;
    assert_eq!(modified, primary.replace("x' = 1", "x' = 2"));
    let header = primary
        .lines()
        .find(|line| line.contains("BEGIN TRANSLATION"))
        .ok_or("header")?;
    let plain_marker = modified.replace(header, "\\* BEGIN TRANSLATION");
    let body_crc = header
        .split_once("chksum(tla) = \"")
        .and_then(|(_, tail)| tail.split_once('"').map(|(crc, _)| crc))
        .ok_or("body CRC")?;
    let disabled = modified.replace(
        &format!("chksum(tla) = \"{body_crc}\""),
        "chksum(tla) \\in STRING",
    );
    for (kind, input) in [
        ("modified", &modified),
        ("missing", &plain_marker),
        ("disabled", &disabled),
    ] {
        let native = RewriteRequest::default();
        let protected = rewrite_source("Case.tla", input, &native);
        assert!(!protected.accepted(), "{kind}: default native protection");
        assert!(protected.to_json().get("replacement").is_none());
        let mut explicit = native.clone();
        explicit.editing.replace_modified = true;
        let granted = rewrite_source("Case.tla", input, &explicit);
        assert!(granted.accepted(), "{kind}: {}", granted.to_json());
        if kind == "modified" {
            assert_eq!(granted.to_json()["replacement"]["module_source"], primary);
        } // Only this exact damaged-input route has fresh complete primary equality.
        let mut compatible = native;
        compatible.translation.compat_tlc = true;
        let overwritten = rewrite_source("Case.tla", input, &compatible);
        assert!(overwritten.accepted(), "{kind}: {}", overwritten.to_json());
        let doc = overwritten.to_json();
        if kind == "modified" {
            assert_eq!(doc["replacement"]["module_source"], primary);
        } else {
            // Missing/disabled header routes retain native real-body CRC policy;
            // no unlike-header primary module identity is claimed.
            assert!(
                doc["replacement"]["module_source"]
                    .as_str()
                    .is_some_and(|module| module.contains("x' = 1") && !module.contains("x' = 2"))
            );
        }
        assert_eq!(doc["editing"]["replace_modified"], true);
        assert_eq!(doc["requested_editing"]["replace_modified"], false);
        assert_eq!(
            doc["replacement"]["selection"]["checksum"]["matched"],
            false
        );
        assert_eq!(doc["replacement"]["translation_checksum"], body_crc);
        assert_eq!(doc["analysis"]["accepted"], true);
    }
    Ok(())
}
