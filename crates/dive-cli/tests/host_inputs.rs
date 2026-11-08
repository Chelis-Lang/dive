//! External input grants: Core refuses them by name before any host operation.
#![allow(
    clippy::expect_used,
    reason = "fixture setup and behavioral assertions"
)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-cli-inputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("fixture");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-api/tests/fixtures/host-inputs");
        for suffix in ["tla", "cfg"] {
            let name = format!("Captured.{suffix}");
            std::fs::copy(source.join(&name), path.join(name)).expect("spec fixture");
        }
        std::fs::write(path.join("input.txt"), "hello\n").expect("input");
        Self(path)
    }
    fn run(&self, command: &str, tlc: bool, extra: &[&str]) -> Output {
        let modules = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-eval/tests/fixtures/community-modules");
        let mut run = Command::new(env!("CARGO_BIN_EXE_dive"));
        run.env_clear()
            .envs([("DIVE_COUNT", "2"), ("DIVE_LABEL", "outside")])
            .current_dir(&self.0)
            .arg(command)
            .arg("Captured.tla")
            .args([
                "--output",
                "json",
                "--seed",
                "7",
                "--fp",
                "0",
                "--library-path",
            ])
            .arg(modules)
            .args(extra);
        if tlc {
            run.arg("--compat=tlc");
        }
        run.output().expect("CLI")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("JSON output")
}
fn status(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_categories_and_invalid_flags_never_run_a_spec_host_operation() {
    for tlc in [false, true] {
        let fixture = Fixture::new();
        for categories in [None, Some("env"), Some("files"), Some("exec")] {
            let flags = categories.map(|categories| format!("--allow-io={categories}"));
            let args = flags.as_deref().map(|s| vec![s]).unwrap_or_default();
            let output = fixture.run("check", tlc, &args);
            status(&output, 3);
            if categories.is_none() {
                // Without a grant the model refuses, naming the grant it needs.
                let report = json(&output);
                assert_eq!(report["verdict"], "unsupported");
                assert!(
                    report["message"]
                        .as_str()
                        .expect("refusal")
                        .contains("--allow-io=")
                );
            } else {
                // Core implements no external inputs: a grant is refused.
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("not implemented on Core")
                );
            }
            assert_eq!(
                std::fs::read_to_string(fixture.0.join("input.txt")).expect("original input"),
                "hello\n"
            );
        }
        for categories in ["", "network", "env,env"] {
            let output = fixture.run("check", tlc, &[&format!("--allow-io={categories}")]);
            status(&output, 2);
        }
    }
}

#[test]
fn the_default_engine_refuses_external_inputs_by_name() {
    let fixture = Fixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .env_clear()
        .current_dir(&fixture.0)
        .args([
            "check",
            "Captured.tla",
            "--allow-io=env,files",
            "--workers",
            "1",
        ])
        .args(["--library-path"])
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../dive-eval/tests/fixtures/community-modules"),
        )
        .output()
        .expect("dive runs");
    status(&output, 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("external inputs"), "{stderr}");
    assert!(stderr.contains("not implemented on Core"), "{stderr}");
}
