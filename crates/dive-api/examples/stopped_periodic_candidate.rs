//! Keep an actual source winner separate from the stopped-queue diagnostic.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, capture::WorkerStop, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE StoppedGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=1\nInv == x=0\nLive == <>(x=0)\nHook == FALSE\n====\n";
    let a = dive_sema::analyze_source("StoppedGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).map_err(|e| e.to_string())?);
    let cfg =
        dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\n")
            .into_config()
            .ok_or("configuration")?;
    let r = periodic_candidate::check_optional_stopped(
        ir,
        cfg,
        CheckOptions {
            workers: 4,
            ..CheckOptions::tlc(0, 0)
        },
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        progress::Options {
            interval: Duration::from_secs(3600),
            force: false,
        },
    )
    .map_err(|e| e.to_string())?;
    let run = r.run().lifecycle();
    let stopped = run.stopped_service().ok_or("no actual stopped queue")?;
    if stopped.kind() != WorkerStop::Invariant
        || stopped.code() != 2110
        || stopped.pool().0 != 4
        || stopped.joined_stats().is_none()
        || run.safety_eof().is_some()
        || !run.final_checks().is_empty()
        || !run.service_evaluations().is_empty()
        || r.witness().is_some()
        || r.model_proof()
    {
        return Err("source-stop qualification".into());
    }
    let phase_graph::Error::Capture(original) =
        &run.backend().stop().ok_or("no original winner")?.error
    else {
        return Err("wrong original stop".into());
    };
    println!(
        "original={} code={} source={} workers={} joined={} hook_calls={} model_proof={}",
        original.verdict.kind(),
        stopped.code(),
        stopped.source(),
        stopped.pool().0,
        stopped.joined_stats().is_some(),
        run.service_evaluations().len(),
        r.model_proof()
    );
    Ok(())
}
