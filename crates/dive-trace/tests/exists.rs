//! Finite-cover soundness controls, independent of the successor enumerator.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_eval::St;
use dive_model::{Model, Pred};
use dive_trace::{
    State,
    exists::{Decision, ExistsError, Limits, exists_successor},
};
use dive_value::Value;
use std::sync::Arc;

fn model(body: &str) -> Model {
    let src = format!(
        "---- MODULE Cover ----\nEXTENDS Integers\nVARIABLES x, y\nInit == x = 0 /\\ y = 0\nNext == UNCHANGED <<x, y>>\n{body}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Cover.tla", &src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}
fn pred(m: &Model, name: &str) -> Pred {
    let d = m.ir.root_def(name).expect(name);
    Pred::new(m.ir.def(d).body, m.evaluator.scope(d))
}
fn state(x: i64, y: i64) -> State {
    Arc::from([Some(x.into()), Some(y.into())])
}
fn enabled(d: &Decision) -> bool {
    matches!(d, Decision::Enabled { .. })
}

#[test]
fn finite_relations_match_exhaustive_assignments_and_return_real_witnesses() {
    let m = model(
        "A == x' \\in 0..2 /\\ y' \\in 0..2 /\\ x' + y' = x + y\n\
                   B == (x' = 0 /\\ y' = 1) \\/ (x' = 2 /\\ y' = 0)\n\
                   C == x' \\in {} /\\ y' = y\n\
                   D == IF x = 0 THEN UNCHANGED <<x,y>> ELSE x' = 1 /\\ UNCHANGED y\n\
                   E == x' \\in 0..2 /\\ y' \\in 0..2 /\\ ~(x' = y')\n\
                   F == (x' = 0) \\/ (x' = 2)\n\
                   G == (x' = 0 /\\ FALSE) \\/ (x' = 2)\n\
                   H == x' \\in 0..2 /\\ x' = 5\n\
                   I == ((x' = 0 /\\ y' = 1) \\/ (x' = 2 /\\ y' = 0)) /\\ x' = 2 /\\ y' = 0\n",
    );
    for x in -1..=4 {
        for y in -1..=4 {
            let before = state(x, y);
            for name in ["A", "B", "C", "D", "E", "F", "G", "H", "I"] {
                let p = pred(&m, name);
                let brute = (0..=2).any(|a| {
                    (0..=2).any(|b| {
                        m.eval(&p, &St::action(before.clone(), state(a, b)))
                            .expect("evaluate")
                            == Value::Bool(true)
                    })
                });
                let result =
                    exists_successor(&m, &p, &before, None, Limits::default()).expect(name);
                // D's unchanged values may be outside the finite brute-force box.
                let expected = if name == "D" { true } else { brute };
                assert_eq!(enabled(&result), expected, "{name}, {x}, {y}");
                if let Decision::Enabled { witness } = result {
                    assert_eq!(
                        m.eval(&p, &St::action(before.clone(), witness))
                            .expect("witness"),
                        Value::Bool(true)
                    );
                }
            }
        }
    }
}

#[test]
fn constraints_and_reachable_states_never_restrict_enabled() {
    let m = model("A == x' = 99\nSub == x\n");
    let p = pred(&m, "A");
    let sub = pred(&m, "Sub");
    assert!(enabled(
        &exists_successor(&m, &p, &state(0, 0), Some(&sub), Limits::default())
            .expect("outside Init range")
    ));
    assert!(!enabled(
        &exists_successor(&m, &p, &state(99, 0), Some(&sub), Limits::default())
            .expect("only stutter")
    ));
}

#[test]
fn incomplete_covers_and_budgets_are_refusals_never_disabled() {
    let m = model(
        "A == x' > 0\nB == x' \\in 0..2 /\\ y' \\in 0..2\n\
                   C == x' = 0 \\/ y' = 1\nD == x' = y'\nE == x' \\in Nat\n",
    );
    for name in ["A", "C", "D", "E"] {
        assert!(
            matches!(
                exists_successor(&m, &pred(&m, name), &state(0, 0), None, Limits::default()),
                Err(ExistsError::Unsupported(_))
            ),
            "{name}"
        );
    }
    for limits in [
        Limits {
            assignments: 8,
            ..Limits::default()
        },
        Limits {
            domain_values: 2,
            ..Limits::default()
        },
        Limits {
            expressions: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            exists_successor(&m, &pred(&m, "B"), &state(0, 0), None, limits),
            Err(ExistsError::Limit(_))
        ));
    }
    assert!(matches!(
        exists_successor(
            &m,
            &pred(&m, "B"),
            &Arc::from([None, None]),
            None,
            Limits::default()
        ),
        Err(ExistsError::Invalid(_))
    ));
}

#[test]
fn missing_subscript_bounds_cannot_certify_fair_disabledness() {
    let m = model("A == x' = 0\nSub == y\nFalseA == FALSE\n");
    let sub = pred(&m, "Sub");
    assert!(matches!(
        exists_successor(
            &m,
            &pred(&m, "A"),
            &state(0, 0),
            Some(&sub),
            Limits::default()
        ),
        Err(ExistsError::Unsupported(_))
    ));
    assert!(matches!(
        exists_successor(
            &m,
            &pred(&m, "FalseA"),
            &state(0, 0),
            Some(&sub),
            Limits::default()
        )
        .expect("false covers all assignments"),
        Decision::Disabled
    ));
}

#[test]
fn pinned_tlc_enabledness_matches_every_finite_cover_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source =
        std::fs::read_to_string(root.join("specs/EnabledFiniteCover/EnabledFiniteCover.tla"))
            .expect("source");
    let cfg = std::fs::read_to_string(root.join("specs/EnabledFiniteCover/EnabledFiniteCover.cfg"))
        .expect("cfg");
    let a = dive_sema::analyze_source(
        "EnabledFiniteCover.tla",
        &source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(&cfg).into_config().expect("config");
    let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    let oracle = root.join("tlc/tlc-2026.10.06.014338");
    assert_eq!(
        std::fs::read_to_string(oracle.join("EnabledFiniteCover.exit"))
            .expect("exit")
            .trim(),
        "0"
    );
    let raw = std::fs::read_to_string(oracle.join("EnabledFiniteCover.stdout")).expect("output");
    assert!(raw.contains("72 states generated, 36 distinct states found"));
    for x in -1..=4 {
        for y in -1..=4 {
            let d = exists_successor(
                &m,
                &pred(&m, "Relation"),
                &state(x, y),
                None,
                Limits::default(),
            )
            .expect("decide");
            assert_eq!(enabled(&d), (0..=4).contains(&(x + y)), "{x}, {y}");
        }
    }
}

#[test]
fn constant_action_calls_preserve_contexts_and_return_real_witnesses() {
    let m = model(
        "Relation(a,b) == x' \\in {a,b} /\\ y' \\in 0..1 /\\ x'+y'=x+y\nWrap(pair) == Relation(pair[1],pair[2])\nR01 == Wrap(<<0,1>>)\nR12 == Wrap(<<1,2>>)\nUnion == Relation(0,1) \\/ Relation(1,2)\n",
    );
    for x in -1..=4 {
        for y in -1..=4 {
            let before = state(x, y);
            for (name, min, max) in [("R01", 0, 2), ("R12", 1, 3), ("Union", 0, 3)] {
                let p = pred(&m, name);
                let result =
                    exists_successor(&m, &p, &before, None, Limits::default()).expect(name);
                let brute = (0..=2).any(|a| {
                    (0..=1).any(|b| {
                        m.eval(&p, &St::action(before.clone(), state(a, b)))
                            .expect("evaluate")
                            == Value::Bool(true)
                    })
                });
                assert_eq!(enabled(&result), brute, "{name} {x} {y}");
                assert_eq!(
                    enabled(&result),
                    (min..=max).contains(&(x + y)),
                    "{name} {x} {y}"
                );
                if let Decision::Enabled { witness } = result {
                    assert_eq!(
                        m.eval(&p, &St::action(before.clone(), witness))
                            .expect("witness"),
                        Value::Bool(true)
                    );
                }
            }
        }
    }
}

#[test]
fn scoped_covers_keep_missing_bounds_and_lazy_arguments_as_refusals() {
    let m = model(
        "Pick(i) == IF i=0 THEN x'=0 ELSE y'=0\nUnion == Pick(0) \\/ Pick(1)\nStateArg == Pick(x)\nUnbounded(i) == x'>i\nNoCover == Unbounded(0)\nMove(i) == x'=i /\\ UNCHANGED y\nGood == Move(1)\nSub == x\n",
    );
    for name in ["Union", "StateArg", "NoCover"] {
        assert!(
            matches!(
                exists_successor(&m, &pred(&m, name), &state(0, 0), None, Limits::default()),
                Err(ExistsError::Unsupported(_))
            ),
            "{name}"
        );
    }
    let sub = pred(&m, "Sub");
    assert!(enabled(
        &exists_successor(
            &m,
            &pred(&m, "Good"),
            &state(0, 0),
            Some(&sub),
            Limits::default()
        )
        .expect("changed")
    ));
    assert!(matches!(
        exists_successor(
            &m,
            &pred(&m, "Good"),
            &state(1, 0),
            Some(&sub),
            Limits::default()
        )
        .expect("only stutter"),
        Decision::Disabled
    ));
    let limits = Limits {
        expressions: 3,
        ..Limits::default()
    };
    assert!(matches!(
        exists_successor(&m, &pred(&m, "Good"), &state(0, 0), None, limits),
        Err(ExistsError::Limit(_))
    ));
}

#[test]
fn pinned_scoped_enabledness_agrees_on_all_initial_states() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source =
        std::fs::read_to_string(root.join("specs/EnabledScopedCover/EnabledScopedCover.tla"))
            .expect("source");
    let cfg = std::fs::read_to_string(root.join("specs/EnabledScopedCover/EnabledScopedCover.cfg"))
        .expect("cfg");
    let a = dive_sema::analyze_source(
        "EnabledScopedCover.tla",
        &source,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{a:?}");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(&cfg).into_config().expect("config");
    let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    let oracle = root.join("tlc/tlc-2026.10.06.014338");
    assert_eq!(
        std::fs::read_to_string(oracle.join("EnabledScopedCover.exit"))
            .expect("exit")
            .trim(),
        "0"
    );
    let stdout = std::fs::read_to_string(oracle.join("EnabledScopedCover.stdout")).expect("stdout");
    assert!(stdout.contains("72 states generated, 36 distinct states found"));
    for x in -1..=4 {
        for y in -1..=4 {
            for (name, min, max) in [("R01", 0, 2), ("R12", 1, 3), ("Union", 0, 3)] {
                let result =
                    exists_successor(&m, &pred(&m, name), &state(x, y), None, Limits::default())
                        .expect(name);
                assert_eq!(
                    enabled(&result),
                    (min..=max).contains(&(x + y)),
                    "{name} {x} {y}"
                );
            }
        }
    }
}
