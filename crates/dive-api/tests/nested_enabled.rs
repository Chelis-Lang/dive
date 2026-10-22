//! Original nested-query primary statuses, counts and independently checked words.
use dive_api::{CheckRequest, ExitConvention, Store, TemporalMode};
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
                let report = dive_api::check_file(&CheckRequest {
                    file: directory.join("NestedEnabled.tla"),
                    config: Some(directory.join(format!("{name}.cfg"))),
                    compat_tlc,
                    store,
                    temporal: TemporalMode::FinalShort,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                })?;
                let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                assert_eq!(value["schema_version"], 3);
                assert_eq!(value["statistics"]["generated"], oracle["generated"]);
                assert_eq!(value["statistics"]["distinct"], oracle["distinct"]);
                assert_eq!(value["statistics"]["depth"], oracle["depth"]);
                assert_eq!(
                    Value::from(report.exit_code_for(ExitConvention::Tlc)),
                    oracle[name]["physical_exit"]
                );
                assert_eq!(
                    value["verdict"],
                    if violation {
                        "liveness-violation"
                    } else {
                        "success"
                    }
                );
                assert!(
                    value["tlc"].is_null(),
                    "explicit FinalShort retains its existing qualification"
                );
                if compat_tlc && matches!(store, Store::Fingerprint) {
                    let ordinary = dive_api::check_file(&CheckRequest {
                        file: directory.join("NestedEnabled.tla"),
                        config: Some(directory.join(format!("{name}.cfg"))),
                        compat_tlc: true,
                        temporal: TemporalMode::Ordinary,
                        workers: Some(1),
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    })?
                    .to_json_with_exit_convention(ExitConvention::Tlc);
                    assert_eq!(ordinary["verdict"], value["verdict"]);
                    assert_eq!(ordinary["statistics"], value["statistics"]);
                    assert_eq!(ordinary["tlc"]["code"], oracle[name]["verdict_code"]);
                    assert_eq!(ordinary["tlc"]["verified"], true);
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
            let report = dive_api::check_file(&CheckRequest {
                file: file.clone(),
                config: Some(config),
                compat_tlc: true,
                temporal: TemporalMode::FinalShort,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(
                report.exit_code_for(ExitConvention::Tlc),
                expected,
                "{value}"
            );
            assert_eq!(value["statistics"]["generated"], generated);
            assert_eq!(value["statistics"]["distinct"], 1);
            assert_eq!(value["statistics"]["depth"], 1);
            assert!(value["counterexample"].is_null());
            if expected == 255 {
                assert_eq!(value["verdict"], "evaluation-error");
                assert_eq!(value["error_site"], "temporal-action");
                assert_eq!(value["execution"]["stage"], "atoms");
                assert_eq!(value["tlc"]["code"], 1000);
                assert_eq!(value["tlc"]["verified"], true);
            } else {
                assert_eq!(value["verdict"], "success");
            }
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
                let report = dive_api::check_file(&CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    compat_tlc,
                    store,
                    temporal: TemporalMode::FinalShort,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                })?;
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
