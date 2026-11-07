//! Bounded `PointOnLine` checker witnesses are independently validated.
#![allow(clippy::expect_used, reason = "functional assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store};
use std::path::PathBuf;

#[test]
fn point_witness_matches_counts_and_rejects_forged_coordinates() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg-point/Point.tla");
    let modules =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-eval/tests/fixtures/svg-point");
    for compat_tlc in [true, false] {
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                module_paths: vec![modules.clone()],
                ..TraceOptions::default()
            },
        )
        .expect("independent model");
        for store in [Store::Exact, Store::Fingerprint] {
            let report = dive_api::check_file_engine(
                &CheckRequest {
                    file: file.clone(),
                    compat_tlc,
                    store,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    module_paths: vec![modules.clone()],
                    ..CheckRequest::default()
                },
                dive_api::Engine::Reference,
            )
            .expect("check");
            assert_eq!(report.exit_code(), if compat_tlc { 12 } else { 10 });
            let json = report.to_json();
            assert_eq!(json["verdict"], "invariant-violation", "{json}");
            assert_eq!(json["property"], "Inv");
            assert_eq!(json["tlc"]["code"], 2110);
            assert_eq!(json["statistics"]["generated"], 2);
            assert_eq!(json["statistics"]["distinct"], 2);
            assert_eq!(json["statistics"]["depth"], 2);
            let doc = TraceDoc::from_json(&json).expect("witness");
            assert_eq!(
                doc.states[0]["p"],
                serde_json::json!({"record":{"x":0,"y":1}})
            );
            assert_eq!(model.validate(&doc).validation, Validation::Valid);
            let mut forged = doc;
            forged.states[0].insert("p".into(), serde_json::json!({"record":{"x":-1,"y":1}}));
            assert!(matches!(
                model.validate(&forged).validation,
                Validation::Invalid(_)
            ));
        }
    }
}

#[test]
fn unsupported_shape_has_no_authoritative_witness() {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg-point/Missing.tla");
    let modules =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-eval/tests/fixtures/svg-point");
    for compat_tlc in [true, false] {
        let report = dive_api::check_file_engine(
            &CheckRequest {
                file: file.clone(),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                module_paths: vec![modules.clone()],
                ..CheckRequest::default()
            },
            dive_api::Engine::Reference,
        )
        .expect("check");
        assert_eq!(report.exit_code(), 3);
        let json = report.to_json();
        assert_eq!(json["verdict"], "unsupported", "{json}");
        assert!(
            json["message"]
                .as_str()
                .expect("named refusal")
                .contains("SVG!PointOnLine")
        );
        assert!(json["counterexample"].is_null());
        assert!(json["collision"].is_null());
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                module_paths: vec![modules.clone()],
                ..TraceOptions::default()
            },
        )
        .expect("model syntax");
        let doc = TraceDoc::behavior(vec![serde_json::json!({"p":{"record":{"x":0,"y":0}}})])
            .expect("word");
        assert!(
            matches!(model.validate(&doc).validation,Validation::Unsupported(feature) if feature.contains("SVG!PointOnLine"))
        );
    }
}
