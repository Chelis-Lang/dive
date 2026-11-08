//! `ENABLED` has its own successor search, even inside a step predicate.
//! Primed definitions must neither reuse the enclosing pair's value nor keep
//! one candidate's value for the next candidate (issue #588).
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-m0")
        .join(format!("{name}.tla"))
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({
        "verdict": doc["verdict"],
        "property": doc["property"],
        "distinct": doc["statistics"]["distinct"],
        "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"],
        "error_class": doc["tlc"]["name"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "trace": doc["counterexample"]["trace"],
    })
}

fn check(names: &[&str]) {
    // Pinned TLC tlc-2026.10.06.014338, jar SHA-256
    // 7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d,
    // BFS, one worker, seed 0, fp 0, -lncheck final: Cache and Search
    // succeed with 3 distinct / 4 generated / depth 3; CacheBad violates
    // Prop with 2 / 2 / 2 and exit 13, trace x=0 -> x=1.
    for &name in names {
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                let file = fixture(name);
                let req = CheckRequest {
                    config: Some(file.with_extension("cfg")),
                    file: file.clone(),
                    workers: Some(workers),
                    seed: Some(0),
                    fp: Some(0),
                    compat_tlc,
                    ..CheckRequest::default()
                };
                let reference = check_file_engine(&req, Engine::Core).expect("reference runs");
                let expected = observed(&reference);
                let bad = name == "EnabStepCacheBad";
                assert_eq!(
                    expected["verdict"],
                    if bad {
                        "action-property-violation"
                    } else {
                        "success"
                    },
                    "{name}: {expected:#}"
                );
                assert_eq!(expected["distinct"], if bad { 2 } else { 3 });
                assert_eq!(expected["generated"], if bad { 2 } else { 4 });
                assert_eq!(expected["depth"], if bad { 2 } else { 3 });
                assert_eq!(expected["exit"], if bad { 13 } else { 0 });
                if bad {
                    assert_eq!(expected["property"], "Prop");
                }
                for switches in ["", "-memo", "-step-memo", "-memo,-step-memo"] {
                    let core = check_file_core_opts(&req, switches).expect("core runs");
                    let actual = observed(&core);
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(
                        actual, expected,
                        "{name}, compat={compat_tlc}, workers={workers}, opts={switches}"
                    );
                    if bad {
                        let options = TraceOptions {
                            compat_tlc,
                            run: Some((0, 0)),
                            ..TraceOptions::default()
                        };
                        let model = SpecModel::load(&file, &file.with_extension("cfg"), &options)
                            .expect("independent validator binds");
                        let trace = TraceDoc::from_json(&actual["trace"]).expect("trace decodes");
                        assert!(
                            matches!(model.validate(&trace).validation, Validation::Valid),
                            "{name}: independent validator rejected the counterexample"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn enabled_does_not_reuse_enclosing_step_definitions() {
    check(&["EnabStepCache", "EnabStepCacheBad"]);
}

#[test]
fn enabled_search_does_not_cache_candidates_as_step() {
    check(&["EnabStepSearch"]);
}
