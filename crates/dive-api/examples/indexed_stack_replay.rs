//! Independently replay a source-proved finite function of owner stacks.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/data/indexed-stack-replay/IndexedStack.tla");
    for compat_tlc in [true, false] {
        let report = check_file(&CheckRequest {
            file: file.clone(),
            compat_tlc,
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        })?
        .to_json();
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )?;
        let original = TraceDoc::from_json(&report)?;
        if model.validate(&original).validation != Validation::Valid {
            return Err("source-proved indexed stack word failed replay".into());
        }
        let mut altered = report;
        altered["counterexample"]["trace"]["states"][1]["values"]["x"] = 99.into();
        if !matches!(
            model.validate(&TraceDoc::from_json(&altered)?).validation,
            Validation::Invalid(_)
        ) {
            return Err("altered indexed Next accepted".into());
        }
    }
    println!("both profiles: indexed stack Valid; altered Next Invalid");
    Ok(())
}
