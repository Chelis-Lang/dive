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

#[test]
fn temporal_properties_keep_the_cores_startup_constants_they_do_not_reach()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        // `Live` reads no start-up definition; `Never` reads `Limit`, which
        // the binder evaluates for the liveness checker.
        for (config, verdict, core_values) in
            [("LiveHolding", "success", 3), ("LiveFailing", "error", 2)]
        {
            let req = request(config, compat_tlc);
            let expected = observed(&check_file_engine(&req, Engine::Reference)?);
            if verdict == "success" {
                assert_eq!(expected["verdict"], "success", "{expected:#}");
            } else {
                assert_eq!(expected["property"], "Never", "{expected:#}");
            }
            let core = check_file_engine(&req, Engine::Core)?;
            let doc = core.to_json();
            assert_eq!(doc["engine"], "core", "{doc:#}");
            assert_eq!(observed(&core), expected, "{config}/{compat_tlc}");
            let plan = &doc["core"]["metrics"]["plan"];
            let want = if compat_tlc { 0 } else { core_values };
            assert_eq!(
                plan["startup_values"], want,
                "{config}/{compat_tlc}: {plan:#}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_startup_constant_the_core_cannot_evaluate_takes_the_binders_value() {
    // The reference evaluator recurses on the native stack, as the
    // engines' own threads allow.
    let joined = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| fallback().map_err(|e| e.to_string()))
        .map(std::thread::JoinHandle::join);
    assert!(matches!(joined, Ok(Ok(Ok(())))), "{joined:?}");
}

fn fallback() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-startup-constants");
    let req = CheckRequest {
        file: root.join("StartupFallback.tla"),
        config: Some(root.join("StartupFallback.cfg")),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    };
    let expected = observed(&check_file_engine(&req, Engine::Reference)?);
    assert_eq!(expected["verdict"], "success", "{expected:#}");
    assert_eq!(expected["statistics"]["distinct"], 4, "{expected:#}");
    let core = check_file_engine(&req, Engine::Core)?;
    let doc = core.to_json();
    assert_eq!(doc["engine"], "core", "{doc:#}");
    assert_eq!(observed(&core), expected);
    // `Total` exceeds the core's recursive-call depth limit at start-up;
    // the binder's evaluation gives its value.
    let plan = &doc["core"]["metrics"]["plan"];
    assert_eq!(plan["startup_values"], 0, "{plan:#}");
    assert_eq!(plan["startup_fallbacks"], 1, "{plan:#}");
    Ok(())
}

#[test]
fn a_pure_assume_evaluated_by_the_core_matches_the_reference_engine()
-> Result<(), Box<dyn std::error::Error>> {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-startup-constants");
    for compat_tlc in [false, true] {
        for (config, verdict) in [
            ("AssumeHolding", "success"),
            ("AssumeFalse", "assumption-false"),
            ("AssumeError", "error"),
        ] {
            let req = CheckRequest {
                file: root.join("StartupAssume.tla"),
                config: Some(root.join(format!("{config}.cfg"))),
                workers: Some(1),
                compat_tlc,
                seed: Some(0),
                fp: Some(0),
                ..Default::default()
            };
            let expected = observed(&check_file_engine(&req, Engine::Reference)?);
            if verdict == "success" {
                assert_eq!(expected["verdict"], "success", "{expected:#}");
            } else {
                assert_ne!(expected["verdict"], "success", "{expected:#}");
            }
            let core = check_file_engine(&req, Engine::Core)?;
            let doc = core.to_json();
            assert_eq!(doc["engine"], "core", "{doc:#}");
            assert_eq!(observed(&core), expected, "{config}/{compat_tlc}");
            // In the default profile `Test`, which only the ASSUME reads, is
            // the core's start-up value. A false or failing ASSUME is then
            // checked again by the reference evaluator, which reports it.
            if verdict == "success" {
                let plan = &doc["core"]["metrics"]["plan"];
                // `Test` and `Graphs`, which only `Test` reads.
                let expected_values = if compat_tlc { 0 } else { 2 };
                assert_eq!(plan["startup_values"], expected_values, "{plan:#}");
            }
        }
    }
    Ok(())
}

/// The verdict of checking `module` on `engine`, or `None` when it takes
/// longer than `limit`.
fn bounded(
    module: &str,
    engine: Engine,
    compat_tlc: bool,
    limit: std::time::Duration,
) -> Option<Value> {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-startup-constants");
    let req = CheckRequest {
        file: root.join(format!("{module}.tla")),
        config: Some(root.join(format!("{module}.cfg"))),
        workers: Some(1),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(check_file_engine(&req, engine).map(|r| observed(&r)).ok());
    });
    rx.recv_timeout(limit).ok().flatten()
}

#[test]
fn a_false_assume_guards_the_pure_assumes_after_it() {
    // TLC stops at the first ASSUME that is not TRUE. The core, which
    // evaluates the pure ASSUMEs when it compiles its plan, must not
    // evaluate the expensive one a false guard precedes (PR #877's review).
    let limit = std::time::Duration::from_secs(30);
    for module in ["GuardedAssume", "GuardedAssumeImpure"] {
        for compat_tlc in [false, true] {
            let expected = bounded(module, Engine::Reference, compat_tlc, limit);
            let expected = expected.unwrap_or_else(|| json!("reference timed out"));
            assert_eq!(expected["class"], "TLC_ASSUMPTION_FALSE", "{expected:#}");
            assert_eq!(expected["exit"], 10, "{expected:#}");
            // Core is the default engine; Auto was removed with its fallback.
            let got = bounded(module, Engine::Core, compat_tlc, limit);
            assert_eq!(got.as_ref(), Some(&expected), "{module}/core/{compat_tlc}");
        }
    }
}
