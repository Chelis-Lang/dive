//! Run from the repository root: native recovery keeps the stored function kind.
use dive_api::trace::{SpecModel, TraceDoc, TraceEncoding, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file_checkpoint, checkpoint};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/storage-tuple-function/specs/Witness/TupleStorage.tla");
    let request = CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc: true,
        seed: Some(7),
        fp: Some(0),
        workers: Some(4),
        frontier_memory_bytes: Some(0),
        predecessor_memory_bytes: Some(0),
        trace_encoding: TraceEncoding::RepresentationV2,
        ..CheckRequest::default()
    };
    let path = std::env::temp_dir().join(format!(
        "dive-tuple-storage-example-{}.cp",
        std::process::id()
    ));
    let checkpoint = checkpoint::Request {
        path: path.clone(),
        action: checkpoint::Action::PauseAfterLevels(0),
        counters: checkpoint::Counters::Retain,
        config_source_sha256: None,
    };
    let paused = check_file_checkpoint(&request, &checkpoint)?.to_json();
    if paused["checkpoint"]["status"] != "paused-incomplete" {
        return Err("expected incomplete pause".into());
    }
    let report = check_file_checkpoint(
        &request,
        &checkpoint::Request {
            action: checkpoint::Action::Recover,
            ..checkpoint
        },
    )?
    .to_json();
    let model = SpecModel::load(
        &request.file,
        request.config.as_ref().ok_or("config")?,
        &TraceOptions {
            compat_tlc: true,
            run: Some((7, 0)),
            ..TraceOptions::default()
        },
    )?;
    let word = TraceDoc::from_json(&report)?;
    if model.validate(&word).validation != Validation::Valid {
        return Err("independent word invalid".into());
    }
    if report["statistics"]["distinct"] != 4
        || report["checkpoint"]["transient_storage_cleanup_completed"] != true
    {
        return Err("unexpected search or cleanup".into());
    }
    std::fs::remove_file(path)?;
    println!("{report}");
    Ok(())
}
