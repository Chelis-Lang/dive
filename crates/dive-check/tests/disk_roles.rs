//! Disk seen-state retention gives the resident store's results for every role shape.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_check::{CheckOptions, SeenOptions, Verdict, check};
use dive_store::StoreMode;
use std::{path::Path, sync::Arc};

fn verify(name: &str) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/disk-seen-roles/{name}/{name}.tla"));
    let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("cfg"))
            .into_config()
            .expect("configuration");
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            opts.store = store;
            opts.seen_disk = None;
            opts.frontier_memory_bytes = None;
            opts.predecessor_memory_bytes = None;
            opts.sequential = true;
            let memory = check(&ir, &cfg, &opts);
            assert!(memory.seen.is_none());
            for width in [1, 4, 8] {
                for chaos in [None, Some(0), Some(7)] {
                    for budgets in [false, true] {
                        opts.seen_disk = Some(SeenOptions::default());
                        opts.workers = width;
                        opts.sequential = false;
                        opts.chaos = chaos;
                        opts.frontier_memory_bytes = budgets.then_some(0);
                        opts.predecessor_memory_bytes = budgets.then_some(0);
                        let disk = check(&ir, &cfg, &opts);
                        eprintln!(
                            "{name} {:?} {store:?} memory={:?} disk={:?} stats={:?}",
                            opts.profile, memory.verdict, disk.verdict, disk.stats
                        );
                        assert_eq!(disk.verdict, memory.verdict, "{name}");
                        assert_eq!(disk.stats, memory.stats, "{name}");
                        assert_eq!(
                            format!("{:?}", disk.counterexample),
                            format!("{:?}", memory.counterexample),
                            "{name}"
                        );
                        assert_eq!(disk.output, memory.output, "{name}");
                        assert_eq!(disk.warnings, memory.warnings, "{name}");
                        assert_eq!(disk.notes, memory.notes, "{name}");
                        assert_eq!(disk.collision, memory.collision, "{name}");
                        if disk.stats.is_some() {
                            assert_eq!(disk.workers.used, 1);
                            assert!(
                                disk.seen
                                    .expect("storage")
                                    .after_cleanup
                                    .expect("cleanup")
                                    .cleanup_completed
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Checks `ir` with resident and disk seen-state storage and requires equal results.
fn same_on_disk(
    ir: &Arc<dive_ir::Ir>,
    cfg: &dive_config::ModelConfig,
    label: &str,
) -> dive_check::CheckResult {
    let mut last = None;
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        let memory = check(ir, cfg, &opts);
        opts.seen_disk = Some(SeenOptions::default());
        let disk = check(ir, cfg, &opts);
        assert_eq!(disk.verdict, memory.verdict, "{label}");
        assert_eq!(disk.stats, memory.stats, "{label}");
        assert_eq!(disk.output, memory.output, "{label}");
        assert_eq!(disk.warnings, memory.warnings, "{label}");
        assert_eq!(
            format!("{:?}", disk.counterexample),
            format!("{:?}", memory.counterexample),
            "{label}"
        );
        last = Some(disk);
    }
    last.expect("two profiles")
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            verify($name);
        }
    };
}
case!(action_property, "ActionProperty");
case!(always_property, "AlwaysProperty");
case!(configured_specification, "ConfiguredSpecification");
case!(fair_specification, "FairSpecification");
case!(let_specification, "LetSpecification");
case!(printed_property, "PrintedProperty");
case!(printed_specification, "PrintedSpecification");
case!(pure_specification, "PureSpecification");
case!(state_property, "StateProperty");
case!(wrapper_specification, "WrapperSpecification");
case!(action_level_property, "ActionLevelProperty");
case!(arity_property, "ArityProperty");
case!(arity_specification, "AritySpecification");
case!(assume_false_safety, "AssumeFalseSafety");
case!(assume_print_safety, "AssumePrintSafety");
case!(conjunction_property, "ConjunctionProperty");
case!(constant_false_property, "ConstantFalseProperty");
case!(constant_specification, "ConstantSpecification");
case!(constant_true_property, "ConstantTrueProperty");
case!(init_next_counterpart, "InitNextCounterpart");
case!(instance_property, "InstanceProperty");
case!(instance_specification, "InstanceSpecification");
case!(let_property, "LetProperty");
case!(missing_property, "MissingProperty");
case!(missing_specification, "MissingSpecification");
case!(mixed_roles, "MixedRoles");
case!(property_alias, "PropertyAlias");
case!(quantified_property, "QuantifiedProperty");
case!(temporal_property, "TemporalProperty");

#[test]
fn configured_native_effects_match_resident_storage() {
    for rhs in ["PrintT", "Alias"] {
        let source = "---- MODULE RoleNative ----\nEXTENDS Naturals, TLC\nCONSTANT C(_)\nVARIABLE x\nR == INSTANCE TLC\nAlias(a) == R!PrintT(a)\nStartup == PrintT(\"must-not-start\")\nInit == x=0\nNext == x'=x+1 /\\ C(\"must-not-next\")\nSpec == Init /\\ [][Next]_x\nBound == x < 3\n====\n";
        let analysis =
            dive_sema::analyze_source("RoleNative.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse(&format!(
            "CONSTANT C <- {rhs}\nSPECIFICATION Spec\nCONSTRAINT Bound\nCHECK_DEADLOCK FALSE\n"
        ))
        .into_config()
        .expect("cfg");
        same_on_disk(&ir, &cfg, rhs);
    }
}

#[test]
fn actual_pure_override_does_not_execute_effectful_documented_body() {
    let source = "---- MODULE RoleOverride ----\nEXTENDS Naturals, TLC\nVARIABLE x\nDanger == PrintT(\"must-not-start\")\nSafe == TRUE\nInit == x=0\nNext == x<3 /\\ x'=x+1\nSpec == Init /\\ [][Next]_x\n====\n";
    let analysis =
        dive_sema::analyze_source("RoleOverride.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg =
        dive_config::parse("CONSTANT Danger <- Safe\nSPECIFICATION Spec\nCHECK_DEADLOCK FALSE\n")
            .into_config()
            .expect("cfg");
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        opts.seen_disk = Some(SeenOptions::default());
        let result = check(&ir, &cfg, &opts);
        assert!(
            matches!(result.verdict, Verdict::Success),
            "{:?}",
            result.verdict
        );
        assert_eq!(result.output, Vec::<String>::new());
        assert_eq!(result.stats.expect("stats").distinct, 4);
    }
}

#[test]
fn configured_redirect_cycles_are_not_a_purity_certificate() {
    let source = "---- MODULE RoleCycle ----\nEXTENDS Naturals\nVARIABLE x\nA == TRUE\nB == TRUE\nInit == x=0\nNext == x'=x\nSpec == Init /\\ [][Next]_x\n====\n";
    let analysis =
        dive_sema::analyze_source("RoleCycle.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg =
        dive_config::parse("CONSTANT A <- B B <- A\nSPECIFICATION Spec\nCHECK_DEADLOCK FALSE\n")
            .into_config()
            .expect("cfg");
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        let resident = check(&ir, &cfg, &opts);
        opts.seen_disk = Some(SeenOptions::default());
        let result = check(&ir, &cfg, &opts);
        assert!(
            matches!(result.verdict, Verdict::ModelError { code: 2223, .. }),
            "{:?}",
            result.verdict
        );
        assert_eq!(result.verdict, resident.verdict);
        assert_eq!(result.output, Vec::<String>::new());
        assert!(result.stats.is_none());
        assert!(result.seen.is_none());
    }
}

#[test]
fn selected_native_zero_argument_role_refuses_before_startup() {
    let source = "---- MODULE RoleClock ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0\nNext == x'=x\n====\n";
    let analysis =
        dive_sema::analyze_source("RoleClock.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY JavaTime\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        opts.seen_disk = Some(SeenOptions::default());
        let result = check(&ir, &cfg, &opts);
        assert!(
            matches!(result.verdict, Verdict::Unsupported { .. }),
            "{:?}",
            result.verdict
        );
        assert_eq!(result.output, Vec::<String>::new());
        assert!(result.stats.is_none());
        assert!(result.seen.is_none());
    }
}

#[test]
fn safety_role_admission_retains_positioned_entry_bounds() {
    let source = "---- MODULE RoleLimit ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<3 /\\ x'=x+1\nSpec == Init /\\ [][Next]_x\nP == [](x<3)\n====\n";
    let analysis =
        dive_sema::analyze_source("RoleLimit.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            opts.store = store;
            let mut disk = SeenOptions::default();
            disk.limits.entries = 1;
            opts.seen_disk = Some(disk);
            opts.workers = 8;
            let result = check(&ir, &cfg, &opts);
            assert!(
                matches!(result.verdict, Verdict::ResourceLimit { .. }),
                "{:?}",
                result.verdict
            );
            let stats = result.stats.expect("prefix");
            assert_eq!((stats.generated, stats.distinct), (2, 1));
            let usage = result.seen.expect("evidence");
            let failure = usage.first_failure.expect("first admission error");
            assert_eq!(failure.phase, dive_check::SeenPhase::Insertion);
            assert_eq!(failure.generated, Some(2));
            assert_eq!(failure.predecessor, Some(0));
            assert_eq!(usage.admitted_keys, 1);
            assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
        }
    }
}

#[test]
fn deep_shared_definition_paths_match_resident_storage() {
    use std::fmt::Write;
    let mut source =
        "---- MODULE RoleDepth ----\nEXTENDS Naturals\nVARIABLE x\nN0 == TRUE\n".to_owned();
    for i in 1..=80 {
        writeln!(source, "N{i} == N{}", i - 1).expect("String");
    }
    source.push_str("Init == x=0\nNext == x'=x\nSpec == N80 /\\ Init /\\ [][Next]_x\n====\n");
    let analysis =
        dive_sema::analyze_source("RoleDepth.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("SPECIFICATION Spec\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    let disk = same_on_disk(&ir, &cfg, "RoleDepth");
    assert_eq!(disk.verdict, Verdict::Success);
}

#[test]
fn strict_subscript_only_substitution_matches_resident_storage() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/strict-subscript/StrictRoleSubscript.tla");
    let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    assert!(ir.nodes().iter().any(|node| matches!(
        &node.expr,
        dive_ir::Expr::Subst { bindings, .. } if bindings.iter().any(|binding| matches!(
            binding.value,
            dive_ir::Arg::Expr { pass: dive_ir::Pass::Strict, .. }
        ))
    )));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("cfg"))
            .into_config()
            .expect("configuration");
    for opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        let resident = check(&ir, &cfg, &opts);
        assert_eq!(resident.verdict, Verdict::Success);
        assert_eq!(resident.stats.expect("stats").distinct, 3);
        assert!(resident.warnings.iter().any(|(code, _)| *code == 2140));
    }
    same_on_disk(&ir, &cfg, "StrictRoleSubscript");
}
