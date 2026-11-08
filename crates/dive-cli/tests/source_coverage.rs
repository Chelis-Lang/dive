//! Core refuses source coverage and TLC logical coverage by name.
use std::process::Command;
#[test]
fn source_and_tlc_coverage_are_named_core_refusals() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples");
    let file = root.join("Counter.tla");
    for compat in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
        command
            .args(["check", file.to_str().ok_or("path")?, "--source-coverage"])
            .env_remove("DIVE_ENGINE")
            .env_remove("DIVE_COMPAT");
        if compat {
            command.arg("--compat=tlc");
        }
        let output = command.output()?;
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("source coverage and TLC logical coverage (--source-coverage, --coverage) are not implemented on Core")
        );
    }
    let refused = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["tlc", "-coverage", "0", file.to_str().ok_or("path")?])
        .output()?;
    assert_eq!(refused.status.code(), Some(3));
    Ok(())
}
