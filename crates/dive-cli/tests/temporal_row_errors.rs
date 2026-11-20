//! Ordinary temporal state-row errors: TLC's report, Core's named refusal.
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn std::error::Error>>;
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
        .env_remove("DIVE_ENGINE")
        .output()
}
#[test]
fn pinned_row_errors_are_named_core_refusals() -> TestResult {
    // TLC reports each of these as a temporal state-row evaluation error
    // (exit 255, see the goldens). Core does not build that report yet and
    // refuses by name instead of approximating it.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        assert_eq!(oracle["result"]["exit_code"], 255, "{name}");
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
            assert_eq!(output.status.code(), Some(3), "{name}/{profile}: {report}");
            assert_eq!(report["schema_version"], 4);
            assert_eq!(report["verdict"], "unsupported", "{name}/{profile}");
            assert!(
                report["message"].as_str().is_some_and(
                    |message| message.ends_with("its report is not implemented on Core")
                ),
                "{name}/{profile}: {report}"
            );
            assert!(report["counterexample"].is_null());
            assert!(report["tlc"].is_null());
        }
    }
    Ok(())
}
