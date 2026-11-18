//! Genuine ordinary native resource stops retain the actual capture boundary.
#![allow(clippy::expect_used, reason = "bounded native resource controls")]
use dive_api::{BudgetSetting, CheckRequest, check_file};
use std::time::Duration;

#[test]
fn ordinary_native_capture_budget_stops_keep_schema_phase_and_actual_work() {
    let directory =
        std::env::temp_dir().join(format!("dive-native-resources-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    let file = directory.join("ManyInitial.tla");
    std::fs::write(&file, "---- MODULE ManyInitial ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..1000000\nNext == UNCHANGED x\nP == <>(x = 1000001)\n====\n").expect("spec");
    let config = file.with_extension("cfg");
    std::fs::write(&config, "INIT Init\nNEXT Next\nPROPERTY P\n").expect("config");
    for compat_tlc in [false, true] {
        for workers in [1, 8] {
            let request = CheckRequest {
                file: file.clone(),
                config: Some(config.clone()),
                compat_tlc,
                workers: Some(workers),
                seed: Some(0),
                fp: Some(0),
                time_limit: BudgetSetting::Explicit(Duration::from_millis(150)),
                max_memory: BudgetSetting::Unlimited,
                ..CheckRequest::default()
            };
            {
                let report = check_file(&request).expect("check");
                let json = report.to_json();
                assert_eq!(json["schema_version"], 3, "{json:#}");
                assert_eq!(json["engine"], "core", "{json:#}");
                assert_eq!(json["verdict"], "stopped", "{json:#}");
                assert_eq!(json["stopped"]["resource"], "time");
                assert_eq!(json["stopped"]["phase"], "graph-capture");
                assert_eq!(json["execution"]["stage"], "capture");
                assert_eq!(json["execution"]["capture_completed"], false);
                assert!(
                    json["statistics"]["distinct"].as_u64() > Some(0),
                    "{json:#}"
                );
                assert!(json["counterexample"].is_null());
                assert!(json["tlc"].is_null());
                assert_eq!(report.exit_code(), 5);
                assert!(json["engine_fallback"].is_null());
            }
        }
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn ordinary_native_postcapture_budget_is_a_structured_stop_without_fallback() {
    let directory =
        std::env::temp_dir().join(format!("dive-native-atom-resource-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    let file = directory.join("ExpensiveAtom.tla");
    // The state variable keeps this supported quantifier out of startup
    // constant folding. Capture has exactly one state and two edges; the real
    // run budget then interrupts evaluation of its temporal state atom.
    std::fs::write(&file, "---- MODULE ExpensiveAtom ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nP == []<>(\\A i \\in 1..1000000000 : i + x >= 0)\n====\n").expect("spec");
    let config = file.with_extension("cfg");
    std::fs::write(&config, "INIT Init\nNEXT Next\nPROPERTY P\n").expect("config");
    for compat_tlc in [false, true] {
        {
            let report = check_file(&CheckRequest {
                file: file.clone(),
                config: Some(config.clone()),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                time_limit: BudgetSetting::Explicit(Duration::from_millis(300)),
                max_memory: BudgetSetting::Unlimited,
                ..CheckRequest::default()
            })
            .expect("check");
            let json = report.to_json();
            assert_eq!(json["engine"], "core", "{json:#}");
            assert_eq!(json["verdict"], "stopped", "{json:#}");
            assert_eq!(json["stopped"]["resource"], "time");
            assert_eq!(json["stopped"]["phase"], "temporal-analysis");
            assert_eq!(json["execution"]["stage"], "atoms");
            assert_eq!(json["execution"]["capture_completed"], true);
            assert_eq!(json["statistics"]["distinct"], 1);
            assert_eq!(json["statistics"]["generated"], 2);
            assert_eq!(json["statistics"]["left_on_queue"], 0);
            assert!(json["counterexample"].is_null());
            assert!(json["tlc"].is_null());
            assert!(json["engine_fallback"].is_null());
            assert_eq!(report.exit_code(), 5);
        }
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}
