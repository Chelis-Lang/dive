//! Admit only an actual producer-selected word; never an ordinary model proof.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE TerminalGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\n====\n";
    let analysis =
        dive_sema::analyze_source("TerminalGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("configuration")?;
    let report = periodic_candidate::check_optional_terminal(
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
    if report.run().configured_name().is_some()
        || report.run().definition().is_some()
        || report.run().expression().is_some()
    {
        return Err("original configuration must remain hook-free".into());
    }
    let terminal = report
        .run()
        .lifecycle()
        .terminal_service()
        .ok_or("no actual terminal service")?;
    if !terminal.joins_confirmed() || terminal.stats().left_on_queue != 0 {
        return Err("missing post-service timer/pool joins".into());
    }
    let witness = report.witness().ok_or("no admitted word")?;
    if !matches!(witness.origin(), periodic_candidate::Origin::Eof(_)) || report.model_proof() {
        return Err("EOF witness qualification".into());
    }
    println!(
        "origin=eof memory={:?} json={:?} model_proof={}",
        witness.memory_validation().validation,
        witness.json_validation().validation,
        report.model_proof()
    );
    Ok(())
}
