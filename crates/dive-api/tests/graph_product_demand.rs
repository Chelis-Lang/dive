//! Cartesian graph conversion preserves pinned emptiness and later demand.
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

const CLASSIFIED: &[&str] = &[
    "LazyEmptyProduct",
    "LazyEmptyProductScalar",
    "LazyReverseEmptyProduct",
    "LazyNestedEmptyProduct",
    "LazyIntervalEmptyProduct",
    "LazyTupleIntervalEmptyProduct",
    "LazyEmptyProductViolation",
    "LazyNonemptyProduct",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let file = root()
        .join("conformance/corpus/graph-empty-product/specifications")
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
        .join("conformance/golden/tlc-2026.10.06.014338/graph-empty-product/specifications")
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
    skipped["states"][1]["values"]["x"] = json!(7);
    let skipped = TraceDoc::from_json(&skipped).expect("mutated word");
    assert!(matches!(
        model.validate(&skipped).validation,
        Validation::Invalid(_)
    ));
}

#[test]
fn empty_product_conversions_and_nonempty_control_match_pinned_results() {
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
fn demanded_comparisons_operand_errors_and_axis_history_remain_demanded() {
    // These actual primary streams have no classified model golden. Preserve
    // their demanded cause and the existing Core refusal boundary independently.
    for name in [
        "LazyDemandedProduct",
        "LazyProductAxisOrder",
        "LazyProductHistory",
        "LazyProductOperandError",
    ] {
        assert!(
            !root()
                .join("conformance/golden/tlc-2026.10.06.014338/graph-empty-product/specifications")
                .join(name)
                .join(format!("{name}.cfg.json"))
                .exists()
        );
        for compat in [false, true] {
            let req = request(name, compat, 1);
            let reference = check_file_engine(&req, Engine::Reference).expect("Reference check");
            let doc = reference.to_json();
            assert_eq!(doc["verdict"], "evaluation-error", "{name}: {doc}");
            assert_eq!(reference.exit_code_for(ExitConvention::Tlc), 75);
            let (nested, demanded) = match name {
                "LazyProductOperandError" => (2179, "second argument"),
                "LazyProductHistory" => (2154, "FiniteSets.Cardinality"),
                "LazyProductAxisOrder" => (2154, "Shouldn't call isEmpty() on value 0"),
                _ => (2154, "Graphs.isStronglyConnected"),
            };
            let message = doc["message"].as_str().expect("demanded error");
            assert!(
                message.contains(&format!("STARTMSG {nested}:")),
                "{name}: {doc}"
            );
            assert!(message.contains(demanded), "{name}: {doc}");
            assert_eq!(doc["tlc"]["verified"], name == "LazyProductOperandError");
            for switches in ["", "-fold,-memo", "-hashcons"] {
                let core = check_file_core_opts(&req, switches).expect("Core check");
                let core_doc = core.to_json();
                if matches!(name, "LazyDemandedProduct" | "LazyProductHistory") {
                    assert_eq!(core_doc["verdict"], "unsupported", "{name}: {core_doc}");
                    assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
                    assert!(core_doc["message"].as_str().is_some_and(|message| {
                        message.contains("local scalar/tuple comparison")
                    }));
                } else {
                    assert_eq!(core_doc["verdict"], doc["verdict"]);
                    assert_eq!(core_doc["tlc"], doc["tlc"]);
                    assert_eq!(core_doc["message"], doc["message"]);
                    assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
                }
            }
        }
    }
}
