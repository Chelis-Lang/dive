//! TLC's actual `CapturedConcatFails` word replays independently; invalid claims
//! are rejected. The final-analysis checks of these models run on Core in
//! `crates/dive-api/tests/liveness_concat_capture_types.rs` (#791).
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "primary reference assertions"
)]

use std::{path::Path, sync::Arc};

use dive_model::Model;
use dive_runtime::CheckOptions;
use dive_trace::{Ending, TraceDoc, Validation};
use serde_json::{Value, json};

fn bind(name: &str, options: &CheckOptions) -> Model {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/selectseq-concat-type-capability/specifications")
        .join(name);
    let source = dive_sema::analyze_file(
        &directory.join(format!("{name}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("original source");
    assert!(source.accepted(), "{:?}", source.diagnostics());
    let config = dive_config::parse(
        &std::fs::read_to_string(directory.join(format!("{name}.cfg"))).expect("original config"),
    )
    .into_config()
    .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&source).expect("IR")),
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            ..dive_model::BindOptions::tlc()
        },
    )
    .expect("bound model")
}

fn primary(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/golden/tlc-2026.10.06.014338/selectseq-concat-type-capability/specifications")
        .join(name)
        .join(format!("{name}.cfg.json"));
    serde_json::from_slice(&std::fs::read(path).expect("generated reference")).expect("JSON")
}

#[test]
fn actual_pinned_stuttering_word_replays_independently_and_rejects_invalid_claims() {
    let expected = primary("CapturedConcatFails");
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        let word = TraceDoc::from_json(&expected["result"]["trace"]["json"])
            .expect("actual original TLC word")
            .with_ending(
                Ending::from_json(&expected["result"]["trace"]["ending"]).expect("actual ending"),
            )
            .with_properties(vec![
                expected["result"]["violated_property"]
                    .as_str()
                    .expect("property")
                    .to_owned(),
            ]);
        assert_eq!(
            dive_trace::validate_doc(&bind("CapturedConcatFails", &options), &word, &[])
                .0
                .validation,
            Validation::Valid
        );
        let mut bad_init = word.clone();
        bad_init.states[0].insert("x".to_owned(), json!(2));
        assert!(matches!(
            dive_trace::validate_doc(&bind("CapturedConcatFails", &options), &bad_init, &[])
                .0
                .validation,
            Validation::Invalid(_)
        ));
        assert!(
            matches!(
                dive_trace::validate_doc(&bind("CapturedConcatHolds", &options), &word, &[])
                    .0
                    .validation,
                Validation::Invalid(_)
            ),
            "a holding property cannot validate the same counterexample claim"
        );
    }
}
