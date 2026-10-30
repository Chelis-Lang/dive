//! An actual unproved post-Next temporal primary is refused, never guessed.
#![allow(clippy::expect_used, clippy::panic, reason = "pinned primary controls")]
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fairness-primary")
}
fn request(name: &str, tlc: bool, workers: usize) -> CheckRequest {
    CheckRequest {
        file: directory().join(format!("{name}.tla")),
        config: Some(directory().join(format!("{name}.cfg"))),
        compat_tlc: tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

#[test]
fn unproved_temporal_next_primaries_are_refusals_not_verified_75_or_general() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/fairness-primary/oracle.json")).expect("pin");
    for name in ["MinChooseFair", "MinChoosePlain", "MinChooseOtherFair"] {
        for tlc in [false, true] {
            for workers in [1, 8] {
                for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                    let report =
                        check_file_engine(&request(name, tlc, workers), engine).expect("check");
                    let json = report.to_json();
                    assert_eq!(
                        report.verdict(),
                        "unsupported",
                        "{name} {tlc} {workers}: {json:#}"
                    );
                    assert_eq!(report.exit_code(), 3);
                    assert!(
                        json["message"]
                            .as_str()
                            .expect("named refusal")
                            .contains("post-Next temporal-demand primary"),
                        "{json:#}"
                    );
                    assert!(
                        json["tlc"].is_null(),
                        "neither75 nor255 is a certified class: {json:#}"
                    );
                    assert!(json["property"].is_null());
                    assert!(
                        json["counterexample"].is_null(),
                        "no unconfirmed mandatory word can earn agreement"
                    );
                    assert_eq!(json["statistics"]["distinct"], 1);
                    assert_eq!(json["statistics"]["generated"], 1);
                    assert_eq!(json["statistics"]["depth"], 1);
                    assert_eq!(json["statistics"]["left_on_queue"], 0);
                }
            }
        }
    }
    // Existing official goldens establish the disagreement; refusal fixes the
    // unsupported claim, and does not implement this reached-fairness pipeline.
    assert_eq!(
        oracle["positive_goldens"]["MinChooseFair"]["error_class"],
        "GENERAL"
    );
    assert_eq!(
        oracle["positive_goldens"]["MinChooseFair"]["exit_code"],
        255
    );
}

#[test]
fn fairness_errors_without_a_next_error_keep_the_existing_explicit_refusal() {
    for tlc in [false, true] {
        for workers in [1, 8] {
            for engine in [Engine::Reference, Engine::Core] {
                let report =
                    check_file_engine(&request("MinChooseFairSafeNext", tlc, workers), engine)
                        .expect("check");
                let json = report.to_json();
                assert_eq!(json["verdict"], "unsupported", "{json:#}");
                assert!(json["tlc"].is_null());
                assert_eq!(report.exit_code(), 3);
                // The reference route already retained these actual completed
                // counts. Do not invent previously missing native evidence.
                if engine == Engine::Reference {
                    assert_eq!(json["statistics"]["distinct"], 1);
                    assert_eq!(json["statistics"]["generated"], 2);
                }
            }
        }
    }
}

#[test]
fn safety_only_next_errors_stay_unchanged_but_the_word_certifies_no_temporal_primary() {
    for tlc in [false, true] {
        for engine in [Engine::Reference, Engine::Core] {
            let mut req = request("MinChooseFair", tlc, 1);
            req.config = Some(directory().join("NoTemporal.cfg"));
            let report = check_file_engine(&req, engine).expect("safety check");
            let json = report.to_json();
            assert_eq!(json["verdict"], "evaluation-error", "{json:#}");
            assert_eq!(json["tlc"]["exit_status"], 75);
            assert_ne!(json["tlc"]["code"], 1000);
            assert_eq!(json["statistics"]["distinct"], 1);
            assert_eq!(json["statistics"]["generated"], 1);
            // Reusing the actual source-bound Next document under the original
            // temporal obligations supplies no fairness/demand replay proof.
            let document = dive_trace::TraceDoc::from_json(&json["counterexample"]["trace"])
                .expect("actual Next word");
            let source = std::fs::read_to_string(&req.file).expect("source");
            let analysis = dive_sema::analyze_source(
                "MinChooseFair.tla",
                &source,
                &dive_sema::Options::default(),
            );
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let config = dive_config::parse(
                &std::fs::read_to_string(directory().join("MinChooseFair.cfg")).expect("config"),
            )
            .into_config()
            .expect("parsed");
            let options = if tlc {
                dive_eval::Options::tlc()
            } else {
                dive_eval::Options::dive()
            };
            let (validation, _) =
                dive_trace::Session::new(dive_model::Recipe::new(ir, config, options, false), None)
                    .read(&document);
            assert!(
                matches!(
                    validation.validation,
                    dive_trace::Validation::Unsupported(_)
                ),
                "unproved temporal replay cannot be Valid: {validation:?}"
            );
        }
    }
}
