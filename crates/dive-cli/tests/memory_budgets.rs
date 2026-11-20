//! Boundary memory observations use the existing stopped wire result.
use std::process::Command;

#[test]
fn explicit_one_byte_ceiling_is_enforced_in_native_and_tlc_transports()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    for transport in ["check", "tlc"] {
        let mut command = Command::new(&binary);
        command
            .current_dir(&root)
            .args([
                transport,
                "docs/user/examples/Counter.tla",
                "--output=json",
                "--max-memory=1",
            ])
            .env_remove("DIVE_COMPAT")
            .env_remove("DIVE_CHAOS");
        if transport == "check" {
            command.arg("--time-limit=none");
        }
        let output = command.output()?;
        assert_eq!(output.status.code(), Some(5), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(json["schema_version"], 4);
        assert_eq!(json["verdict"], "stopped");
        assert_eq!(json["stopped"]["phase"], "check-startup");
        assert_eq!(json["stopped"]["resource"], "memory");
        assert_eq!(json["stopped"]["limit"], 1);
        assert_eq!(json["check_kind"], "safety");
        assert!(json["execution"].is_null());
        assert_eq!(json["resources"]["checking_started"], false);
        assert!(json["statistics"].is_null());
        assert!(json["counterexample"].is_null());
        assert!(json["tlc"].is_null());
        assert_eq!(json["resources"]["memory"]["initial_sampled"], true);
        assert_eq!(json["resources"]["memory"]["final_sampled"], true);
    }
    Ok(())
}

#[test]
fn memory_measurement_diagnostic_has_a_registered_cli_explanation()
-> Result<(), Box<dyn std::error::Error>> {
    let binary =
        std::env::var_os("DIVE_TEST_CLI").unwrap_or_else(|| env!("CARGO_BIN_EXE_dive").into());
    let output = Command::new(binary)
        .args(["explain", "DIVE-R0002", "--output", "json"])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["known"], true);
    assert_eq!(json["code"], "DIVE-R0002");
    assert!(
        json["explanation"]["remediation"]
            .as_str()
            .is_some_and(|help| help.contains("--max-memory=none"))
    );
    assert!(json.get("verdict").is_none());
    Ok(())
}
