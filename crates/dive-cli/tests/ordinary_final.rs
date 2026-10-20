//! Real ordinary compat CLI reports use the bounded final API.
use std::path::PathBuf;
use std::process::{Command, Output};
struct Spec(PathBuf);
impl Drop for Spec {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run(
    file: &std::path::Path,
    profile: &str,
    workers: usize,
    extra: &[&str],
) -> std::io::Result<Output> {
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    Command::new(binary)
        .arg("check")
        .arg(file)
        .args(["--output=json", "--seed=7", "--fp=0"])
        .arg(format!("--compat={profile}"))
        .arg(format!("--workers={workers}"))
        .args(extra)
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_CHAOS")
        .output()
}
#[test]
fn actual_ordinary_compat_statuses_and_schema_match_the_selected_backend()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!(
        "dive-cli-ordinary-final-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let spec = Spec(directory);
    let file = spec.0.join("Ordinary.tla");
    for (property, status) in [("[]<>(x<=2)", 0), ("<>(x=3)", 13), ("<>1", 255)] {
        std::fs::write(
            &file,
            format!(
                "---- MODULE Ordinary ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == {property}\n====\n"
            ),
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        for workers in [1, 4] {
            let output = run(&file, "tlc", workers, &[])?;
            assert_eq!(output.status.code(), Some(status));
            let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(json["schema_version"], 2);
            assert_eq!(json["options"]["profile"], "tlc");
            assert_eq!(json["options"]["exit_codes"], "tlc");
            assert_eq!(json["options"]["lncheck"], "final");
            if status == 13 {
                assert_eq!(json["counterexample"]["validation"], "valid");
            }
            if status != 255 {
                assert_eq!(json["execution"]["exhausted"], true);
                assert_eq!(json["statistics"]["distinct"], 3);
            }
        }
        let explicit = run(&file, "tlc", 1, &["--lncheck=final", "--lasso=short"])?;
        assert_eq!(explicit.status.code(), Some(status));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&explicit.stdout)?["options"]["profile"],
            "tlc+overrides"
        );
        let native = run(&file, "dive", 1, &[])?;
        assert_eq!(native.status.code(), Some(status));
        let native: serde_json::Value = serde_json::from_slice(&native.stdout)?;
        assert_eq!(native["schema_version"], 2);
        assert_eq!(native["options"]["profile"], "dive");
        assert_eq!(native["options"]["lncheck"], "deterministic");
        if status == 13 {
            assert_eq!(native["counterexample"]["validation"], "valid");
        }
        if status != 255 {
            // Three states never reach the default schedule's minimum: the
            // only check is the final one, with complete counts.
            assert_eq!(native["periodic"]["prefix_checks"], 0);
            assert_eq!(native["periodic"]["final_check"], true);
            assert_eq!(native["execution"]["exhausted"], true);
            assert_eq!(native["statistics"]["distinct"], 3);
        }
        let excluded = run(&file, "tlc", 1, &["--sequential"])?;
        assert_eq!(excluded.status.code(), Some(3));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&excluded.stdout)?["schema_version"],
            1
        );
    }
    Ok(())
}

#[test]
fn existing_tlc_composition_option_keeps_counts_and_safe_validation_refusal()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!(
        "dive-cli-ordinary-cdot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let spec = Spec(directory);
    let file = spec.0.join("CompositionFinal.tla");
    std::fs::write(
        &file,
        include_str!("../../dive-api/tests/fixtures/ordinary-cdot/CompositionFinal.tla"),
    )?;
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-api/tests/fixtures/ordinary-cdot/oracle.json"
    ))?;
    for case in oracle["cases"].as_array().ok_or("cases")? {
        let holds = case["case"] == "hold";
        std::fs::write(
            file.with_extension("cfg"),
            if holds {
                include_str!("../../dive-api/tests/fixtures/ordinary-cdot/hold.cfg")
            } else {
                include_str!("../../dive-api/tests/fixtures/ordinary-cdot/fail.cfg")
            },
        )?;
        for workers in [1, 4] {
            let output = run(&file, "tlc", workers, &["--action-composition=tlc"])?;
            assert_eq!(output.status.code(), Some(if holds { 0 } else { 3 }));
            let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(value["schema_version"], 2);
            assert_eq!(value["options"]["profile"], "tlc");
            assert_eq!(value["execution"]["capture_completed"], true);
            for field in ["generated", "distinct", "depth"] {
                assert_eq!(value["statistics"][field], case[field]);
            }
            assert_eq!(
                value["verdict"],
                if holds { "success" } else { "unsupported" }
            );
            assert!(value["counterexample"].is_null());
        }
        for extra in [
            vec!["--action-composition=tla"],
            vec!["--action-composition=tlc", "--sequential"],
            vec!["--action-composition=tlc", "--store=exact"],
        ] {
            let output = run(&file, "tlc", 1, &extra)?;
            let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(output.status.code(), Some(3));
            assert_eq!(value["schema_version"], 1);
            assert_eq!(value["verdict"], "unsupported");
        }
    }
    Ok(())
}

/// The default schedule checks the first completed expansion level with at
/// least 2^16 distinct states. A violation there stops with that level's actual counts.
#[test]
fn default_schedule_stops_at_the_first_due_level() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!(
        "dive-cli-ordinary-periodic-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let spec = Spec(directory);
    let file = spec.0.join("Wide.tla");
    std::fs::write(
        &file,
        "---- MODULE Wide ----\nEXTENDS Naturals\nVARIABLES x, y\nInit == x=0 /\\ y \\in 0..65535\nNext == x'=(x+1)%3 /\\ UNCHANGED y\nP == <>(x=1)\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    for workers in [1, 4] {
        let native = run(&file, "dive", workers, &[])?;
        assert_eq!(native.status.code(), Some(13));
        let json: serde_json::Value = serde_json::from_slice(&native.stdout)?;
        assert_eq!(json["options"]["lncheck"], "deterministic");
        assert_eq!(json["verdict"], "liveness-violation");
        assert_eq!(json["property"], "P");
        assert_eq!(json["counterexample"]["validation"], "valid");
        assert_eq!(json["execution"]["capture_completed"], false);
        assert_eq!(json["execution"]["exhausted"], false);
        assert_eq!(json["periodic"]["prefix_checks"], 1);
        // Level 1 (the first expansion) commits 131,072 states, 65,536 queued.
        assert_eq!(json["execution"]["level"], 1);
        assert_eq!(json["periodic"]["last_level"], 1);
        assert_eq!(json["periodic"]["last_size"], 131_072);
        assert_eq!(json["statistics"]["distinct"], 131_072);
        assert_eq!(json["statistics"]["depth"], 2);
        assert_eq!(json["statistics"]["left_on_queue"], 65_536);
    }
    // The TLC profile keeps final checking and the complete graph's counts.
    let tlc = run(&file, "tlc", 1, &[])?;
    assert_eq!(tlc.status.code(), Some(13));
    let tlc: serde_json::Value = serde_json::from_slice(&tlc.stdout)?;
    assert_eq!(tlc["options"]["lncheck"], "final");
    assert_eq!(tlc["property"], "P");
    assert_eq!(tlc["statistics"]["distinct"], 196_608);
    Ok(())
}
