//! `TLCGet("config")` in each run mode, at 1 and 4 workers, in both profiles
//! (issue #1249). The records match the pinned TLC's
//! (`conformance/corpus/tlcget-config`): `Simulator.createConfig` under
//! `-simulate` and `-generate`, `AbstractChecker.createConfig` otherwise.
//! The worker count is the logical one, 1, by default and the physical one
//! under `--compat=tlc` (D-011); `traces` is the run's total (D-045).
use std::path::{Path, PathBuf};

use dive_api::trace::{RunMode, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, SimulateRequest, check_file, simulate_file};
use serde_json::Value;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "dive-tlcget-config-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    Ok(Scratch(root))
}

/// A module whose only assumption is `assume`, with a three-state cycle.
fn module(dir: &Path, assume: &str) -> Result<PathBuf> {
    let file = dir.join("Cfg.tla");
    std::fs::write(
        &file,
        format!(
            "---- MODULE Cfg ----\nEXTENDS Naturals, TLC\nC == TLCGet(\"config\")\n\
             ASSUME {assume}\nVARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 3\n====\n"
        ),
    )?;
    std::fs::write(dir.join("Cfg.cfg"), "INIT Init\nNEXT Next\n")?;
    Ok(file)
}

/// The record without its install location, which is the executable's path.
fn record(fields: &str) -> String {
    format!(
        "[f \\in DOMAIN C \\ {{\"install\"}} |-> C[f]] = [{fields}] /\\ \"install\" \\in DOMAIN C"
    )
}

fn simulation(file: &Path, compat_tlc: bool, workers: usize, generate: bool) -> SimulateRequest {
    {
        let mut built = SimulateRequest::default();
        file.clone_into(&mut built.file);
        built.compat_tlc = compat_tlc;
        built.seed = Some(5);
        built.traces = 3;
        built.depth = Some(7);
        built.workers = Some(workers);
        built.generation = generate;
        built.keep_traces = Some(dive_api::KEEP_ALL_TRACES);
        built
    }
}

fn sampled(report: &Value) {
    assert_eq!(report["result"], "sampled", "{report}");
    for trace in report["traces"].as_array().into_iter().flatten() {
        assert_eq!(trace["validation"]["result"], "valid", "{report}");
    }
}

#[test]
fn simulation_records_match_tlc_in_every_mode() -> Result {
    let dir = scratch()?;
    for generate in [false, true] {
        for compat in [false, true] {
            for workers in [1, 4] {
                let mode = if generate { "generate" } else { "simulate" };
                let worker = if compat { workers } else { 1 };
                let file = module(
                    &dir.0,
                    &record(&format!(
                        "mode |-> \"{mode}\", deadlock |-> TRUE, seed |-> \"5\", \
                         worker |-> {worker}, traces |-> 3, depth |-> 7, aril |-> \"0\", \
                         sched |-> \"random\""
                    )),
                )?;
                let report =
                    simulate_file(&simulation(&file, compat, workers, generate))?.to_json();
                sampled(&report);
                // Every sampled trace carries the record's simulation fields.
                for trace in report["traces"].as_array().into_iter().flatten() {
                    let run = &trace["document"]["run"];
                    assert_eq!(run["mode"], mode, "{report}");
                    assert_eq!(run["traces"], 3, "{report}");
                    assert_eq!(run["depth"], 7, "{report}");
                    assert_eq!(run["worker"], worker, "{report}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn the_simulation_record_is_not_the_checking_record() -> Result {
    let dir = scratch()?;
    for compat in [false, true] {
        for workers in [1, 4] {
            // The fingerprint is checking's alone; "bfs" is never a sampling
            // mode; `traces` is the total, never TLC's per-worker `num` times
            // the workers (dive's `num` is a total in both profiles).
            for assume in [
                "\"fingerprint\" \\in DOMAIN C",
                "C.mode = \"bfs\"",
                "C.traces = 12",
            ] {
                let file = module(&dir.0, assume)?;
                let report = simulate_file(&simulation(&file, compat, workers, false))?.to_json();
                // TLC's ASSUMPTION_FALSE.
                assert_eq!(report["tlc_code"], 2104, "{assume}: {report}");
            }
        }
    }
    Ok(())
}

#[test]
fn smoke_ewd998_assumption_holds_under_simulation() -> Result {
    let dir = scratch()?;
    let file = module(
        &dir.0,
        "TLCGet(\"config\").mode \\in {\"generate\", \"simulate\"}",
    )?;
    for compat in [false, true] {
        for workers in [1, 4] {
            for generate in [false, true] {
                sampled(&simulate_file(&simulation(&file, compat, workers, generate))?.to_json());
            }
            let checked = check_file(&CheckRequest {
                file: file.clone(),
                compat_tlc: compat,
                seed: Some(5),
                fp: Some(0),
                workers: Some(workers),
                ..CheckRequest::default()
            })?
            .to_json();
            assert_eq!(checked["verdict"], "assumption-failure", "{checked}");
        }
    }
    Ok(())
}

#[test]
fn checking_record_is_unchanged() -> Result {
    let dir = scratch()?;
    for compat in [false, true] {
        for workers in [1, 4] {
            let worker = if compat { workers } else { 1 };
            let file = module(
                &dir.0,
                &format!(
                    "DOMAIN C = {{\"mode\", \"deadlock\", \"seed\", \"fingerprint\", \"worker\", \"install\"}} \
                     /\\ C.mode = \"bfs\" /\\ C.deadlock /\\ C.seed = \"5\" /\\ C.worker = {worker}"
                ),
            )?;
            let checked = check_file(&CheckRequest {
                file,
                compat_tlc: compat,
                seed: Some(5),
                fp: Some(0),
                workers: Some(workers),
                ..CheckRequest::default()
            })?
            .to_json();
            assert_eq!(checked["verdict"], "success", "{checked}");
        }
    }
    Ok(())
}

#[test]
fn unbounded_runs_report_minus_one_and_trace_ids_refuse() -> Result {
    let dir = scratch()?;
    let file = module(&dir.0, "C.traces + 1 = 0")?;
    for compat in [false, true] {
        let report = simulate_file(&{
            let mut built = simulation(&file, compat, 1, false);
            built.traces = dive_api::UNLIMITED_TRACES;
            built.depth = Some(2);
            built.max_generated = Some(30);
            built.time_limit =
                dive_api::BudgetSetting::Explicit(std::time::Duration::from_secs(60));
            built
        })?
        .to_json();
        // The assumption held: sampling started.
        assert!(report["tlc_code"].is_null(), "{report}");
        assert!(
            report["statistics"]["generated"].as_u64() > Some(0),
            "{report}"
        );
        // `--trace-ids` regenerates part of a run whose total it does not know.
        let report = simulate_file(&{
            let mut built = simulation(&file, compat, 1, false);
            built.trace_ids = Some((2, 4));
            built
        })?
        .to_json();
        assert_eq!(report["result"], "unsupported", "{report}");
        assert!(
            report["message"]
                .as_str()
                .is_some_and(|m| m.contains("TLCGet(\"config\") under --trace-ids")),
            "{report}"
        );
    }
    Ok(())
}

#[test]
fn the_validator_evaluates_assumptions_under_the_recorded_run() -> Result {
    let dir = scratch()?;
    let file = module(&dir.0, "C.mode = \"simulate\" /\\ C.traces = 3")?;
    for compat in [false, true] {
        let report = simulate_file(&simulation(&file, compat, 1, false))?.to_json();
        sampled(&report);
        let document = TraceDoc::from_json(&report["traces"][0]["document"])?;
        let run = document
            .run
            .clone()
            .ok_or("a sampled trace records its run")?;
        let validate = |mode: RunMode| -> Result<Validation> {
            let model = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc: compat,
                    run: Some((5, 0)),
                    run_mode: mode,
                    run_workers: Some(run.workers),
                    ..TraceOptions::default()
                },
            )?;
            Ok(model.try_validate_sample(&document)?.validation)
        };
        assert_eq!(
            validate(RunMode::Simulate(run.simulation.clone()))?,
            Validation::Valid
        );
        // Under any other record the assumption is false.
        let mut generate = run.simulation.clone();
        generate.generate = true;
        for mode in [RunMode::Check, RunMode::Simulate(generate)] {
            assert!(
                matches!(validate(mode)?, Validation::Invalid(_)),
                "the assumption must fail outside the recorded run"
            );
        }
        // The record survives the document's round trip.
        assert_eq!(TraceDoc::from_json(&document.to_json())?.run, Some(run));
    }
    Ok(())
}
