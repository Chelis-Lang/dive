//! Configured safety-role final selection preserves real CLI statuses and stages.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(file: &Path, profile: &str, workers: usize, final_mode: bool) -> std::io::Result<Output> {
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    let mut command = Command::new(binary);
    command
        .arg("check")
        .arg(file)
        .args(["--output=json", "--seed=0", "--fp=0"])
        .arg(format!("--workers={workers}"))
        .arg(format!("--compat={profile}"))
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_CHAOS");
    if final_mode {
        command.args(["--lncheck=final", "--lasso=short"]);
    }
    command.output()
}

#[test]
fn real_configured_role_exits_observables_and_error_origins_match_ordinary()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "dive-final-roles-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&dir)?;
    let scratch = Scratch(dir);
    for (body, extra, expected, stage) in [
        (
            "Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == x=0 /\\ [](x<=2) /\\ [][x'>=x]_x",
            "",
            0,
            "capture",
        ),
        ("Init == x=0\nNext == x'=x\nP == x=1", "", 13, "capture"),
        (
            "Init == x=0\nNext == x'=(x+1)%2\nP == [][x'=x+1]_x",
            "",
            13,
            "capture",
        ),
        (
            "Init == x=0\nNext == x'=x\nP == x=0\nSym == TRUE",
            "SYMMETRY Sym\n",
            255,
            "binding",
        ),
        (
            "Init == x=0\nNext == x'=x\nP == x=0\nPost == TRUE",
            "POSTCONDITION Post\n",
            255,
            "capture",
        ),
    ] {
        let file = scratch.0.join("Roles.tla");
        std::fs::write(
            &file,
            format!("---- MODULE Roles ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n"),
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            format!("INIT Init\nNEXT Next\nPROPERTY P\n{extra}"),
        )?;
        for profile in ["dive", "tlc"] {
            for workers in [1, 4] {
                let ordinary = run(&file, profile, workers, false)?;
                let final_mode = run(&file, profile, workers, true)?;
                assert_eq!(ordinary.status.code(), Some(expected));
                assert_eq!(final_mode.status.code(), Some(expected));
                let a: serde_json::Value = serde_json::from_slice(&ordinary.stdout)?;
                let b: serde_json::Value = serde_json::from_slice(&final_mode.stdout)?;
                for key in [
                    "verdict",
                    "property",
                    "statistics",
                    "counterexample",
                    "output",
                    "diagnostics",
                    "error_site",
                    "tlc",
                    "exit_code",
                ] {
                    assert_eq!(a[key], b[key], "{body}/{profile}/{workers}/{key}");
                }
                assert_eq!(b["execution"]["stage"], stage);
                assert_eq!(b["execution"]["capture_completed"], false);
            }
        }
    }
    Ok(())
}
