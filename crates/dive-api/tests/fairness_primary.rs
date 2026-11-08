//! Actual source rows establish bounded temporal error authority; other paths refuse.
#![allow(clippy::expect_used, clippy::panic, reason = "pinned primary controls")]
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fairness-primary")
}
fn same_corpus_input(name: &str) {
    let corpus = directory().join(format!(
        "../../../../../conformance/corpus/fairness-primary/specifications/{name}/{name}.tla"
    ));
    assert_eq!(
        std::fs::read(directory().join(format!("{name}.tla"))).expect("API source"),
        std::fs::read(&corpus).expect("corpus source")
    );
    assert_eq!(
        std::fs::read(directory().join(format!("{name}.cfg"))).expect("API config"),
        std::fs::read(corpus.with_extension("cfg")).expect("corpus config")
    );
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
fn proved_source_rows_publish_only_freshly_validated_v5_words() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/fairness-primary/oracle.json")).expect("pin");
    for name in [
        "MinChooseFair",
        "MinChooseFairSafeNext",
        "RowActionBeforeState",
        "RowStateAfterQuietAction",
        "RowSelfThenError",
    ] {
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(
            directory().join(format!("../../../../../conformance/golden/tlc-2026.10.06.014338/fairness-primary/specifications/{name}/{name}.cfg.json"))
        ).expect("official oracle golden")).expect("golden JSON");
        for tlc in [false, true] {
            {
                let engine = Engine::Core;
                let req = request(name, tlc, 1);
                let report = check_file_engine(&req, engine).expect("owned source run");
                let json = report.to_json();
                assert_eq!(json["verdict"], "evaluation-error", "{name}/{tlc}: {json}");
                assert_eq!(report.exit_code_for(dive_api::ExitConvention::Tlc), 255);
                assert_eq!(json["tlc"]["exit_status"], 255);
                assert_eq!(json["tlc"]["code"], 1000);
                assert_eq!(json["tlc"]["reported"], 1000);
                assert_eq!(json["tlc"]["verified"], true);
                assert_eq!(json["error_site"], "temporal-source-row");
                assert_eq!(json["property"], Value::Null);
                assert_eq!(json["execution"]["capture_completed"], false);
                for (actual, expected) in [
                    ("generated", "states_generated"),
                    ("distinct", "distinct_states"),
                    ("depth", "depth"),
                    ("left_on_queue", "states_left_on_queue"),
                ] {
                    assert_eq!(
                        json["statistics"][actual],
                        golden["result"]["counts"][expected]
                    );
                    if name.starts_with("MinChoose") {
                        assert_eq!(
                            golden["result"]["counts"][expected],
                            oracle["positive_goldens"][name]["counts"][expected]
                        );
                    }
                }
                let word = &json["counterexample"]["trace"];
                assert_eq!(word["version"], 5);
                assert_eq!(word["ending"]["endpoint"]["stage"], "temporal-source-row");
                assert_eq!(json["counterexample"]["length"], 1);
                assert!(
                    report
                        .render_human()
                        .contains("The behavior up to this point is:")
                );
                let model = dive_api::trace::SpecModel::load(
                    &req.file,
                    req.config.as_deref().expect("config"),
                    &dive_api::trace::TraceOptions {
                        compat_tlc: tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    },
                )
                .expect("fresh source model");
                let word = dive_trace::TraceDoc::from_json(word).expect("actual v5 document");
                assert_eq!(
                    model
                        .try_validate(&word)
                        .expect("fresh fallible replay")
                        .validation,
                    dive_trace::Validation::Valid
                );
            }
        }
    }
}

#[test]
fn different_primary_and_candidate_error_lifecycles_do_not_borrow_source_row_authority() {
    for name in ["RowDifferentPrimary", "RowCandidateBeforeTemporal"] {
        same_corpus_input(name);
        for tlc in [false, true] {
            {
                let engine = Engine::Core;
                let report =
                    check_file_engine(&request(name, tlc, 1), engine).expect("actual boundary");
                let json = report.to_json();
                assert_eq!(
                    json["verdict"], "unsupported",
                    "{name}/{tlc}/{engine:?}: {json}"
                );
                assert!(
                    json["tlc"].is_null(),
                    "unproved first-primary/ending contract cannot verify a class"
                );
                assert_ne!(
                    json["counterexample"]["trace"]["ending"]["endpoint"]["stage"],
                    "temporal-source-row",
                    "a diagnostic prefix is not the new endpoint"
                );
                assert_eq!(json["statistics"]["distinct"], 1);
            }
        }
    }
}

#[test]
fn an_unproved_first_row_or_multiple_init_emissions_cannot_publish_v5() {
    let scratch =
        std::env::temp_dir().join(format!("dive-temporal-first-source-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("temporary source directory");
    let file = scratch.join("FirstSource.tla");
    let config = scratch.join("FirstSource.cfg");
    std::fs::write(&config, "SPECIFICATION Spec\nPROPERTY Live\n").expect("configuration");
    for (initial, next) in [
        ("x = 0", "IF x = 0 THEN x' = 1 ELSE UNCHANGED x"),
        ("x \\in {0,1}", "UNCHANGED x"),
        ("(x = 0) \\/ (x = 0)", "UNCHANGED x"),
    ] {
        std::fs::write(
            &file,
            format!(
                "---- MODULE FirstSource ----\nEXTENDS Naturals\nVARIABLE x\nInit == {initial}\nNext == {next}\nPick == x' = CHOOSE n \\in {{1,2}}: FALSE\nSpec == Init /\\ [][Next]_x /\\ WF_x(Pick)\nLive == <>(x = 2)\n====\n"
            ),
        )
        .expect("actual source");
        for tlc in [false, true] {
            {
                let engine = Engine::Core;
                let req = CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    compat_tlc: tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let json = check_file_engine(&req, engine)
                    .expect("bounded source run")
                    .to_json();
                assert_eq!(json["verdict"], "unsupported", "{json}");
                assert!(json["tlc"].is_null(), "unproved first source: {json}");
                assert_ne!(json["counterexample"]["trace"]["version"], 5);
            }
        }
    }
    std::fs::remove_dir_all(scratch).expect("remove temporary source inputs");
}

#[test]
fn unproved_temporal_next_primaries_are_refusals_not_verified_75_or_general() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/fairness-primary/oracle.json")).expect("pin");
    for name in ["MinChooseFair", "MinChoosePlain", "MinChooseOtherFair"] {
        same_corpus_input(name);
        for tlc in [false, true] {
            for workers in [1, 8] {
                for engine in std::iter::once(Engine::Core) {
                    if name == "MinChooseFair" && workers == 1 {
                        continue; // The separately asserted reached-row capability is now proved.
                    }
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
    // Uncovered worker/order scopes cannot borrow the qualified W1 path.
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
fn unproved_parallel_fairness_errors_keep_the_existing_explicit_refusal() {
    for tlc in [false, true] {
        for workers in std::iter::once(8) {
            {
                let engine = Engine::Core;
                let report =
                    check_file_engine(&request("MinChooseFairSafeNext", tlc, workers), engine)
                        .expect("check");
                let json = report.to_json();
                assert_eq!(json["verdict"], "unsupported", "{json:#}");
                assert!(json["tlc"].is_null());
                assert_eq!(report.exit_code(), 3);
            }
        }
    }
}

#[test]
fn safety_only_next_errors_stay_unchanged_but_the_word_certifies_no_temporal_primary() {
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
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
