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
        let refused = run(&file, "dive", 1, &[])?;
        assert_eq!(refused.status.code(), Some(3));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&refused.stdout)?["schema_version"],
            1
        );
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
