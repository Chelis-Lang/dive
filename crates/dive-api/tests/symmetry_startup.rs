//! Symmetry startup error classification through the public checker API.
#![allow(clippy::unwrap_used, reason = "test assertions and fixture setup")]

use std::path::Path;

use dive_api::CheckRequest;

#[test]
fn symmetry_errors_precede_assumptions_and_preserve_general_exit_class() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-model/tests/fixtures/symmetry-group/specs");
    let scratch = std::env::temp_dir().join(format!(
        "dive-symmetry-startup-api-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    for name in ["SyDivZero", "SyMixedElements"] {
        let file = scratch.join(format!("{name}.tla"));
        let config = file.with_extension("cfg");
        let source =
            std::fs::read_to_string(fixtures.join(name).join(format!("{name}.tla"))).unwrap();
        std::fs::copy(fixtures.join(name).join(format!("{name}.cfg")), &config).unwrap();
        for assume_false in [false, true] {
            std::fs::write(
                &file,
                if assume_false {
                    source.replace("====", "ASSUME FALSE\n====")
                } else {
                    source.clone()
                },
            )
            .unwrap();
            for compat_tlc in [false, true] {
                let report = dive_api::check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        compat_tlc,
                        seed: Some(0),
                        fp: Some(0),
                        workers: Some(1),
                        ..CheckRequest::default()
                    },
                    dive_api::Engine::Reference,
                )
                .unwrap();
                assert_eq!(report.verdict(), "model-error", "{name}");
                assert_eq!(report.exit_code(), if compat_tlc { 255 } else { 1 });
                let doc = report.to_json();
                assert_eq!(doc["tlc"]["code"], 1000);
                assert_eq!(doc["tlc"]["name"], "GENERAL");
                assert_eq!(doc["tlc"]["reported"], 1000);
                assert_eq!(doc["tlc"]["exit_status"], 255);
                assert_eq!(doc["tlc"]["verified"], true);
                assert!(doc["statistics"].is_null());
                assert!(doc["counterexample"].is_null());
                assert!(doc["collision"].is_null());
                assert_eq!(doc["output"], serde_json::json!([]));
                assert!(
                    doc["message"]
                        .as_str()
                        .unwrap()
                        .contains("TLC threw an unexpected exception")
                );
                if name == "SyDivZero" {
                    assert!(
                        doc["message"]
                            .as_str()
                            .unwrap()
                            .contains("tlc2.tool.EvalException")
                    );
                    assert!(doc["message"].as_str().unwrap().contains("STARTMSG 2179:0"));
                }
            }
        }
    }
    std::fs::remove_dir_all(&scratch).unwrap();
}
