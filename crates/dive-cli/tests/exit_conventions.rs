//! Actual process statuses are independent of evaluator profile selection.
use std::path::PathBuf;
use std::process::{Command, Output};
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch, Box<dyn std::error::Error>> {
    let p = std::env::temp_dir().join(format!(
        "dive-cli-exits-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&p)?;
    Ok(Scratch(p))
}
fn run(file: &std::path::Path, args: &[&str], env: Option<&str>) -> Result<Output, std::io::Error> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_dive"));
    c.arg("check")
        .arg(file)
        .args(["--workers=1", "--seed=0", "--fp=0"])
        .args(args)
        .env_remove("DIVE_COMPAT");
    if let Some(v) = env {
        c.env("DIVE_COMPAT", v);
    }
    c.output()
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "actual environment/profile/convention/argv-order matrix"
)]
fn default_and_native_override_preserve_profiles_in_both_argument_orders()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("Conventions.tla");
    std::fs::write(
        &file,
        "---- MODULE Conventions ----\nVARIABLE x\nInit == x=0\nNext == x'=1\nInv == x=0\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    for env in [None, Some("tlc"), Some("dive")] {
        for flags in [vec![], vec!["--compat=dive"], vec!["--compat=tlc"]] {
            for (exit_flag, exit) in [(None, 12), (Some("native"), 10), (Some("tlc"), 12)] {
                for before in [false, true] {
                    let mut args = flags.clone();
                    let option = exit_flag.map(|s| format!("--exit-codes={s}"));
                    if let Some(ref s) = option {
                        if before {
                            args.insert(0, s);
                        } else {
                            args.push(s);
                        }
                    }
                    args.push("--output=json");
                    let out = run(&file, &args, env)?;
                    assert_eq!(out.status.code(), Some(exit));
                    let doc: serde_json::Value = serde_json::from_slice(&out.stdout)?;
                    let compat = flags
                        .last()
                        .map_or(env == Some("tlc"), |s| *s == "--compat=tlc");
                    assert_eq!(doc["exit_code"], exit);
                    assert_eq!(doc["options"]["exit_codes"], exit_flag.unwrap_or("tlc"));
                    assert_eq!(
                        doc["options"]["semantic_profile"],
                        if compat { "tlc" } else { "dive" }
                    );
                    assert_eq!(
                        doc["options"]["profile"],
                        if compat && exit == 10 {
                            "tlc+overrides"
                        } else if compat {
                            "tlc"
                        } else {
                            "dive"
                        }
                    );
                    assert_eq!(doc["options"]["int"], if compat { "tlc" } else { "big" });
                    assert_eq!(
                        doc["options"]["choose_order"],
                        if compat { "tlc" } else { "canonical" }
                    );
                    assert_eq!(doc["tlc"]["exit_status"], 12);
                    assert_eq!(doc["counterexample"]["length"], 2);
                }
            }
        }
    }
    for (flags, exit, profile) in [
        (vec![], 12, "dive"),
        (vec!["--exit-codes=native"], 10, "dive"),
        (
            vec!["--compat=tlc", "--exit-codes=native"],
            10,
            "tlc+overrides",
        ),
    ] {
        let out = run(&file, &flags, None)?;
        assert_eq!(out.status.code(), Some(exit));
        let text = String::from_utf8(out.stdout)?;
        assert!(text.contains(&format!("profile {profile},")));
        assert!(text.contains(&format!("process status {exit}")));
    }
    for flags in [vec!["--exit-codes=wrong"], vec!["--exit-codes"]] {
        assert_eq!(run(&file, &flags, None)?.status.code(), Some(2));
    }
    Ok(())
}
#[test]
fn frontend_rejections_use_physical_tlc_codes_and_native_remains_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("Rejected.tla");
    for source in [
        "---- MODULE Rejected ----\nx == @@@\n====\n",
        "---- MODULE Rejected ----\nx == missing\n====\n",
    ] {
        std::fs::write(&file, source)?;
        for compat in ["--compat=dive", "--compat=tlc"] {
            for (convention, status) in [("tlc", 150), ("native", 1)] {
                let option = format!("--exit-codes={convention}");
                let out = run(&file, &[compat, &option, "--output=json"], None)?;
                assert_eq!(out.status.code(), Some(status));
                let doc: serde_json::Value = serde_json::from_slice(&out.stdout)?;
                assert_eq!(doc["exit_code"], status);
                assert_eq!(doc["verdict"], "rejected");
                assert_eq!(doc["tlc"]["code"], 3002);
            }
        }
    }
    Ok(())
}
