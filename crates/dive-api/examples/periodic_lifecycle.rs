//! Run a producer-owned clock/pause diagnostic; never print a model verdict.
use dive_check::CheckOptions;
use dive_liveness::{
    periodic_lifecycle::{self, Options},
    phase_graph::Limits,
};
use std::sync::Arc;
fn main() -> Result<(), String> {
    let source = "---- MODULE Backend ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    let analysis = dive_sema::analyze_source("Backend.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("configuration")?;
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let model = dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let run = periodic_lifecycle::execute(
        &model,
        &options,
        Limits::default(),
        Options {
            progress_interval: 0,
            force: true,
            ..Options::default()
        },
    );
    let invocation = run
        .invocations()
        .last()
        .ok_or("no actual timer invocation")?;
    if run.backend().stop().is_none() || run.safety_eof().is_some() || run.model_proof() {
        return Err("expected a stopped diagnostic candidate".into());
    }
    println!(
        "actual_pool={} timer_invocations={} eof=false candidate_requires_replay=true model_proof=false",
        invocation.workers(),
        run.invocations().len()
    );
    Ok(())
}
