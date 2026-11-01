//! Compiled finite simulation retains check semantics and independent witnesses.
use dive_api::{Engine, SimulateRequest, simulate_file, simulate_file_engine};
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
        let plain = simulate_file_engine(&request, Engine::Core)?.to_json();
        checked(&plain);
        assert_eq!(plain["result"], "sampled");
        assert_eq!(
            plain,
            simulate_file_engine(&request, Engine::Core)?.to_json()
        );
        let counted = simulate_file_engine(
            &SimulateRequest {
                action_stats: true,
                ..request
            },
            Engine::Core,
        )?
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
            let reference = simulate_file(&request)?.to_json();
            let native = simulate_file_engine(&request, Engine::Core)?.to_json();
            assert_eq!(native["engine"], "core", "{name}: {native}");
            for field in ["result", "tlc_code", "exit_code", "violation_validated"] {
                assert_eq!(native[field], reference[field], "{name}/{field}: {native}");
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
            let reference = simulate_file(&request)?.to_json();
            let native = simulate_file_engine(&request, Engine::Core)?.to_json();
            checked(&native);
            for field in [
                "result",
                "tlc_code",
                "runtime_output",
                "startup_output",
                "violation_validated",
            ] {
                assert_eq!(native[field], reference[field], "{name}/{field}: {native}");
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
            let report = simulate_file_engine(
                &SimulateRequest {
                    file: file.clone(),
                    compat_tlc: compat,
                    seed: Some(0),
                    traces: 10,
                    depth: Some(10),
                    ..Default::default()
                },
                Engine::Core,
            )?
            .to_json();
            checked(&report);
            assert_eq!(report["result"], "violation", "{report}");
            assert_eq!(report["observed_violation"]["name"], "Inv");
            assert_eq!(report["violation_validated"], true);
        }
    }
    Ok(())
}

#[test]
fn auto_selects_reference_before_runtime_effects_and_core_never_retries() -> Result {
    for compat in [false, true] {
        let request = request("runtime-print/RuntimePrintRegister", compat);
        let reference = simulate_file(&request)?.to_json();
        let automatic = simulate_file_engine(&request, Engine::Auto)?.to_json();
        assert_eq!(automatic["engine"], "reference");
        assert_eq!(automatic["engine_fallback"]["stage"], "static");
        assert_eq!(automatic["runtime_output"], reference["runtime_output"]);
        assert_eq!(automatic["statistics"], reference["statistics"]);
        let explicit = simulate_file_engine(&request, Engine::Core)?.to_json();
        assert_eq!(explicit["engine"], "core");
        assert!(explicit["engine_fallback"].is_null());
        assert_eq!(explicit["result"], "unsupported");
        assert_eq!(explicit["runtime_output"], reference["runtime_output"]);
    }
    let (_scratch, file) = source(
        "DeniedArgs",
        "---- MODULE DeniedArgs ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x = 0\nNext == TLCSet(0, PrintT(\"must-not-run\")) /\\ x' = x + 1\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    let report = simulate_file_engine(
        &SimulateRequest {
            file,
            seed: Some(0),
            traces: 1,
            depth: Some(1),
            ..Default::default()
        },
        Engine::Core,
    )?
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
        let report = simulate_file_engine(
            &SimulateRequest {
                file: file.clone(),
                compat_tlc: compat,
                seed: Some(7),
                traces: 100_000,
                depth: Some(10),
                time_limit: dive_api::BudgetSetting::Explicit(std::time::Duration::from_millis(50)),
                max_memory: dive_api::BudgetSetting::Unlimited,
                ..Default::default()
            },
            Engine::Core,
        )?
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
