//! The M0 core engine (`--engine=core`) agrees with the reference engine on
//! verdicts and TLC's counts, under every strategy switch, and refuses what
//! it does not implement.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-m0")
        .join(format!("{name}.tla"))
}

fn run(name: &str, engine: Engine) -> Value {
    let file = fixture(name);
    let report = check_file_engine(
        &CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        engine,
    )
    .expect("check runs");
    report.to_json()
}

fn counts(v: &Value) -> (Value, Value, Value, Value) {
    (
        v["verdict"].clone(),
        v["statistics"]["generated"].clone(),
        v["statistics"]["distinct"].clone(),
        v["statistics"]["depth"].clone(),
    )
}

#[test]
fn core_matches_reference_counts_and_verdicts() {
    for name in [
        "Mix", "Viol", "Dead", "Fold", "Lazy", "Inst", "Big", "Bag", "Mv", "Emp",
    ] {
        let reference = run(name, Engine::Reference);
        let core = run(name, Engine::Core);
        assert_eq!(core["engine"], "core", "{name}");
        assert!(reference.get("engine").is_none(), "{name}");
        assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
        assert_eq!(core["property"], reference["property"], "{name}");
    }
}

/// A false or failing ASSUME ends a core run with the reference engine's
/// verdict (`04-plan-compiler.md` §2).
#[test]
fn assumptions_end_the_run_as_the_reference_engine_reports() {
    for name in ["AsFalse", "AsErr"] {
        let reference = run(name, Engine::Reference);
        let core = run(name, Engine::Core);
        assert_eq!(core["engine"], "core", "{name}");
        assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
        assert_eq!(core["property"], reference["property"], "{name}");
    }
}

/// R9: a closed filter beyond the fold budget is left to run time, the
/// abandoned fold is reported, and the counts do not change.
#[test]
fn large_folds_are_bounded_and_reported() {
    let core = run("Fold", Engine::Core);
    let plan = &core["core"]["metrics"]["plan"];
    assert_eq!(plan["fold_abandoned"], 1, "{plan:#}");
    assert!(
        plan["fold_fuel"].as_u64().is_some_and(|f| f <= 1 << 22),
        "{plan:#}"
    );
    assert!(plan["fold_ms"].is_number(), "{plan:#}");
    assert_eq!(counts(&core), counts(&run("Fold", Engine::Reference)));
}

/// The binder's initial predicates are conjoined, as TLC's
/// (`04-plan-compiler.md` §1).
#[test]
fn initial_predicates_are_conjoined() {
    let name = "InitConj";
    let reference = run(name, Engine::Reference);
    let core = run(name, Engine::Core);
    assert_eq!(core["engine"], "core", "{name}");
    assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
    assert_eq!(core["property"], reference["property"], "{name}");
}

/// Laziness (`04-plan-compiler.md` §4.1): recursive functions, lazy
/// functions over `Nat` applied without being built, structural membership
/// in lazy sets, and recursive operators run in the core, with the
/// reference engine's counts.
#[test]
fn lazy_values_are_applied_without_being_built() {
    let core = run("Lazy", Engine::Core);
    assert_eq!(core["verdict"], "success", "{core:#}");
    let plan = &core["core"]["metrics"]["plan"];
    assert!(plan["beta"].as_u64().is_some_and(|b| b >= 1), "{plan:#}");
    assert!(
        plan["speculative"].as_u64().is_some_and(|s| s >= 1),
        "{plan:#}"
    );
    assert_eq!(counts(&core), counts(&run("Lazy", Engine::Reference)));
}

/// Operator arguments, constant operators and nested operator
/// substitutions (`04-plan-compiler.md` §5.4).
#[test]
fn operator_arguments_are_applied_where_passed() {
    for name in ["OpArg", "OpSub"] {
        let reference = run(name, Engine::Reference);
        let core = run(name, Engine::Core);
        assert_eq!(core["engine"], "core", "{name}");
        assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
        assert_eq!(core["property"], reference["property"], "{name}");
    }
}

#[test]
fn violations_and_deadlocks_are_reported() {
    assert_eq!(run("Viol", Engine::Core)["verdict"], "invariant-violation");
    assert_eq!(run("Viol", Engine::Core)["property"], "Small");
    assert_eq!(run("Dead", Engine::Core)["verdict"], "deadlock");
}

/// `LET` values around a split action are bound lazily
/// (`04-plan-compiler.md` §2.1).
#[test]
fn let_values_of_split_actions_are_lazy() {
    let name = "RoleLet";
    let reference = run(name, Engine::Reference);
    let core = run(name, Engine::Core);
    assert_eq!(core["engine"], "core", "{name}");
    assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
    assert_eq!(core["property"], reference["property"], "{name}");
}

#[test]
fn unsupported_features_are_named_without_fallback() {
    let core = run("Unsup", Engine::Core);
    assert_eq!(core["verdict"], "unsupported");
    assert_eq!(core["engine"], "core");
    assert!(
        core["message"]
            .as_str()
            .is_some_and(|m| m.contains("temporal properties")),
        "{core:#}"
    );
}

/// Unions with an infinite side stay lazy (`04-plan-compiler.md` §4.2).
#[test]
fn unions_with_an_infinite_side_stay_lazy() {
    let name = "InfCup";
    let reference = run(name, Engine::Reference);
    let core = run(name, Engine::Core);
    assert_eq!(core["engine"], "core", "{name}");
    assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
    assert_eq!(core["property"], reference["property"], "{name}");
}

/// `UNION {S}` of a set constructor stays lazy; fixture `UnionOne` (TLC:
/// `Inv` violated, 8 distinct, 11 generated, depth 2).
#[test]
fn a_union_of_one_set_constructor_stays_lazy() {
    let name = "UnionOne";
    let reference = run(name, Engine::Reference);
    let core = run(name, Engine::Core);
    assert_eq!(core["engine"], "core", "{name}");
    assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
    assert_eq!(core["property"], reference["property"], "{name}");
}

#[test]
fn zero_to_the_zero_is_tlcs_evaluation_error() {
    // M0 answered 0^0 = 1 and reported success; TLC raises 2180.
    let reference = run("Pow0", Engine::Reference);
    assert_eq!(reference["verdict"], "evaluation-error", "{reference:#}");
    let core = run("Pow0", Engine::Core);
    assert_eq!(counts(&core), counts(&reference), "{core:#}");
    assert_eq!(core["tlc"], reference["tlc"], "{core:#}");
}

#[test]
fn every_strategy_switch_leaves_counts_unchanged() {
    for name in ["Mix", "Fold", "Lazy", "Inst", "Bag", "Mv", "OpArg", "OpSub"] {
        let file = fixture(name);
        let req = CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let base = counts(&run(name, Engine::Reference));
        let mut all = Vec::new();
        for switch in dive_core_switches() {
            all.push(format!("-{switch}"));
            for sign in ['-', '+'] {
                let opt = format!("{sign}{switch}");
                let r = dive_api::check_file_core_opts(&req, &opt).expect("runs");
                assert_eq!(counts(&r.to_json()), base, "{name} {opt}");
            }
        }
        let r = dive_api::check_file_core_opts(&req, &all.join(",")).expect("runs");
        assert_eq!(counts(&r.to_json()), base, "{name} all off");
    }
}

fn dive_core_switches() -> [&'static str; 9] {
    [
        "delta-hash",
        "early-probe",
        "hashcons",
        "symbolic-member",
        "memo",
        "parent-skip",
        "arena",
        "eager",
        "fold",
    ]
}

#[test]
fn unknown_switches_are_refused() {
    let file = fixture("Viol");
    let req = CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(1),
        ..CheckRequest::default()
    };
    let r = dive_api::check_file_core_opts(&req, "-turbo").expect("runs");
    assert_eq!(r.to_json()["verdict"], "unsupported");
}
