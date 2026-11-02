//! Constructive mode and sample validation reach their explicit public paths.
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-api/tests/fixtures/generation")
}

#[test]
fn generate_flag_and_tlc_spelling_preserve_physical_exits() -> Result {
    for engine in ["reference", "core"] {
        for (prefix, exit) in [
            (vec!["simulate", "--generate", "--traces", "1"], 10),
            (vec!["tlc", "-generate", "num=1"], 12),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_dive"))
                .args(prefix)
                .args([
                    "Generation.tla",
                    "--config",
                    "Violation.cfg",
                    "--engine",
                    engine,
                    "--output",
                    "json",
                ])
                .current_dir(fixture())
                .env("DIVE_COMPAT", "dive")
                .output()?;
            assert_eq!(output.status.code(), Some(exit), "{output:?}");
            let report: Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(report["exit_code"], exit);
            assert_eq!(report["options"]["algorithm"], "constructive");
            assert_eq!(report["traces"][0]["validation"]["result"], "valid");
        }
    }
    for args in [
        vec!["-generate"],
        vec!["-generate", "num=1,file=trace"],
        vec!["-generate", "num=1", "-config", "Embedded.tla"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tlc"))
            .args(args)
            .arg("Generation.tla")
            .current_dir(fixture())
            .output()?;
        assert_eq!(output.status.code(), Some(3), "{output:?}");
    }
    Ok(())
}

#[test]
fn explicit_sample_operation_is_visible() -> Result {
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args([
            "simulate",
            "Generation.tla",
            "--config",
            "Wide.cfg",
            "--generate",
            "--traces",
            "1",
            "--depth",
            "1",
            "--output",
            "json",
        ])
        .current_dir(fixture())
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout)?;
    let path = std::env::temp_dir().join(format!(
        "dive-generation-sample-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, value["traces"][0]["document"].to_string())?;
    let checked = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args([
            "validate-trace",
            "Generation.tla",
            "--config",
            "Wide.cfg",
            "--sample",
            "--trace",
        ])
        .arg(&path)
        .args(["--output", "json"])
        .current_dir(fixture())
        .output()?;
    assert!(checked.status.success(), "{checked:?}");
    let checked: Value = serde_json::from_slice(&checked.stdout)?;
    assert_eq!(checked["validation_goal"], "sample");
    for sample in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
        command
            .args([
                "validate-trace",
                "Generation.tla",
                "--config",
                "Violation.cfg",
                "--trace",
            ])
            .arg(&path)
            .args(["--output", "json"])
            .current_dir(fixture());
        if sample {
            command.arg("--sample");
        }
        let checked = command.output()?;
        assert_eq!(
            checked.status.code(),
            Some(i32::from(sample)),
            "{checked:?}"
        );
        let checked: Value = serde_json::from_slice(&checked.stdout)?;
        assert_eq!(checked["result"], if sample { "invalid" } else { "valid" });
    }
    std::fs::remove_file(path)?;
    Ok(())
}
