//! Rejected successor candidates stop through the native API at every width.
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn run(compat_tlc: bool, workers: usize) -> Result<Output, Box<dyn std::error::Error>> {
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/user/examples/RejectedCandidates.tla");
    let mut command = Command::new(binary);
    command.env_remove("DIVE_COMPAT");
    command.arg("check").arg(file).args([
        "--seed",
        "0",
        "--fp",
        "0",
        "--workers",
        &workers.to_string(),
        "--time-limit",
        "1s",
        "--max-memory",
        "1g",
        "--output",
        "json",
    ]);
    if compat_tlc {
        command.arg("--compat=tlc");
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(child.wait_with_output()?),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        if started.elapsed() >= Duration::from_secs(5) {
            let _ = child.kill();
            let output = child.wait_with_output()?;
            return Err(format!(
                "rejected-candidate check exceeded its 5s backstop: status {}, stderr {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn rejected_candidates_return_stopped_with_only_actual_external_progress() -> TestResult {
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            let output = run(compat_tlc, workers)?;
            assert_eq!(
                output.status.code(),
                Some(5),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(json["options"]["workers"]["requested"], workers, "{json}");
            assert_eq!(json["options"]["workers"]["used"], workers, "{json}");
            assert_eq!(json["verdict"], "stopped", "{json}");
            assert_eq!(json["stopped"]["resource"], "time", "{json}");
            assert_eq!(json["stopped"]["phase"], "safety-search", "{json}");
            assert_eq!(json["stopped"]["limit"], 1000, "{json}");
            assert!(json["stopped"]["observed"].as_u64() >= Some(1000), "{json}");
            assert_eq!(json["statistics"]["initial_generated"], 1, "{json}");
            assert_eq!(json["statistics"]["generated"], 1, "{json}");
            assert_eq!(json["statistics"]["distinct"], 1, "{json}");
            assert_eq!(json["statistics"]["depth"], 1, "{json}");
            assert_eq!(json["statistics"]["left_on_queue"], 1, "{json}");
            assert_eq!(json["statistics"]["exhaustive"], false, "{json}");
            assert!(json["counterexample"].is_null(), "{json}");
            assert!(json["tlc"].is_null(), "{json}");
            assert_ne!(json["execution"]["capture_completed"], true, "{json}");
        }
    }
    Ok(())
}
