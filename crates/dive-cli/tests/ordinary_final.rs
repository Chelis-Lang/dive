//! Real ordinary compat CLI reports on Core, and its named refusals.
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
        .env_remove("DIVE_ENGINE")
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
    // TLC reports `<>1` as an evaluation error (exit 255); Core does not
    // build that temporal failure report yet and refuses it by name.
    for (property, status) in [("[]<>(x<=2)", 0), ("<>(x=3)", 13), ("<>1", 3)] {
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
            assert_eq!(json["schema_version"], 4);
            assert_eq!(json["options"]["profile"], "tlc");
            assert_eq!(json["options"]["exit_codes"], "tlc");
            if status != 3 {
                assert_eq!(json["options"]["lncheck"], "final");
            }
            if status == 13 {
                assert_eq!(json["counterexample"]["validation"], "valid");
            }
            if status == 3 {
                assert_eq!(
                    json["message"],
                    "a temporal validation failure (core): its report is not implemented on Core"
                );
            } else {
                assert_eq!(json["execution"]["exhausted"], true);
                assert_eq!(json["statistics"]["distinct"], 3);
            }
        }
        // Core has no bounded final route.
        let explicit = run(&file, "tlc", 1, &["--lncheck=final", "--lasso=short"])?;
        assert_eq!(explicit.status.code(), Some(3));
        let explicit: serde_json::Value = serde_json::from_slice(&explicit.stdout)?;
        assert_eq!(explicit["options"]["profile"], "tlc");
        assert!(explicit["message"].as_str().is_some_and(|message| message
            .ends_with("bounded final checking (--lncheck final --lasso short) is not implemented on Core")));
        let native = run(&file, "dive", 1, &[])?;
        assert_eq!(native.status.code(), Some(status));
        let native: serde_json::Value = serde_json::from_slice(&native.stdout)?;
        assert_eq!(native["schema_version"], 4);
        assert_eq!(native["options"]["profile"], "dive");
        if status != 3 {
            assert_eq!(native["options"]["lncheck"], "deterministic");
        }
        if status == 13 {
            assert_eq!(native["counterexample"]["validation"], "valid");
        }
        if status != 3 {
            // Three states never reach the default schedule's minimum: the
            // only check is the final one, with complete counts.
            assert_eq!(native["periodic"]["prefix_checks"], 0);
            assert_eq!(native["periodic"]["final_check"], true);
            assert_eq!(native["execution"]["exhausted"], true);
            assert_eq!(native["statistics"]["distinct"], 3);
        }
        let sequential = run(&file, "tlc", 1, &["--sequential"])?;
        assert_eq!(sequential.status.code(), Some(status));
        let excluded = run(&file, "tlc", 1, &["--chaos", "0"])?;
        assert_eq!(excluded.status.code(), Some(3));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&excluded.stdout)?["schema_version"],
            4
        );
    }
    Ok(())
}

#[test]
fn existing_tlc_composition_option_is_a_named_core_refusal()
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
    for config in [
        include_str!("../../dive-api/tests/fixtures/ordinary-cdot/hold.cfg"),
        include_str!("../../dive-api/tests/fixtures/ordinary-cdot/fail.cfg"),
    ] {
        std::fs::write(file.with_extension("cfg"), config)?;
        for (extra, message) in [
            (
                vec!["--action-composition=tlc"],
                "--action-composition=tlc under the core engine",
            ),
            (
                vec!["--action-composition=tlc", "--sequential"],
                "--action-composition=tlc under the core engine",
            ),
            (
                vec!["--action-composition=tla"],
                "temporal properties with evaluation options other than the profile defaults (core)",
            ),
        ] {
            for workers in [1, 4] {
                let output = run(&file, "tlc", workers, &extra)?;
                let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
                assert_eq!(output.status.code(), Some(3), "{extra:?}");
                assert_eq!(value["schema_version"], 4);
                assert_eq!(value["verdict"], "unsupported");
                assert_eq!(value["message"], message, "{extra:?}");
                assert!(value["counterexample"].is_null());
            }
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

#[test]
fn temporal_threshold_controls_only_scheduled_checks() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/report-schema");
    let file = fixtures.join("Reports.tla");
    let cfg = fixtures.join("Liveness.cfg");
    let cfg = cfg.to_str().ok_or("fixture path")?;
    let args = ["--config", cfg, "--periodic-min-states=1"];
    let output = run(&file, "dive", 1, &args)?;
    let result: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result["verdict"], "liveness-violation");
    assert_eq!(result["periodic"]["min_states"], 1);
    assert_eq!(result["periodic"]["growth_factor"], 2);
    assert!(
        result["periodic"]["prefix_checks"]
            .as_u64()
            .is_some_and(|n| n > 0)
    );
    assert_eq!(result["counterexample"]["validation"], "valid");
    assert_eq!(result["statistics"]["distinct"], 2);

    let final_output = run(&file, "tlc", 1, &args)?;
    let final_result: serde_json::Value = serde_json::from_slice(&final_output.stdout)?;
    assert_eq!(final_result["verdict"], "liveness-violation");
    assert!(final_result["periodic"].is_null());
    for value in ["0", "-1", "18446744073709551616", "abc"] {
        let invalid = run(&file, "dive", 1, &["--periodic-min-states", value])?;
        assert!(!invalid.status.success());
        assert!(String::from_utf8(invalid.stderr)?.contains("positive state count"));
    }
    Ok(())
}
