//! Run budgets (ADR 0107): `--time-limit` and `--max-memory` stop a run with
//! a structured `stopped` result in both profiles, at every worker count and
//! in every phase, and never with a verdict.
#[path = "support/refused.rs"]
mod refused;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::{BudgetSetting, CheckRequest, ExitConvention, exit};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch, Box<dyn std::error::Error>> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "dive-budget-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    Ok(Scratch(root))
}

/// A wide, deep state space no test finishes: `x` and `y` count to 10^9.
const ENDLESS: &str = "---- MODULE Endless ----\nEXTENDS Naturals\nVARIABLES x, y\nInit == x = 0 /\\ y = 0\nNext == \\/ x < 1000000000 /\\ x' = x + 1 /\\ y' = y\n        \\/ y < 1000000000 /\\ y' = y + 1 /\\ x' = x\nLive == <>(x = 1000000000)\nBounded == []<>(x <= 1000000000)\n====\n";

fn write(dir: &Path, cfg: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let file = dir.join("Endless.tla");
    std::fs::write(&file, ENDLESS)?;
    std::fs::write(dir.join("Endless.cfg"), cfg)?;
    Ok(file)
}

fn request(file: &Path, compat_tlc: bool, workers: usize) -> CheckRequest {
    CheckRequest {
        file: file.to_owned(),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        workers: Some(workers),
        ..CheckRequest::default()
    }
}

fn assert_stopped(json: &serde_json::Value, resource: &str, phase: &str) {
    assert_eq!(json["verdict"], "stopped", "{json}");
    assert_eq!(json["stopped"]["resource"], resource, "{json}");
    assert_eq!(json["stopped"]["phase"], phase, "{json}");
    assert!(json["stopped"]["observed"].as_u64() > Some(0), "{json}");
    assert!(json["stopped"]["hint"].as_str().is_some(), "{json}");
    assert!(json["tlc"].is_null(), "a stop has no TLC class: {json}");
    assert!(json["counterexample"].is_null(), "{json}");
}

#[test]
fn explicit_search_caps_report_actual_prefixes_and_never_temporal_completion()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    for temporal in [false, true] {
        let file = write(
            &scratch.0,
            if temporal {
                "INIT Init\nNEXT Next\nPROPERTY Live\n"
            } else {
                "INIT Init\nNEXT Next\n"
            },
        )?;
        for compat_tlc in [false, true] {
            for workers in [1, 4] {
                for (resource, states, depth, limit) in
                    [("states", Some(5), None, 5), ("depth", None, Some(2), 2)]
                {
                    let report = dive_api::check_file(&CheckRequest {
                        max_states: states,
                        max_depth: depth,
                        ..request(&file, compat_tlc, workers)
                    })?;
                    let json = report.to_json();
                    if temporal {
                        // A temporal capture stops at the cap without a
                        // verdict (ADR 0158).
                        assert_stopped(&json, resource, "graph-capture");
                        assert_eq!(json["stopped"]["limit"], limit);
                        assert_eq!(json["stopped"]["observed"], limit + 1);
                        assert_eq!(json["execution"]["capture_completed"], false, "{json}");
                        for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                            assert_eq!(report.exit_code_for(convention), exit::STOPPED);
                        }
                        continue;
                    }
                    assert_stopped(&json, resource, "safety-search");
                    assert_eq!(json["stopped"]["limit"], limit);
                    assert_eq!(json["stopped"]["observed"], limit + 1);
                    assert_eq!(json["statistics"]["exhaustive"], false);
                    assert_ne!(json["execution"]["capture_completed"], true);
                    assert_eq!(
                        json["options"]["budget"]["max_states"],
                        serde_json::json!(states)
                    );
                    assert_eq!(
                        json["options"]["budget"]["max_depth"],
                        serde_json::json!(depth)
                    );
                    assert_eq!(
                        json["options"]["profile"],
                        if compat_tlc { "tlc+overrides" } else { "dive" }
                    );
                    for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                        assert_eq!(report.exit_code_for(convention), exit::STOPPED);
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn an_exhausted_time_limit_stops_safety_search_in_both_profiles_and_widths()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = write(&scratch.0, "INIT Init\nNEXT Next\n")?;
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            let report = dive_api::check_file(&CheckRequest {
                time_limit: BudgetSetting::Explicit(Duration::from_millis(300)),
                ..request(&file, compat_tlc, workers)
            })?;
            let json = report.to_json();
            assert_stopped(&json, "time", "safety-search");
            assert_eq!(json["schema_version"], 4);
            assert_eq!(json["stopped"]["limit"], 300);
            assert!(json["statistics"]["distinct"].as_u64() > Some(0), "{json}");
            assert_eq!(json["options"]["budget"]["time_limit_ms"], 300);
            assert_eq!(json["options"]["budget"]["time_limit_source"], "explicit");
            // TLC has no time limit: an explicit one is a compatibility override.
            assert_eq!(
                json["options"]["profile"],
                if compat_tlc { "tlc+overrides" } else { "dive" }
            );
            assert_eq!(json["statistics"]["reported_by_tlc"], false);
            for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                assert_eq!(report.exit_code_for(convention), exit::STOPPED);
            }
            assert!(
                report
                    .render_human()
                    .contains("no verdict produced (raise --time-limit")
            );
        }
    }
    Ok(())
}

#[test]
fn an_exhausted_time_limit_stops_graph_capture_with_no_temporal_claim()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    // Every behavior satisfies Bounded, including stuttering at the initial state.
    let file = write(&scratch.0, "INIT Init\nNEXT Next\nPROPERTY Bounded\n")?;
    for compat_tlc in [false, true] {
        let report = dive_api::check_file(&CheckRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(300)),
            ..request(&file, compat_tlc, 1)
        })?;
        let json = report.to_json();
        // Core explores the liveness product while it captures the graph
        // (#913), so the stop lands in either phase depending on the
        // machine; neither claims a temporal result.
        let phase = json["stopped"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(
            ["graph-capture", "temporal-analysis"].contains(&phase.as_str()),
            "{json}"
        );
        assert_stopped(&json, "time", &phase);
        assert_eq!(report.exit_code(), exit::STOPPED);
    }
    Ok(())
}

#[test]
fn a_memory_ceiling_below_the_footprint_stops_the_run() -> Result<(), Box<dyn std::error::Error>> {
    if dive_runtime::budget::resident_bytes().is_none() {
        return Ok(());
    }
    let scratch = scratch()?;
    let file = write(&scratch.0, "INIT Init\nNEXT Next\n")?;
    for compat_tlc in [false, true] {
        let report = dive_api::check_file(&CheckRequest {
            max_memory: BudgetSetting::Explicit(1 << 20),
            time_limit: BudgetSetting::Explicit(Duration::from_secs(120)),
            ..request(&file, compat_tlc, 1)
        })?;
        let json = report.to_json();
        assert_stopped(&json, "memory", "check-startup");
        assert!(json["statistics"].is_null());
        assert_eq!(json["resources"]["checking_started"], false);
        assert!(json["execution"].is_null());
        assert_eq!(json["stopped"]["limit"], 1 << 20);
        assert_eq!(json["options"]["budget"]["max_memory_source"], "explicit");

        let unsupported = dive_api::check_file(&CheckRequest {
            chaos: Some(0),
            max_memory: BudgetSetting::Explicit(1 << 20),
            ..request(&file, compat_tlc, 1)
        })?;
        refused::assert_refused(
            &unsupported.to_json(),
            unsupported.exit_code(),
            refused::CHAOS,
        );
    }
    Ok(())
}

#[test]
fn an_exhausted_time_limit_stops_a_long_initial_state_enumeration()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("WideInit.tla");
    std::fs::write(
        &file,
        "---- MODULE WideInit ----\nEXTENDS Naturals\nVARIABLES x, y\nInit == x \\in 0..100000 /\\ y \\in 0..100000\nNext == UNCHANGED <<x, y>>\n====\n",
    )?;
    std::fs::write(scratch.0.join("WideInit.cfg"), "INIT Init\nNEXT Next\n")?;
    for compat_tlc in [false, true] {
        let json = dive_api::check_file(&CheckRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(300)),
            ..request(&file, compat_tlc, 1)
        })?
        .to_json();
        assert_stopped(&json, "time", "safety-search");
        assert_eq!(json["statistics"]["reported_by_tlc"], false, "{json}");
        assert!(
            json["stopped"]["elapsed_ms"].as_u64() >= json["stopped"]["observed"].as_u64(),
            "{json}"
        );
    }
    Ok(())
}

/// A budget never changes a result it reports: every run is either stopped
/// or exactly the unbudgeted result, at every width and claim order. Across
/// the sweep some limits trip while a level's blocks are in flight.
#[test]
fn a_budget_either_stops_or_reports_exactly_the_unbudgeted_result()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("Grid.tla");
    std::fs::write(
        &file,
        "---- MODULE Grid ----\nEXTENDS Naturals\nVARIABLES x, y, z\nInit == x = 0 /\\ y = 0 /\\ z = 0\nNext == \\/ x' = x + 1 /\\ UNCHANGED <<y, z>>\n        \\/ y' = y + 1 /\\ UNCHANGED <<x, z>>\n        \\/ z' = z + 1 /\\ UNCHANGED <<x, y>>\nInv == x + y + z < 40 \\/ x # 13\n====\n",
    )?;
    std::fs::write(
        scratch.0.join("Grid.cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let key = |json: &serde_json::Value| {
        (
            json["verdict"].clone(),
            json["property"].clone(),
            json["statistics"]["distinct"].clone(),
            json["statistics"]["generated"].clone(),
            json["statistics"]["depth"].clone(),
            json["counterexample"]["length"].clone(),
        )
    };
    for compat_tlc in [false, true] {
        let reference = dive_api::check_file(&CheckRequest {
            time_limit: BudgetSetting::Unlimited,
            ..request(&file, compat_tlc, 1)
        })?
        .to_json();
        assert_eq!(reference["verdict"], "invariant-violation", "{reference}");
        let mut stopped = 0;
        for (i, ms) in [1_u64, 2, 4, 8, 16, 32, 64, 128, 256]
            .into_iter()
            .enumerate()
        {
            let mut budgeted = CheckRequest {
                time_limit: BudgetSetting::Explicit(Duration::from_millis(ms)),
                chaos: Some(i as u64),
                ..request(&file, compat_tlc, 4)
            };
            // Only the retired reference engine ran --chaos (#791, #910).
            let chaotic = dive_api::check_file(&budgeted)?;
            refused::assert_refused(&chaotic.to_json(), chaotic.exit_code(), refused::CHAOS);
            budgeted.chaos = None;
            let json = dive_api::check_file(&budgeted)?.to_json();
            if json["verdict"] == "stopped" {
                stopped += 1;
            } else {
                assert_eq!(key(&json), key(&reference), "{ms} ms: {json}");
            }
        }
        // The sweep must exercise stops, or it proves nothing.
        assert!(stopped > 0);
    }
    Ok(())
}

#[test]
fn profiles_resolve_their_default_budgets_and_generous_budgets_change_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let spec = scratch.0.join("Small.tla");
    std::fs::write(
        &spec,
        "---- MODULE Small ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x < 9 /\\ x' = x + 1\nInv == x < 10\n====\n",
    )?;
    std::fs::write(
        scratch.0.join("Small.cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let known_memory = dive_runtime::budget::default_memory();
    for compat_tlc in [false, true] {
        let mut req = request(&spec, compat_tlc, 1);
        req.no_deadlock = true;
        let json = dive_api::check_file(&req)?.to_json();
        assert_eq!(json["verdict"], "success", "{json}");
        let budget = &json["options"]["budget"];
        // Neither profile has a default time limit, as in TLC: only a
        // user-set --time-limit arms the wall-clock budget.
        assert!(budget["time_limit_ms"].is_null(), "{compat_tlc}: {budget}");
        assert_eq!(
            budget["time_limit_source"], "none",
            "{compat_tlc}: {budget}"
        );
        match known_memory {
            Some(bytes) => {
                assert_eq!(budget["max_memory_bytes"], bytes);
                assert_eq!(budget["max_memory_source"], "default");
            }
            None => assert_eq!(budget["max_memory_source"], "unavailable"),
        }
        let unlimited = dive_api::check_file(&CheckRequest {
            time_limit: BudgetSetting::Unlimited,
            max_memory: BudgetSetting::Unlimited,
            ..req.clone()
        })?
        .to_json();
        assert_eq!(unlimited["verdict"], "success");
        assert_eq!(unlimited["statistics"], json["statistics"]);
        assert!(unlimited["options"]["budget"]["time_limit_ms"].is_null());
        assert!(unlimited["options"]["budget"]["max_memory_bytes"].is_null());
    }
    Ok(())
}

#[test]
fn a_large_interval_reaches_the_initial_state_budget_without_materialization()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("WideInterval.tla");
    std::fs::write(
        &file,
        "---- MODULE WideInterval ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..50000000\nNext == x' = x\n====\n",
    )?;
    std::fs::write(scratch.0.join("WideInterval.cfg"), "INIT Init\nNEXT Next\n")?;
    for compat_tlc in [false, true] {
        let report = dive_api::check_file(&CheckRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(50)),
            max_memory: BudgetSetting::Explicit(1 << 30),
            ..request(&file, compat_tlc, 1)
        })?;
        let json = report.to_json();
        assert_stopped(&json, "time", "safety-search");
        assert!(
            json["stopped"]["elapsed_ms"].as_u64() < Some(1500),
            "{json}"
        );
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), exit::STOPPED);
    }
    Ok(())
}

#[test]
fn a_large_interval_reaches_the_successor_budget_at_each_worker_width()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("WideNextInterval.tla");
    std::fs::write(
        &file,
        "---- MODULE WideNextInterval ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' \\in 0..50000000\n====\n",
    )?;
    std::fs::write(
        scratch.0.join("WideNextInterval.cfg"),
        "INIT Init\nNEXT Next\n",
    )?;
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            let report = dive_api::check_file(&CheckRequest {
                time_limit: BudgetSetting::Explicit(Duration::from_millis(50)),
                max_memory: BudgetSetting::Explicit(1 << 30),
                ..request(&file, compat_tlc, workers)
            })?;
            let json = report.to_json();
            assert_stopped(&json, "time", "safety-search");
            assert!(
                json["stopped"]["elapsed_ms"].as_u64() < Some(1500),
                "{json}"
            );
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), exit::STOPPED);
        }
    }
    Ok(())
}

#[test]
fn a_one_byte_ceiling_stops_short_safety_and_temporal_requests_before_binding()
-> Result<(), Box<dyn std::error::Error>> {
    if dive_runtime::budget::resident_bytes().is_none() {
        return Ok(());
    }
    let scratch = scratch()?;
    let file = scratch.0.join("Tiny.tla");
    std::fs::write(
        &file,
        "---- MODULE Tiny ----\nEXTENDS Naturals, TLC\nVARIABLE x\nASSUME PrintT(\"binding-ran\") /\\ FALSE\nInit == x \\in 0..1000000000\nNext == UNCHANGED x\nP == <>TRUE\n====\n",
    )?;
    for compat in [false, true] {
        for workers in [1, 8] {
            for (property, temporal) in [
                (false, refused::Mode::Ordinary),
                (true, refused::Mode::Ordinary),
                (true, refused::Mode::FinalShort),
            ] {
                std::fs::write(
                    file.with_extension("cfg"),
                    if property {
                        "INIT Init\nNEXT Next\nPROPERTY P\n"
                    } else {
                        "INIT Init\nNEXT Next\n"
                    },
                )?;
                let report = dive_api::check_file(&CheckRequest {
                    max_memory: BudgetSetting::Explicit(1),
                    time_limit: BudgetSetting::Unlimited,
                    temporal_refusal: temporal.refusal(),
                    ..request(&file, compat, workers)
                })?;
                let json = report.to_json();
                assert_stopped(&json, "memory", "check-startup");
                assert_eq!(report.exit_code(), 5);
                assert_eq!(json["stopped"]["limit"], 1);
                assert!(json["stopped"]["observed"].as_u64() > Some(1));
                assert!(json["statistics"].is_null(), "no search began: {json}");
                assert_eq!(json["resources"]["checking_started"], false);
                // A start-up stop precedes binding, so no temporal report
                // family is chosen, with or without a refused strategy.
                assert_eq!(json["check_kind"], "safety", "{temporal:?}");
                assert!(
                    json["execution"].is_null(),
                    "safety envelope cannot claim temporal execution"
                );
                assert_eq!(json["resources"]["memory"]["initial_sampled"], true);
                assert_eq!(json["resources"]["memory"]["final_sampled"], true);
                assert!(json["resources"]["memory"]["samples"].as_u64() >= Some(2));
                assert_eq!(json["options"]["budget"]["max_memory_source"], "explicit");
                assert_eq!(
                    json["output"],
                    serde_json::json!([]),
                    "future ASSUME was not evaluated"
                );
            }
        }
        // This control proves the binding effect/error would occur without the cut.
        std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
        let ordinary = dive_api::check_file(&CheckRequest {
            max_memory: BudgetSetting::Unlimited,
            time_limit: BudgetSetting::Unlimited,
            ..request(&file, compat, 1)
        })?
        .to_json();
        // Without the cut, startup reaches the printing ASSUME. Only the
        // retired reference engine reported its failure with the output
        // (#791, #910); Core refuses that report by name.
        assert_eq!(ordinary["verdict"], "unsupported", "{ordinary}");
        assert!(
            ordinary["message"]
                .as_str()
                .is_some_and(|message| message.contains("ASSUME")),
            "{ordinary}"
        );
    }
    Ok(())
}

#[test]
fn loading_errors_remain_before_initial_memory_sampling() -> Result<(), Box<dyn std::error::Error>>
{
    let scratch = scratch()?;
    let file = scratch.0.join("Small.tla");
    std::fs::write(
        &file,
        "---- MODULE Small ----\nVARIABLE x\nInit == x = TRUE\nNext == UNCHANGED x\n====\n",
    )?;
    let req = CheckRequest {
        max_memory: BudgetSetting::Explicit(1),
        ..request(&file, false, 1)
    };
    assert!(matches!(
        dive_api::check_file(&req),
        Err(dive_api::ApiError::Io { .. })
    ));
    std::fs::write(file.with_extension("cfg"), "INIT\n")?;
    let rejected = dive_api::check_file(&req)?.to_json();
    assert_eq!(rejected["verdict"], "rejected");
    assert_eq!(rejected["resources"]["memory"]["status"], "not-started");
    assert_eq!(rejected["resources"]["memory"]["samples"], 0);
    assert!(rejected.get("stopped").is_none());
    Ok(())
}

#[test]
fn evaluator_loops_stop_next_even_when_no_successor_is_emitted()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("LoopNext.tla");
    std::fs::write(
        &file,
        "---- MODULE LoopNext ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..31\nNext == /\\ x' = x\n        /\\ (\\A y \\in 1..30000 : \\A z \\in 1..1000 : x+y+z >= 0)\n        /\\ FALSE\n====\n",
    )?;
    std::fs::write(scratch.0.join("LoopNext.cfg"), "INIT Init\nNEXT Next\n")?;
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            let report = dive_api::check_file(&CheckRequest {
                time_limit: BudgetSetting::Explicit(Duration::from_millis(50)),
                max_memory: BudgetSetting::Unlimited,
                ..request(&file, compat_tlc, workers)
            })?;
            let json = report.to_json();
            assert_stopped(&json, "time", "safety-search");
            assert!(
                json["stopped"]["elapsed_ms"].as_u64() < Some(1500),
                "{json}"
            );
            assert_eq!(json["statistics"]["generated"], 32, "{json}");
            assert_eq!(json["statistics"]["initial_generated"], 32, "{json}");
            assert_eq!(json["statistics"]["distinct"], 32, "{json}");
            assert_eq!(json["statistics"]["depth"], 1, "{json}");
            // The 50 ms stop lands while some initial state is expanding:
            // those already expanded (no successors) leave the queue, and the
            // interrupted one stays on it. How many finish first depends on
            // the machine.
            let left = json["statistics"]["left_on_queue"].as_u64();
            assert!(left.is_some_and(|left| (1..=32).contains(&left)), "{json}");
            assert_ne!(json["execution"]["capture_completed"], true, "{json}");
            assert_eq!(report.exit_code(), exit::STOPPED);
        }
    }
    Ok(())
}

/// The refusal and the phase-specific stop anchor their 50 ms deadline at the
/// entry to atom evaluation, so a runner descheduled before then cannot
/// expire the budget in an earlier phase (#1336). The same budget counted
/// from the start still stops the run by wall clock, in whichever phase the
/// clock runs out.
#[test]
fn evaluator_loops_stop_temporal_atoms_after_actual_complete_capture()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    let file = scratch.0.join("LoopAtom.tla");
    std::fs::write(
        &file,
        "---- MODULE LoopAtom ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == []<>(\\A y \\in 1..1000 : \\A z \\in 1..10000 : x+y+z >= 0)\n====\n",
    )?;
    std::fs::write(
        scratch.0.join("LoopAtom.cfg"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    for compat_tlc in [false, true] {
        let mut budgeted = CheckRequest {
            temporal_refusal: Some(refused::FINAL_SHORT.into()),
            time_limit: BudgetSetting::Explicit(Duration::from_millis(50)),
            max_memory: BudgetSetting::Unlimited,
            hold_time_until_temporal_atoms: true,
            ..request(&file, compat_tlc, 1)
        };
        // Only the retired reference engine ran bounded final checking
        // (#791, #910); Core refuses it and checks the property in its
        // ordinary temporal mode.
        let final_short = dive_api::check_file(&budgeted)?;
        refused::assert_refused(
            &final_short.to_json(),
            final_short.exit_code(),
            refused::FINAL_SHORT,
        );
        budgeted.temporal_refusal = None;
        let wall_clock = dive_api::check_file(&CheckRequest {
            hold_time_until_temporal_atoms: false,
            ..budgeted.clone()
        })?;
        let json = wall_clock.to_json();
        assert_eq!(json["verdict"], "stopped", "{json}");
        assert_eq!(json["stopped"]["resource"], "time", "{json}");
        assert!(json["tlc"].is_null(), "{json}");
        assert!(json["counterexample"].is_null(), "{json}");
        assert_eq!(wall_clock.exit_code(), exit::STOPPED, "{json}");
        let report = dive_api::check_file(&budgeted)?;
        let json = report.to_json();
        assert_stopped(&json, "time", "temporal-analysis");
        assert_eq!(json["execution"]["stage"], "atoms", "{json}");
        assert_eq!(json["execution"]["capture_completed"], true, "{json}");
        assert_eq!(json["statistics"]["generated"], 2, "{json}");
        assert_eq!(json["statistics"]["distinct"], 1, "{json}");
        assert_eq!(json["statistics"]["initial_generated"], 1, "{json}");
        assert_eq!(json["statistics"]["depth"], 1, "{json}");
        assert!(
            json["stopped"]["elapsed_ms"].as_u64() < Some(1500),
            "{json}"
        );
        assert_eq!(report.exit_code(), exit::STOPPED, "{json}");
    }
    Ok(())
}

/// A time budget spent by start-up work (Core's constant evaluation while it
/// compiles the plan, or the binder's, which also evaluates a printing
/// constant and every constant under `--compat=tlc`) stops the run in
/// `check-startup`, before `Init`, with no statistics and the start-up output
/// printed so far, in the safety and temporal envelopes (#968).
#[test]
fn a_time_budget_spent_at_startup_stops_in_check_startup() -> Result<(), Box<dyn std::error::Error>>
{
    let scratch = scratch()?;
    let file = scratch.0.join("Startup.tla");
    for print in [false, true] {
        // `Big` is a constant definition: start-up evaluates it once, building
        // a 250 000-pair set, far longer than the 10 ms limit. The model is
        // endless afterwards, so a later stop is never mistaken for a verdict.
        let effect = if print {
            "PrintT(\"startup-print\") /\\ "
        } else {
            ""
        };
        std::fs::write(
            &file,
            format!(
                "---- MODULE Startup ----\nEXTENDS Naturals,FiniteSets,TLC\nVARIABLE x\nBig == {effect}Cardinality({{<<i,j>>:i \\in 1..500,j \\in 1..500}}) > 0\nInit == x = 0\nNext == Big /\\ x' = x + 1\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nLive == <>(x = 1000000000)\n====\n"
            ),
        )?;
        for (cfg, temporal) in [
            ("INIT Init\nNEXT Next\n", false),
            ("SPECIFICATION Spec\nPROPERTY Live\n", true),
        ] {
            std::fs::write(scratch.0.join("Startup.cfg"), cfg)?;
            for compat_tlc in [false, true] {
                let case = format!("print={print} temporal={temporal} compat_tlc={compat_tlc}");
                let report = dive_api::check_file(&CheckRequest {
                    time_limit: BudgetSetting::Explicit(Duration::from_millis(10)),
                    max_memory: BudgetSetting::Explicit(1 << 34),
                    ..request(&file, compat_tlc, 1)
                })?;
                let json = report.to_json();
                assert_stopped(&json, "time", "check-startup");
                assert_eq!(report.exit_code(), exit::STOPPED, "{case}");
                assert!(json["statistics"].is_null(), "{case}: {json}");
                // Binding ran: this is not the stop before checking starts.
                assert!(
                    json["resources"]["checking_started"].is_null(),
                    "{case}: {json}"
                );
                let output = if print {
                    vec!["\"startup-print\""]
                } else {
                    vec![]
                };
                assert_eq!(json["output"], serde_json::json!(output), "{case}: {json}");
                assert_eq!(
                    json["check_kind"],
                    if temporal { "temporal" } else { "safety" },
                    "{case}"
                );
                if temporal {
                    assert_eq!(
                        json["execution"]["stage"], "check-startup",
                        "{case}: {json}"
                    );
                    assert_eq!(
                        json["execution"]["capture_completed"], false,
                        "{case}: {json}"
                    );
                    assert!(json["execution"]["started"].is_null(), "{case}: {json}");
                } else {
                    assert!(json["execution"].is_null(), "{case}: {json}");
                }
            }
        }
    }
    // A refusal that holds at any budget still precedes the start-up stop.
    std::fs::write(
        scratch.0.join("Startup.cfg"),
        "SPECIFICATION Spec\nPROPERTY Live\n",
    )?;
    for compat_tlc in [false, true] {
        let report = dive_api::check_file(&CheckRequest {
            time_limit: BudgetSetting::Explicit(Duration::from_millis(10)),
            max_memory: BudgetSetting::Explicit(1 << 34),
            frontier_memory_bytes: Some(0),
            ..request(&file, compat_tlc, 1)
        })?;
        let json = report.to_json();
        assert_eq!(json["verdict"], "unsupported", "{json}");
        refused::assert_refused(&json, report.exit_code(), refused::TEMPORAL_SPILLED_STORE);
        assert_eq!(report.exit_code(), exit::UNSUPPORTED, "{json}");
    }
    Ok(())
}
