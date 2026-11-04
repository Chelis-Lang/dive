//! A real source-coverage CLI run reports source locations, native counts and limits.
use std::process::Command;
#[test]
fn coverage_reports_native_work_and_keeps_unimplemented_tlc_property_roles_refused()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples");
    let file = root.join("Counter.tla");
    for compat in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
        command.args([
            "check",
            file.to_str().ok_or("path")?,
            "--source-coverage",
            "--engine",
            "auto",
            "--workers",
            "4",
            "--seed",
            "7",
            "--fp",
            "0",
            "--output",
            "json",
        ]);
        if compat {
            command.arg("--compat=tlc");
        }
        let output = command.output()?;
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(doc["verdict"], "success");
        assert_eq!(doc["engine"], "reference");
        assert_eq!(doc["options"]["workers"]["requested"], 4);
        assert_eq!(doc["options"]["workers"]["used"], 1);
        assert_eq!(doc["source_coverage"]["metric"], "native-source-work");
        assert_eq!(doc["source_coverage"]["tlc_cost_equivalent"], false);
        assert!(
            doc["source_coverage"]["rows"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row["module"] == "Counter"
                    && row["counts"]["evaluations"].as_u64().is_some_and(|n| n > 0)))
        );
    }
    let refused = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["tlc", "-coverage", "0", file.to_str().ok_or("path")?])
        .output()?;
    assert_eq!(refused.status.code(), Some(3));
    Ok(())
}
