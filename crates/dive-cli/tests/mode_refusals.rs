//! `dive simulate` and `dive dfid` refusals exit 3 and name the refused
//! feature in human and JSON output, in both profiles (#956).
use std::path::PathBuf;
use std::process::Command;

/// Removes a per-test scratch directory of specifications.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> Result<Scratch, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "dive-cli-mode-refusals-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos(),
    ));
    std::fs::create_dir(&dir)?;
    std::fs::write(
        dir.join("Counter.tla"),
        "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
         Next == x' = IF x < 4 THEN x + 1 ELSE x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\n\
         EventuallyFour == <>(x = 4)\n====\n",
    )?;
    std::fs::write(dir.join("Counter.cfg"), "INIT Init\nNEXT Next\n")?;
    std::fs::write(
        dir.join("Temporal.cfg"),
        "SPECIFICATION Spec\nPROPERTY EventuallyFour\n",
    )?;
    Ok(Scratch(dir))
}

const CASES: &[(&[&str], &str)] = &[
    (
        &["simulate", "Counter.tla", "--traces", "2", "--workers", "2"],
        "parallel simulation workers",
    ),
    (
        &["dfid", "Counter.tla", "--max-level", "6", "--workers", "2"],
        "DFID multiple workers",
    ),
    (
        &[
            "dfid",
            "Counter.tla",
            "--config",
            "Temporal.cfg",
            "--max-level",
            "6",
        ],
        "DFID temporal properties and fairness",
    ),
];

#[test]
fn simulate_and_dfid_refusals_exit_three_and_name_the_feature_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    for (arguments, feature) in CASES {
        for profile in [None, Some("--compat=tlc")] {
            for json in [false, true] {
                let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
                command
                    .args(*arguments)
                    .args(profile)
                    .current_dir(&scratch.0)
                    .env_remove("DIVE_COMPAT")
                    .env_remove("DIVE_ENGINE")
                    .env_remove("DIVE_CHAOS");
                if json {
                    command.args(["--output", "json"]);
                }
                let output = command.output()?;
                let case = format!("{arguments:?} {profile:?} json={json}");
                assert_eq!(output.status.code(), Some(3), "{case}: {output:?}");
                let stdout = String::from_utf8(output.stdout)?;
                if json {
                    let report: serde_json::Value = serde_json::from_str(&stdout)?;
                    assert_eq!(report["result"], "unsupported", "{case}: {stdout}");
                    assert_eq!(report["message"], *feature, "{case}: {stdout}");
                    assert_eq!(report["exit_code"], 3, "{case}: {stdout}");
                    assert_eq!(report["model_proved"], false, "{case}: {stdout}");
                } else {
                    assert!(
                        stdout
                            .lines()
                            .any(|line| line == format!("unsupported: {feature}")),
                        "{case} does not name {feature}: {stdout}"
                    );
                }
            }
        }
    }
    Ok(())
}
