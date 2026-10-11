//! One genuine empty request with a configuration-owned hook, never a model proof.
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE ServiceGuide ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("ServiceGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let run = snapshot
        .execute_service(
            &CheckOptions {
                workers: 4,
                ..CheckOptions::tlc(0, 0)
            },
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                progress_interval: i32::MAX,
                ..periodic_lifecycle::Options::default()
            },
            progress::Options {
                interval: Duration::from_secs(3600),
                force: true,
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let [service] = lifecycle.request_services() else {
        return Err("one genuine initial request required".into());
    };
    if service.graph_source().is_some()
        || !service.committed().is_empty()
        || service.stats().distinct != 1
        || service.initial_stats().distinct != 1
        || service.resume_requested()
        || lifecycle.safety_eof().is_some()
        || lifecycle.model_proof()
        || !matches!(
            service.outcome(),
            Ok(dive_liveness::periodic::Outcome::PeriodicAssumptionFalse)
        )
    {
        return Err("actual empty-source stop contract".into());
    }
    println!("distinct=1 source=none resumed=false safety_eof=false model_proof=false");
    Ok(())
}
