//! Compare the depth-zero finite query with ordinary concrete enumeration.
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(path: &Path, extra: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .arg("check")
        .arg(path)
        .args(["--output", "json", "--compat", "dive", "--time-limit", "5s"])
        .args(extra)
        .output()?;
    assert!(
        !output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn finite_function_assignments_agree_with_every_small_permutation_outcome()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        Directory(std::env::temp_dir().join(format!("dive-initial-query-{}", std::process::id())));
    std::fs::create_dir(&directory.0)?;
    let file = directory.0.join("Orders.tla");
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT P\nCHECK_DEADLOCK FALSE\n",
    )?;
    let source = r"---- MODULE Orders ----
EXTENDS Naturals
VARIABLES left, right
Perms == {f \in [1..2 -> {1, 2}] : f[1] /= f[2]}
Init == left \in Perms /\ right \in Perms
Next == UNCHANGED <<left,right>>
P == ~(left[1] = L /\ right[1] = R)
====
";
    for left in 1..=3 {
        for right in 1..=3 {
            let text = source
                .replace("= L", &format!("= {left}"))
                .replace("= R", &format!("= {right}"));
            std::fs::write(&file, text)?;
            let ordinary = check(&file, &["--workers", "1", "--store", "exact"])?;
            let initial = check(&file, &["--strategy", "bounded-path", "--path-bound", "0"])?;
            if left <= 2 && right <= 2 {
                assert_eq!(ordinary["verdict"], "invariant-violation");
                assert_eq!(initial["result"], "invariant-violation", "{initial}");
                assert_eq!(initial["initial_query"]["status"], "sat");
                assert_eq!(initial["violation_validated"], true);
                assert_eq!(
                    initial["counterexample"]["trace"]["states"],
                    ordinary["counterexample"]["trace"]["states"]
                );
            } else {
                assert_eq!(ordinary["verdict"], "success");
                assert_eq!(ordinary["statistics"]["distinct"], 4);
                assert_eq!(initial["result"], "bounded-safe", "{initial}");
                assert_eq!(initial["initial_query"]["status"], "unsat");
                assert_eq!(
                    initial["statistics"]["checked_layers"],
                    serde_json::json!([4])
                );
            }
        }
    }
    // A demanded bad function index cannot disappear in the joined query.
    std::fs::write(
        &file,
        source
            .replace("= L", "= 1")
            .replace("right[1] = R", "right[3] = 1"),
    )?;
    let demanded = check(&file, &["--strategy", "bounded-path", "--path-bound", "0"])?;
    assert!(
        matches!(
            demanded["result"].as_str(),
            Some("evaluation-error" | "unsupported")
        ),
        "{demanded}"
    );
    assert!(demanded["initial_query"].is_null());
    Ok(())
}
