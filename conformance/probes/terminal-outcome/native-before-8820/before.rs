//! Keep an actual source winner separate from the stopped-queue diagnostic.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, capture::WorkerStop, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE StoppedGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=1\nInv == x=0\nLive == <>(x=2)\nPost == x=x\nHook == FALSE\n====\n";
    let a = dive_sema::analyze_source("StoppedGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).map_err(|e| e.to_string())?);
    let cfg =
        dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nPOSTCONDITION Post\n")
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
    println!("witness={} eof={:?} checks={} failure={:?} model_proof={}",r.witness().is_some(),run.safety_eof(),run.final_checks().len(),run.backend().stop().map(|f| &f.error), r.model_proof());
    Ok(())
}
