//! Original nested-query primary statuses, counts and independently checked words.
#[path = "support/refused.rs"]
mod refused;
use dive_api::{CheckRequest, ExitConvention, Store};
use serde_json::Value;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn nested_enabled_primary_public_results_keep_existing_schema_and_statuses() -> TestResult {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/nested-enabled");
    let oracle: Value = serde_json::from_str(include_str!(
        "../../dive-liveness/tests/fixtures/nested-enabled/oracle.json"
    ))?;
    assert_eq!(oracle["oracle"], "tlc-2026.10.06.014338");
    assert_eq!(
        oracle["upstream_commit"],
        "94d0c5070852b37b801bb8998c436383ab7ee37b"
    );
    assert_eq!(oracle["workers"], 1);
    assert_eq!(oracle["seed"], 0);
    assert_eq!(oracle["fingerprint"], 0);
    assert_eq!(oracle["lncheck"], "final");
    for compat_tlc in [true, false] {
        for store in [Store::Fingerprint, Store::Exact] {
            for (name, violation) in [("hold", false), ("fail", true)] {
                let mut request = CheckRequest {
                    file: directory.join("NestedEnabled.tla"),
                    config: Some(directory.join(format!("{name}.cfg"))),
                    compat_tlc,
                    store,
                    temporal_refusal: Some(refused::FINAL_SHORT.into()),
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                // Only the retired reference engine ran bounded final
                // checking (#791, #910); Core's ordinary temporal mode must
                // reach the oracle's final-mode truth.
                let final_short = dive_api::check_file(&request)?;
                refused::assert_refused(
                    &final_short.to_json(),
                    final_short.exit_code(),
                    refused::FINAL_SHORT,
                );
                request.temporal_refusal = None;
                let report = dive_api::check_file(&request)?;
                let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                assert_eq!(value["schema_version"], 4);
                assert_eq!(value["statistics"]["generated"], oracle["generated"]);
                assert_eq!(value["statistics"]["distinct"], oracle["distinct"]);
                assert_eq!(value["statistics"]["depth"], oracle["depth"]);
                assert_eq!(
                    oracle[name]["physical_exit"],
                    Value::from(report.exit_code_for(ExitConvention::Tlc)),
                    "{name}: {value}"
                );
                assert_eq!(
                    value["verdict"],
                    if violation {
                        "liveness-violation"
                    } else {
                        "success"
                    }
                );
                if compat_tlc {
                    assert_eq!(oracle[name]["verdict_code"], value["tlc"]["code"], "{name}");
                    assert_eq!(value["tlc"]["verified"], true);
                }
                if violation {
                    assert_eq!(value["property"], oracle[name]["property"]);
                    assert!(!value["counterexample"].is_null());
                } else {
                    assert!(value["counterexample"].is_null());
                }
            }
        }
    }
    Ok(())
}

#[test]
fn true_query_guard_preserves_final_selected_target_error_and_function_control() -> TestResult {
    // Native-only interaction: a fresh inner x'=x witness makes Guard true.
    // This is not a new primary observation or a general error-parity claim.
    let source = include_str!("../../dive-liveness/tests/fixtures/constructor-alias.tla")
        .replace("ActionCheck ==", "Guard == ENABLED (x'=x)\nActionCheck ==")
        .replace("~(x' = <<0, 0>>)", "Guard /\\ ~(x' = <<0, 0>>)");
    let directory = std::env::temp_dir().join(format!(
        "dive-nested-final-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let result = (|| -> TestResult {
        let file = directory.join("ConstructorAlias.tla");
        std::fs::write(&file, source)?;
        for (next, expected, generated) in [
            ("Next", 255, 2),
            ("NextRecordFirst", 255, 3),
            ("NextFunctionFirst", 0, 3),
        ] {
            let config = directory.join("ConstructorAlias.cfg");
            std::fs::write(
                &config,
                format!("INIT Init\nNEXT {next}\nPROPERTY ActionCheck\n"),
            )?;
            let mut request = CheckRequest {
                file: file.clone(),
                config: Some(config),
                compat_tlc: true,
                temporal_refusal: Some(refused::FINAL_SHORT.into()),
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            // Only the retired reference engine ran bounded final checking
            // (#791, #910); Core checks the property in its ordinary mode.
            let final_short = dive_api::check_file(&request)?;
            refused::assert_refused(
                &final_short.to_json(),
                final_short.exit_code(),
                refused::FINAL_SHORT,
            );
            request.temporal_refusal = None;
            let report = dive_api::check_file(&request)?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            if expected == 255 {
                // The retired reference engine reported the selected target's
                // action error (1000, exit 255, after {generated} generated
                // states); Core refuses to build that temporal failure report.
                refused::assert_refused(
                    &report.to_json(),
                    report.exit_code(),
                    refused::TEMPORAL_FAILURE_REPORT,
                );
                continue;
            }
            assert_eq!(
                report.exit_code_for(ExitConvention::Tlc),
                expected,
                "{value}"
            );
            assert_eq!(value["statistics"]["generated"], generated);
            assert_eq!(value["statistics"]["distinct"], 1);
            assert_eq!(value["statistics"]["depth"], 1);
            assert!(value["counterexample"].is_null());
            assert_eq!(value["verdict"], "success");
        }
        Ok(())
    })();
    std::fs::remove_dir_all(directory)?;
    result
}

#[test]
fn direct_query_instance_properties_keep_original_native_holds() -> TestResult {
    let directory = std::env::temp_dir().join(format!(
        "dive-instance-enabled-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join("InstanceEnabled.tla");
    std::fs::write(
        &file,
        include_str!("../../dive-liveness/tests/fixtures/nested-enabled/InstanceEnabled.tla"),
    )?;
    for name in ["D", "F", "L", "Alias", "Neg", "Or"] {
        let config = directory.join(format!("{name}.cfg"));
        std::fs::write(
            &config,
            format!("INIT Init\nNEXT Next\nPROPERTY P_{name}\n"),
        )?;
        for compat_tlc in [true, false] {
            for store in [Store::Fingerprint, Store::Exact] {
                let mut request = CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    compat_tlc,
                    store,
                    temporal_refusal: Some(refused::FINAL_SHORT.into()),
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                // Only the retired reference engine ran bounded final
                // checking (#791, #910); Core holds each in ordinary mode.
                let final_short = dive_api::check_file(&request)?;
                refused::assert_refused(
                    &final_short.to_json(),
                    final_short.exit_code(),
                    refused::FINAL_SHORT,
                );
                request.temporal_refusal = None;
                let report = dive_api::check_file(&request)?;
                let v = report.to_json_with_exit_convention(ExitConvention::Tlc);
                assert_eq!(report.exit_code_for(ExitConvention::Tlc), 0, "{name}: {v}");
                assert_eq!(v["verdict"], "success", "{name}");
                for (key, expected) in [("generated", 3), ("distinct", 2), ("depth", 2)] {
                    assert_eq!(v["statistics"][key], expected, "{name}");
                }
                assert!(v["counterexample"].is_null());
            }
        }
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
