//! Saved invariant-error words pass the standalone validator only with exact claims.
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn standalone_invariant_error_validation_preserves_cause_and_rejects_forgery() -> TestResult {
    let scratch = Scratch(std::env::temp_dir().join(format!(
            "dive-invariant-error-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        )));
    std::fs::create_dir(&scratch.0)?;
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-api/tests/fixtures/core-lib/specs/ErAssertInv/ErAssertInv.tla");
    let trace = scratch.0.join("trace.json");
    for profile in ["dive", "tlc"] {
        let run = |operation: &str, extra: &[&std::ffi::OsStr]| {
            Command::new(env!("CARGO_BIN_EXE_dive"))
                .arg(operation)
                .arg(&file)
                .arg(format!("--compat={profile}"))
                .args(["--output", "json"])
                .args(extra)
                .env_remove("DIVE_COMPAT")
                .env_remove("DIVE_CHAOS")
                .output()
        };
        let output = run(
            "check",
            &[
                "--engine=core".as_ref(),
                "--workers=1".as_ref(),
                "--seed=0".as_ref(),
                "--fp=0".as_ref(),
            ],
        )?;
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output.status.code(), Some(76), "{report}");
        assert_eq!(report["tlc"]["code"], 2111);
        let original = report["counterexample"]["trace"].clone();
        assert_eq!(original["ending"]["kind"], "invariant-error");
        assert_eq!(original["version"], 2);
        assert_eq!(report["schema_version"], 4);
        assert_eq!(original["ending"]["ec"], "TLC_VALUE_ASSERT_FAILED");
        for (mutation, expected) in [
            (None, "valid"),
            (Some(("ec", "TLC_INVARIANT_EVALUATION_FAILED")), "invalid"),
            (Some(("name", "absent")), "invalid"),
        ] {
            let mut document = original.clone();
            if let Some((key, value)) = mutation {
                document["ending"][key] = value.into();
            }
            std::fs::write(&trace, serde_json::to_vec(&document)?)?;
            let output = run("validate-trace", &["--trace".as_ref(), trace.as_os_str()])?;
            let validation: Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(validation["result"], expected, "{validation}");
            assert_eq!(output.status.success(), expected == "valid", "{validation}");
        }
    }
    Ok(())
}
