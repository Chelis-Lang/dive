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

#[test]
fn store_memory_metrics_report_components_and_their_coverage_gap() {
    let core = run("Mix", Engine::Core);
    let memory = &core["core"]["metrics"]["store"]["memory"];
    let mut known = 0;
    for field in [
        "fixed_bytes",
        "slab_bytes",
        "boxed_payload_bytes",
        "current_table_bytes",
        "retired_table_bytes",
        "retirement_tracking_bytes",
    ] {
        known += memory[field].as_u64().expect("measured component");
    }
    for field in [
        "unique_payload_bytes",
        "text_slots_bytes",
        "index_entries_bytes_lower_bound",
    ] {
        known += memory["strings"][field].as_u64().expect("string component");
    }
    assert_eq!(memory["known_retained_bytes_lower_bound"], known);
    assert_eq!(memory["accounting_complete"], false);
    assert!(memory["fixed_bytes"].as_u64() > Some(0));
    assert!(memory["slab_bytes"].as_u64() > Some(0));
    let scope = memory["scope"].as_str().expect("scope");
    assert!(scope.contains("lower bound, not RSS or peak"));
    assert!(scope.contains("opaque string-index bucket"));
    assert_eq!(counts(&core), counts(&run("Mix", Engine::Reference)));
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

/// `ENABLED` searches its action for one successor; fixture `Enab` (TLC:
/// `Inv` violated, 216 distinct, 672 generated, depth 12).
#[test]
fn enabled_matches_the_reference_engine() {
    // `EnabStep` (TLC: success, 3 distinct, 4 generated, depth 3): ENABLED
    // in an implied action and an action constraint, where a primed
    // definition must read the ENABLED action's successor, not the step's.
    for name in ["Enab", "EnabStep"] {
        let reference = run(name, Engine::Reference);
        let core = run(name, Engine::Core);
        assert_eq!(core["engine"], "core", "{name}");
        assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
        assert_eq!(core["property"], reference["property"], "{name}");
    }
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

/// Pair-domain functions are indexed by position or by search, and small
/// differences are taken directly; fixture `GridFcn` (TLC: `Inv` violated,
/// 1665 distinct, 5174 generated, depth 6).
#[test]
fn pair_domain_lookups_and_small_differences_match_the_reference() {
    let name = "GridFcn";
    let reference = run(name, Engine::Reference);
    let core = run(name, Engine::Core);
    assert_eq!(core["engine"], "core", "{name}");
    assert_eq!(counts(&core), counts(&reference), "{name}: {core:#}");
    assert_eq!(core["property"], reference["property"], "{name}");
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

/// A standard operator passed to a higher-order operator (`+`, `\o`, `*`)
/// is applied as the `LAMBDA` of its arity, and `DyadicRationals!Reduce`
/// is the Java override's (#849).
#[test]
fn std_operators_as_operator_arguments_and_dyadic_reduce() {
    let reference = run("HoStd", Engine::Reference);
    let core = run("HoStd", Engine::Core);
    assert_eq!(core["engine"], "core");
    assert_eq!(core["verdict"], "success", "{core:#}");
    assert_eq!(counts(&core), counts(&reference), "{core:#}");
}

/// TLC reports an error inside such an operator through the override's
/// Java-method report, which the core does not reproduce: it refuses.
#[test]
fn an_error_inside_a_passed_std_operator_is_refused() {
    let core = run("HoStdErr", Engine::Core);
    assert_eq!(core["engine"], "core");
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
}

/// Built-in value operators passed as operator arguments (`\intersect`
/// through `KeyValueStore`'s LET-recursive `ReduceSet`, the set, membership
/// and Boolean operators, `SUBSET`, `UNION`, `DOMAIN`, also under a prime)
/// apply as written in place, with `/\` and `\/` short-circuiting (#851).
/// The pinned TLC gives success, 5 generated, 4 distinct, depth 4.
#[test]
fn built_in_operators_as_operator_arguments() {
    let tlc = (
        Value::from("success"),
        Value::from(5),
        Value::from(4),
        Value::from(4),
    );
    let reference = run("HoBuiltin", Engine::Reference);
    let core = run("HoBuiltin", Engine::Core);
    assert_eq!(core["engine"], "core");
    assert_eq!(counts(&core), tlc, "{core:#}");
    assert_eq!(counts(&reference), tlc, "{reference:#}");
}

/// `=>` (short-circuiting) and `<=>` passed as operator arguments, which
/// the reference engine refuses; the counts are the pinned TLC's.
#[test]
fn implication_and_equivalence_as_operator_arguments() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-ho-builtin/HoBuiltinImp.tla");
    let report = check_file_engine(
        &CheckRequest {
            config: Some(file.with_extension("cfg")),
            file,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        Engine::Core,
    )
    .expect("check runs");
    let core = report.to_json();
    assert_eq!(core["engine"], "core");
    assert_eq!(
        counts(&core),
        (
            Value::from("success"),
            Value::from(5),
            Value::from(4),
            Value::from(4)
        ),
        "{core:#}"
    );
}

/// A built-in passed as an operator argument and applied as an action
/// stays a named refusal: TLC's action splitting of it is not reproduced.
#[test]
fn a_built_in_operator_argument_applied_as_an_action_is_refused() {
    let core = run("HoBuiltinAct", Engine::Core);
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
    assert!(
        core["message"]
            .as_str()
            .is_some_and(|m| m.contains("applied as an action")),
        "{core:#}"
    );
}

/// `Reduce` keeps the reference engine's refusal of operands beyond 32 bits.
#[test]
fn dyadic_reduce_refuses_operands_beyond_32_bits() {
    let core = run("DyadicBig", Engine::Core);
    assert_eq!(core["verdict"], "unsupported", "{core:#}");
    assert!(
        core["message"]
            .as_str()
            .is_some_and(|m| m.contains("DyadicRationals") && m.contains("32-bit")),
        "{core:#}"
    );
}
