//! Finite execution modes stop with actual resource evidence and prefix progress.
use dive_api::{BudgetSetting, DfidRequest, SimulateRequest};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn model(init: &str) -> Result<(Scratch, PathBuf)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dive-mode-budget-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join("ModeBudget.tla");
    std::fs::write(
        &file,
        format!(
            "---- MODULE ModeBudget ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = {init}\nNext == x' = x + 1\n====\n"
        ),
    )?;
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
    Ok((Scratch(directory), file))
}
fn simulation(file: &Path, compat_tlc: bool) -> SimulateRequest {
    {
        let mut built = SimulateRequest::default();
        built.file = file.into();
        built.compat_tlc = compat_tlc;
        built.seed = Some(7);
        built.traces = 100_000;
        built.depth = Some(100_000);
        built.max_memory = BudgetSetting::Unlimited;
        built
    }
}
fn dfid(file: &Path, compat_tlc: bool) -> DfidRequest {
    DfidRequest {
        file: file.into(),
        compat_tlc,
        seed: Some(7),
        max_level: 10_000,
        max_memory: BudgetSetting::Unlimited,
        ..Default::default()
    }
}
fn stopped(document: &Value, resource: &str, phase: &str) {
    assert_eq!(document["result"], "stopped", "{document}");
    assert_eq!(document["exit_code"], 5);
    assert_eq!(document["stopped"]["resource"], resource);
    assert_eq!(document["stopped"]["phase"], phase);
    assert_eq!(document["model_proved"], false);
    assert_eq!(document["exhaustive"], false);
    assert_eq!(document["violation_validated"], false);
    assert!(document["tlc_code"].is_null());
    assert!(document["counterexample"].is_null());
    assert!(document["stopped"]["observed"].as_u64() >= document["stopped"]["limit"].as_u64());
}

#[test]
fn both_modes_stop_on_time_and_memory_in_both_profiles() -> Result {
    let (_scratch, file) = model("0")?;
    for compat in [false, true] {
        let simulation = dive_api::simulate_file(&{
            let mut built = simulation(&file, compat);
            built.time_limit = BudgetSetting::Explicit(Duration::from_millis(20));
            built
        })?;
        assert_eq!(simulation.exit_code(), 5);
        stopped(&simulation.to_json(), "time", "simulation");
        let iterative = dive_api::dfid_file(&DfidRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(20)),
            ..dfid(&file, compat)
        })?;
        assert_eq!(iterative.exit_status(), 5, "{}", iterative.to_json());
        stopped(&iterative.to_json(), "time", "dfid");
        for (document, phase) in [
            (
                dive_api::simulate_file(&{
                    let mut built = self::simulation(&file, compat);
                    built.max_memory = BudgetSetting::Explicit(1);
                    built.time_limit = BudgetSetting::Unlimited;
                    built
                })?
                .to_json(),
                "simulation",
            ),
            (
                dive_api::dfid_file(&DfidRequest {
                    max_memory: BudgetSetting::Explicit(1),
                    time_limit: BudgetSetting::Unlimited,
                    ..dfid(&file, compat)
                })?
                .to_json(),
                "dfid",
            ),
        ] {
            stopped(&document, "memory", phase);
        }
    }
    Ok(())
}

#[test]
fn bulk_initial_predicate_is_interrupted_before_emitting_a_state() -> Result {
    let (_scratch, file) =
        model("IF (\\A i \\in 1..1000: \\A j \\in 1..1000: i + j >= 0) THEN 0 ELSE 1")?;
    for compat in [false, true] {
        let simulation = dive_api::simulate_file(&{
            let mut built = simulation(&file, compat);
            built.time_limit = BudgetSetting::Explicit(Duration::from_millis(10));
            built
        })?
        .to_json();
        stopped(&simulation, "time", "simulation");
        assert_eq!(simulation["statistics"]["initial_candidates"], 0);
        let iterative = dive_api::dfid_file(&DfidRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(10)),
            ..dfid(&file, compat)
        })?
        .to_json();
        stopped(&iterative, "time", "dfid");
        assert_eq!(iterative["statistics"]["initial_candidates"], 0);
    }
    Ok(())
}

#[test]
fn default_time_limit_is_none_in_both_profiles() -> Result {
    // Neither profile arms a wall-clock budget unless the user sets one, as in TLC.
    let (_scratch, file) = model("0")?;
    for compat in [false, true] {
        let sampled = dive_api::simulate_file(&{
            let mut built = simulation(&file, compat);
            built.traces = 2;
            built.depth = Some(4);
            built
        })?
        .to_json();
        let iterative = dive_api::dfid_file(&DfidRequest {
            max_level: 4,
            ..dfid(&file, compat)
        })?
        .to_json();
        for document in [&sampled, &iterative] {
            let budget = &document["options"]["budget"];
            assert!(budget["time_limit_ms"].is_null(), "{compat}: {document}");
            assert_eq!(budget["time_limit_source"], "none", "{compat}: {document}");
        }
    }
    Ok(())
}

#[test]
fn unlimited_and_generous_budgets_preserve_seeded_samples() -> Result {
    let (_scratch, file) = model("0")?;
    for compat in [false, true] {
        let mut request = simulation(&file, compat);
        request.traces = 2;
        request.depth = Some(4);
        request.time_limit = BudgetSetting::Unlimited;
        let unlimited = dive_api::simulate_file(&request)?.to_json();
        let generous = dive_api::simulate_file(&{
            let mut built = request;
            built.time_limit = BudgetSetting::Explicit(Duration::from_secs(60));
            built
        })?
        .to_json();
        assert_eq!(unlimited["traces"], generous["traces"]);
        assert_eq!(unlimited["statistics"], generous["statistics"]);
        assert_eq!(unlimited["result"], "sampled");
        assert_eq!(generous["result"], "sampled");
        assert_eq!(
            generous["options"]["budget"]["time_limit_source"],
            "explicit"
        );
        let refused = dive_api::simulate_file(&{
            let mut built = simulation(&file, compat);
            built.workers = Some(8);
            built.traces = 0;
            built.max_memory = BudgetSetting::Explicit(1);
            built
        })?
        .to_json();
        assert_eq!(refused["result"], "unsupported");
        assert_eq!(refused["options"]["workers_used"], 0);
    }
    Ok(())
}

#[test]
fn preflight_errors_and_refusals_keep_precedence_over_run_budgets() -> Result {
    let (_scratch, file) = model("0")?;
    for compat in [false, true] {
        for workers in [0, 2] {
            let request = DfidRequest {
                workers: Some(workers),
                time_limit: BudgetSetting::Unlimited,
                ..dfid(&file, compat)
            };
            let expected = dive_api::dfid_file(&request)?;
            let limited = dive_api::dfid_file(&DfidRequest {
                max_memory: BudgetSetting::Explicit(1),
                ..request
            })?;
            let expected_json = expected.to_json();
            let limited_json = limited.to_json();
            assert_eq!(limited.exit_status(), expected.exit_status());
            for field in ["result", "tlc_code", "message", "statistics"] {
                assert_eq!(limited_json[field], expected_json[field], "{field}");
            }
            assert_eq!(limited_json["options"]["workers_used"], 0);
            assert!(limited_json["stopped"].is_null());
        }
        let mut request = simulation(&file, compat);
        request.workers = Some(2);
        request.traces = 0;
        request.time_limit = BudgetSetting::Unlimited;
        let expected = dive_api::simulate_file(&request)?;
        let limited = dive_api::simulate_file(&{
            let mut built = request;
            built.max_memory = BudgetSetting::Explicit(1);
            built
        })?;
        assert_eq!(limited.exit_code(), expected.exit_code());
        assert_eq!(limited.to_json()["result"], expected.to_json()["result"]);
        assert_eq!(limited.to_json()["message"], expected.to_json()["message"]);
        assert_eq!(limited.to_json()["options"]["workers_used"], 0);
        assert!(limited.to_json()["stopped"].is_null());

        let request = DfidRequest {
            max_level: 10_001,
            time_limit: BudgetSetting::Unlimited,
            ..dfid(&file, compat)
        };
        let expected = dive_api::dfid_file(&request)?;
        let limited = dive_api::dfid_file(&DfidRequest {
            max_memory: BudgetSetting::Explicit(1),
            ..request
        })?;
        assert_eq!(expected.to_json()["result"], "resource-limit");
        assert_eq!(limited.exit_status(), expected.exit_status());
        assert_eq!(limited.to_json()["result"], expected.to_json()["result"]);
        assert_eq!(limited.to_json()["message"], expected.to_json()["message"]);
        assert!(limited.to_json()["stopped"].is_null());
    }
    let mut request = simulation(&file, true);
    request.aril = 1_000_001;
    request.time_limit = BudgetSetting::Unlimited;
    let expected = dive_api::simulate_file(&request)?;
    let limited = dive_api::simulate_file(&{
        let mut built = request;
        built.max_memory = BudgetSetting::Explicit(1);
        built
    })?;
    assert_eq!(expected.to_json()["result"], "resource-limit");
    assert_eq!(limited.exit_code(), expected.exit_code());
    assert_eq!(limited.to_json()["result"], expected.to_json()["result"]);
    assert_eq!(limited.to_json()["message"], expected.to_json()["message"]);
    assert!(limited.to_json()["stopped"].is_null());
    Ok(())
}

#[test]
fn simulation_does_not_replay_a_retained_prefix_after_exhaustion() -> Result {
    let (_scratch, file) = model("0")?;
    for compat in [false, true] {
        for (traces, depth) in [(100_000, 10), (1, 10_000_000)] {
            let start = std::time::Instant::now();
            let report = dive_api::simulate_file(&{
                let mut built = simulation(&file, compat);
                built.traces = traces;
                built.depth = Some(depth);
                built.time_limit = BudgetSetting::Explicit(Duration::from_millis(50));
                built
            })?;
            stopped(&report.to_json(), "time", "simulation");
            assert_eq!(report.to_json()["traces"], serde_json::json!([]));
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
    Ok(())
}
