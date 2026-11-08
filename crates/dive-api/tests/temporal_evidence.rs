//! Temporal execution evidence on Core: a binding failure proves that no
//! search ran, and a validated word is rejected once forged. The caller-set
//! analysis, product and acceptance limits these tests once varied belonged
//! to the retired reference final route (#791); Core's resource stops are
//! tested in `core_temporal_resources.rs`.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

#[path = "support/core_final.rs"]
mod core_final;

use dive_check::CheckOptions;
use std::sync::Arc;

const SOURCE: &str =
    "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

#[test]
fn binding_failures_prove_zero_work() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let report = core_final::run(SOURCE, "INIT Missing\nNEXT Next\nPROPERTY P\n", &options);
        let json = report.to_json();
        assert_eq!(json["verdict"], "model-error", "{json}");
        assert_eq!(json["tlc"]["code"], 2229, "{json}");
        assert!(json["statistics"].is_null(), "{json}");
        assert!(json["counterexample"].is_null(), "{json}");
    }
}

#[test]
fn validated_words_replay_independently_and_forged_words_fail() {
    let source =
        "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = x\nP == []<>x\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(CONFIG)
        .into_config()
        .expect("configuration");
    for mut options in [CheckOptions::dive(7), CheckOptions::tlc(7, 0)] {
        for workers in [1, 4] {
            options.workers = workers;
            let model = dive_model::bind(
                ir.clone(),
                &config,
                &dive_model::BindOptions {
                    eval: options.eval,
                    symmetry_liveness: options.symmetry_liveness,
                    no_deadlock: options.no_deadlock,
                    host: None,
                },
            )
            .expect("model");
            let run = core_final::check(source, CONFIG, &options).expect("verdict");
            let core_final::Outcome::Violation(word) = run.outcome() else {
                panic!("validated violation: {:?}", run.outcome())
            };
            assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
            assert_eq!(
                dive_trace::validate_doc(&model, word.document(), &[])
                    .0
                    .validation,
                dive_trace::Validation::Valid
            );
            let mut forged = word.document().clone();
            forged.states[0].insert("x".into(), true.into());
            assert!(matches!(
                dive_trace::validate_doc(&model, &forged, &[]).0.validation,
                dive_trace::Validation::Invalid(_)
            ));
        }
    }
}
