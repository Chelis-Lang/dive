//! Opt-in stored trace transport keeps concrete safety replay independent.
#![allow(clippy::expect_used, clippy::panic, reason = "regression assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceEncoding, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file};

fn verify(name: &str) {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/trace-representation-v2/specs")
        .join(name)
        .join("Transport.tla");
    for compat_tlc in [true, false] {
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((7, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("model");
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let request = CheckRequest {
                        file: file.clone(),
                        compat_tlc,
                        store,
                        workers: Some(workers),
                        chaos,
                        seed: Some(7),
                        fp: Some(0),
                        trace_encoding: TraceEncoding::RepresentationV2,
                        ..CheckRequest::default()
                    };
                    let current = check_file(&request).expect("check").to_json();
                    assert_eq!(current["verdict"], "invariant-violation", "{current}");
                    assert_eq!(current["statistics"]["generated"], 4);
                    assert_eq!(current["statistics"]["distinct"], 4);
                    assert_eq!(current["statistics"]["depth"], 4);
                    assert_eq!(current["counterexample"]["trace"]["version"], 2);
                    assert_eq!(
                        current["counterexample"]["trace_encoding"]["semantic_validation"],
                        false
                    );
                    let doc = TraceDoc::from_json(&current).expect("doc");
                    let (report, trace) = model.read(&doc);
                    assert_eq!(report.validation, Validation::Valid, "{name}: {report:?}");
                    let trace = trace.expect("decoded word");
                    let again = model
                        .to_doc_with_encoding(&trace, TraceEncoding::RepresentationV2)
                        .expect("stored writer");
                    assert_eq!(again.to_json(), doc.to_json());
                    for at in [0, 1, 3] {
                        let mut mutant = doc.clone();
                        mutant.states[at].insert("n".into(), serde_json::json!(99));
                        assert!(
                            matches!(model.validate(&mutant).validation, Validation::Invalid(_)),
                            "mutant {at}"
                        );
                    }
                    let legacy = check_file(&CheckRequest {
                        trace_encoding: TraceEncoding::LegacyV1,
                        ..request
                    })
                    .expect("legacy")
                    .to_json();
                    assert_eq!(legacy["counterexample"]["trace"]["version"], 1);
                    assert!(legacy["counterexample"].get("trace_encoding").is_none());
                    assert_eq!(legacy["statistics"], current["statistics"]);
                    if name == "Mixed" {
                        assert!(matches!(
                            model
                                .validate(&TraceDoc::from_json(&legacy).expect("legacy doc"))
                                .validation,
                            Validation::Invalid(_)
                        ));
                    }
                }
            }
        }
    }
}
#[test]
fn mixed_stored_words_validate() {
    verify("Mixed");
}
#[test]
fn tuple_function_stored_words_validate() {
    verify("TupleFunction");
}
#[test]
fn record_function_stored_words_validate() {
    verify("RecordFunction");
}
