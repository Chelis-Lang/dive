//! Configuration-owned diagnostic requests; no ordinary temporal proof.
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::sync::Arc;
use std::time::Duration;
fn main() -> Result<(), String> {
    let source = "---- MODULE ProgressGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("ProgressGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let run = snapshot
        .execute_requests(
            &CheckOptions {
                workers: 4,
                ..CheckOptions::tlc(0, 0)
            },
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                progress_interval: 0,
                ratio_limit: 0.0,
                ..periodic_lifecycle::Options::default()
            },
            progress::Options {
                interval: Duration::from_secs(1),
                force: true,
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let pause = lifecycle
        .expansion_pauses()
        .first()
        .ok_or("actual request pause")?;
    let receipt = pause.receipt();
    if !receipt.request().forced()
        || !receipt.claimed().is_empty()
        || !receipt.completed().is_empty()
        || lifecycle.model_proof()
        || lifecycle.hook_evaluations().is_empty()
        || lifecycle.safety_eof().is_some()
    {
        return Err("request/commit distinction".into());
    }
    println!(
        "configured={} request={} actual_pool={} uncommitted={} model_proof=false",
        run.configured_name(),
        receipt.request().sequence(),
        pause.pool().0,
        receipt.completed().len()
    );
    Ok(())
}
