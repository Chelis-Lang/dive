//! Run a source-owned explicit partial checkpoint; never print a model verdict.
use dive_check::CheckOptions;
use dive_liveness::{
    periodic_backend::{self, Decision, Policy},
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
    let run = periodic_backend::execute(
        &model,
        &options,
        Limits::default(),
        Policy {
            force: true,
            ..Policy::default()
        },
    );
    let checkpoint = run.checkpoints().last().ok_or("no source checkpoint")?;
    if checkpoint.decision() != Decision::Candidate || run.complete().is_some() || run.model_proof()
    {
        return Err("expected a stopped diagnostic candidate".into());
    }
    let barrier = checkpoint.barrier().ok_or("no actual joined wave")?;
    println!(
        "sources={} pointer_size={} last_check_size={} actual_pool={} candidate_requires_replay=true model_proof=false",
        checkpoint.sources(),
        checkpoint.after()[0].size(),
        checkpoint.after()[0].last_check_size(),
        barrier.workers()
    );
    Ok(())
}
