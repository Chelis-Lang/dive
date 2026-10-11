//! Real ordinary safety through the fallible disk store and explicit cleanup.
use dive_api::{CheckRequest, SeenOptions, Store, check_file};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/seen-safety/Counter.tla");
    for compat_tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            let report = check_file(&CheckRequest {
                file: file.clone(),
                compat_tlc,
                store,
                seen_disk: Some(SeenOptions::default()),
                workers: Some(8),
                seed: Some(7),
                fp: Some(0),
                ..CheckRequest::default()
            })?;
            let json = report.to_json();
            if json["verdict"] != "success"
                || json["options"]["workers"]["used"] != 1
                || json["seen_storage"]["after_cleanup"]["cleanup_completed"] != true
            {
                return Err(format!("no completed safety/cleanup result: {json}").into());
            }
            println!(
                "profile={},store={},actual_workers=1,cleanup_completed=true",
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
