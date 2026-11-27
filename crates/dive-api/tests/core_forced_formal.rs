//! D-047: Core expands a forced Boolean formal used as an action in both profiles.

use dive_api::{CheckRequest, ExitConvention, check_file_core_opts};
use std::path::PathBuf;

#[test]
fn forced_and_unforced_formals_generate_three_states_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/forced-boolean-formal/specifications/ForcedFormal");
    for compat_tlc in [false, true] {
        for name in ["ForcedFormal", "UnforcedFormal"] {
            for workers in [1, 4] {
                let req = CheckRequest {
                    file: root.join(format!("{name}.tla")),
                    config: Some(root.join(format!("{name}.cfg"))),
                    workers: Some(workers),
                    compat_tlc,
                    seed: Some(0),
                    fp: Some(0),
                    ..Default::default()
                };
                let core = check_file_core_opts(&req, "")?;
                let doc = core.to_json();
                let at = format!("{name} compat_tlc={compat_tlc} workers={workers}: {doc}");
                assert_eq!(doc["engine"], "core", "{at}");
                assert_eq!(doc["verdict"], "success", "{at}");
                assert_eq!(doc["tlc"]["code"], 2193, "{at}");
                assert_eq!(core.exit_code_for(ExitConvention::Tlc), 0, "{at}");
                assert_eq!(doc["statistics"]["distinct"], 1, "{at}");
                assert_eq!(doc["statistics"]["depth"], 1, "{at}");
                if workers == 1 {
                    assert_eq!(doc["statistics"]["generated"], 3, "{at}");
                }
            }
        }
    }
    Ok(())
}
