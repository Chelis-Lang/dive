//! Inspect source-produced partial temporal diagnostics without claiming a model proof.
use std::{path::Path, sync::Arc};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = Path::new(
        "conformance/probes/tlc-partial-scc/models/PartialGrowthSafe/PartialGrowthSafe.tla",
    );
    let analysis = dive_sema::analyze_file(file, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse("SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("invalid configuration")?;
    let options = dive_check::CheckOptions {
        workers: 4,
        ..dive_check::CheckOptions::tlc(0, 0)
    };
    let model = dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )?;
    let result = dive_liveness::phase_execution::execute_partial(
        &model,
        &options,
        dive_liveness::phase_graph::Limits::default(),
    )?;
    assert!(!result.model_proof());
    assert_eq!(result.execution().capture.workers.used, 4);
    assert!(
        result
            .checks()
            .iter()
            .all(|c| c.selection().candidate().is_none() && !c.model_proof())
    );
    assert!(result.inspections() <= result.graph_inspections());
    println!(
        "{} diagnostic checks; no model proof",
        result.checks().len()
    );
    Ok(())
}
