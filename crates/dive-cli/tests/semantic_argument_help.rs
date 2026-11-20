//! Selected semantic explanations are informational, never model results.

use std::process::Command;

#[test]
fn semantic_argument_codes_have_distinct_versioned_explanations()
-> Result<(), Box<dyn std::error::Error>> {
    let mut remediations = std::collections::BTreeSet::new();
    for code in [
        "DIVE-E0107",
        "DIVE-E0110",
        "DIVE-E0112",
        "DIVE-E0113",
        "DIVE-E0114",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dive"))
            .args(["explain", code, "--output", "json"])
            .output()?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stderr, [] as [u8; 0]);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["schema_version"], dive_api::REPORT_SCHEMA_VERSION);
        assert_eq!(report["command"], "explain");
        assert_eq!(report["code"], code);
        assert_eq!(report["known"], true);
        assert!(report.get("verdict").is_none());
        remediations.insert(
            report["explanation"]["remediation"]
                .as_str()
                .ok_or("remediation")?
                .to_owned(),
        );
    }
    assert_eq!(remediations.len(), 5);
    let human = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["explain", "DIVE-E0110"])
        .output()?;
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)?.contains("tuple-pattern parameter counts as one index")
    );
    Ok(())
}

#[test]
fn unlisted_semantic_codes_and_unknown_codes_fail_closed() -> Result<(), Box<dyn std::error::Error>>
{
    for code in ["DIVE-E0102", "DIVE-E0115", "DIVE-E9999"] {
        let output = Command::new(env!("CARGO_BIN_EXE_dive"))
            .args(["explain", code, "--output", "json"])
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["code"], code);
        assert_eq!(report["known"], false);
        assert!(report["explanation"].is_null());
        assert!(report.get("verdict").is_none());
    }
    Ok(())
}

#[test]
fn unresolved_reference_codes_have_informational_cli_explanations()
-> Result<(), Box<dyn std::error::Error>> {
    for code in ["DIVE-E0100", "DIVE-E0101"] {
        let output = Command::new(env!("CARGO_BIN_EXE_dive"))
            .args(["explain", code, "--output", "json"])
            .output()?;
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr, [] as [u8; 0]);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["schema_version"], 4);
        assert_eq!(report["command"], "explain");
        assert_eq!(report["code"], code);
        assert_eq!(report["known"], true);
        assert!(report.get("verdict").is_none());
        let explanation = dive_api::diagnostics::explain(code).ok_or("reference explanation")?;
        assert_eq!(report["explanation"]["cause"], explanation.cause);
        assert_eq!(
            report["explanation"]["remediation"],
            explanation.remediation
        );
        let human = Command::new(env!("CARGO_BIN_EXE_dive"))
            .args(["explain", code])
            .output()?;
        assert_eq!(human.status.code(), Some(0));
        assert_eq!(human.stderr, [] as [u8; 0]);
        let human = String::from_utf8(human.stdout)?;
        assert!(human.contains(code));
        assert!(human.contains(explanation.cause));
        assert!(human.contains(explanation.remediation));
    }
    Ok(())
}
