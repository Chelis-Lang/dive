//! Run bounded final liveness checking with a pure native operator argument.
use std::{path::Path, sync::Arc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = &Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/data/native-temporal-operator/NativeHeadFails.tla");
    let analysis = dive_sema::analyze_file(path, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config()
        .ok_or("invalid config")?;
    let options = dive_check::CheckOptions {
        workers: 4,
        ..dive_check::CheckOptions::tlc(0, 0)
    };
    let report = dive_api::tlc_temporal::check_final(
        &ir,
        &cfg,
        &options,
        dive_liveness::phase_graph::Limits::default(),
    )?;
    let dive_api::tlc_temporal::Outcome::Violation(violation) = report.outcome() else {
        return Err("expected liveness violation".into());
    };
    assert_eq!(violation.document().properties, ["Live"]);
    assert_eq!(
        violation.validation().validation,
        dive_trace::Validation::Valid
    );
    println!("{:?}", violation.document().properties);
    Ok(())
}
