//! Actual occurrences must not disappear behind an extensional representative.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "adversarial outcome controls"
)]
use dive_api::temporal::{self, Outcome};
use dive_check::{CheckOptions, capture};
use dive_liveness::{
    analyze::{AnalysisError, Limits},
    periodic_backend::Schedule,
};
use dive_store::StoreMode;
use std::sync::Arc;

fn input(source: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let a = dive_sema::analyze_source("Concrete.tla", source, &dive_sema::Options::default());
    assert!(
        a.accepted(),
        "{:?} {:?}",
        a.diagnostics(),
        a.syntax_diagnostics()
    );
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(cfg).into_config().expect("config"),
    )
}
const NATIVE: &str = "---- MODULE Concrete ----\nEXTENDS Naturals\nVARIABLE x,y\nInit == x=[i \\in {\"a\"}|->1] /\\ y \\in BOOLEAN\nNext == x'=[a|->1] /\\ y'=~y\nSpec == Init /\\ [][Next]_<<x,y>> /\\ WF_<<x,y>>(Next)\nP == []<><<x' /= <<0,0>>>>_<<x,y>>\n====\n";

#[test]
fn original_native_action_error_survives_both_stores_widths_and_schedules() {
    let (ir, cfg) = input(NATIVE, "SPECIFICATION Spec\nPROPERTY P\n");
    for tlc in [false, true] {
        for workers in [1, 4] {
            for store in [StoreMode::Exact, StoreMode::Fingerprint] {
                let opts = CheckOptions {
                    workers,
                    store,
                    ..if tlc {
                        CheckOptions::tlc(0, 7)
                    } else {
                        CheckOptions::dive(7)
                    }
                };
                for schedule in [Schedule::Default, Schedule::Final] {
                    let result = temporal::check_with_schedule_with_evidence(
                        &ir,
                        &cfg,
                        &opts,
                        schedule,
                        Limits::default(),
                    );
                    if tlc {
                        let failure = result.expect_err("original record comparison errors");
                        let temporal::Error::Analysis(AnalysisError::Atom {
                            error, source, ..
                        }) = failure.error()
                        else {
                            panic!("{failure:?}")
                        };
                        assert_eq!(error.class(), dive_eval::ErrorClass::Eval);
                        assert_eq!(error.ec().code(), 1000);
                        assert!(error.message().contains("equality of record"));
                        assert!(source.location.contains("Concrete"));
                        let evidence = failure.exploration().expect("actual completed evidence");
                        assert_eq!(
                            (
                                evidence.stats.generated,
                                evidence.stats.distinct,
                                evidence.stats.depth,
                                evidence.stats.left_on_queue
                            ),
                            (4, 2, 1, 0)
                        );
                        assert_eq!(evidence.workers.used, workers);
                    } else {
                        let r = result.expect("coherent extensional positive control");
                        let Outcome::Holds(c) = r.outcome() else {
                            panic!("{r:?}")
                        };
                        assert_eq!(
                            (
                                c.stats.generated,
                                c.stats.distinct,
                                c.stats.depth,
                                c.stats.left_on_queue
                            ),
                            (4, 2, 1, 0)
                        );
                        assert_eq!(c.workers.used, workers);
                    }
                }
            }
        }
    }
}

#[test]
fn incoming_and_initial_state_variants_keep_typed_errors_and_initial_order() {
    let incoming = NATIVE.replace(
        "P == []<><<x' /= <<0,0>>>>_<<x,y>>",
        "P == []<>(x /= <<0,0>>)",
    );
    let initial = "---- MODULE Concrete ----\nEXTENDS Naturals\nVARIABLE x,y\nInit == (x=[i \\in {\"a\"}|->1] /\\ y=FALSE) \\/ (x=[a|->1] /\\ y=FALSE)\nNext == UNCHANGED <<x,y>>\nP == []<>(x /= <<0,0>>)\n====\n";
    let reverse = initial.replace(
        "(x=[i \\in {\"a\"}|->1] /\\ y=FALSE) \\/ (x=[a|->1] /\\ y=FALSE)",
        "(x=[a|->1] /\\ y=FALSE) \\/ (x=[i \\in {\"a\"}|->1] /\\ y=FALSE)",
    );
    for source in [&incoming, initial, &reverse] {
        for workers in [1, 4] {
            let cfgtext = if source == incoming {
                "SPECIFICATION Spec\nPROPERTY P\n"
            } else {
                "INIT Init\nNEXT Next\nPROPERTY P\n"
            };
            let (ir, cfg) = input(source, cfgtext);
            let opts = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 7)
            };
            let failure = temporal::check_with_schedule_with_evidence(
                &ir,
                &cfg,
                &opts,
                Schedule::Final,
                Limits::default(),
            )
            .expect_err("state variant errors");
            assert!(
                matches!(failure.error(),temporal::Error::Analysis(AnalysisError::Atom {error,..}) if error.class()==dive_eval::ErrorClass::Eval && error.ec().code()==1000),
                "{failure:?}"
            );
        }
    }
    let (ir, cfg) = input(initial, "INIT Init\nNEXT Next\n");
    let opts = CheckOptions::tlc(0, 7);
    let model = dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("bind");
    let capture::Exploration::Complete(c) =
        capture::explore_with_evidence(&model, &opts, capture::Limits::default()).expect("capture")
    else {
        panic!("capture")
    };
    assert_eq!(c.graph.initials.len(), 2);
    assert_eq!(c.graph.roots, [0]);
    assert_eq!(c.stats.initial_generated, 2);
    assert_eq!(c.stats.generated, 3);
    assert_eq!(c.stats.distinct, 1);
    let flags: Vec<_> = c
        .graph
        .initials
        .iter()
        .map(|i| match &i.state[0] {
            Some(dive_value::Value::Fcn(f)) => f.is_record_literal(),
            _ => panic!("function"),
        })
        .collect();
    assert_eq!(flags, [false, true]);
}

const SETS: &str = "---- MODULE Concrete ----\nEXTENDS Naturals,TLC\nVARIABLE x,y,z\nInit == x=1..2 /\\ y \\in BOOLEAN /\\ z=0\nNext == x'={1,2} /\\ y'=~y /\\ z'=0\nSpec == Init /\\ [][Next]_<<x,y,z>> /\\ WF_<<x,y,z>>(Next)\nP == []<>(y=TRUE)\nAC == TRUE\n====\n";

#[test]
fn coherent_set_aliases_hold_without_lifting_replay_permissions() {
    let (ir, cfg) = input(SETS, "SPECIFICATION Spec\nPROPERTY P\n");
    for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for store in [StoreMode::Exact, StoreMode::Fingerprint] {
            for workers in [1, 4] {
                let opts = CheckOptions {
                    workers,
                    store,
                    ..base.clone()
                };
                let r = temporal::check_with_schedule_with_evidence(
                    &ir,
                    &cfg,
                    &opts,
                    Schedule::Final,
                    Limits::default(),
                )
                .expect("coherent set occurrences");
                assert!(matches!(r.outcome(), Outcome::Holds(_)), "{r:?}");
            }
        }
    }
}

#[test]
fn boolean_atom_aliases_and_next_constraint_projection_refuse_at_runtime() {
    let cases = [
        SETS.replace("P == []<>(y=TRUE)", "P == []<>(ToString(x)=\"1..2\")"),
        SETS.replace("P == []<>(y=TRUE)", "P == []<><<ToString(x')=\"1..2\">>_y"),
        SETS.replace("z'=0", "z'=IF ToString(x)=\"1..2\" THEN 0 ELSE 1"),
        SETS.replace("AC == TRUE", "AC == ToString(x)=\"1..2\""),
        SETS.replace("AC == TRUE", "AC == ToString(x')=\"{1, 2}\""),
        SETS.replace("z'=0", "z'=0 /\\ ToString(x')=\"{1, 2}\""),
    ];
    for (index, source) in cases.iter().enumerate() {
        // Fairness itself labels Next and correctly catches these mutants
        // earlier. Remove it in the new row/projection controls so the actual
        // bounded replay obligation, rather than that atom, is exercised.
        let source = if index >= 2 {
            source.replace(" /\\ WF_<<x,y,z>>(Next)", "")
        } else {
            source.clone()
        };
        for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for workers in [1, 4] {
                let cfgtext = if index == 3 || index == 4 {
                    "SPECIFICATION Spec\nPROPERTY P\nACTION_CONSTRAINT AC\n"
                } else {
                    "SPECIFICATION Spec\nPROPERTY P\n"
                };
                let (ir, cfg) = input(&source, cfgtext);
                let opts = CheckOptions {
                    workers,
                    ..base.clone()
                };
                let failure = temporal::check_with_schedule_with_evidence(
                    &ir,
                    &cfg,
                    &opts,
                    Schedule::Final,
                    Limits::default(),
                )
                .expect_err("incoherent actual occurrences");
                assert!(
                    matches!(
                        failure.error(),
                        temporal::Error::Analysis(AnalysisError::Concrete { .. })
                    ),
                    "case{index} {failure:?}"
                );
                assert!(failure.exploration().is_some());
                if opts.eval.ctx == dive_value::Ctx::dive() {
                    let temporal::Error::Analysis(AnalysisError::Concrete { detail, .. }) =
                        failure.error()
                    else {
                        panic!("{failure:?}")
                    };
                    let expected = match index {
                        0 => "state atom differs",
                        1 => "representative word projection",
                        2 => "accepted concrete occurrence",
                        3 => "accepted successor row",
                        _ => "reject representative word projection",
                    };
                    assert!(detail.contains(expected), "case{index} {failure:?}");
                }
            }
        }
    }
}

#[test]
fn initial_alias_truth_stutter_sources_and_row_order_are_operational_controls() {
    let initial = SETS
        .replace(
            "Init == x=1..2 /\\ y \\in BOOLEAN /\\ z=0",
            "Init == ((x=1..2) \\/ (x={1,2})) /\\ y \\in BOOLEAN /\\ z=0",
        )
        .replace(
            "Next == x'={1,2} /\\ y'=~y /\\ z'=0",
            "Next == UNCHANGED <<x,y,z>>",
        )
        .replace("P == []<>(y=TRUE)", "P == []<>(ToString(x)=\"1..2\")");
    let reverse = initial.replace("(x=1..2) \\/ (x={1,2})", "(x={1,2}) \\/ (x=1..2)");
    let stutter = SETS
        .replace(
            "Init == x=1..2 /\\ y \\in BOOLEAN /\\ z=0",
            "Init == x=1..2 /\\ y=FALSE /\\ z=0",
        )
        .replace("y'=~y", "y'=y")
        .replace("P == []<>(y=TRUE)", "P == []<>(ToString(x)=\"1..2\")");
    let order = SETS.replace("Next == x'={1,2} /\\ y'=~y /\\ z'=0",
        "A == x'={1,2} /\\ y'=TRUE /\\ z'=0\nB == x'={1,2} /\\ y'=FALSE /\\ z'=0\nNext == IF ToString(x)=\"1..2\" THEN A \\/ B ELSE B \\/ A");
    for (source, expected) in [
        (&initial, "state atom differs"),
        (&reverse, "state atom differs"),
        (&stutter, "state atom differs"),
        (&order, "accepted concrete occurrence"),
    ] {
        let (ir, cfg) = input(source, "SPECIFICATION Spec\nPROPERTY P\n");
        for workers in [1, 4] {
            for store in [StoreMode::Exact, StoreMode::Fingerprint] {
                let opts = CheckOptions {
                    workers,
                    store,
                    ..CheckOptions::dive(7)
                };
                let failure = temporal::check_with_schedule_with_evidence(
                    &ir,
                    &cfg,
                    &opts,
                    Schedule::Final,
                    Limits::default(),
                )
                .expect_err("actual occurrence coherence");
                let temporal::Error::Analysis(AnalysisError::Concrete { detail, .. }) =
                    failure.error()
                else {
                    panic!("{failure:?}")
                };
                assert!(detail.contains(expected), "{failure:?}");
            }
        }
    }
}

#[test]
fn additional_concrete_work_and_initial_storage_are_really_bounded() {
    let (ir, cfg) = input(SETS, "SPECIFICATION Spec\nPROPERTY P\n");
    let opts = CheckOptions::dive(7);
    let limits = Limits {
        atom_cells: 100,
        ..Limits::default()
    };
    let failure =
        temporal::check_with_schedule_with_evidence(&ir, &cfg, &opts, Schedule::Final, limits)
            .expect_err("extra alias work counts");
    assert!(
        matches!(
            failure.error(),
            temporal::Error::Analysis(AnalysisError::Limit(_))
        ),
        "{failure:?}"
    );
    let limits = Limits {
        capture: capture::Limits {
            edges: 1,
            ..capture::Limits::default()
        },
        ..Limits::default()
    };
    let failure =
        temporal::check_with_schedule_with_evidence(&ir, &cfg, &opts, Schedule::Final, limits)
            .expect("raw bound route");
    let Outcome::Safety(result) = failure.outcome() else {
        panic!("{failure:?}")
    };
    assert!(matches!(
        result.verdict,
        dive_check::Verdict::ResourceLimit { .. }
    ));
    assert_eq!(result.stats.expect("actual init work").generated, 2);
}
