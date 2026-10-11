fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(
        "conformance/probes/material-capture/models/MatNestedFails/MatNestedFails.tla",
    );
    let analysis = dive_sema::analyze_file(path, &Default::default())?;
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).map_err(|e| format!("{e:?}"))?);
    let cfg = dive_config::parse(&std::fs::read_to_string(path.with_extension("cfg"))?)
        .into_config().ok_or("invalid configuration")?;
    let options = dive_check::CheckOptions {
        workers: 4, ..dive_check::CheckOptions::tlc(0, 0)
    };
    let report = dive_api::tlc_temporal::check_final(
        &ir, &cfg, &options, dive_liveness::phase_graph::Limits::default(),
    )?;
    if let dive_api::tlc_temporal::Outcome::Violation(v) = report.outcome() {
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
        println!("{:?}", v.document().properties);
    } else { return Err("expected validated violation".into()); }
    Ok(())
}
