//! Runnable pure VIEW random checking and independent concrete replay.

use std::error::Error;
use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};

fn main() -> Result<(), Box<dyn Error>> {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/data/view-random/ViewStream.tla");
    let report = check_file(&CheckRequest {
        file: file.clone(),
        compat_tlc: true,
        seed: Some(7),
        fp: Some(0),
        workers: Some(8),
        ..CheckRequest::default()
    })?;
    let json = report.to_json();
    if report.exit_code() != 12
        || json["statistics"]["distinct"] != 4
        || json["options"]["workers"]["requested"] != 8
        || json["options"]["workers"]["used"] != 1
    {
        return Err(std::io::Error::other("unexpected check outcome or worker disclosure").into());
    }
    let doc = TraceDoc::parse(&serde_json::to_string(&json["counterexample"]["trace"])?)
        .map_err(std::io::Error::other)?;
    let model = SpecModel::load(
        &file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: true,
            run: Some((7, 0)),
            ..TraceOptions::default()
        },
    )?;
    if !matches!(model.validate(&doc).validation, Validation::Valid) {
        return Err(std::io::Error::other("concrete word did not independently replay").into());
    }
    println!("pure VIEW: 4 states, actual inline width 1, independently valid concrete word");
    Ok(())
}
