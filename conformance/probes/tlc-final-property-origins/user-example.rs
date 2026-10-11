fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(
        "conformance/probes/tlc-final-property-origins/models/TfBooleanFails/TfBooleanFails.tla",
    );
    let analysis = dive_sema::analyze_file(path, &dive_sema::Options::default())?;
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).map_err(|e| format!("{e:?}"))?);
    let cfg = dive_config::parse(&std::fs::read_to_string(path.with_extension("cfg"))?)
        .into_config().ok_or("invalid configuration")?;
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())?;
    assert_eq!(model.implied_temporal_action_names(), ["P", "P"]);
    let options = dive_check::CheckOptions { workers: 4, ..dive_check::CheckOptions::tlc(0, 0) };
    let report = dive_api::tlc_temporal::check_final(
        &ir, &cfg, &options, dive_liveness::phase_graph::Limits::default(),
    )?;
    if let dive_api::tlc_temporal::Outcome::Violation(v) = report.outcome() {
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
        assert_eq!(v.candidate().evidence.execution.expanded_property_names, ["P", "P"]);
        println!("configured={:?}; expanded={:?}", v.document().properties,
            v.candidate().evidence.execution.expanded_property_names);
    } else { return Err("expected validated violation".into()); }
    Ok(())
}
