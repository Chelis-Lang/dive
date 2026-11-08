//! ENABLED through set and function constructors never certifies a disabled action that has a successor.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

#[path = "support/core_final.rs"]
mod core_final;

use dive_eval::St;
use dive_model::{BindOptions, Model, Pred};
use dive_trace::enabled::{EnabledError, enabled};
use dive_value::{Ctx, Value};
use std::sync::Arc;

const SOURCE: &str = r"---- MODULE ConstructorCovers ----
EXTENDS TLC
VARIABLES S,t
Init == S={} /\ t=FALSE
Next == UNCHANGED <<S,t>>
Map == UNCHANGED S /\ t' \in BOOLEAN /\ {t'=b : b \in BOOLEAN}={TRUE,FALSE}
Filter == UNCHANGED S /\ t' \in BOOLEAN /\ {b \in BOOLEAN : b=t'}={TRUE}
Function == UNCHANGED S /\ t' \in BOOLEAN /\ [b \in BOOLEAN |-> b=t'][TRUE]
Nested == UNCHANGED S /\ t' \in BOOLEAN /\ TRUE \in [b \in BOOLEAN |-> {b=c /\ t'=c : c \in BOOLEAN}][TRUE]
Current == UNCHANGED S /\ t' \in BOOLEAN /\ {b \in S : b=t'}={TRUE}
Empty == UNCHANGED S /\ {t'=b : b \in {}}={}
Missing == UNCHANGED S /\ {t'=b : b \in BOOLEAN}={TRUE,FALSE}
PrimedMap == ({t=b : b \in S}'={TRUE})
PrimedFilter == ({b \in S : b=t}'={TRUE})
PrimedFunction == ([b \in S |-> b=t]'=[b \in {TRUE} |-> TRUE])
Effect == UNCHANGED S /\ t' \in BOOLEAN /\ {Print(b,b) : b \in BOOLEAN}={TRUE,FALSE}
====
";
fn model() -> Model {
    let a = dive_sema::analyze_source(
        "ConstructorCovers.tla",
        SOURCE,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{a:?}");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, &BindOptions::tlc()).expect("bind")
}
fn state(m: &Model, mask: usize, t: bool) -> dive_trace::State {
    let values = (0..2)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| Value::from(i == 1))
        .collect();
    let s = Value::set(values, Ctx::tlc()).expect("set");
    m.ir.vars()
        .iter()
        .map(|d| {
            Some(match m.ir.decl(*d).name.as_str() {
                "S" => s.clone(),
                "t" => t.into(),
                name => panic!("unknown variable {name}"),
            })
        })
        .collect::<Vec<_>>()
        .into()
}
fn pred(m: &Model, name: &str) -> Pred {
    let d = m.ir.root_def(name).expect(name);
    Pred::new(m.ir.def(d).body, m.evaluator.scope(d).expect("valid scope"))
}
#[test]
fn finite_constructor_decisions_match_every_concrete_boolean_assignment() {
    let m = model();
    for name in ["Map", "Filter", "Function", "Nested", "Current", "Empty"] {
        let p = pred(&m, name);
        for mask in 0..4 {
            for t in [false, true] {
                let before = state(&m, mask, t);
                let expected = (0..4).any(|after_mask| {
                    [false, true].into_iter().any(|after_t| {
                        m.eval(
                            &p,
                            &St::action(before.clone(), state(&m, after_mask, after_t)),
                        )
                        .expect("concrete")
                            == true.into()
                    })
                });
                let result = enabled(&m, &p, None, &before).expect(name);
                assert_eq!(result, expected, "{name}/{mask}/{t}");
            }
        }
    }
}
#[test]
fn constructors_never_certify_disabled_with_a_successor() {
    let m = model();
    let result = enabled(&m, &pred(&m, "Missing"), None, &state(&m, 0, false));
    assert_ne!(result, Ok(false));
}
#[test]
fn primed_constructor_domains_are_not_bound_from_the_current_empty_state() {
    let m = model();
    for name in ["PrimedMap", "PrimedFilter", "PrimedFunction"] {
        let p = pred(&m, name);
        let before = state(&m, 0, false);
        assert_eq!(
            m.eval(&p, &St::action(before.clone(), state(&m, 2, true)))
                .expect("concrete witness"),
            true.into(),
            "{name}"
        );
        let result = enabled(&m, &p, None, &before);
        assert_ne!(result, Ok(false), "{name}");
    }
}
#[test]
fn constructor_effects_refuse_before_evaluation() {
    let m = model();
    let result = enabled(&m, &pred(&m, "Effect"), None, &state(&m, 0, false));
    assert!(
        matches!(result, Err(EnabledError::Unsupported(_))),
        "{result:?}"
    );
    assert_eq!(m.take_output(), Vec::<String>::new());
}

#[test]
fn pinned_constructor_fairness_runs_have_independently_valid_counterexamples() {
    use core_final::Outcome;
    use dive_check::CheckOptions;
    use dive_store::StoreMode;
    use dive_trace::Validation;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/constructor-covers");
    for name in [
        "CcMapDisabled",
        "CcFilterDisabled",
        "CcFunctionDisabled",
        "CcFunctionReach",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let cfg =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("cfg");
        let raw = root.join("tlc-2026.10.06.014338");
        let code: i32 = std::fs::read_to_string(raw.join(format!("{name}.exit")))
            .expect("exit")
            .trim()
            .parse()
            .expect("exit number");
        let stdout =
            std::fs::read_to_string(raw.join(format!("{name}.stdout"))).expect("oracle stdout");
        assert!(
            stdout.contains("4 states generated, 2 distinct states found"),
            "{stdout}"
        );
        assert!(
            stdout.contains("depth of the complete state graph search is 2"),
            "{stdout}"
        );
        for mut options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            // Exact seen storage with temporal properties is a Core refusal.
            {
                let store = StoreMode::Fingerprint;
                options.store = store;
                let report = core_final::check(&src, &cfg, &options).expect(name);
                let exploration = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(code, 0, "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(code, 13, "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (
                        exploration.stats.generated,
                        exploration.stats.distinct,
                        exploration.stats.depth
                    ),
                    (4, 2, 2),
                    "{name}"
                );
            }
        }
    }
}
#[test]
fn pinned_enabled_constructor_identities_cover_every_initial_state() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/constructor-covers");
    let name = "EnabledConstructorCover";
    let src =
        std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
    let cfg = std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("cfg");
    let a = dive_sema::analyze_source(&format!("{name}.tla"), &src, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(&cfg).into_config().expect("config");
    let m = dive_model::bind(ir, &cfg, &BindOptions::tlc()).expect("bind");
    assert_eq!(
        std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit")
            .trim(),
        "0"
    );
    for mask in 0..4 {
        for t in [false, true] {
            let before = state(&m, mask, t);
            for name in ["Map", "Filter", "Function", "Nested", "Current", "Empty"] {
                let p = pred(&m, name);
                let result = enabled(&m, &p, None, &before).expect(name);
                assert_eq!(
                    result,
                    name != "Current" || mask & 2 != 0,
                    "{name}/{mask}/{t}"
                );
            }
        }
    }
}
