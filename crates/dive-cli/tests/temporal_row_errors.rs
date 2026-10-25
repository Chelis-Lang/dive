//! Ordinary temporal errors carry independently replayable finite endpoints.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "dive-row-endpoint-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&path)?;
    Ok(Scratch(path))
}
fn command(
    operation: &str,
    file: &Path,
    profile: &str,
    extra: &[&std::ffi::OsStr],
) -> Result<Output, std::io::Error> {
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    Command::new(binary)
        .arg(operation)
        .arg(file)
        .arg(format!("--compat={profile}"))
        .arg("--output")
        .arg("json")
        .args(extra)
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_CHAOS")
        .output()
}
#[test]
fn pinned_row_errors_keep_physical_status_counts_and_standalone_trace_validation() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temporary = scratch()?;
    for (corpus, name) in [
        ("selectseq-concat-type-capability", "ConcatFirstTypeError"),
        ("selectseq-concat-type-capability", "ConcatSecondTypeError"),
        (
            "selectseq-concat-type-capability",
            "RightChildErrorBeforeKernel",
        ),
        ("liveness-atom-error-timing", "DelayedSiblingStateError"),
        ("temporal-demand-primary-probes", "CurrentStateBinPriority"),
    ] {
        let file = root.join(format!(
            "conformance/corpus/{corpus}/specifications/{name}/{name}.tla"
        ));
        let oracle:Value=serde_json::from_slice(&std::fs::read(root.join(format!("conformance/golden/tlc-2026.10.06.014338/{corpus}/specifications/{name}/{name}.cfg.json")))?)?;
        for profile in ["dive", "tlc"] {
            let extra = [
                "--workers=1",
                "--seed=0",
                "--fp=0",
                "--time-limit=60s",
                "--max-memory=512m",
            ]
            .map(std::ffi::OsStr::new);
            let output = command("check", &file, profile, &extra)?;
            let report: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
                format!(
                    "{name}/{profile}/check: invalid JSON: {error}; status={}; stderr={:?}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )
            })?;
            assert_eq!(
                output.status.code(),
                Some(255),
                "{name}/{profile}: {report}"
            );
            assert_eq!(report["schema_version"], 3);
            assert_eq!(report["check_kind"], "temporal");
            assert_eq!(report["verdict"], oracle["result"]["verdict"]);
            assert_eq!(report["tlc"]["code"], oracle["result"]["verdict_code"]);
            assert_eq!(report["tlc"]["name"], oracle["result"]["error_class"]);
            assert_eq!(report["exit_code"], oracle["result"]["exit_code"]);
            for (field, original) in [
                ("distinct", "distinct_states"),
                ("generated", "states_generated"),
                ("left_on_queue", "states_left_on_queue"),
                ("depth", "depth"),
            ] {
                assert_eq!(
                    report["statistics"][field], oracle["result"]["counts"][original],
                    "{name}/{profile}/{field}"
                );
            }
            assert_eq!(report["execution"]["capture_completed"], false);
            assert!(report["property"].is_null());
            assert_eq!(report["error_site"], "temporal-state-row");
            assert_eq!(report["counterexample"]["validation"], "valid");
            let document = &report["counterexample"]["trace"];
            assert_eq!(document["ending"]["kind"], "error");
            assert_eq!(
                document["ending"]["endpoint"]["stage"],
                "temporal-state-row"
            );
            assert!(document["properties"].is_null());
            let trace = temporary.0.join("trace.json");
            std::fs::write(&trace, serde_json::to_vec(document)?)?;
            let validation = command(
                "validate-trace",
                &file,
                profile,
                &[std::ffi::OsStr::new("--trace"), trace.as_os_str()],
            )?;
            let report: Value = serde_json::from_slice(&validation.stdout).map_err(|error| {
                format!(
                    "{name}/{profile}/validate-trace: invalid JSON: {error}; status={}; stderr={:?}",
                    validation.status,
                    String::from_utf8_lossy(&validation.stderr)
                )
            })?;
            assert!(validation.status.success(), "{name}/{profile}: {report}");
            assert_eq!(report["result"], "valid", "{name}/{profile}: {report}");
        }
    }
    Ok(())
}
