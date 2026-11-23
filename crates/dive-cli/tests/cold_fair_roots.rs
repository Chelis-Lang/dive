//! Cold searches must reach a later component without granting a holding verdict.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn search(file: &Path, extra: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let out = Command::new(env!("CARGO_BIN_EXE_dive"))
        .arg("check")
        .arg(file)
        .args([
            "--compat=dive",
            "--output=json",
            "--seed=8",
            "--fp=0",
            "--workers=1",
            "--lncheck=cold-fair",
            "--max-states=30",
            "--periodic-min-states=1",
        ])
        .args(extra)
        .output()?;
    assert!(
        !out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(serde_json::from_slice(&out.stdout)?)
}

#[test]
fn cold_selection_visits_later_distinct_components_and_replays_original_fairness()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        Directory(std::env::temp_dir().join(format!("dive-cold-roots-{}", std::process::id())));
    fs::create_dir(&directory.0)?;
    let file = directory.0.join("Components.tla");
    fs::write(
        file.with_extension("cfg"),
        "SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n",
    )?;
    let source = r"---- MODULE Components ----
EXTENDS Naturals
VARIABLES component, n
vars == <<component, n>>
A == component = 0 /\ n = 0
B == component = 1 /\ n = 0
Init == ORDER
Next == /\ UNCHANGED component
        /\ CASE component = 0 -> n' = (n + 1) % 1000
           [] component = 1 -> n' = 1 - n
           [] OTHER -> n' = 0
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)
Live == <>(component = 0 \/ n = 99)
====
";
    for (order, first_verdict, bad_ordinal) in [
        (r"A \/ A \/ B", "stopped", 2),
        (r"B \/ B \/ A", "liveness-violation", 0),
    ] {
        fs::write(&file, source.replace("ORDER", order))?;
        let first = search(&file, &[])?;
        assert_eq!(first["verdict"], first_verdict);
        let report = search(&file, &["--cold-roots=2", "--cold-init-emissions=12"])?;
        assert_eq!(report["verdict"], "liveness-violation", "{report}");
        assert_eq!(report["model_proved"], false);
        let roots = report["initial_roots"]["roots"]
            .as_array()
            .ok_or("missing roots")?;
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[usize::from(bad_ordinal != 0)]["ordinal"], bad_ordinal);
        assert_eq!(report["initial_roots"]["emissions"], 3);
        assert!(
            roots
                .iter()
                .all(|root| root["expanded"].as_u64().is_some_and(|n| n > 0))
        );
        let mut word = report["counterexample"]["trace"].clone();
        assert_eq!(word["ending"]["kind"], "lasso");
        assert_eq!(word["properties"], serde_json::json!(["Live"]));
        let witness = directory.0.join("word.json");
        for expected in ["valid", "invalid"] {
            fs::write(&witness, serde_json::to_vec(&word)?)?;
            let validation = Command::new(env!("CARGO_BIN_EXE_dive"))
                .arg("validate-trace")
                .arg(&file)
                .arg("--trace")
                .arg(&witness)
                .args(["--output", "json", "--seed", "8", "--fp", "0"])
                .output()?;
            assert!(
                !validation.stdout.is_empty(),
                "trace validation status {}: {}",
                validation.status,
                String::from_utf8_lossy(&validation.stderr)
            );
            let validation: Value = serde_json::from_slice(&validation.stdout)?;
            assert_eq!(validation["result"], expected, "{validation}");
            word["ending"]["kind"] = serde_json::json!("stuttering");
            word["ending"]
                .as_object_mut()
                .ok_or("missing ending")?
                .remove("back_to");
        }
    }
    Ok(())
}

#[test]
fn seeded_windows_select_from_many_roots_with_reproducible_work()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        Directory(std::env::temp_dir().join(format!("dive-cold-seed-{}", std::process::id())));
    fs::create_dir(&directory.0)?;
    let file = directory.0.join("Channel.tla");
    fs::write(
        &file,
        r"---- MODULE Channel ----
EXTENDS Naturals
VARIABLES delay, pending
vars == <<delay, pending>>
Init == delay \in 0..8191 /\ pending = 0
Send == pending = 0 /\ pending' \in {1, 2} /\ UNCHANGED delay
Deliver == pending # 0 /\ pending' = 0 /\ UNCHANGED delay
Next == Send \/ Deliver
Spec == Init /\ [][Next]_vars /\ WF_vars(Send) /\ WF_vars(Deliver)
Live == <>(delay < 100 \/ pending = 99)
====
",
    )?;
    fs::write(
        file.with_extension("cfg"),
        "SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n",
    )?;
    let first = search(&file, &[])?;
    assert_eq!(first["verdict"], "bounded-inconclusive");
    let options = ["--cold-roots=4", "--cold-init-emissions=2048"];
    let selected = search(&file, &options)?;
    let repeated = search(&file, &options)?;
    let changed_seed = search(
        &file,
        &["--cold-roots=4", "--cold-init-emissions=2048", "--seed=13"],
    )?;
    assert_eq!(selected["verdict"], "liveness-violation", "{selected}");
    assert_eq!(
        changed_seed["verdict"], "liveness-violation",
        "{changed_seed}"
    );
    assert_eq!(
        selected["initial_roots"]["roots"],
        repeated["initial_roots"]["roots"]
    );
    assert_ne!(
        selected["initial_roots"]["roots"],
        changed_seed["initial_roots"]["roots"]
    );
    assert_eq!(
        selected["counterexample"]["trace"],
        repeated["counterexample"]["trace"]
    );
    assert!(
        selected["initial_roots"]["emissions"]
            .as_u64()
            .is_some_and(|n| n <= 2048)
    );
    Ok(())
}
