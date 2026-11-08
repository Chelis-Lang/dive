//! TLC logical coverage: both argument transports reach Core's named refusal.
use std::{path::Path, process::Command};

#[test]
fn real_cli_and_tlc_transport_refuse_logical_costs_by_name()
-> Result<(), Box<dyn std::error::Error>> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/tlc-coverage-primary/specifications/Coverage/Multi.tla");
    for args in [
        vec!["check", "--coverage", "0"],
        vec!["check", "--coverage", "0", "--compat=tlc"],
        vec!["tlc", "-coverage", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dive"))
            .env_remove("DIVE_ENGINE")
            .args(&args)
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
        assert_eq!(output.status.code(), Some(3), "{args:?}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(
                "source coverage and TLC logical coverage (--source-coverage, --coverage) are not implemented on Core"
            ),
            "{args:?}: {output:?}"
        );
    }
    let unsupported = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["tlc", "-coverage", "1"])
        .arg(&file)
        .output()?;
    assert_eq!(unsupported.status.code(), Some(3));
    assert!(String::from_utf8(unsupported.stderr)?.contains("final report only"));
    Ok(())
}
