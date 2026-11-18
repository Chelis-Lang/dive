//! The real empty-BASE Init witness reaches its invariant and independent replay.
#![allow(clippy::expect_used, reason = "bounded primary corpus controls")]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention};
#[path = "support/refused.rs"]
mod refused;

#[test]
fn empty_subset_bases_reach_the_initial_invariant_and_validate_the_finite_word() {
    let specs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/empty-subset-base-primary/specifications");
    for compat_tlc in [false, true] {
        for name in ["EmptyBase", "DirectEmptyBase"] {
            let file = specs.join(name).join(format!("{name}.tla"));
            let config = file.with_extension("cfg");
            let report = dive_api::check_file(&CheckRequest {
                file: file.clone(),
                config: Some(config.clone()),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })
            .expect("ordinary source-bound check");
            // Only the retired reference engine ran TLC's checked
            // mixed-shape comparison on EmptyBase (#791, #910).
            if compat_tlc && name == "EmptyBase" {
                refused::assert_refused(
                    &report.to_json(),
                    report.exit_code(),
                    refused::MIXED_SHAPES,
                );
                continue;
            }
            assert_eq!(report.verdict(), "invariant-violation");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 12);
            let value = report.to_json();
            assert_eq!(value["property"], "Inv");
            assert_eq!(value["tlc"]["code"], 2107);
            assert_eq!(value["tlc"]["verified"], true);
            assert_eq!(value["counterexample"]["length"], 1);
            assert_eq!(value["counterexample"]["ending"], "finite");
            assert_eq!(
                value["counterexample"]["states"][0]["values"]["x"],
                serde_json::json!([])
            );
            assert_ne!(
                value["statistics"]["reported_by_tlc"],
                serde_json::json!(true)
            );
            let word = TraceDoc::from_json(&value["counterexample"]["trace"])
                .expect("original finite invariant word");
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )
            .expect("fresh independent model");
            assert_eq!(model.validate(&word).validation, Validation::Valid);
        }
    }
}
