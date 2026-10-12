//! Source-owned graph finalization and final-before-post diagnostic outcomes.
use dive_api::periodic_candidate::{self, Terminal};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    for (live, post, expected) in [
        ("<>(x=2)", "x=x", Terminal::EofWitness),
        (
            "<>(x=0)",
            "x=x",
            Terminal::PostconditionFailure { code: Some(2405) },
        ),
        (
            "<>(x=0)",
            "TRUE \\/ (x=x)",
            Terminal::FinalSelectionsComplete,
        ),
    ] {
        let source = format!(
            "---- MODULE FinalGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1\nLive == {live}\nPost == {post}\n====\n"
        );
        let a =
            dive_sema::analyze_source("FinalGuide.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&a).map_err(|e| e.to_string())?);
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nPOSTCONDITION Post\n")
            .into_config()
            .ok_or("configuration")?;
        let r = periodic_candidate::check_optional_stopped(
            ir,
            cfg,
            dive_check::CheckOptions {
                workers: 4,
                ..dive_check::CheckOptions::tlc(0, 0)
            },
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            dive_check::progress::Options {
                interval: Duration::from_secs(3600),
                force: false,
            },
        )
        .map_err(|e| e.to_string())?;
        if r.terminal() != &expected || r.model_proof() {
            return Err("terminal qualification".into());
        }
        let lifecycle = r.run().lifecycle();
        let f = lifecycle
            .finalization()
            .ok_or("no actual finalized graph")?;
        if let Some(w) = r.witness()
            && (w.memory_validation().validation != dive_trace::Validation::Valid
                || w.json_validation().validation != dive_trace::Validation::Valid)
        {
            return Err("independent witness replay".into());
        }
        println!(
            "terminal={:?} graph_states={} final_checks={}/{} post_passed={} model_proof={}",
            r.terminal(),
            f.capture().entries().0,
            f.checks().0,
            f.checks().1,
            lifecycle.postconditions_passed(),
            r.model_proof()
        );
    }
    Ok(())
}
