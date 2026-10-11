//! Disk membership preserves original inline safety events and concrete states.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_check::{CheckOptions, CheckResult, SeenOptions, Verdict, check};
use dive_store::StoreMode;
use std::path::Path;
use std::sync::Arc;

fn observable(result: &CheckResult) -> String {
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{}",
        result.verdict,
        result.stats,
        result.counterexample,
        result.collision,
        result.output,
        result.warnings,
        result.notes,
        result.tlc_prints_stats
    )
}

fn verify(name: &str) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/disk-seen/{name}/{name}.tla"));
    let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("config"))
            .into_config()
            .expect("configuration");
    for mut options in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            options.store = store;
            options.sequential = true;
            let expected = check(&ir, &cfg, &options);
            assert!(
                expected.verdict.is_verdict(),
                "{name}: {:?}",
                expected.verdict
            );
            assert!(expected.seen.is_none());
            for width in [1, 4, 8] {
                for chaos in [None, Some(0), Some(7)] {
                    for budgets in [false, true] {
                        let mut disk = options.clone();
                        disk.workers = width;
                        disk.sequential = false;
                        disk.chaos = chaos;
                        disk.seen_disk = Some(SeenOptions::default());
                        if budgets {
                            disk.frontier_memory_bytes = Some(0);
                            disk.predecessor_memory_bytes = Some(0);
                        }
                        let result = check(&ir, &cfg, &disk);
                        assert_eq!(
                            observable(&result),
                            observable(&expected),
                            "{name} {store:?} {:?} width{width} chaos{chaos:?}",
                            options.profile
                        );
                        assert_eq!(result.workers.requested, width);
                        assert_eq!(result.workers.used, 1);
                        assert!(result.workers.sequential);
                        assert!(
                            result
                                .workers
                                .reason
                                .as_ref()
                                .is_some_and(|r| r.contains("immutable disk read views"))
                        );
                        let usage = result.seen.expect("explicit evidence");
                        assert_eq!(usage.admitted_keys, result.stats.expect("stats").distinct);
                        let before = usage.before_cleanup.expect("created store");
                        let after = usage.after_cleanup.expect("explicit close");
                        assert_eq!(before.physical_entries, usage.admitted_keys);
                        assert_eq!(before.resident_key_bytes, 0);
                        assert!(!before.poisoned);
                        assert!(after.cleanup_completed);
                        assert_eq!(after.disk_bytes, 0);
                        assert!(usage.first_failure.is_none());
                        assert!(usage.deciding_event_before_failure.is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn duplicate_init() {
    verify("DuplicateInit");
}
#[test]
fn duplicate_next() {
    verify("DuplicateNext");
}
#[test]
fn early_invariant() {
    verify("EarlyInvariant");
}
#[test]
fn constraint() {
    verify("Constraint");
}
#[test]
fn deadlock() {
    verify("Deadlock");
}
#[test]
fn deterministic_error() {
    verify("DeterministicError");
}
#[test]
fn pure_view() {
    verify("PureView");
}
#[test]
fn safety_symmetry() {
    verify("Symmetry");
}

#[test]
fn tiny_entry_budget_is_not_a_duplicate_or_success() {
    let source = "---- MODULE M ----\nVARIABLE x\nInit == x \\in {0,1}\nNext == x'=x\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    for mut options in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            options.store = store;
            options.workers = 8;
            let mut policy = SeenOptions::default();
            policy.limits.entries = 1;
            options.seen_disk = Some(policy);
            let result = check(&ir, &cfg, &options);
            assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
            let stats = result.stats.expect("stopping prefix");
            assert_eq!((stats.generated, stats.distinct), (2, 1));
            let usage = result.seen.expect("storage facts");
            assert_eq!(usage.admitted_keys, 1);
            let failure = usage.first_failure.expect("first error");
            assert_eq!(failure.phase, dive_check::SeenPhase::Insertion);
            assert_eq!(
                failure.error.operation,
                dive_store::seen::Operation::Admission
            );
            assert_eq!(failure.generated, Some(2));
            assert_eq!(failure.predecessor, None);
            assert_eq!(failure.successor_ordinal, Some(1));
            assert_eq!(usage.before_cleanup.expect("before").physical_entries, 1);
            assert!(usage.after_cleanup.expect("after").cleanup_completed);
        }
    }
}

#[test]
fn capture_and_dfid_refuse_before_assumption_work() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nASSUME FALSE\nInit == x=0\nNext == x'=x\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    for (bind, mut options) in [
        (dive_model::BindOptions::tlc(), CheckOptions::tlc(0, 7)),
        (dive_model::BindOptions::dive(), CheckOptions::dive(7)),
    ] {
        // Binding supplied by the caller is not work performed by these endpoints.
        let model = dive_model::bind(ir.clone(), &config, &bind).expect("bind");
        options.seen_disk = Some(SeenOptions::default());
        options.workers = 8;
        let limits = dive_check::capture::Limits::default();
        for capture in [
            dive_check::capture::explore(&model, &options, limits),
            dive_check::capture::explore_concrete_parallel_with_evidence(&model, &options, limits)
                .expect("typed unsupported capture"),
        ] {
            assert!(
                matches!(capture, dive_check::capture::Exploration::Stopped(_)),
                "unsupported route completed"
            );
            let dive_check::capture::Exploration::Stopped(stop) = capture else {
                continue;
            };
            assert!(
                matches!(stop.verdict,Verdict::Unsupported{ref feature} if feature.contains("disk seen-state"))
            );
            assert!(stop.stats.is_none());
            assert!(stop.seen.is_none());
        }
        let result = dive_check::dfid::check(&model, &dive_check::dfid::Options::new(options, 3));
        assert!(
            matches!(result.outcome,dive_check::dfid::Outcome::Unsupported(ref feature) if feature.contains("disk seen-state"))
        );
        assert_eq!(result.generated, 0);
        assert_eq!(result.initial_candidates, 0);
        assert_eq!(result.distinct, 0);
        assert_eq!(result.stage, "preflight");
    }
}
