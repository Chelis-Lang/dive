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
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}
fn pred(m: &Model, name: &str) -> Pred {
    let d = m.ir.root_def(name).expect(name);
    Pred::new(m.ir.def(d).body, m.evaluator.scope(d).expect("valid scope"))
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
                if let Decision::Enabled { witness, .. } = result {
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
                if let Decision::Enabled { witness, .. } = result {
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

#[test]
fn finite_quantified_actions_match_every_small_assignment() {
    let m = model(
        "Move(i) == x'=i /\\ y' \\in 0..2 /\\ x'+y'=x+y\n\
         Any == \\E i \\in 0..2 : Move(i)\n\
         All == \\A i \\in {0,1} : x'=i\n\
         One == \\A i \\in {1} : Move(i)\n\
         EmptyAny == \\E i \\in {} : x'>y'\n\
         EmptyAll == \\A i \\in {} : x'>y'\n\
         Pair == \\E <<i,j>> \\in {<<0,1>>,<<2,0>>} : x'=i /\\ y'=j /\\ i+j=x+y\n\
         Two == \\E i \\in 0..1, j \\in 0..1 : x'=i /\\ y'=j /\\ i+j=x+y\n\
         StateDomain == \\E i \\in {x,y} : x'=i /\\ y'=0 /\\ x'+y'=1\n\
         Both == (\\A i \\in {0} : x'=i) /\\ (\\A j \\in {1} : y'=j)\n",
    );
    for x in -1..=4 {
        for y in -1..=4 {
            let before = state(x, y);
            for name in [
                "Any",
                "All",
                "One",
                "EmptyAny",
                "EmptyAll",
                "Pair",
                "Two",
                "StateDomain",
                "Both",
            ] {
                let p = pred(&m, name);
                let brute = (-1..=4).any(|a| {
                    (-1..=4).any(|b| {
                        m.eval(&p, &St::action(before.clone(), state(a, b)))
                            .expect("relation")
                            == Value::Bool(true)
                    })
                });
                let result =
                    exists_successor(&m, &p, &before, None, Limits::default()).expect(name);
                assert_eq!(enabled(&result), brute, "{name} {x} {y}");
                if let Decision::Enabled { witness, .. } = result {
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
fn quantified_missing_domains_correlations_and_limits_never_guess_disabled() {
    let m = model(
        "Pick(i) == IF i=0 THEN x'=0 ELSE y'=0\n\
         Missing == \\E i \\in {0,1} : Pick(i)\n\
         Infinite == \\E i \\in Nat : x'=i\n\
         Dependent == \\E i \\in {0,1} : \\E j \\in {i} : x'=j\n\
         PrimeDomain == \\E i \\in {x'} : x'=i\n\
         Many == \\E i \\in 0..8 : x'=i\n\
         Joint == \\E i,j \\in 0..2 : x'=i /\\ y'=j /\\ i+j=4\n\
         Grid == \\E i,j \\in 0..2 : x'=i /\\ y'=j\n",
    );
    for name in ["Missing", "Infinite", "PrimeDomain"] {
        assert!(
            matches!(
                exists_successor(&m, &pred(&m, name), &state(0, 0), None, Limits::default()),
                Err(ExistsError::Unsupported(_))
            ),
            "{name}"
        );
    }
    for (name, limits) in [
        (
            "Many",
            Limits {
                domain_values: 8,
                ..Limits::default()
            },
        ),
        (
            "Joint",
            Limits {
                domain_values: 8,
                ..Limits::default()
            },
        ),
        (
            "Grid",
            Limits {
                assignments: 8,
                ..Limits::default()
            },
        ),
        (
            "Joint",
            Limits {
                expressions: 2,
                ..Limits::default()
            },
        ),
    ] {
        assert!(
            matches!(
                exists_successor(&m, &pred(&m, name), &state(0, 0), None, limits),
                Err(ExistsError::Limit(_))
            ),
            "{name}"
        );
    }
    assert!(enabled(
        &exists_successor(
            &m,
            &pred(&m, "Dependent"),
            &state(0, 0),
            None,
            Limits::default()
        )
        .expect("nested dependent domain")
    ));
    let result = exists_successor(
        &m,
        &pred(&m, "Joint"),
        &state(0, 0),
        None,
        Limits::default(),
    )
    .expect("correlated");
    let Decision::Enabled { witness, .. } = result else {
        panic!("i=j=2 satisfies relation");
    };
    assert_eq!(witness, state(2, 2));
}

#[test]
fn pinned_quantified_enabledness_matches_all_36_states() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let name = "EnabledQuantifiedCover";
    let src =
        std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
    let config =
        std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
    let analysis =
        dive_sema::analyze_source(&format!("{name}.tla"), &src, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(&config).into_config().expect("cfg");
    let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    let oracle = root.join("tlc-2026.10.06.014338");
    let exit = std::fs::read_to_string(oracle.join(format!("{name}.exit"))).expect("exit");
    assert_eq!(exit.trim(), "0");
    let stdout =
        std::fs::read_to_string(oracle.join(format!("{name}.stdout"))).expect("transcript");
    assert!(stdout.contains("72 states generated, 36 distinct states found"));
    for x in -1..=4 {
        for y in -1..=4 {
            for (name, expected) in [
                ("Any", (0..=4).contains(&(x + y))),
                ("All", false),
                ("EmptyAny", false),
            ] {
                let d =
                    exists_successor(&m, &pred(&m, name), &state(x, y), None, Limits::default())
                        .expect(name);
                assert_eq!(enabled(&d), expected, "{name} {x} {y}");
            }
        }
    }
}

#[test]
fn let_bound_necessary_covers_preserve_outer_values_and_real_witnesses() {
    let m = model(
        "A == LET n==0 IN x'=n /\\ y'=1\n\
        B == LET sum==x+y Good==x' \\in 0..2 /\\ y' \\in 0..2 /\\ x'+y'=sum IN Good\n\
        C == \\E i \\in 0..2 : LET Move(q)==x'=q /\\ y'=1 IN Move(i)\n\
        D == LET Keep==UNCHANGED <<x,y>> IN Keep\n\
        Missing == LET Pick(q)==IF q=0 THEN x'=0 ELSE y'=0 IN Pick(0) \\/ Pick(1)\n\
        StateArg == LET n==x Move(q)==x'=q IN Move(n)\n\
        Unbounded == LET n==0 IN x'>n\n",
    );
    for x in -1..=4 {
        for y in -1..=4 {
            let before = state(x, y);
            for name in ["A", "B", "C", "D"] {
                let p = pred(&m, name);
                let brute = (-1..=4).any(|a| {
                    (-1..=4).any(|b| {
                        m.eval(&p, &St::action(before.clone(), state(a, b)))
                            .expect("relation")
                            == Value::Bool(true)
                    })
                });
                let result =
                    exists_successor(&m, &p, &before, None, Limits::default()).expect(name);
                assert_eq!(enabled(&result), brute, "{name} {x} {y}");
                if let Decision::Enabled { witness, .. } = result {
                    assert_eq!(
                        m.eval(&p, &St::action(before.clone(), witness))
                            .expect("original relation"),
                        Value::Bool(true)
                    );
                }
            }
        }
    }
    for name in ["Missing", "StateArg", "Unbounded"] {
        assert!(
            matches!(
                exists_successor(&m, &pred(&m, name), &state(0, 0), None, Limits::default()),
                Err(ExistsError::Unsupported(_))
            ),
            "{name}"
        );
    }
    assert!(matches!(
        exists_successor(
            &m,
            &pred(&m, "B"),
            &state(0, 0),
            None,
            Limits {
                expressions: 3,
                ..Limits::default()
            }
        ),
        Err(ExistsError::Limit(_))
    ));
}

fn selectseq_model(body: &str, options: &dive_model::BindOptions) -> Model {
    let src = format!(
        "---- MODULE SelectCover ----\nEXTENDS Integers, Sequences\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == UNCHANGED <<x,y>>\nAlways(z)==TRUE\nPure(S)==SelectSeq(S,LAMBDA z:z=x)\n{body}\n===="
    );
    let analysis =
        dive_sema::analyze_source("SelectCover.tla", &src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, options).expect("bind")
}

#[test]
fn selectseq_independent_lazy_helper_subscript_requires_sequence_and_callback_state_covers() {
    use dive_trace::exists::exists_successor_pure;
    for options in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::default(),
    ] {
        let m = selectseq_model(
            "A==x'=1 /\\ y'=1\nMissingX==y'=1\nMissingY==x'=1\nSub==Pure(<<y>>)\nHidden(S)==LET T==x' IN SelectSeq(S,LAMBDA z:z=T)\nBad==x'=1 /\\ y'=1 /\\ Hidden(<<y>>) = <<y>>",
            &options,
        );
        let sub = pred(&m, "Sub");
        let d = exists_successor_pure(
            &m,
            &pred(&m, "A"),
            &state(0, 0),
            Some(&sub),
            Limits::default(),
            4096,
        )
        .expect("complete cover includes sequence and captured callback state dependencies");
        let Decision::Enabled { witness, .. } = d else {
            panic!("concrete changed witness")
        };
        assert_eq!(witness, state(1, 1));
        for name in ["MissingX", "MissingY", "Bad"] {
            assert!(
                matches!(
                    exists_successor_pure(
                        &m,
                        &pred(&m, name),
                        &state(0, 0),
                        Some(&sub),
                        Limits::default(),
                        4096
                    ),
                    Err(ExistsError::Unsupported(_))
                ),
                "{name}"
            );
        }
        assert!(
            matches!(
                exists_successor(
                    &m,
                    &pred(&m, "A"),
                    &state(0, 0),
                    Some(&sub),
                    Limits::default()
                ),
                Err(ExistsError::Unsupported(_))
            ),
            "ordinary API keeps its original state-helper refusal"
        );
    }
}

#[test]
fn selectseq_independent_search_shares_aggregate_work_and_refuses_exhaustion_as_a_limit() {
    use dive_trace::exists::exists_successor_pure;
    let items = std::iter::repeat_n("x", 20).collect::<Vec<_>>().join(",");
    let m = selectseq_model(
        &format!("A==x'=1 /\\ y'=1\nSub==SelectSeq(<<{items}>>,Always)"),
        &dive_model::BindOptions::tlc(),
    );
    let sub = pred(&m, "Sub");
    let st = St::state(state(0, 0));
    let minimum = (1..4096)
        .find(|limit| {
            m.evaluator
                .eval_scoped_pure(&sub.scope, sub.expr, &st, *limit)
                .is_ok()
        })
        .expect("one subscript fits");
    for _ in 0..2 {
        assert!(
            matches!(
                exists_successor_pure(
                    &m,
                    &pred(&m, "A"),
                    &state(0, 0),
                    Some(&sub),
                    Limits::default(),
                    minimum
                ),
                Err(ExistsError::Limit(_))
            ),
            "before/after share context; never Disabled or Invalid on exhaustion"
        );
        assert!(
            matches!(
                exists_successor_pure(
                    &m,
                    &pred(&m, "A"),
                    &state(0, 0),
                    Some(&sub),
                    Limits::default(),
                    4096
                ),
                Ok(Decision::Enabled { .. })
            ),
            "fresh search after exhaustion"
        );
    }
    let mut foreign = selectseq_model("A==x'=1 /\\ y'=1", &dive_model::BindOptions::tlc());
    let foreign_action = pred(&foreign, "A");
    foreign.evaluator = m.evaluator.clone();
    assert!(
        matches!(
            exists_successor_pure(
                &foreign,
                &foreign_action,
                &state(0, 0),
                None,
                Limits::default(),
                4096
            ),
            Err(ExistsError::Unsupported(_))
        ),
        "foreign model/evaluator context"
    );
}

#[test]
fn selectseq_independent_reached_values_keep_first_error_and_stored_profile_refusals() {
    use dive_trace::exists::exists_successor_pure;
    let m = selectseq_model(
        "Priority(S)==SelectSeq(S,LAMBDA z:z[1]=0)\nA==x'=x /\\ y'=y /\\ Priority(x) = <<>>\nOwned==x'=x /\\ y'=y /\\ Pure(x) = x",
        &dive_model::BindOptions::tlc(),
    );
    let large = Value::tuple(vec![0.into(); 30_000]);
    let first: State = Arc::from([
        Some(Value::tuple(vec![0.into(), large.clone()])),
        Some(0.into()),
    ]);
    let error = exists_successor_pure(&m, &pred(&m, "A"), &first, None, Limits::default(), 4096)
        .expect_err("first reached callback scalar error");
    assert!(
        matches!(error,ExistsError::Invalid(ref text) if text.contains("2154")),
        "{error}"
    );
    let reverse: State = Arc::from([Some(Value::tuple(vec![large, 0.into()])), Some(0.into())]);
    assert!(
        matches!(
            exists_successor_pure(&m, &pred(&m, "A"), &reverse, None, Limits::default(), 4096),
            Err(ExistsError::Limit(_))
        ),
        "first reached resource value wins in reverse order"
    );
    let wrong_order = Value::set(vec![0.into()], dive_value::Ctx::dive()).expect("foreign order");
    let wrong: State = Arc::from([Some(Value::tuple(vec![wrong_order])), Some(0.into())]);
    let error = exists_successor_pure(
        &m,
        &pred(&m, "Owned"),
        &wrong,
        None,
        Limits::default(),
        4096,
    )
    .expect_err("stored profile checked before callback comparison");
    assert!(
        matches!(error,ExistsError::Unsupported(ref text) if text.contains("order")),
        "{error}"
    );
}

#[test]
fn selectseq_independent_synthetic_warm_cache_isolation_enforces_fresh_search_budget() {
    use dive_trace::exists::exists_successor_pure;
    let items = std::iter::repeat_n("0", 20).collect::<Vec<_>>().join(",");
    let mut m = selectseq_model(
        &format!(
            "Selected==SelectSeq(<<{items}>>,Always)\nWrapper==Selected\nA==x'=1 /\\ y'=1 /\\ Selected = <<{items}>>"
        ),
        &dive_model::BindOptions::tlc(),
    );
    // Synthetic cache-mechanics coverage only: actual bound startup values are
    // authoritative and retain their original behavior. Rebuild scopes after
    // installing a source-identical raw evaluator with no prebound startup data.
    let mut inputs = dive_eval::Model::new(m.ir.clone());
    inputs.names = m.names.clone();
    m.evaluator = dive_eval::Evaluator::new(inputs, dive_eval::Options::tlc())
        .expect("raw source-identical evaluator");
    // Snapshot only the synthetic evaluator's already-owned initial namespace
    // before any warm/evaluation operation. No authoritative values are added.
    m.evaluator.seal_startup_names();
    let selected = pred(&m, "Wrapper");
    let warm = m
        .evaluator
        .eval_scoped(&selected.scope, selected.expr, &St::constant())
        .expect("warm result cache");
    let a = pred(&m, "A");
    for _ in 0..2 {
        let result = exists_successor_pure(&m, &a, &state(0, 0), None, Limits::default(), 1);
        assert!(
            matches!(result, Err(ExistsError::Limit(_))),
            "whole enclosing warm cache cannot bypass callback work: {result:?}"
        );
        assert!(
            matches!(
                exists_successor_pure(&m, &a, &state(0, 0), None, Limits::default(), 4096),
                Ok(Decision::Enabled { .. })
            ),
            "fresh invocation after exhaustion"
        );
    }
    let after = m
        .evaluator
        .eval_scoped(&selected.scope, selected.expr, &St::constant())
        .expect("caller cache retained");
    assert!(
        warm.as_fcn()
            .expect("tuple")
            .ptr_eq(after.as_fcn().expect("tuple"))
    );
}

fn configured_model(target: &str) -> Model {
    let source = "---- MODULE ConfiguredCover ----\nEXTENDS Naturals, Sequences\nCONSTANT Op(_,_)\nVARIABLES x,y\nInit==x=0 /\\ y=0\nNext==UNCHANGED <<x,y>>\nCorrelated(a,b)==a \\in 0..2 /\\ b \\in 0..2 /\\ a+b=x+y\nIncomplete(a,b)==a>0 /\\ b=0\nUnbounded(a,b)==a \\in Nat /\\ b=0\nIgnore(a,unused)==a=1\nScoped(a,b)==a=b+1\nNative(a,b)==a=Len(b)\nStateRhs==Op(x',y)\nOtherRhs==Op(x',x)\nPrimedRhs==Op(x',y')\nNativeCall==Op(x',<<0>>)\nA==Op(x',y')\nUnused==Op(x',\\A i \\in (1 \\div 0):i=0)\n====\n";
    let analysis = dive_sema::analyze_source(
        "ConfiguredCover.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse(&format!("INIT Init\nNEXT Next\nCONSTANT Op <- {target}\n"))
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions::tlc(),
    )
    .expect("configured model")
}

#[test]
fn configured_lazy_covers_match_original_relation_and_keep_incomplete_refusals() {
    let m = configured_model("Correlated");
    let p = pred(&m, "A");
    for x in 0..=3 {
        for y in 0..=3 {
            let before = state(x, y);
            let expected = (0..=2).any(|a| {
                (0..=2).any(|b| {
                    m.eval(&p, &St::action(before.clone(), state(a, b)))
                        .expect("original relation")
                        == Value::Bool(true)
                })
            });
            let result = dive_trace::exists::exists_successor_pure(
                &m,
                &p,
                &before,
                None,
                Limits::default(),
                4096,
            )
            .expect("complete configured cover");
            assert_eq!(enabled(&result), expected, "{x},{y}");
            if let Decision::Enabled { witness, .. } = result {
                assert_eq!(
                    m.eval(&p, &St::action(before, witness))
                        .expect("original witness"),
                    Value::Bool(true)
                );
            }
        }
    }
    for target in ["Incomplete", "Unbounded"] {
        let m = configured_model(target);
        assert!(
            matches!(
                dive_trace::exists::exists_successor_pure(
                    &m,
                    &pred(&m, "A"),
                    &state(0, 0),
                    None,
                    Limits::default(),
                    4096
                ),
                Err(ExistsError::Unsupported(_))
            ),
            "{target}"
        );
    }
    let m = configured_model("Ignore");
    assert!(enabled(
        &dive_trace::exists::exists_successor_pure(
            &m,
            &pred(&m, "Unused"),
            &state(0, 0),
            None,
            Limits::default(),
            4096
        )
        .expect("unused argument domain is not forced")
    ));
    assert!(matches!(
        dive_trace::exists::exists_successor_pure(
            &m,
            &pred(&m, "A"),
            &state(0, 0),
            None,
            Limits {
                expressions: 0,
                ..Limits::default()
            },
            4096
        ),
        Err(ExistsError::Limit(_))
    ));
    let mut foreign = pred(&m, "A");
    foreign.scope = pred(&configured_model("Ignore"), "A").scope;
    assert!(matches!(
        dive_trace::exists::exists_successor_pure(
            &m,
            &foreign,
            &state(0, 0),
            None,
            Limits::default(),
            4096
        ),
        Err(ExistsError::Unsupported(_))
    ));
}

#[test]
fn configured_cover_rhs_retains_each_actual_scope_and_refuses_primed_correlations() {
    let m = configured_model("Scoped");
    for (name, expected) in [("StateRhs", state(5, 4)), ("OtherRhs", state(2, 4))] {
        let action = pred(&m, name);
        let result = dive_trace::exists::exists_successor_pure(
            &m,
            &action,
            &state(1, 4),
            None,
            Limits::default(),
            4096,
        )
        .expect("current-state RHS cover");
        let Decision::Enabled { witness, .. } = result else {
            panic!("known real successor")
        };
        assert_eq!(witness, expected);
        assert_eq!(
            m.eval(&action, &St::action(state(1, 4), witness))
                .expect("original scoped relation"),
            Value::Bool(true)
        );
    }
    assert!(matches!(
        dive_trace::exists::exists_successor_pure(
            &m,
            &pred(&m, "PrimedRhs"),
            &state(1, 4),
            None,
            Limits::default(),
            4096
        ),
        Err(ExistsError::Unsupported(_))
    ));
}

#[test]
fn configured_public_exists_context_validation_precedes_original_native_hooks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[derive(Debug)]
    struct Hooks(AtomicUsize);
    impl dive_eval::Host for Hooks {
        fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(dive_eval::HostError {
                code: 1000,
                message: "configured-original-native-hook-error".into(),
            })
        }
    }
    for mutation in ["names", "ir", "scope"] {
        let mut m = configured_model("Native");
        let host = Arc::new(Hooks(AtomicUsize::new(0)));
        m.evaluator.set_host(host.clone());
        let mut action = pred(&m, "NativeCall");
        match mutation {
            "names" => m.names = Arc::new(dive_value::Interner::new()),
            "ir" => m.ir = configured_model("Native").ir,
            "scope" => action.scope = pred(&configured_model("Native"), "NativeCall").scope,
            _ => unreachable!(),
        }
        assert!(matches!(
            dive_trace::exists::exists_successor_pure(
                &m,
                &action,
                &state(0, 0),
                None,
                Limits::default(),
                4096
            ),
            Err(ExistsError::Unsupported(_))
        ));
        assert_eq!(host.0.load(Ordering::SeqCst), 0, "{mutation}");
    }
    let m = configured_model("Native");
    let host = Arc::new(Hooks(AtomicUsize::new(0)));
    m.evaluator.set_host(host.clone());
    let failure = dive_trace::exists::exists_successor_pure(
        &m,
        &pred(&m, "NativeCall"),
        &state(0, 0),
        None,
        Limits::default(),
        4096,
    )
    .expect_err("original native error");
    assert!(
        failure
            .to_string()
            .contains("configured-original-native-hook-error"),
        "{failure}"
    );
    assert_eq!(host.0.load(Ordering::SeqCst), 1);
}

#[test]
fn ordinary_binding_covers_preserve_the_expression_budget_boundary() {
    let m = model("A == x' = 1\n");
    let p = pred(&m, "A");
    for expressions in 0..=6 {
        let result = exists_successor(
            &m,
            &p,
            &state(0, 0),
            None,
            Limits {
                expressions,
                ..Limits::default()
            },
        );
        if expressions < 4 {
            assert!(matches!(result, Err(ExistsError::Limit(_))));
        } else {
            let Decision::Enabled { witness, .. } = result.expect("original four-step cover")
            else {
                panic!("ordinary assignment must be enabled");
            };
            assert_eq!(witness[0], Some(Value::from(1)));
        }
    }
}
