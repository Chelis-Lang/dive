fn main() -> Result<(), Box<dyn std::error::Error>> {
let path = std::path::Path::new(
    "conformance/probes/tlc-final-scopes/models/TfClaimPairFails/TfClaimPairFails.tla",
);
let analysis = dive_sema::analyze_file(path, &dive_sema::Options::default())?;
let ir = std::sync::Arc::new(dive_ir::lower(&analysis).map_err(|error| format!("{error:?}"))?);
let config_text = std::fs::read_to_string(path.with_extension("cfg"))?;
let cfg = dive_config::parse(&config_text).into_config().ok_or("configuration parsing failed")?;
let options = dive_check::CheckOptions {
    workers: 4,
    ..dive_check::CheckOptions::tlc(0, 0)
};
let report = dive_api::tlc_temporal::check_final(
    &ir, &cfg, &options, dive_liveness::phase_graph::Limits::default(),
)?;
match report.outcome() {
    dive_api::tlc_temporal::Outcome::Violation(witness) => {
        assert_eq!(witness.validation().validation, dive_trace::Validation::Valid);
        println!("{:?}", witness.document().properties);
    }
    dive_api::tlc_temporal::Outcome::Holds(_) => {}
    dive_api::tlc_temporal::Outcome::Safety(_) => {}
}
Ok(())
}
