//! Trace replay compares compound values, even without hash-consing (#603).
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({
        "verdict": doc["verdict"], "property": doc["property"],
        "statistics": {"distinct": doc["statistics"]["distinct"],
            "generated": doc["statistics"]["generated"], "depth": doc["statistics"]["depth"]},
        "tlc": {"code": doc["tlc"]["code"], "name": doc["tlc"]["name"]},
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "trace": doc["counterexample"]["trace"],
    })
}

#[test]
fn compound_trace_survives_hashcons_ablation_and_validates() {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-m0/CompoundTrace.tla");
    for compat_tlc in [false, true] {
        let options = TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        };
        let model = SpecModel::load(&file, &file.with_extension("cfg"), &options)
            .expect("independent validator binds");
        for workers in [1, 4, 8] {
            let req = CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc,
                workers: Some(workers),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let reference = check_file_engine(&req, Engine::Reference).expect("reference runs");
            let expected = observed(&reference);
            // Pinned TLC tlc-2026.10.06.014338, BFS/W1/seed0/fp0:
            // Inv/2110/exit12, 3 generated / 3 distinct / depth3.
            assert_eq!(expected["verdict"], "invariant-violation");
            assert_eq!(expected["property"], "Inv");
            assert_eq!(expected["tlc"]["code"], 2110);
            assert_eq!(expected["exit"], 12);
            for key in ["distinct", "generated", "depth"] {
                assert_eq!(expected["statistics"][key], 3);
            }
            let all_off = dive_core::opts::SWITCHES
                .iter()
                .map(|name| format!("-{name}"))
                .collect::<Vec<_>>()
                .join(",");
            for switches in ["", "-hashcons", "-hashcons,-memo,-step-memo", &all_off] {
                let actual = check_file_core_opts(&req, switches).expect("core runs");
                assert_eq!(actual.to_json()["engine"], "core");
                let actual = observed(&actual);
                assert_eq!(actual, expected, "{compat_tlc}/{workers}/{switches}");
                let trace = TraceDoc::from_json(&actual["trace"]).expect("word decodes");
                assert!(matches!(
                    model.validate(&trace).validation,
                    Validation::Valid
                ));
                let mut mutant = actual["trace"].clone();
                mutant["states"][1]["values"]["x"] = json!({"seq": [7]});
                let mutant = TraceDoc::from_json(&mutant).expect("mutant decodes");
                assert!(matches!(
                    model.validate(&mutant).validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
