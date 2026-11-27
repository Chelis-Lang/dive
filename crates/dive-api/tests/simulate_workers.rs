//! Simulation phase S2 (ADR 0162 §7, §9, §10): workers, retention and scale.
//!
//! The report is a function of the seed and the request, not of the worker
//! count: the lowest violating trace id decides, statistics cover the traces
//! before it, output is released in trace order, and the kept traces are the
//! lowest ids. Cases G1 to G6 and W1 refer to the pinned TLC receipts in
//! `crates/dive-sim/tests/fixtures/temporal/oracle.json`.

use dive_api::{BudgetSetting, ExitConvention, KEEP_ALL_TRACES, SimulateRequest, simulate_file};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn written(module: &str, text: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dive-sim-s2-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join(format!("{module}.tla"));
    std::fs::write(&file, text)?;
    std::fs::write(file.with_extension("cfg"), cfg)?;
    Ok((Scratch(directory), file))
}

fn fixture(module: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sim/tests/fixtures/temporal")
        .join(format!("{module}.tla"));
    written(module, &std::fs::read_to_string(source)?, cfg)
}

/// The report without what legitimately depends on the run: the worker
/// options and the measured budget figures.
fn comparable(report: &Value) -> Value {
    let mut report = report.clone();
    if let Some(options) = report["options"].as_object_mut() {
        options.remove("workers_requested");
        options.remove("workers_used");
        options.remove("budget");
    }
    if let Some(map) = report.as_object_mut() {
        map.remove("budget");
        map.remove("resources");
    }
    report
}

fn same_at_every_width(request: &SimulateRequest) -> Result<Value> {
    let one = simulate_file(&{
        let mut built = request.clone();
        built.workers = Some(1);
        built
    })?
    .to_json();
    for workers in [2, 4, 8] {
        let many = simulate_file(&{
            let mut built = request.clone();
            built.workers = Some(workers);
            built
        })?
        .to_json();
        assert_eq!(many["options"]["workers_used"], workers);
        assert_eq!(comparable(&many), comparable(&one), "{workers} workers");
    }
    Ok(one)
}

/// W1: a walk whose invariant fails only on rare traces. The deciding trace
/// is the lowest violating id at every width, the traces after it are
/// abandoned, and the statistics cover exactly the traces up to it.
#[test]
fn the_lowest_violating_trace_decides_at_every_worker_count() -> Result {
    let (_scratch, file) = fixture("Walk", "SPECIFICATION Spec\nINVARIANT NotFourteen\n")?;
    for compat_tlc in [false, true] {
        let report = same_at_every_width(&{
            let mut built = SimulateRequest::default();
            file.clone_into(&mut built.file);
            built.compat_tlc = compat_tlc;
            built.seed = Some(7);
            built.traces = 100_000;
            built.depth = Some(20);
            built
        })?;
        assert_eq!(report["result"], "violation", "{report}");
        assert_eq!(report["violation_validated"], true);
        let traces = report["traces"].as_array().ok_or("traces")?;
        let deciding = traces.last().ok_or("deciding trace")?;
        let id = deciding["id"].as_u64().ok_or("id")?;
        // Ten samples are kept besides the deciding trace, the lowest ids.
        assert!(id >= 10, "{id}");
        assert_eq!(traces.len(), 11);
        for (index, trace) in traces[..10].iter().enumerate() {
            assert_eq!(trace["id"], index as u64);
            assert_eq!(trace["validation"]["result"], "valid");
        }
        assert_eq!(report["statistics"]["traces_started"], id + 1);
        assert_eq!(report["observed_violation"]["trace"], 10);
        // Trace `id` alone, requested from a fresh run, is the same trace.
        let alone = simulate_file(&{
            let mut built = SimulateRequest::default();
            file.clone_into(&mut built.file);
            built.compat_tlc = compat_tlc;
            built.seed = Some(7);
            built.traces = id + 1;
            built.depth = Some(20);
            built.keep_traces = Some(0);
            built
        })?
        .to_json();
        assert_eq!(alone["traces"][0]["document"], deciding["document"]);
    }
    Ok(())
}

/// Print output is released in trace order, so the output and where an
/// output bound stops are the same at every width.
#[test]
fn output_is_released_in_trace_order() -> Result {
    let (_scratch, file) = written(
        "Printed",
        "---- MODULE Printed ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\n\
         Next == \\/ x' = x + 1 /\\ PrintT(<<\"up\", x>>)\n        \\/ x' = 0 /\\ PrintT(<<\"reset\", x>>)\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    let report = same_at_every_width(&{
        let mut built = SimulateRequest::default();
        built.file = file;
        built.seed = Some(3);
        built.traces = 200;
        built.depth = Some(6);
        built
    })?;
    assert_eq!(report["result"], "sampled", "{report}");
    assert_eq!(
        report["runtime_output"].as_array().map(Vec::len),
        Some(200 * 6)
    );
    Ok(())
}

/// The kept sample is the lowest ids; `--keep-traces 0` keeps only the
/// deciding trace, which is still validated.
#[test]
fn retention_keeps_the_lowest_ids_and_always_validates_the_violation() -> Result {
    let (_scratch, file) = fixture("Walk", "SPECIFICATION Spec\nINVARIANT NotFourteen\n")?;
    let mut request = SimulateRequest::default();
    request.file = file;
    request.seed = Some(7);
    request.traces = 100_000;
    request.depth = Some(20);
    request.keep_traces = Some(0);
    let report = same_at_every_width(&request)?;
    let traces = report["traces"].as_array().ok_or("traces")?;
    assert_eq!(traces.len(), 1);
    assert_eq!(report["violation_validated"], true);
    assert_eq!(report["observed_violation"]["trace"], 0);
    assert_eq!(report["options"]["keep_traces"], 0);
    let sampled = simulate_file(&{
        let mut built = request.clone();
        built.traces = 30;
        built.keep_traces = Some(KEEP_ALL_TRACES);
        built
    })?
    .to_json();
    assert_eq!(sampled["options"]["keep_traces"], "all");
    let ids: Vec<u64> = sampled["traces"]
        .as_array()
        .ok_or("traces")?
        .iter()
        .filter_map(|trace| trace["id"].as_u64())
        .collect();
    assert_eq!(ids, (0..30).collect::<Vec<_>>());
    Ok(())
}

/// G1 at a reduced scale: a million successors per state run without a
/// resource limit, with successor choice in constant memory.
#[test]
fn large_successor_sets_are_sampled_without_a_resource_limit() -> Result {
    let (_scratch, file) = fixture("BigNext", "INIT Init\nNEXT Next\nINVARIANT Small\n")?;
    let report = simulate_file(&{
        let mut built = SimulateRequest::default();
        built.file = file;
        built.seed = Some(1);
        built.traces = 1;
        built.depth = Some(2);
        built
    })?
    .to_json();
    assert_eq!(report["result"], "sampled", "{report}");
    // Init is enumerated once, then every successor of both steps.
    assert_eq!(report["statistics"]["generated"], 3_000_000);
    assert_eq!(report["traces"][0]["validation"]["result"], "valid");
    Ok(())
}

/// G2 with `--constructive-init`: no Init is enumerated; each trace
/// constructs its initial state.
#[test]
fn constructive_init_never_enumerates_init() -> Result {
    let (_scratch, file) = fixture("BigNext", "INIT Init\nNEXT Next\nINVARIANT Small\n")?;
    let report = same_at_every_width(&{
        let mut built = SimulateRequest::default();
        built.file = file;
        built.seed = Some(1);
        built.traces = 5;
        built.depth = Some(20);
        built.generation = true;
        built.constructive_init = true;
        built
    })?;
    assert_eq!(report["result"], "sampled", "{report}");
    assert_eq!(report["options"]["initial_states"], "constructed");
    assert_eq!(report["statistics"]["generated"], 5 * 21);
    assert_eq!(report["statistics"]["initial_candidates"], 5);
    for trace in report["traces"].as_array().ok_or("traces")? {
        assert_eq!(trace["validation"]["result"], "valid");
    }
    Ok(())
}

/// G3 to G6: with sampling off (TLC's profile, or `--sample-unbounded off`),
/// a set dive cannot enumerate is TLC's GENERAL 1000 error, also under the
/// constructive generator. Sampling, on by default, is tested in
/// `simulate_unbounded.rs`.
#[test]
fn unenumerable_sets_are_tlc_general_errors() -> Result {
    for module in ["InfInit", "InfNext"] {
        let (_scratch, file) = fixture(module, "INIT Init\nNEXT Next\nINVARIANT Small\n")?;
        for generation in [false, true] {
            for compat_tlc in [false, true] {
                let report = simulate_file(&{
                    let mut built = SimulateRequest::default();
                    file.clone_into(&mut built.file);
                    built.compat_tlc = compat_tlc;
                    built.seed = Some(1);
                    built.traces = 5;
                    built.depth = Some(20);
                    built.generation = generation;
                    built.constructive_init = generation;
                    built.sample_unbounded =
                        (!compat_tlc).then_some(dive_api::SampleUnbounded::Off);
                    built
                })?;
                let json = report.to_json();
                let case = format!("{module} generate={generation} tlc={compat_tlc}: {json}");
                assert_eq!(json["result"], "evaluation-error", "{case}");
                assert_eq!(json["tlc_code"], 1000, "{case}");
                assert_eq!(json["options"]["sample_unbounded"], "off", "{case}");
                assert_eq!(report.exit_code_for(ExitConvention::Tlc), 255, "{case}");
            }
        }
    }
    Ok(())
}

/// A trace far deeper than the default retention bound once allowed.
#[test]
fn deep_traces_run_in_trace_memory() -> Result {
    let (_scratch, file) = written(
        "Deep",
        "---- MODULE Deep ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    let report = simulate_file(&{
        let mut built = SimulateRequest::default();
        built.file = file;
        built.seed = Some(1);
        built.traces = 2;
        built.depth = Some(200_000);
        built.keep_traces = Some(0);
        built
    })?
    .to_json();
    assert_eq!(report["result"], "sampled", "{report}");
    assert_eq!(report["statistics"]["generated"], 1 + 2 * 200_000);
    assert_eq!(report["traces"].as_array().map(Vec::len), Some(0));
    Ok(())
}

/// The byte bound drops samples from the highest id down, keeps the
/// deciding trace and counts what it dropped; `json_len` is exact.
#[test]
fn the_report_byte_bound_drops_samples_first() -> Result {
    let (_scratch, file) = fixture("Walk", "SPECIFICATION Spec\nINVARIANT NotFourteen\n")?;
    let mut request = SimulateRequest::default();
    request.file = file;
    request.seed = Some(7);
    request.traces = 100_000;
    request.depth = Some(20);
    let full = simulate_file(&request)?;
    let full_len = full.json_len(None);
    let mut text = Vec::new();
    full.write_json(&mut text, None)?;
    assert_eq!(text.len(), full_len);
    let bound = full_len - 2_000;
    let fitted = simulate_file(&{
        let mut built = request.clone();
        built.max_report_bytes = Some(bound);
        built
    })?;
    for convention in [
        None,
        Some(ExitConvention::Tlc),
        Some(ExitConvention::Native),
    ] {
        assert!(fitted.json_len(convention) <= bound);
    }
    let json = fitted.to_json();
    let dropped = json["statistics"]["traces_dropped"]
        .as_u64()
        .ok_or("dropped")?;
    assert!(dropped > 0);
    let traces = json["traces"].as_array().ok_or("traces")?;
    assert_eq!(traces.len() as u64 + dropped, 11);
    assert_eq!(json["observed_violation"]["trace"], traces.len() - 1);
    assert_eq!(
        traces.last(),
        full.to_json()["traces"].as_array().and_then(|t| t.last())
    );
    // A bound too small for the deciding trace leaves its summary.
    let tiny = simulate_file(&{
        let mut built = request;
        built.max_report_bytes = Some(1);
        built
    })?
    .to_json();
    let summary = &tiny["traces"][0]["summary"];
    assert_eq!(summary["reason"], "report byte bound");
    assert_eq!(tiny["traces"][0]["validation"]["result"], "valid");
    assert_eq!(tiny["violation_validated"], true);
    assert!(
        summary["document_sha256"]
            .as_str()
            .is_some_and(|h| h.len() == 64)
    );
    Ok(())
}

/// `--traces unlimited` runs until a violation or its time limit.
#[test]
fn unlimited_traces_need_a_time_limit_and_stop_on_it() -> Result {
    let (_scratch, file) = written(
        "Loop",
        "---- MODULE Loop ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = 1 - x\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    let mut request = SimulateRequest::default();
    request.file = file;
    request.seed = Some(1);
    request.traces = dive_api::UNLIMITED_TRACES;
    request.depth = Some(10);
    assert!(simulate_file(&request).is_err());
    let report = simulate_file(&{
        let mut built = request;
        built.time_limit = BudgetSetting::Explicit(Duration::from_millis(300));
        built.workers = Some(2);
        built
    })?
    .to_json();
    assert_eq!(report["result"], "stopped", "{report}");
    assert!(report["options"]["traces"].is_null());
    assert!(report["statistics"]["traces_started"].as_u64() > Some(0));
    Ok(())
}
