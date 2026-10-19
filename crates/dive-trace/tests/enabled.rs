//! ENABLED decisions on given states agree with brute force and pinned TLC.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_eval::St;
use dive_model::{Model, Pred};
use dive_trace::State;
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
fn enabled(m: &Model, action: &Pred, sub: Option<&Pred>, state: &State) -> bool {
    dive_trace::enabled::enabled(m, action, sub.map(|s| s.expr), state).expect("decide")
}

#[test]
fn finite_relations_match_exhaustive_assignments() {
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
                let result = enabled(&m, &p, None, &before);
                // D's unchanged values may be outside the finite brute-force box.
                let expected = if name == "D" { true } else { brute };
                assert_eq!(result, expected, "{name}, {x}, {y}");
            }
        }
    }
}

#[test]
fn constraints_and_reachable_states_never_restrict_enabled() {
    let m = model("A == x' = 99\nSub == x\n");
    let p = pred(&m, "A");
    let sub = pred(&m, "Sub");
    // Outside Init's range, and only a stuttering successor.
    assert!(enabled(&m, &p, Some(&sub), &state(0, 0)));
    assert!(!enabled(&m, &p, Some(&sub), &state(99, 0)));
}

#[test]
fn pinned_tlc_enabledness_matches_every_state() {
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
            let d = enabled(&m, &pred(&m, "Relation"), None, &state(x, y));
            assert_eq!(d, (0..=4).contains(&(x + y)), "{x}, {y}");
        }
    }
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
                let result = enabled(&m, &pred(&m, name), None, &state(x, y));
                assert_eq!(result, (min..=max).contains(&(x + y)), "{name} {x} {y}");
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
                let result = enabled(&m, &p, None, &before);
                assert_eq!(result, brute, "{name} {x} {y}");
            }
        }
    }
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
                let d = enabled(&m, &pred(&m, name), None, &state(x, y));
                assert_eq!(d, expected, "{name} {x} {y}");
            }
        }
    }
}
