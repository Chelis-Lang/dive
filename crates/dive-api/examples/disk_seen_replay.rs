//! Actual fallible invariant-error continuation, separate from a safety certificate.
use dive_api::{CheckRequest, SeenOptions, Store, check_file};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/seen-replay/ReplayNew.tla");
    for compat_tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let json = check_file(&CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc,
                store,
                seen_disk: Some(SeenOptions::default()),
                workers: Some(8),
                seed: Some(7),
                fp: Some(0),
                ..CheckRequest::default()
            })?
            .to_json();
            if json["statistics"]["generated"] != 2
                || json["statistics"]["distinct"] != 4
                || json["statistics"]["left_on_queue"] != 2
                || json["tlc"]["code"] != 2111
                || json["tlc"]["verified"] != true
                || json["options"]["workers"]["used"] != 1
                || json["seen_storage"]["replay"]["admitted_keys"] != 2
                || json["seen_storage"]["replay"]["queued"] != 2
                || json["seen_storage"]["replay"]["halted_by"] != serde_json::Value::Null
                || json["seen_storage"]["first_failure"] != serde_json::Value::Null
                || json["seen_storage"]["after_cleanup"]["cleanup_completed"] != true
            {
                return Err(format!("no completed known invariant-error replay: {json}").into());
            }
            println!(
                "profile={},store={},generated=2,distinct=4,queued=2,replay_admitted=2,actual_workers=1,cleanup_completed=true",
                if compat_tlc { "tlc" } else { "dive" },
                if store == Store::Exact {
                    "exact"
                } else {
                    "fingerprint"
                }
            );
        }
    }
    Ok(())
}
