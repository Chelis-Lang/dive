//! Inspect a configuration-owned hook under bounded native diagnostic pauses.
use dive_check::CheckOptions;
use dive_liveness::{periodic_lifecycle::Options, periodic_snapshot, phase_graph::Limits};
use std::sync::Arc;
fn main() -> Result<(), String> {
    let source = "---- MODULE SnapshotGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("SnapshotGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    // Admission occurs before constant startup evaluation. The selection comes
    // from the configuration's pre-constant definition snapshot, not a Pred.
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let run = snapshot
        .execute(
            &options,
            Limits::default(),
            Options {
                progress_interval: 0,
                ratio_limit: 0.0,
                ..Options::default()
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let call = lifecycle.invocations().first().ok_or("actual invocation")?;
    let hook = lifecycle.hook_evaluations().first().ok_or("actual hook")?;
    if !*hook.result().as_ref().map_err(ToString::to_string)?
        || call.resume_requested()
        || lifecycle.safety_eof().is_some()
        || lifecycle.model_proof()
    {
        return Err("expected diagnostic false hook stop".into());
    }
    println!(
        "configured={} selected={} actual_pool={} false_equality=true model_proof=false",
        run.configured_name(),
        snapshot.model().ir.def(run.definition()).name,
        call.workers()
    );
    Ok(())
}
