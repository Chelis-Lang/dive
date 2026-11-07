//! The core evaluates the pure constant definitions its roles use only as
//! values when it compiles its plan, instead of the binder (issue #846).

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

fn request(config: &str, compat_tlc: bool) -> CheckRequest {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-startup-constants");
    CheckRequest {
        file: root.join("StartupConstants.tla"),
        config: Some(root.join(format!("{config}.cfg"))),
        workers: Some(1),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict": doc["verdict"], "statistics": doc["statistics"],
        "property": doc["property"], "class": doc["tlc"]["name"],
        "code": doc["tlc"]["code"], "message": doc["message"],
        "trace": doc["counterexample"]["trace"],
        "exit": report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn startup_constants_evaluated_by_the_core_match_the_reference_engine()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for (config, verdict) in [("Holding", "success"), ("Failing", "error")] {
            let req = request(config, compat_tlc);
            let reference = check_file_engine(&req, Engine::Reference)?;
            let expected = observed(&reference);
            assert_eq!(expected["statistics"]["distinct"], 11, "{expected:#}");
            if verdict == "success" {
                assert_eq!(expected["verdict"], "success", "{expected:#}");
            } else {
                assert_ne!(expected["verdict"], "success", "{expected:#}");
            }
            let core = check_file_engine(&req, Engine::Core)?;
            let doc = core.to_json();
            assert_eq!(doc["engine"], "core", "{doc:#}");
            assert_eq!(observed(&core), expected, "{config}/{compat_tlc}");
            // In the default profile `Ring`, `Start` and `Limit` are the
            // core's, and
            // `Ring`'s search answers repeated applications of the recursive
            // function from its call results. `Bad` fails, so its body is
            // evaluated where `NextBad` uses it, with TLC's error. Under
            // `--compat=tlc` the binder evaluates them all.
            let plan = &doc["core"]["metrics"]["plan"];
            if compat_tlc {
                assert_eq!(plan["startup_values"], 0, "{plan:#}");
            } else {
                assert_eq!(plan["startup_values"], 3, "{plan:#}");
                assert!(
                    plan["startup_call_hits"].as_u64().is_some_and(|h| h > 0),
                    "{plan:#}"
                );
            }
        }
    }
    Ok(())
}
