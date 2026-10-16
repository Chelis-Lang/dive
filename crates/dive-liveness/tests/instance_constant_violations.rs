//! Final analysis preserves pinned constant-fallback violations and counts.
#![allow(clippy::expect_used, reason = "pinned primary fixture assertions")]

use dive_check::CheckOptions;
use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use dive_trace::{Ending, Trace, TraceDoc, Validation};
use std::{path::Path, sync::Arc};

#[test]
fn absent_constants_keep_primary_violations_counts_and_valid_words() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-trace/tests/fixtures/instance-constants");
    let facts: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-trace/tests/fixtures/instance-constants/primary-violations.json"
    ))
    .expect("primary facts");
    let cases = facts["cases"].as_array().expect("two cases");
    assert_eq!(cases.len(), 2);
    for defaults in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for store in [
            dive_store::StoreMode::Exact,
            dive_store::StoreMode::Fingerprint,
        ] {
            let options = CheckOptions {
                store,
                ..defaults.clone()
            };
            for case in cases {
                let id = case["id"].as_str().expect("id");
                let path = fixtures.join(id);
                let source = dive_sema::analyze_file(
                    &path.join("Constant.tla"),
                    &dive_sema::Options::default(),
                )
                .expect("source");
                assert!(source.accepted(), "{:?}", source.diagnostics());
                let config = dive_config::parse(
                    &std::fs::read_to_string(path.join("Constant.cfg")).expect("config"),
                )
                .into_config()
                .expect("config");
                let model = dive_model::bind(
                    Arc::new(dive_ir::lower(&source).expect("IR")),
                    &config,
                    &dive_model::BindOptions {
                        eval: options.eval,
                        ..dive_model::BindOptions::tlc()
                    },
                )
                .expect("model");
                for _ in 0..2 {
                    let analysis =
                        analyze_final(&model, &options, Limits::default()).expect("analysis");
                    assert!(
                        matches!(analysis, Analysis::Candidate(_)),
                        "{id}: {analysis:?}"
                    );
                    let Analysis::Candidate(candidate) = analysis else {
                        continue;
                    };
                    assert_eq!(candidate.target_property, "Bad");
                    assert_eq!(candidate.properties, ["Bad"]);
                    let stats = candidate.exploration.stats;
                    assert_eq!(
                        serde_json::json!(stats.generated),
                        case["generated"],
                        "{id}"
                    );
                    assert_eq!(serde_json::json!(stats.distinct), case["distinct"], "{id}");
                    assert_eq!(serde_json::json!(stats.depth), case["depth"], "{id}");
                    let trace = Trace {
                        labels: vec![None; candidate.states.len()],
                        length: Some(candidate.states.len()),
                        states: candidate.states,
                        ending: Ending::Lasso {
                            back_to: candidate.back_to,
                        },
                        properties: candidate.properties,
                    };
                    assert_eq!(
                        dive_trace::validate(&model, &trace).validation,
                        Validation::Valid
                    );
                    let doc = trace.to_doc(&model).expect("document");
                    let parsed = TraceDoc::parse(&doc.to_json().to_string()).expect("transport");
                    assert_eq!(
                        dive_trace::validate_doc(&model, &parsed, &config.model_values)
                            .0
                            .validation,
                        Validation::Valid,
                        "{id}"
                    );
                }
            }
        }
    }
}
