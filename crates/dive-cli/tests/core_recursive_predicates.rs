//! Unbounded model recursion must return a structured result, never abort the CLI.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn recursive_calls_and_recursive_arguments_stop_before_host_stack_exhaustion()
-> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-api/tests/fixtures/core-let-recursive");
    for name in ["UnknownRecursive", "UnknownRecursiveArgument"] {
        for profile in ["dive", "tlc"] {
            for workers in ["1", "4"] {
                let output = Command::new(env!("CARGO_BIN_EXE_dive"))
                    .args([
                        "check",
                        "--engine",
                        "core",
                        "--output",
                        "json",
                        "--seed",
                        "0",
                        "--fp",
                        "0",
                        "--time-limit",
                        "2s",
                        "--max-memory",
                        "2g",
                        "--compat",
                        profile,
                        "--workers",
                        workers,
                    ])
                    .arg("--config")
                    .arg(root.join(format!("{name}.cfg")))
                    .arg(root.join(format!("{name}.tla")))
                    .env_remove("DIVE_COMPAT")
                    .env_remove("DIVE_ENGINE")
                    .env_remove("DIVE_CORE_OPT")
                    .env_remove("DIVE_CHAOS")
                    .output()?;
                assert_eq!(
                    output.status.code(),
                    Some(3),
                    "{name}/{profile}/{workers}: {output:?}"
                );
                let doc: Value = serde_json::from_slice(&output.stdout)?;
                assert_eq!(doc["verdict"], "unsupported");
                assert!(
                    doc["message"]
                        .as_str()
                        .is_some_and(|s| s.contains("recursive-call depth limit"))
                );
                assert!(doc["counterexample"].is_null());
                assert!(doc["tlc"].is_null());
            }
        }
    }
    Ok(())
}
