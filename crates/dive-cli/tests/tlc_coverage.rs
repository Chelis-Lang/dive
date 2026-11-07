//! The native and TLC argument transports drive the same actual logical collector.
use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn real_cli_and_tlc_transport_report_multi_variable_logical_costs()
-> Result<(), Box<dyn std::error::Error>> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/tlc-coverage-primary/specifications/Coverage/Multi.tla");
    for args in [
        vec!["check", "--coverage", "0"],
        vec!["check", "--coverage", "0", "--compat=tlc"],
        vec!["tlc", "-coverage", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dive"))
            // TLC logical coverage is implemented on the reference engine only.
            .env("DIVE_ENGINE", "reference")
            .args(args)
            .arg(&file)
            .args([
                "--seed",
                "7",
                "--fp",
                "0",
                "--workers",
                "4",
                "--output",
                "json",
            ])
            .output()?;
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(doc["verdict"], "success");
        assert_eq!(doc["statistics"]["distinct"], 3);
        assert_eq!(doc["statistics"]["generated"], 4);
        assert_eq!(doc["options"]["workers"]["used"], 1);
        assert_eq!(doc["tlc_coverage"]["status"], "available");
        assert_eq!(
            doc["tlc_coverage"]["actions"]
                .as_array()
                .ok_or("actions")?
                .len(),
            3
        );
        assert!(doc.get("source_coverage").is_none());
    }
    let unsupported = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["tlc", "-coverage", "1"])
        .arg(&file)
        .output()?;
    assert_eq!(unsupported.status.code(), Some(3));
    assert!(String::from_utf8(unsupported.stderr)?.contains("final report only"));
    Ok(())
}
