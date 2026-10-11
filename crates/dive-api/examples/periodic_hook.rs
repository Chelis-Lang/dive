//! Compose a supplied pure expression, without configured-hook or model proof.
use dive_check::CheckOptions;
use dive_liveness::{periodic_hook, periodic_lifecycle::Options, phase_graph::Limits};
use std::sync::Arc;
fn main() -> Result<(), String> {
    let source = "---- MODULE HookGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("HookGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let def = dive_ir::DefId(
        u32::try_from(
            ir.defs()
                .iter()
                .position(|d| d.name == "Hook")
                .ok_or("Hook definition")?,
        )
        .map_err(|e| e.to_string())?,
    );
    // This config deliberately contains no _PERIODIC: configured hook binding
    // remains refused. The expression is supplied separately, with its scope.
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("config")?;
    let model =
        dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).map_err(|e| e.to_string())?;
    let pred = dive_model::Pred::new(
        model.ir.def(def).body,
        model.evaluator.scope(def).map_err(|e| e.to_string())?,
    );
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let run = periodic_hook::execute(
        &model,
        &pred,
        &options,
        Limits::default(),
        Options {
            progress_interval: 0,
            ratio_limit: 0.0,
            ..Options::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let call = run.invocations().first().ok_or("actual invocation")?;
    let hook = run
        .hook_evaluations()
        .first()
        .ok_or("actual expression call")?;
    if !*hook.result().as_ref().map_err(ToString::to_string)?
        || call.resume_requested()
        || run.safety_eof().is_some()
        || run.model_proof()
    {
        return Err("expected false hook diagnostic stop".into());
    }
    println!(
        "actual_pool={} false_equality=true resume=false eof=false configured_hook=false model_proof=false",
        call.workers()
    );
    Ok(())
}
