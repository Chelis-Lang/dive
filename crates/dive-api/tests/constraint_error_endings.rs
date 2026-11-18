//! Terminal safety claims outside constraints preserve ordered demand.
#![allow(clippy::expect_used, reason = "pinned finite oracle controls")]

use std::path::PathBuf;

use dive_api::trace::{Check, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file};
use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn source(name: &str) -> PathBuf {
    root().join(format!(
        "conformance/corpus/constraint-error-endings/specifications/{name}/{name}.tla"
    ))
}

fn options(compat_tlc: bool) -> TraceOptions {
    TraceOptions {
        compat_tlc,
        run: Some((0, 0)),
        ..TraceOptions::default()
    }
}

fn load(name: &str, compat_tlc: bool) -> SpecModel {
    let file = source(name);
    SpecModel::load(&file, &file.with_extension("cfg"), &options(compat_tlc)).expect("model")
}

fn error() -> Ending {
    Ending::InvariantError {
        name: "Inv".into(),
        ec: "TLC_MODULE_DIVISION_BY_ZERO".into(),
    }
}

fn word(states: &[i64], ending: Ending) -> TraceDoc {
    TraceDoc::behavior(states.iter().map(|x| json!({"x":x})).collect())
        .expect("word")
        .with_ending(ending)
}

fn invalid(model: &SpecModel, word: &TraceDoc, check: Check, state: usize) {
    let validation = model.validate(word).validation;
    assert!(
        matches!(&validation, Validation::Invalid(f) if f.check==check && f.state==state),
        "{validation:?}"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one fixture matrix separates primary unknowns from logical replay"
)]
fn qualified_invariant_error_words_validate_but_unclassified_streams_stay_unknown() {
    let oracle: Value = serde_json::from_str(include_str!(
        "fixtures/constraint-error-endings/oracle.json"
    ))
    .expect("compact oracle");
    for name in [
        "ConstraintInvError",
        "ActionConstraintInvError",
        "FirstFalseConstraint",
    ] {
        let pinned = &oracle["cases"][name];
        assert_eq!(pinned["golden_written"], false);
        assert_eq!(pinned["physical_exit"], 255);
        assert_eq!(pinned["counts"]["generated"], 3);
        assert_eq!(pinned["counts"]["distinct"], 2);
        assert!(
            pinned["codes"]
                .as_array()
                .expect("codes")
                .contains(&json!(2111))
        );
        assert!(
            pinned["codes"]
                .as_array()
                .expect("codes")
                .contains(&json!(2103))
        );
        let golden = root().join(format!("conformance/golden/tlc-2026.10.06.014338/constraint-error-endings/specifications/{name}/{name}.cfg.json"));
        assert!(
            !golden.exists(),
            "unclassified stream must not gain a model golden"
        );
        for compat_tlc in [false, true] {
            let model = load(name, compat_tlc);
            let valid = word(&[0, 1, 2], error());
            assert_eq!(
                model.validate(&valid).validation,
                Validation::Valid,
                "{name}/{compat_tlc}"
            );
            {
                let path = source(name);
                let report = check_file(&CheckRequest {
                    file: path.clone(),
                    config: Some(path.with_extension("cfg")),
                    compat_tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                })
                .expect("native check")
                .to_json();
                assert_eq!(report["tlc"]["code"], 2179);
                assert_eq!(report["tlc"]["reported"], 2111);
                let native = TraceDoc::from_json(&report["counterexample"]["trace"])
                    .expect("actual original invariant word");
                assert_eq!(native.ending, Some(error()));
                assert_eq!(model.validate(&native).validation, Validation::Valid);
            }
            invalid(&model, &word(&[0, 1], error()), Check::Ending, 1);
            invalid(&model, &word(&[0, 2], error()), Check::Next, 1);
            invalid(
                &model,
                &word(&[0, 1, 2, 3], error()),
                if name == "ActionConstraintInvError" {
                    Check::ActionConstraint
                } else {
                    Check::Constraint
                },
                2,
            );
            invalid(
                &model,
                &word(
                    &[0, 1, 2],
                    Ending::InvariantError {
                        name: "Inv".into(),
                        ec: "TLC_INVARIANT_EVALUATION_FAILED".into(),
                    },
                ),
                Check::Ending,
                2,
            );
            invalid(
                &model,
                &word(
                    &[0, 1, 2],
                    Ending::Error {
                        ec: Some("TLC_MODULE_DIVISION_BY_ZERO".into()),
                    },
                ),
                if name == "ActionConstraintInvError" {
                    Check::ActionConstraint
                } else {
                    Check::Constraint
                },
                2,
            );
        }
    }
}

#[test]
fn actual_property_words_match_their_primary_goldens_and_validate() {
    for name in [
        "ConstraintAction",
        "ActionConstraintAction",
        "InitialConstraintProperty",
    ] {
        let path = source(name);
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(root().join(format!("conformance/golden/tlc-2026.10.06.014338/constraint-error-endings/specifications/{name}/{name}.cfg.json"))).expect("official golden")).expect("golden JSON");
        for compat_tlc in [false, true] {
            let model = load(name, compat_tlc);
            {
                for workers in [1, 4] {
                    let report = check_file(&CheckRequest {
                        file: path.clone(),
                        config: Some(path.with_extension("cfg")),
                        compat_tlc,
                        workers: Some(workers),
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    })
                    .expect("check");
                    let actual = report.to_json();
                    assert_eq!(
                        actual["tlc"]["code"], golden["result"]["verdict_code"],
                        "{name}/{compat_tlc}/{workers}"
                    );
                    assert_eq!(actual["property"], golden["result"]["violated_property"]);
                    assert_eq!(
                        i64::from(report.exit_code_for(ExitConvention::Tlc)),
                        golden["result"]["exit_code"]
                            .as_i64()
                            .expect("physical exit")
                    );
                    if !golden["result"]["counts"]["states_generated"].is_null() {
                        assert_eq!(
                            actual["statistics"]["generated"],
                            golden["result"]["counts"]["states_generated"]
                        );
                        assert_eq!(
                            actual["statistics"]["distinct"],
                            golden["result"]["counts"]["distinct_states"]
                        );
                        assert_eq!(
                            actual["statistics"]["depth"],
                            golden["result"]["counts"]["depth"]
                        );
                    }
                    let doc = TraceDoc::from_json(&actual["counterexample"]["trace"])
                        .expect("actual native word");
                    assert_eq!(
                        model.validate(&doc).validation,
                        Validation::Valid,
                        "{name}/{compat_tlc}/{workers}"
                    );
                    let mut forged = doc;
                    forged.ending = Some(Ending::Property("Missing".into()));
                    invalid(&model, &forged, Check::Ending, forged.states.len() - 1);
                }
            }
        }
    }
}

#[test]
fn constraint_errors_and_non_booleans_decide_before_a_terminal_invariant() {
    for compat_tlc in [false, true] {
        let first_error = load("FirstErrorConstraint", compat_tlc);
        // The old invariant-ending shortcut skipped this demanded constraint
        // error and incorrectly certified the claimed invariant violation.
        invalid(
            &first_error,
            &word(&[0, 1, 2], Ending::Invariant("Inv".into())),
            Check::Constraint,
            2,
        );
        let file = source("ConstraintInvError");
        let text = std::fs::read_to_string(&file).expect("source");
        let config = std::fs::read_to_string(file.with_extension("cfg")).expect("config");
        for terminal in [
            "IF x=2 THEN 7 ELSE TRUE",
            "IF x=2 THEN 1 \\div 0=0 ELSE TRUE",
        ] {
            let text = text.replace("Limit == x<2", &format!("Limit == {terminal}"));
            let model = SpecModel::from_source(
                "ConstraintInvError.tla",
                &text,
                &config,
                &options(compat_tlc),
            )
            .expect("model");
            invalid(&model, &word(&[0, 1, 2], error()), Check::Constraint, 2);
        }
        let text = text.replace("Limit == x<2", "Limit == x<1");
        let model = SpecModel::from_source(
            "ConstraintInvError.tla",
            &text,
            &config,
            &options(compat_tlc),
        )
        .expect("earlier exclusion");
        invalid(&model, &word(&[0, 1, 2], error()), Check::Constraint, 1);
    }
}

#[test]
fn first_false_constraint_skips_later_state_and_action_constraint_errors() {
    for compat_tlc in [false, true] {
        for (name, extra, config_suffix) in [
            (
                "ConstraintInvError",
                "Later == IF x=2 THEN 1 \\div 0=0 ELSE TRUE",
                "CONSTRAINT Later\n",
            ),
            (
                "ConstraintInvError",
                "Later == IF x'=2 THEN 1 \\div 0=0 ELSE TRUE",
                "ACTION_CONSTRAINT Later\n",
            ),
            (
                "ActionConstraintInvError",
                "Later == IF x'=2 THEN 1 \\div 0=0 ELSE TRUE",
                "ACTION_CONSTRAINT Later\n",
            ),
        ] {
            let file = source(name);
            let text = std::fs::read_to_string(&file)
                .expect("source")
                .replace("====", &format!("{extra}\n===="));
            let config = std::fs::read_to_string(file.with_extension("cfg")).expect("config")
                + config_suffix;
            let model = SpecModel::from_source(
                &format!("{name}.tla"),
                &text,
                &config,
                &options(compat_tlc),
            )
            .expect("model");
            assert_eq!(
                model.validate(&word(&[0, 1, 2], error())).validation,
                Validation::Valid,
                "{name}/{compat_tlc}"
            );
        }
    }
}
