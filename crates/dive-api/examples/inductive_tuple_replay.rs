//! Independently replay a proved tuple history and reject an altered successor.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/inductive-tuple-replay/specs/TupleHistory.tla");
    let report = check_file(&CheckRequest {
        file: file.clone(),
        compat_tlc: true,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    })?
    .to_json();
    let actual = TraceDoc::from_json(&report)?;
    let model = SpecModel::load(
        &file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: true,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )?;
    if model.validate(&actual).validation != Validation::Valid {
        return Err("actual tuple-history word failed independent replay".into());
    }
    let mut altered = report;
    altered["counterexample"]["trace"]["states"][1]["values"]["x"] = 99.into();
    let altered = TraceDoc::from_json(&altered)?;
    if !matches!(model.validate(&altered).validation, Validation::Invalid(_)) {
        return Err("altered successor was not rejected".into());
    }
    println!(
        "tlc: proved tuple history Valid; altered Next Invalid; general namespace history unproved"
    );
    Ok(())
}
