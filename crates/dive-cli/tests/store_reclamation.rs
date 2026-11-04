//! Real CLI value reclamation and recovery keep the observed safety result.
use std::path::Path;
use std::process::Command;

fn check(
    file: &Path,
    flags: &[&str],
    tlc: bool,
) -> Result<(i32, serde_json::Value), Box<dyn std::error::Error>> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
    command.arg("check").arg(file).args([
        "--engine", "core", "--seed", "7", "--fp", "0", "--output", "json",
    ]);
    command.args(flags);
    if tlc {
        command.arg("--compat=tlc");
    }
    let output = command.output()?;
    Ok((
        output.status.code().ok_or("signal")?,
        serde_json::from_slice(&output.stdout)?,
    ))
}

#[test]
fn command_and_fresh_process_recovery_reclaim_values() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let counter = root.join("../../docs/user/examples/Counter.tla");
    let chain = root.join("../dive-api/tests/fixtures/core-retention/RetainedChain.tla");
    for tlc in [false, true] {
        let (_, smoke) = check(
            &counter,
            &[
                "--store-compact-bytes",
                "0",
                "--frontier-memory-bytes",
                "0",
                "--predecessor-memory-bytes",
                "0",
            ],
            tlc,
        )?;
        assert_eq!(smoke["verdict"], "success");
        let (base_exit, base) = check(&chain, &["--store", "exact", "--workers", "4"], tlc)?;
        let archive =
            std::env::temp_dir().join(format!("dive-cli-reclaim-{}-{tlc}.cp", std::process::id()));
        let archive_text = archive.to_str().ok_or("archive path")?;
        let (_, paused) = check(
            &chain,
            &[
                "--store",
                "exact",
                "--checkpoint",
                archive_text,
                "--checkpoint-after-levels",
                "8",
                "--checkpoint-counters",
                "retain",
            ],
            tlc,
        )?;
        assert_eq!(
            paused["checkpoint"]["status"], "paused-incomplete",
            "{paused}"
        );
        let (exit, actual) = check(
            &chain,
            &[
                "--store",
                "exact",
                "--workers",
                "8",
                "--recover",
                archive_text,
                "--checkpoint-counters",
                "retain",
                "--store-compact-bytes",
                "0",
                "--frontier-memory-bytes",
                "0",
                "--predecessor-memory-bytes",
                "0",
            ],
            tlc,
        )?;
        assert_eq!(exit, base_exit);
        for field in ["verdict", "property", "statistics", "counterexample", "tlc"] {
            assert_eq!(actual[field], base[field], "{field}");
        }
        assert_eq!(actual["checkpoint"]["status"], "recovered");
        assert!(
            actual["core"]["metrics"]["store_reclamation"]["collections"]
                .as_u64()
                .is_some_and(|n| n >= 50)
        );
        std::fs::remove_file(archive)?;
    }
    Ok(())
}
