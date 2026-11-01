//! Finite execution modes stop with actual resource evidence and prefix progress.
use dive_api::{BudgetSetting, DfidRequest, SimulateRequest, dfid_file, simulate_file};
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
    SimulateRequest {
        file: file.into(),
        compat_tlc,
        seed: Some(7),
        traces: 100_000,
        depth: Some(100_000),
        max_memory: BudgetSetting::Unlimited,
        ..Default::default()
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
        let simulation = simulate_file(&SimulateRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(20)),
            ..simulation(&file, compat)
        })?;
        assert_eq!(simulation.exit_code(), 5);
        stopped(&simulation.to_json(), "time", "simulation");
        let iterative = dfid_file(&DfidRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(20)),
            ..dfid(&file, compat)
        })?;
        assert_eq!(iterative.exit_status(), 5, "{}", iterative.to_json());
        stopped(&iterative.to_json(), "time", "dfid");
        for (document, phase) in [
            (
                simulate_file(&SimulateRequest {
                    max_memory: BudgetSetting::Explicit(1),
                    time_limit: BudgetSetting::Unlimited,
                    ..self::simulation(&file, compat)
                })?
                .to_json(),
                "simulation",
            ),
            (
                dfid_file(&DfidRequest {
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
        let simulation = simulate_file(&SimulateRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(10)),
            ..simulation(&file, compat)
        })?
        .to_json();
        stopped(&simulation, "time", "simulation");
        assert_eq!(simulation["statistics"]["initial_candidates"], 0);
        let iterative = dfid_file(&DfidRequest {
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
fn unlimited_and_generous_budgets_preserve_seeded_samples() -> Result {
    let (_scratch, file) = model("0")?;
    for compat in [false, true] {
        let request = SimulateRequest {
            traces: 2,
            depth: Some(4),
            time_limit: BudgetSetting::Unlimited,
            ..simulation(&file, compat)
        };
        let unlimited = simulate_file(&request)?.to_json();
        let generous = simulate_file(&SimulateRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_secs(60)),
            ..request
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
        let refused = simulate_file(&SimulateRequest {
            workers: Some(8),
            max_memory: BudgetSetting::Explicit(1),
            ..simulation(&file, compat)
        })?
        .to_json();
        assert_eq!(refused["result"], "unsupported");
        assert_eq!(refused["options"]["workers_used"], 0);
    }
    Ok(())
}
