//! Runnable stored transport guide: two profiles and independent concrete replay.
use dive_api::trace::{SpecModel, TraceDoc, TraceEncoding, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/data/trace-representation/Transport.tla");
    for compat_tlc in [true, false] {
        let request = CheckRequest {
            file: file.clone(),
            compat_tlc,
            seed: Some(7),
            fp: Some(0),
            workers: Some(8),
            trace_encoding: TraceEncoding::RepresentationV2,
            ..CheckRequest::default()
        };
        let checked = check_file(&request)?.to_json();
        let doc = TraceDoc::from_json(&checked)?;
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((7, 0)),
                ..TraceOptions::default()
            },
        )?;
        if model.validate(&doc).validation != Validation::Valid {
            return Err("independent replay failed".into());
        }
        println!(
            "{}: version2 independently valid; transport alone was no certificate",
            if compat_tlc { "tlc" } else { "dive" }
        );
    }
    Ok(())
}
