//! Compiled finite simulation retains check semantics and independent witnesses.
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_simulate.json");
use dive_api::{SimulateRequest, simulate_file};
use serde_json::Value;
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn request(name: &str, compat_tlc: bool) -> SimulateRequest {
    SimulateRequest {
        file: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../dive-sim/tests/fixtures/{name}.tla")),
        compat_tlc,
        seed: Some(7),
        traces: 3,
        depth: Some(5),
        ..Default::default()
    }
}
fn checked(document: &Value) {
    assert_eq!(document["engine"], "core", "{document}");
    assert_eq!(document["engine_requested"], "core");
    assert_eq!(
        document["options"]["execution_scope"],
        "compiled-plan-simulation"
    );
    assert_eq!(document["options"]["workers_used"], 1, "{document}");
    assert_eq!(document["model_proved"], false);
    assert!(document["engine_fallback"].is_null());
    for trace in document["traces"].as_array().into_iter().flatten() {
        assert_eq!(trace["validation"]["result"], "valid", "{document}");
    }
}

#[test]
fn core_samples_replay_and_statistics_do_not_change_choices() -> Result {
    for compat in [false, true] {
        let request = request("SimChoice", compat);
        let plain = simulate_file(&request)?.to_json();
        checked(&plain);
        assert_eq!(plain["result"], "sampled");
        assert_eq!(plain, simulate_file(&request)?.to_json());
        let counted = simulate_file(&SimulateRequest {
            action_stats: true,
            ..request
        })?
        .to_json();
        checked(&counted);
        for field in ["result", "statistics", "traces", "runtime_output"] {
            assert_eq!(plain[field], counted[field]);
        }
        assert_eq!(counted["action_flow"]["mode"], "basic");
    }
    Ok(())
}

#[test]
fn deterministic_violations_and_errors_keep_reference_classes() -> Result {
    for compat in [false, true] {
        for name in [
            "SimInvariant",
            "SimInitialInvariant",
            "SimInitialPrecedence",
            "SimDeadlock",
            "SimActionProperty",
            "SimInvariantError",
            "SimAssumeFalse",
            "SimEmpty",
            "SimExcludedInitial",
            "SimIncomplete",
            "SimInitialIncomplete",
        ] {
            let request = request(name, compat);
            let reference = dive_api::simulate_file(&request)?.to_json();
            let native = simulate_file(&request)?.to_json();
            assert_eq!(native["engine"], "core", "{name}: {native}");
            for field in ["result", "tlc_code", "exit_code", "violation_validated"] {
                agreed::check_eq(
                    AGREED,
                    "deterministic_violations_and_errors_keep_reference_classes/0",
                    &(native[field]),
                    &(reference[field]),
                );
            }
            if native["result"] == "violation" {
                checked(&native);
            }
        }
    }
    Ok(())
}

#[test]
fn direct_captured_and_configured_prints_keep_actual_output() -> Result {
    for compat in [false, true] {
        for name in [
            "RuntimePrintDirect",
            "RuntimePrintCaptured",
            "RuntimePrintConfigured",
            "RuntimePrintConditional",
        ] {
            let request = request(&format!("runtime-print/{name}"), compat);
            let reference = dive_api::simulate_file(&request)?.to_json();
            let native = simulate_file(&request)?.to_json();
            checked(&native);
            if name == "RuntimePrintConditional" {
                // Only the retired reference engine sampled this RandomElement
                // step (#791, #910); Core refuses it by name.
                assert_eq!(native["result"], "unsupported");
                assert!(
                    native["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("RandomElement"))
                );
                assert_eq!(native["statistics"]["generated"], 2);
                assert_eq!(native["statistics"]["scope"], "completed-prefix");
                assert_eq!(native["violation_validated"], false);
                agreed::check_eq(
                    AGREED,
                    "direct_captured_and_configured_prints_keep_actual_output/0",
                    &(native["runtime_output"]),
                    &(reference["runtime_output"]),
                );
                continue;
            }
            for field in [
                "result",
                "tlc_code",
                "runtime_output",
                "startup_output",
                "violation_validated",
            ] {
                agreed::check_eq(
                    AGREED,
                    "direct_captured_and_configured_prints_keep_actual_output/1",
                    &(native[field]),
                    &(reference[field]),
                );
            }
        }
    }
    Ok(())
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn source(name: &str, text: &str, config: &str) -> Result<(Scratch, PathBuf)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dive-core-mode-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join(format!("{name}.tla"));
    std::fs::write(&file, text)?;
    std::fs::write(file.with_extension("cfg"), config)?;
    Ok((Scratch(directory), file))
}

#[test]
fn compound_states_and_fairness_only_specs_produce_independent_violations() -> Result {
    let compound = "---- MODULE Compound ----\nEXTENDS Naturals, Sequences\nVARIABLES q, f\nInit == q = <<>> /\\ f = [i \\in {1, 2} |-> {}]\nNext == \\/ /\\ Len(q) < 3\n           /\\ q' = Append(q, Len(q))\n           /\\ f' = [f EXCEPT ![1] = @ \\cup {Len(q)}]\n        \\/ UNCHANGED <<q, f>>\nInv == Len(q) < 3\n====\n";
    let fair = "---- MODULE Fair ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 4\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nInv == x < 3\n====\n";
    for (name, text, config) in [
        (
            "Compound",
            compound,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        ),
        ("Fair", fair, "SPECIFICATION Spec\nINVARIANT Inv\n"),
    ] {
        let (_scratch, file) = source(name, text, config)?;
        for compat in [false, true] {
            let report = simulate_file(&SimulateRequest {
                file: file.clone(),
                compat_tlc: compat,
                seed: Some(0),
                traces: 10,
                depth: Some(10),
                ..Default::default()
            })?
            .to_json();
            checked(&report);
            assert_eq!(report["result"], "violation", "{name}: {report}");
            assert_eq!(report["observed_violation"]["name"], "Inv");
            assert_eq!(report["violation_validated"], true);
        }
    }
    Ok(())
}

#[test]
fn view_refuses_before_execution_and_budget_exhaustion_in_every_engine() -> Result {
    use dive_api::BudgetSetting;
    use std::time::Duration;

    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/SimView.tla");
    for compat_tlc in [false, true] {
        {
            for limited in [false, true] {
                let report = simulate_file(&SimulateRequest {
                    file: file.clone(),
                    compat_tlc,
                    seed: Some(7),
                    traces: 1,
                    depth: Some(1),
                    time_limit: if limited {
                        BudgetSetting::Explicit(Duration::from_nanos(1))
                    } else {
                        BudgetSetting::Unlimited
                    },
                    max_memory: if limited {
                        BudgetSetting::Explicit(1)
                    } else {
                        BudgetSetting::Unlimited
                    },
                    ..Default::default()
                })?
                .to_json();
                assert_eq!(report["result"], "unsupported", "{report}");
                assert_eq!(
                    report["message"], "simulation VIEW declarations",
                    "{report}"
                );
                assert_eq!(report["options"]["workers_used"], 0);
                assert_eq!(report["statistics"]["generated"], 0);
                assert_eq!(report["statistics"]["traces_started"], 0);
                assert!(report["stopped"].is_null());
                assert!(report["engine_fallback"].is_null());
                for field in ["startup_output", "runtime_output", "traces"] {
                    assert_eq!(report[field], serde_json::json!([]), "{field}: {report}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn the_default_engine_refuses_runtime_effects_on_core_and_never_retries() -> Result {
    for compat in [false, true] {
        let request = request("runtime-print/RuntimePrintRegister", compat);
        let reference = dive_api::simulate_file(&request)?.to_json();
        let explicit = simulate_file(&request)?.to_json();
        assert_eq!(explicit["engine"], "core");
        assert!(explicit["engine_fallback"].is_null());
        assert_eq!(explicit["result"], "unsupported");
        agreed::check_eq(
            AGREED,
            "the_default_engine_refuses_runtime_effects_on_core_and_never_retries/0",
            &(explicit["runtime_output"]),
            &(reference["runtime_output"]),
        );
        let default = dive_api::simulate_file(&request)?.to_json();
        assert_eq!(default["engine"], "core");
        assert_eq!(default["engine_requested"], "core");
        assert!(default["engine_fallback"].is_null());
        assert_eq!(default["result"], "unsupported");
        assert_eq!(default["runtime_output"], explicit["runtime_output"]);
        assert_eq!(default["statistics"], explicit["statistics"]);
    }
    let (_scratch, file) = source(
        "DeniedArgs",
        "---- MODULE DeniedArgs ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x = 0\nNext == TLCSet(0, PrintT(\"must-not-run\")) /\\ x' = x + 1\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    let report = simulate_file(&SimulateRequest {
        file,
        seed: Some(0),
        traces: 1,
        depth: Some(1),
        ..Default::default()
    })?
    .to_json();
    assert_eq!(report["result"], "unsupported");
    assert_eq!(report["options"]["workers_used"], 1, "{report}");
    assert_eq!(report["runtime_output"], serde_json::json!([]));
    Ok(())
}

#[test]
fn native_prefix_stops_do_not_start_unbounded_independent_replay() -> Result {
    let (_scratch, file) = source(
        "Endless",
        "---- MODULE Endless ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    for compat in [false, true] {
        let start = std::time::Instant::now();
        let report = simulate_file(&SimulateRequest {
            file: file.clone(),
            compat_tlc: compat,
            seed: Some(7),
            traces: 100_000,
            depth: Some(10),
            time_limit: dive_api::BudgetSetting::Explicit(std::time::Duration::from_millis(50)),
            max_memory: dive_api::BudgetSetting::Unlimited,
            ..Default::default()
        })?
        .to_json();
        assert_eq!(report["result"], "stopped", "{report}");
        assert_eq!(report["exit_code"], 5);
        assert_eq!(report["stopped"]["phase"], "simulation");
        assert_eq!(report["violation_validated"], false);
        assert_eq!(report["model_proved"], false);
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
    Ok(())
}

/// `ALIAS` is not applied in simulation, as in checking: the traces show the
/// states themselves and the report says so. A failed lookup, which TLC
/// reports only when it prints a trace, stays a named refusal.
#[test]
fn alias_is_noted_not_applied_and_a_failed_lookup_refuses() -> Result {
    let text = "---- MODULE SimAlias ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nInv == x < 3\nAlias == [y |-> x * 2]\n====\n";
    let (_scratch, file) = source(
        "SimAlias",
        text,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nALIAS Alias\n",
    )?;
    let (_missing_scratch, missing) = source(
        "SimAlias",
        text,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nALIAS Missing\n",
    )?;
    for compat_tlc in [false, true] {
        {
            let request = SimulateRequest {
                file: file.clone(),
                compat_tlc,
                seed: Some(7),
                traces: 3,
                depth: Some(10),
                ..Default::default()
            };
            let report = simulate_file(&request)?.to_json();
            assert_eq!(report["result"], "violation", "{report}");
            assert_eq!(report["observed_violation"]["name"], "Inv");
            assert_eq!(report["violation_validated"], true, "{report}");
            assert_eq!(
                report["notes"],
                serde_json::json!([
                    "ALIAS Alias is not applied: the traces show the states themselves (not implemented yet)."
                ]),
                "{report}"
            );
            let last = report["traces"][0]["document"]["states"]
                .as_array()
                .and_then(|states| states.last())
                .cloned()
                .unwrap_or_default();
            assert_eq!(last["values"], serde_json::json!({"x": 3}), "{report}");
            let refused = simulate_file(&SimulateRequest {
                file: missing.clone(),
                ..request
            })?
            .to_json();
            assert_eq!(refused["result"], "unsupported", "{refused}");
            assert_eq!(
                refused["message"], "simulation ALIAS whose definition lookup fails",
                "{refused}"
            );
        }
    }
    Ok(())
}

/// Simulation keeps only successors inside the constraints, so the last
/// in-model state is a deadlock, as pinned TLC `-simulate` reports (#896).
#[test]
fn constraint_excluded_successors_end_in_a_validated_deadlock() -> Result {
    for (name, compat) in ["SimConstraintDeadlock", "SimLevelConstraintDeadlock"]
        .into_iter()
        .flat_map(|name| [(name, false), (name, true)])
    {
        let request = SimulateRequest {
            traces: 1,
            depth: Some(20),
            seed: Some(0),
            ..request(name, compat)
        };
        let report = simulate_file(&request)?.to_json();
        checked(&report);
        assert_eq!(report["result"], "violation", "{name}: {report}");
        assert_eq!(report["observed_violation"]["kind"], "deadlock");
        assert_eq!(report["violation_validated"], true);
        assert_eq!(report["exit_code"], 11);
        assert_eq!(report["statistics"]["generated"], 4);
        assert_eq!(
            report["traces"][0]["tlc_states"],
            serde_json::json!([{"x": 0}, {"x": 1}, {"x": 2}])
        );
    }
    Ok(())
}
