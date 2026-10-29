//! Named invariant errors retain their cause and validate in evaluation order.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use dive_api::trace::{Check, ControlMode, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::json;
use std::path::PathBuf;

const ASSERT: &str = "TLC_VALUE_ASSERT_FAILED";
const SOURCE: &str = r#"---- MODULE Audit ----
EXTENDS Integers, TLC
VARIABLE x
Init == x=0
Next == x'=x+1
Good == TRUE
Inv == Assert(x # 2, "Inv")
Later == Assert(x # 2, "Later")
EarlierFalse == x<2
EarlierError == 2 \div (2-x) >= 0
====
"#;
const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Good Inv Later\n";

fn claim(name: &str, ec: &str) -> Ending {
    Ending::InvariantError {
        name: name.into(),
        ec: ec.into(),
    }
}
fn doc(xs: &[i64], ending: Ending) -> TraceDoc {
    TraceDoc::behavior(xs.iter().map(|x| json!({"x":x})).collect())
        .expect("states")
        .with_ending(ending)
}
fn invalid(model: &SpecModel, doc: &TraceDoc, check: Check, state: usize) {
    let actual = model.validate(doc).validation;
    assert!(
        matches!(&actual, Validation::Invalid(f) if f.check==check && f.state==state),
        "{actual:?}"
    );
}

#[test]
fn exact_cause_named_invariant_and_first_failure_are_required() {
    for compat_tlc in [false, true] {
        let opts = TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        };
        let m = SpecModel::from_source("Audit.tla", SOURCE, CFG, &opts).expect("model");
        let good = doc(&[0, 1, 2], claim("Inv", ASSERT));
        assert_eq!(m.validate(&good).validation, Validation::Valid);
        assert_eq!(
            TraceDoc::from_json(&good.to_json()).expect("round trip"),
            good
        );
        let controls = m.controls_with_mode(&good, ControlMode::ShortestBfs);
        assert!(controls.failures().is_empty(), "{controls:?}");
        assert!(controls.mutants.len() >= 7);
        for (name, ec, check) in [
            ("Missing", ASSERT, Check::Ending),
            ("Good", ASSERT, Check::Ending),
            ("Later", ASSERT, Check::Invariant),
            ("Inv", "TLC_MODULE_DIVISION_BY_ZERO", Check::Ending),
            ("Inv", "TLC_INVARIANT_EVALUATION_FAILED", Check::Ending),
        ] {
            invalid(&m, &doc(&[0, 1, 2], claim(name, ec)), check, 2);
        }
        invalid(&m, &doc(&[0, 1], claim("Inv", ASSERT)), Check::Ending, 1);
        invalid(
            &m,
            &doc(&[0, 1, 2, 3], claim("Inv", ASSERT)),
            Check::Invariant,
            2,
        );
        for earlier in ["EarlierFalse", "EarlierError"] {
            let cfg = CFG.replace("Good Inv", &format!("Good {earlier} Inv"));
            let m = SpecModel::from_source("Audit.tla", SOURCE, &cfg, &opts).expect("model");
            invalid(&m, &good, Check::Invariant, 2);
        }
        let source = SOURCE.replace("Assert(x # 2, \"Inv\")", "IF x=2 THEN 7 ELSE TRUE");
        let non_boolean = SpecModel::from_source("Audit.tla", &source, CFG, &opts).expect("model");
        assert_eq!(
            non_boolean
                .validate(&doc(&[0, 1, 2], claim("Inv", "TLC_EXPECTED_VALUE")))
                .validation,
            Validation::Valid
        );
        invalid(&non_boolean, &good, Check::Ending, 2);
        for replacement in ["x<2", "TRUE"] {
            let source = SOURCE.replace("Assert(x # 2, \"Inv\")", replacement);
            let m = SpecModel::from_source("Audit.tla", &source, CFG, &opts).expect("model");
            invalid(&m, &good, Check::Ending, 2);
        }
    }
}

#[test]
fn named_invariant_word_survives_a_different_diagnostic_replay_error() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/errors");
    let file = root.join("specs/ErInvReplayError/ErInvReplayError.tla");
    // The authoritative clean pinned run reports the original invariant
    // error, then a separate nested replay error. This is not a claim that
    // the canonical oracle classifier certifies the whole mixed stream.
    let pinned =
        std::fs::read_to_string(root.join("tlc-2026.10.06.014338/ErInvReplayError.stdout"))
            .expect("pinned clean TLC transcript");
    assert!(
        pinned.find("STARTMSG 2111:1").expect("first error")
            < pinned.find("STARTMSG 2103:1").expect("replay error")
    );
    for compat_tlc in [false, true] {
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("model");
        for engine in [Engine::Reference, Engine::Core] {
            for workers in [1, 4] {
                let report = check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc,
                        seed: Some(0),
                        fp: Some(0),
                        workers: Some(workers),
                        ..CheckRequest::default()
                    },
                    engine,
                )
                .expect("check")
                .to_json();
                assert_eq!(report["error_site"], "invariant");
                assert_eq!(report["tlc"]["reported"], 2111);
                assert_eq!(report["tlc"]["code"], 2103);
                assert_eq!(report["tlc"]["exit_status"], 75);
                assert_eq!(report["tlc"]["verified"], true);
                if workers == 1 {
                    for (key, value) in [("distinct", 5), ("generated", 4), ("depth", 3)] {
                        assert_eq!(report["statistics"][key], value, "{report}");
                    }
                }
                let trace = &report["counterexample"]["trace"];
                assert_eq!(trace["version"], 2);
                let doc = TraceDoc::from_json(trace).expect("native version 2");
                assert_eq!(
                    doc.states,
                    [
                        json!({"x":0,"y":0}),
                        json!({"x":1,"y":0}),
                        json!({"x":2,"y":0})
                    ]
                    .map(|value| value.as_object().expect("state").clone())
                );
                assert_eq!(
                    doc.ending,
                    Some(claim("InvA", "TLC_MODULE_DIVISION_BY_ZERO"))
                );
                assert_eq!(model.validate(&doc).validation, Validation::Valid);
                let controls = model.controls_with_mode(&doc, ControlMode::Counterexample);
                assert!(controls.failures().is_empty(), "{controls:?}");
                for (name, ec) in [
                    ("InvB", "TLC_MODULE_DIVISION_BY_ZERO"),
                    ("absent", "TLC_MODULE_DIVISION_BY_ZERO"),
                    ("InvA", "GENERAL"),
                    ("InvA", "TLC_NESTED_EXPRESSION"),
                ] {
                    assert!(!matches!(
                        model
                            .validate(&doc.clone().with_ending(claim(name, ec)))
                            .validation,
                        Validation::Valid
                    ));
                }
            }
        }
    }
}

#[test]
fn oracle_backed_invariant_errors_validate_for_both_engines_and_profiles() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-lib/specs");
    // Pinned TLC transcripts in ../tlc-2026.10.06.014338 establish deciding
    // 2111/exit 76 and the same words; causes are independently replayed.
    for (name, ec, length) in [
        ("ErAssertInv", ASSERT, 3),
        ("ErBagCardNonBag", "TLC_MODULE_APPLYING_TO_WRONG_VALUE", 3),
        ("ErBagCupNonBag", "TLC_MODULE_ARGUMENT_ERROR", 2),
    ] {
        let file = root.join(name).join(format!("{name}.tla"));
        for compat_tlc in [false, true] {
            for engine in [Engine::Reference, Engine::Core] {
                for workers in [1, 4] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            workers: Some(workers),
                            seed: Some(0),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )
                    .expect("check")
                    .to_json();
                    assert_eq!(report["tlc"]["code"], 2111, "{report}");
                    assert_eq!(report["tlc"]["reported"], 2111);
                    assert_eq!(report["tlc"]["verified"], true);
                    if workers == 1 {
                        for field in ["distinct", "generated", "depth"] {
                            assert_eq!(
                                report["statistics"][field], length,
                                "{name}/{field}: {report}"
                            );
                        }
                    }
                    assert_eq!(report["tlc"]["exit_status"], 76, "{report}");
                    let doc =
                        TraceDoc::from_json(&report["counterexample"]["trace"]).expect("trace");
                    assert_eq!(doc.states.len(), length);
                    assert_eq!(doc.ending, Some(claim("Inv", ec)), "{report}");
                    let model = SpecModel::load(
                        &file,
                        &file.with_extension("cfg"),
                        &TraceOptions {
                            compat_tlc,
                            run: Some((0, 0)),
                            ..TraceOptions::default()
                        },
                    )
                    .expect("model");
                    assert_eq!(
                        model.validate(&doc).validation,
                        Validation::Valid,
                        "{name} {compat_tlc} {engine:?} {workers}"
                    );
                    let controls = model.controls_with_mode(
                        &doc,
                        if workers == 1 {
                            ControlMode::ShortestBfs
                        } else {
                            ControlMode::Counterexample
                        },
                    );
                    assert!(controls.failures().is_empty(), "{name}: {controls:?}");
                }
            }
        }
    }
}

#[test]
fn named_error_wire_version_is_separate_from_legacy_endings() {
    let named = doc(&[0, 1, 2], claim("Inv", ASSERT));
    let mut wire = named.to_json();
    assert_eq!(wire["version"], 2);
    assert_eq!(TraceDoc::from_json(&wire).expect("v2"), named);
    wire["version"] = json!(1);
    assert!(TraceDoc::from_json(&wire).is_err());
    wire["version"] = json!(3);
    assert!(TraceDoc::from_json(&wire).is_err());
    wire["version"] = json!(2);
    wire["ending"]["kind"] = json!("unknown");
    assert!(TraceDoc::from_json(&wire).is_err());
    for ending in [
        Ending::Invariant("Inv".into()),
        Ending::Assertion,
        Ending::Error {
            ec: Some(ASSERT.into()),
        },
        Ending::Behavior,
        Ending::Deadlock,
    ] {
        let old = doc(&[0, 1, 2], ending);
        let mut wire = old.to_json();
        assert_eq!(wire["version"], 1);
        assert_eq!(TraceDoc::from_json(&wire).expect("legacy v1"), old);
        wire["version"] = json!(2);
        assert_eq!(TraceDoc::from_json(&wire).expect("old ending in v2"), old);
    }
}

#[test]
fn named_error_does_not_relax_constraint_or_effect_refusals() {
    for compat_tlc in [false, true] {
        let opts = TraceOptions {
            compat_tlc,
            ..TraceOptions::default()
        };
        let good = doc(&[0, 1, 2], claim("Inv", ASSERT));
        let cfg = format!("{CFG}CONSTRAINT EarlierFalse\n");
        let constrained = SpecModel::from_source("Audit.tla", SOURCE, &cfg, &opts).expect("model");
        invalid(&constrained, &good, Check::Constraint, 2);
        for body in ["RandomElement({TRUE,FALSE})", "TLCGet(0)"] {
            let source = SOURCE.replace(
                "Assert(x # 2, \"Inv\")",
                &format!("IF x>=0 THEN {body} ELSE TRUE"),
            );
            let m = SpecModel::from_source("Audit.tla", &source, CFG, &opts).expect("model");
            let actual = m.validate(&good).validation;
            assert!(
                matches!(actual, Validation::Unsupported(_)),
                "{body}: {actual:?}"
            );
        }
    }
}
