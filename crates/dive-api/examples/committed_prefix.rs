//! Actual ordered source publication at producer-owned request pauses.
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::sync::Arc;
use std::time::Duration;
fn main() -> Result<(), String> {
    let source = "---- MODULE PrefixGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..8\nNext == x' \\in 0..64\nInv == x>=0\nLive == []<>(x>=0)\nHook == TRUE\n====\n";
    let analysis =
        dive_sema::analyze_source("PrefixGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n")
        .into_config().ok_or("configuration")?;
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let run = snapshot
        .execute_requests(
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
                interval: Duration::from_millis(1),
                force: true,
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let complete = lifecycle
        .backend()
        .complete()
        .ok_or("incomplete safety graph")?;
    let stats = complete.capture.stats;
    if stats.generated != 4234
        || stats.distinct != 65
        || stats.left_on_queue != 0
        || lifecycle.model_proof()
    {
        return Err("actual safety counters/nonproof contract".into());
    }
    for pause in lifecycle.expansion_pauses() {
        let receipt = pause.receipt();
        if receipt.frontier_appended() != receipt.committed()
            || receipt
                .committed()
                .any(|id| !receipt.completed().contains(&id))
            || receipt.predicate_workers() > 1
            || receipt.model_proof()
        {
            return Err("actual returned/committed/append contract".into());
        }
    }
    println!(
        "generated={} distinct={} safety_eof=true model_proof=false",
        stats.generated, stats.distinct
    );
    Ok(())
}
