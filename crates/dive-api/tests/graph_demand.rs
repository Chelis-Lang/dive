//! Graph conversion and normalization follow actual pinned override demand.
#![allow(
    clippy::expect_used,
    reason = "pinned corpus and source-bound test controls"
)]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{
    CheckReport, CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine,
};
use serde_json::{Value, json};

const MIXED_COMPAT: &str = "a set of values of different shapes under --compat=tlc, where TLC's checked comparison may fail or answer by its asymmetric rules (C1–C5; the exact port is not built)";

const CLASSIFIED: &[&str] = &[
    "SelfDemand",
    "EmptyDemand",
    "AbsentDemand",
    "UndirectedSelf",
    "SkippedMalformed",
    "SelfViolation",
    "EmptyViolation",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let file = root()
        .join("conformance/corpus/graph-demand/specifications")
        .join(name)
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        library_paths: vec![root().join("crates/dive-eval/tests/fixtures/community-modules")],
        compat_tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(report: &CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict":doc["verdict"], "property":doc["property"],
        "distinct":doc["statistics"]["distinct"], "generated":doc["statistics"]["generated"],
        "depth":doc["statistics"]["depth"], "code":doc["tlc"]["code"],
        "class":doc["tlc"]["name"], "exit":report.exit_code_for(ExitConvention::Tlc)})
}

fn expected(name: &str) -> Value {
    let path = root()
        .join("conformance/golden/tlc-2026.10.06.014338/graph-demand/specifications")
        .join(name)
        .join(format!("{name}.cfg.json"));
    let doc: Value = serde_json::from_slice(&std::fs::read(path).expect("generated golden"))
        .expect("golden JSON");
    let r = &doc["result"];
    json!({"verdict": if r["verdict"] == "safety-violation" { "invariant-violation" } else { "success" },
        "property":r["violated_property"], "distinct":r["counts"]["distinct_states"],
        "generated":r["counts"]["states_generated"], "depth":r["counts"]["depth"],
        "code":r["verdict_code"], "class":r["error_class"], "exit":r["exit_code"]})
}

fn validate_violation(req: &CheckRequest, report: &CheckReport) {
    if report.verdict() != "invariant-violation" {
        return;
    }
    let model = SpecModel::load(
        &req.file,
        &req.file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            library_paths: req.library_paths.clone(),
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("fresh independent source");
    let word = report.to_json()["counterexample"]["trace"].clone();
    let trace = TraceDoc::from_json(&word).expect("native word");
    assert_eq!(model.validate(&trace).validation, Validation::Valid);
    let mut skipped = word;
    let at = usize::from(skipped["states"].as_array().expect("states").len() > 1);
    skipped["states"][at]["values"]["x"] = json!(7);
    let skipped = TraceDoc::from_json(&skipped).expect("mutated word");
    assert!(matches!(
        model.validate(&skipped).validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn state_dependent_early_and_actual_edge_demands_match_pinned_results() {
    for name in CLASSIFIED {
        let pinned = expected(name);
        for compat in [false, true] {
            for workers in [1, 4] {
                let req = request(name, compat, workers);
                let reference =
                    check_file_engine(&req, Engine::Reference).expect("Reference check");
                assert_eq!(observed(&reference), pinned, "{name}/{compat}/{workers}");
                validate_violation(&req, &reference);
                for switches in ["", "-fold,-memo", "-hashcons"] {
                    let core = check_file_core_opts(&req, switches).expect("Core check");
                    assert_eq!(core.to_json()["engine"], "core");
                    if compat
                        && matches!(
                            *name,
                            "SelfDemand"
                                | "EmptyDemand"
                                | "AbsentDemand"
                                | "UndirectedSelf"
                                | "SelfViolation"
                                | "EmptyViolation"
                        )
                    {
                        assert_eq!(core.verdict(), "unsupported");
                        assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
                        assert_eq!(core.to_json()["message"], MIXED_COMPAT);
                        continue;
                    }
                    assert_eq!(
                        observed(&core),
                        pinned,
                        "{name}/{compat}/{workers}/{switches}"
                    );
                    validate_violation(&req, &core);
                }
            }
        }
    }
}

#[test]
fn demanded_comparisons_and_argument_diagnostics_retain_errors_or_guarded_refusals() {
    // The primary streams for these configs are context-only and have no
    // model golden. Assert their actual demand/error boundaries without
    // promoting a nested code into a classified oracle verdict.
    for name in [
        "DemandedEdges",
        "DemandedEndpoint",
        "SimpleEmpty",
        "FieldPriority",
        "SelfEdgeType",
    ] {
        assert!(
            !root()
                .join("conformance/golden/tlc-2026.10.06.014338/graph-demand/specifications")
                .join(name)
                .join(format!("{name}.cfg.json"))
                .exists()
        );
        for compat in [false, true] {
            let req = request(name, compat, 1);
            let reference = check_file_engine(&req, Engine::Reference).expect("Reference check");
            let doc = reference.to_json();
            if !compat && matches!(name, "DemandedEdges" | "SimpleEmpty") {
                let expected = if name == "DemandedEdges" {
                    json!({"verdict":"invariant-violation", "property":"Inv", "distinct":1, "generated":1,
                        "depth":1, "code":2107, "class":"TLC_INVARIANT_VIOLATED_INITIAL", "exit":12})
                } else {
                    json!({"verdict":"success", "property":null, "distinct":3, "generated":3,
                        "depth":3, "code":2193, "class":"TLC_SUCCESS", "exit":0})
                };
                assert_eq!(observed(&reference), expected, "{name}");
                validate_violation(&req, &reference);
                let core = check_file_engine(&req, Engine::Core).expect("canonical Core check");
                assert_eq!(observed(&core), expected, "{name}");
                validate_violation(&req, &core);
                continue;
            }
            assert_eq!(doc["verdict"], "evaluation-error", "{name}: {doc}");
            assert_eq!(reference.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(doc["tlc"]["verified"], false);
            let nested = if name == "SelfEdgeType" || (!compat && name == "FieldPriority") {
                2169
            } else {
                2154
            };
            assert!(
                doc["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(&format!("STARTMSG {nested}:"))),
                "{name}: {doc}"
            );
            let core = check_file_engine(&req, Engine::Core).expect("Core check");
            let core_doc = core.to_json();
            if compat && matches!(name, "DemandedEdges" | "SimpleEmpty" | "FieldPriority") {
                assert_eq!(core_doc["verdict"], "unsupported", "{name}: {core_doc}");
                assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
                assert_eq!(core_doc["message"], MIXED_COMPAT);
            } else {
                assert_eq!(core_doc["verdict"], doc["verdict"]);
                assert_eq!(core_doc["tlc"], doc["tlc"]);
                assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
            }
        }
    }
}
