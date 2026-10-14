//! Explicit final selection must leave genuine no-property safety checks intact.
use std::path::PathBuf;
use std::process::{Command, Output};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "dive-empty-final-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&dir)?;
    Ok(Scratch(dir))
}
fn run(
    file: &std::path::Path,
    profile: &str,
    workers: usize,
    final_mode: bool,
) -> Result<Output, std::io::Error> {
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
fn final_flags_without_properties_match_ordinary_safety_observables()
-> Result<(), Box<dyn std::error::Error>> {
    let root = scratch()?;
    for (name, body, extra) in [
        (
            "Success",
            "Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x",
            "",
        ),
        (
            "Initial",
            "Init == x=0\nNext == x'=x\nInv == x=1",
            "INVARIANT Inv\n",
        ),
        (
            "Invariant",
            "Init == x=0\nNext == x'=x+1\nInv == x=0",
            "INVARIANT Inv\n",
        ),
        ("Eval", "Init == x=0\nNext == x'=1\\div0", ""),
    ] {
        let file = root.0.join(format!("{name}.tla"));
        std::fs::write(
            &file,
            format!("---- MODULE {name} ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n"),
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            format!("INIT Init\nNEXT Next\n{extra}"),
        )?;
        for profile in ["dive", "tlc"] {
            for workers in [1, 4] {
                let ordinary = run(&file, profile, workers, false)?;
                let selected = run(&file, profile, workers, true)?;
                assert_eq!(
                    ordinary.status.code(),
                    selected.status.code(),
                    "{name}/{profile}/{workers}"
                );
                let a: serde_json::Value = serde_json::from_slice(&ordinary.stdout)?;
                let b: serde_json::Value = serde_json::from_slice(&selected.stdout)?;
                for key in [
                    "verdict",
                    "statistics",
                    "property",
                    "counterexample",
                    "output",
                    "diagnostics",
                    "error_site",
                    "tlc",
                    "exit_code",
                ] {
                    assert_eq!(a[key], b[key], "{name}/{profile}/{workers}/{key}");
                }
                assert_eq!(b["execution"]["stage"], "capture");
                assert_eq!(b["execution"]["capture_completed"], false);
                assert_eq!(b["options"]["lncheck"], "final");
                assert_eq!(a["options"]["workers"], b["options"]["workers"]);
                assert_ne!(b["verdict"], "unsupported");
            }
        }
    }
    Ok(())
}
#[test]
fn genuine_temporal_property_keeps_final_violation_and_refuses_ordinary_mode()
-> Result<(), Box<dyn std::error::Error>> {
    let root = scratch()?;
    let file = root.0.join("HasProperty.tla");
    std::fs::write(
        &file,
        "---- MODULE HasProperty ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == []<>(x=1)\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    for profile in ["dive", "tlc"] {
        let ordinary = run(&file, profile, 1, false)?;
        let selected = run(&file, profile, 1, true)?;
        assert_eq!(ordinary.status.code(), Some(3));
        assert_eq!(selected.status.code(), Some(13));
        let a: serde_json::Value = serde_json::from_slice(&ordinary.stdout)?;
        let b: serde_json::Value = serde_json::from_slice(&selected.stdout)?;
        assert_eq!(a["verdict"], "unsupported");
        assert_eq!(b["verdict"], "liveness-violation");
        assert_eq!(b["property"], "P");
        assert!(b["counterexample"].is_object());
    }
    Ok(())
}
