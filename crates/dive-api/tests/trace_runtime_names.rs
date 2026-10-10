//! Trace loading must not certify runtime namespace history from the witness.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

const SPEC: &str = include_str!("../../dive-trace/tests/fixtures/runtime-names/ReplayNames.tla");
const CFG: &str = include_str!("../../dive-trace/tests/fixtures/runtime-names/ReplayNames.cfg");

#[test]
fn run_metadata_cannot_make_a_forged_namespace_independently_valid() {
    let forged = TraceDoc::parse(include_str!(
        "../../dive-trace/tests/fixtures/runtime-names/bad.json"
    ))
    .expect("doc");
    for run in [None, Some((0, 0)), Some((7, 0))] {
        let m = SpecModel::from_source(
            "ReplayNames.tla",
            SPEC,
            CFG,
            &TraceOptions {
                compat_tlc: true,
                run,
                ..TraceOptions::default()
            },
        )
        .expect("model");
        assert_eq!(
            m.validate(&forged).validation,
            Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
        let default = SpecModel::from_source(
            "ReplayNames.tla",
            SPEC,
            CFG,
            &TraceOptions {
                run,
                ..TraceOptions::default()
            },
        )
        .expect("default");
        assert!(matches!(
            default.validate(&forged).validation,
            Validation::Invalid(_)
        ));
        let valid = TraceDoc::behavior(vec![json!({"x":6,"s":"unset"}), json!({"x":6,"s":"6"})])
            .expect("word");
        assert_eq!(default.validate(&valid).validation, Validation::Valid);
    }
}
