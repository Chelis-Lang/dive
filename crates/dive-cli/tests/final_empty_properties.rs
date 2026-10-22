//! Explicit final selection must leave genuine no-property safety checks intact.
use std::path::PathBuf;
use std::process::{Command, Output};

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};

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
fn genuine_temporal_property_keeps_explicit_and_ordinary_profile_contracts()
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
        assert_eq!(ordinary.status.code(), Some(13));
        assert_eq!(selected.status.code(), Some(13));
        let a: serde_json::Value = serde_json::from_slice(&ordinary.stdout)?;
        let b: serde_json::Value = serde_json::from_slice(&selected.stdout)?;
        let validator = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: profile == "tlc",
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )?;
        if profile == "tlc" {
            assert_eq!(a["schema_version"], 3);
            assert_eq!(a["options"]["profile"], profile);
            assert_eq!(a["options"]["lncheck"], "final");
            assert_eq!(a["verdict"], "liveness-violation");
            assert_eq!(a["property"], "P");
            assert_eq!(a["tlc"]["code"], 2116);
            assert_eq!(a["tlc"]["name"], "TLC_TEMPORAL_PROPERTY_VIOLATED");
            assert_eq!(a["tlc"]["verified"], true);
            assert_eq!(a["tlc"]["exit_status"], 13);
            assert_eq!(a["counterexample"]["validation"], "valid");
            let document = TraceDoc::from_json(&a["counterexample"]["trace"])?;
            assert_eq!(validator.read(&document).0.validation, Validation::Valid);
        } else {
            assert_eq!(a["schema_version"], 3);
            assert_eq!(a["options"]["profile"], profile);
            assert_eq!(a["options"]["lncheck"], "deterministic");
            assert_eq!(a["verdict"], "liveness-violation");
            assert_eq!(a["property"], "P");
            assert_eq!(a["tlc"]["code"], 2116);
            assert_eq!(a["tlc"]["name"], "TLC_TEMPORAL_PROPERTY_VIOLATED");
            assert_eq!(a["tlc"]["verified"], true);
            assert_eq!(a["tlc"]["exit_status"], 13);
            assert_eq!(a["counterexample"]["validation"], "valid");
            assert_eq!(a["execution"]["capture_completed"], true);
            assert_eq!(a["periodic"]["prefix_checks"], 0);
            assert_eq!(a["periodic"]["exhausted"], true);
            assert_eq!(a["periodic"]["final_check"], true);
            let document = TraceDoc::from_json(&a["counterexample"]["trace"])?;
            assert_eq!(validator.read(&document).0.validation, Validation::Valid);
        }
        assert_eq!(b["verdict"], "liveness-violation");
        assert_eq!(b["property"], "P");
        assert!(b["counterexample"].is_object());
        let document = TraceDoc::from_json(&b["counterexample"]["trace"])?;
        assert_eq!(validator.read(&document).0.validation, Validation::Valid);
    }
    Ok(())
}
