//! Physical exit follows the actual selected-action typed error, not its text.
use std::path::PathBuf;
use std::process::Command;
const SOURCE: &str = include_str!("../../dive-liveness/tests/fixtures/constructor-alias.tla");
struct Spec(PathBuf);
impl Drop for Spec {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn selected_original_action_has_physical255_and_function_first_has_zero()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!(
        "dive-selected-action-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let spec = Spec(directory);
    let file = spec.0.join("ConstructorAlias.tla");
    std::fs::write(&file, SOURCE)?;
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    for (next, property, status) in [
        ("Next", "ActionCheck", 255),
        ("NextRecordFirst", "ActionCheck", 255),
        ("NextFunctionFirst", "ActionCheck", 0),
        ("Next", "StateControl", 0),
        ("Next", "EliminatedAction", 0),
    ] {
        std::fs::write(
            file.with_extension("cfg"),
            format!("INIT Init\nNEXT {next}\nPROPERTY {property}\n"),
        )?;
        let output = Command::new(&binary)
            .arg("check")
            .arg(&file)
            .args([
                "--compat=tlc",
                "--workers=1",
                "--seed=0",
                "--fp=0",
                "--output=json",
            ])
            .env_remove("DIVE_COMPAT")
            .output()?;
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output.status.code(), Some(status), "{value}");
        assert_eq!(value["exit_code"], status);
        if status == 255 {
            assert_eq!(value["tlc"]["code"], 1000);
            assert_eq!(value["error_site"], "temporal-action");
        } else {
            assert_eq!(value["verdict"], "success");
        }
    }
    Ok(())
}
