//! Real private-file mutations at deterministic inline insertion/close boundaries.
#![allow(
    clippy::expect_used,
    reason = "test assertions and private test mutation"
)]

use super::*;
use dive_store::seen::Operation;
use std::{
    cell::RefCell,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Copy)]
enum Mutation {
    RemoveIndex,
    TruncateIndex,
    TruncateData,
    CleanupBlocker,
    CloseCorruption,
}
thread_local! { static PLAN: RefCell<Option<(u64, Mutation)>> = const { RefCell::new(None) }; }

fn directory(parent: &Path) -> PathBuf {
    let children = std::fs::read_dir(parent)
        .expect("test parent")
        .map(|e| e.expect("child").path())
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 1, "test owns exactly one private store");
    children[0].clone()
}
fn mutate(parent: &Path, mutation: Mutation) {
    let directory = directory(parent);
    match mutation {
        Mutation::RemoveIndex => {
            std::fs::remove_file(directory.join("index")).expect("remove index");
        }
        Mutation::TruncateIndex | Mutation::CloseCorruption => {
            std::fs::write(directory.join("index"), b"broken").expect("truncate index");
        }
        Mutation::TruncateData => {
            std::fs::write(directory.join("data"), b"broken").expect("truncate data");
        }
        Mutation::CleanupBlocker => {
            std::fs::write(directory.join("caller-added-file"), b"keep").expect("cleanup blocker");
        }
    }
}
pub(super) fn before_insert(parent: &Path, generated: u64) {
    PLAN.with(|plan| {
        let mut plan = plan.borrow_mut();
        if let Some((wanted, action)) = *plan
            && wanted == generated
        {
            *plan = None;
            mutate(parent, action);
        }
    });
}
pub(super) fn before_close(parent: &Path) {
    PLAN.with(|plan| {
        let mut plan = plan.borrow_mut();
        if let Some((wanted, action)) = *plan
            && wanted == u64::MAX
        {
            *plan = None;
            mutate(parent, action);
        }
    });
}

fn temporary() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "dive-seen-engine-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).expect("test directory");
    dir
}
fn run(
    mutation: Option<(u64, Mutation)>,
    entries: u64,
    profile: dive_eval::Profile,
) -> (crate::CheckResult, PathBuf) {
    run_named(
        "EarlyInvariant",
        dive_store::StoreMode::Fingerprint,
        mutation,
        entries,
        profile,
    )
}

fn run_named(
    name: &str,
    store: dive_store::StoreMode,
    mutation: Option<(u64, Mutation)>,
    entries: u64,
    profile: dive_eval::Profile,
) -> (crate::CheckResult, PathBuf) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/disk-seen/{name}/{name}.tla"));
    let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("config"))
            .into_config()
            .expect("cfg");
    let parent = temporary();
    let mut options = if profile == dive_eval::Profile::Tlc {
        crate::CheckOptions::tlc(0, 7)
    } else {
        crate::CheckOptions::dive(7)
    };
    options.workers = 8;
    options.store = store;
    let mut disk = SeenOptions {
        directory: Some(parent.clone()),
        ..SeenOptions::default()
    };
    disk.limits.entries = entries;
    options.seen_disk = Some(disk);
    PLAN.with(|plan| *plan.borrow_mut() = mutation);
    // Same public checker body, run inline on this test's private thread so the
    // cfg(test)-only mutation plan crosses no thread/scheduling authority.
    let result = crate::run_check(&ir, &cfg, &options);
    PLAN.with(|plan| *plan.borrow_mut() = None);
    (result, parent)
}

#[test]
fn earlier_invariant_prevents_any_later_key_write_or_fault() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        let (result, parent) = run(Some((3, Mutation::RemoveIndex)), 2, profile);
        assert_eq!(
            result.verdict,
            Verdict::InvariantViolated {
                property: "Inv".into(),
                initial: false
            }
        );
        let stats = result.stats.expect("prefix");
        assert_eq!((stats.generated, stats.distinct), (2, 2));
        let usage = result.seen.expect("seen");
        assert!(usage.first_failure.is_none());
        assert_eq!(usage.before_cleanup.expect("facts").physical_entries, 2);
        assert!(usage.after_cleanup.expect("close").cleanup_completed);
        assert!(result.counterexample.is_some());
        std::fs::remove_dir(parent).expect("empty parent");
    }
}

#[test]
fn actual_read_and_integrity_failures_stop_at_original_key_position() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for mutation in [
            Mutation::RemoveIndex,
            Mutation::TruncateIndex,
            Mutation::TruncateData,
        ] {
            let (result, parent) = run(Some((2, mutation)), 16, profile);
            assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
            let stats = result.stats.expect("prefix");
            assert_eq!((stats.generated, stats.distinct), (2, 1));
            let usage = result.seen.expect("usage");
            let failure = usage.first_failure.expect("failure");
            assert_eq!(failure.phase, SeenPhase::Insertion);
            assert_eq!(failure.generated, Some(2));
            assert_eq!(failure.predecessor, Some(0));
            assert_eq!(failure.successor_ordinal, Some(0));
            assert!(matches!(
                failure.error.operation,
                Operation::Read | Operation::Corruption
            ));
            assert_eq!(usage.before_cleanup.expect("physical").physical_entries, 1);
            assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
            assert!(
                result.counterexample.is_some(),
                "predecessor evidence remains explicit, not a completed model proof"
            );
            std::fs::remove_dir(parent).expect("empty parent");
        }
    }
}

#[test]
fn cleanup_failure_retains_prior_invariant_and_concrete_diagnostic_trace() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        let (result, parent) = run(Some((u64::MAX, Mutation::CleanupBlocker)), 16, profile);
        assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
        assert!(result.counterexample.is_some());
        let usage = result.seen.expect("usage");
        assert_eq!(
            usage.deciding_event_before_failure,
            Some(Verdict::InvariantViolated {
                property: "Inv".into(),
                initial: false
            })
        );
        let failure = usage.first_failure.expect("failure");
        assert_eq!(failure.phase, SeenPhase::Cleanup);
        assert_eq!(failure.error.operation, Operation::Cleanup);
        assert!(
            !usage
                .after_cleanup
                .expect("partial cleanup")
                .cleanup_completed
        );
        let directory = directory(&parent);
        assert_eq!(
            std::fs::read(directory.join("caller-added-file")).expect("unmodified caller file"),
            b"keep"
        );
        std::fs::remove_file(directory.join("caller-added-file")).expect("test-owned file");
        std::fs::remove_dir(directory).expect("test-owned store directory");
        std::fs::remove_dir(parent).expect("parent");
    }
}

#[test]
fn close_audit_corruption_cannot_certify_prior_outcome() {
    let (result, parent) = run(
        Some((u64::MAX, Mutation::CloseCorruption)),
        16,
        dive_eval::Profile::Tlc,
    );
    assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
    let usage = result.seen.expect("usage");
    assert_eq!(
        usage.deciding_event_before_failure,
        Some(Verdict::InvariantViolated {
            property: "Inv".into(),
            initial: false
        })
    );
    let failure = usage.first_failure.expect("audit failure");
    assert_eq!(failure.error.operation, Operation::Corruption);
    assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
    std::fs::remove_dir(parent).expect("parent");
}

#[test]
fn required_collision_summary_is_fallible_and_not_silently_omitted() {
    let parent = temporary();
    let mut policy = SeenOptions {
        directory: Some(parent.clone()),
        ..SeenOptions::default()
    };
    policy.limits.summary_pairs = 0;
    let mut run = Run::open(StoreMode::Fingerprint, &policy).expect("store");
    for (i, key) in [1, 2, 3].into_iter().enumerate() {
        assert!(
            run.insert(Key::Fingerprint(key), i as u64 + 1, None, i as u64)
                .expect("insert")
        );
    }
    // Synthetic counter input to this pure summary seam, not claimed generated
    // states or an oracle model. It crosses the same actual TLC threshold.
    let error = run
        .collision(StoreMode::Fingerprint, 3, 1_000_000_000)
        .expect_err("required bounded summary must refuse");
    assert_eq!(error.operation, Operation::Summary);
    assert_eq!(
        run.usage.first_failure.as_ref().expect("position").phase,
        SeenPhase::Summary
    );
    assert_eq!(run.usage.admitted_keys, 3);
    assert!(run.set.close().is_err());
    assert!(run.set.stats().cleanup_completed);
    std::fs::remove_dir(parent).expect("parent");
}

#[test]
fn invariant_error_replay_keeps_original_error_and_late_cleanup_failure() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            let (result, parent) = run_named(
                "ReplayMembership",
                store,
                Some((u64::MAX, Mutation::CleanupBlocker)),
                16,
                profile,
            );
            assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
            assert!(
                result
                    .notes
                    .iter()
                    .any(|n| n.contains("EvaluationError") && n.contains("Invariant"))
            );
            assert_eq!(
                result
                    .counterexample
                    .expect("diagnostic error word")
                    .states
                    .len(),
                2
            );
            let stats = result.stats.expect("original stopping prefix");
            assert_eq!(
                (stats.generated, stats.distinct, stats.left_on_queue),
                (3, 3, 1)
            );
            let usage = result.seen.expect("usage");
            assert!(matches!(usage.deciding_event_before_failure,
                Some(Verdict::Unsupported { ref feature })
                    if feature == "disk seen-state invariant-error call-stack replay"));
            let failure = usage.first_failure.expect("late cleanup cause");
            assert_eq!(failure.phase, SeenPhase::Cleanup);
            assert_eq!(failure.error.operation, Operation::Cleanup);
            assert!(
                !usage
                    .after_cleanup
                    .expect("partial cleanup")
                    .cleanup_completed
            );
            let directory = directory(&parent);
            assert_eq!(
                std::fs::read(directory.join("caller-added-file")).expect("caller file"),
                b"keep"
            );
            std::fs::remove_file(directory.join("caller-added-file")).expect("test cleanup");
            std::fs::remove_dir(directory).expect("private directory");
            std::fs::remove_dir(parent).expect("parent");
        }
    }
}

#[test]
fn invariant_error_replay_never_reads_future_keys_but_preserves_prior_read_fault() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            for (generated, should_fail) in [(3, true), (4, false)] {
                let (result, parent) = run_named(
                    "ReplayMembership",
                    store,
                    Some((generated, Mutation::TruncateIndex)),
                    16,
                    profile,
                );
                let usage = result.seen.expect("usage");
                if should_fail {
                    assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
                    let failure = usage.first_failure.expect("ordered prior read failure");
                    assert_eq!(failure.phase, SeenPhase::Insertion);
                    assert_eq!(failure.generated, Some(3));
                    assert_eq!(failure.predecessor, Some(0));
                    assert_eq!(failure.successor_ordinal, Some(0));
                    assert_eq!(failure.error.operation, Operation::Corruption);
                    assert!(!result.notes.iter().any(|n| n.contains("invariant-error")));
                } else {
                    assert!(
                        matches!(result.verdict, Verdict::Unsupported { ref feature }
                        if feature == "disk seen-state invariant-error call-stack replay")
                    );
                    assert!(usage.first_failure.is_none());
                    assert_eq!(usage.admitted_keys, 3);
                    assert!(result.notes.iter().any(|n| n.contains("EvaluationError")));
                }
                assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
                std::fs::remove_dir(parent).expect("empty parent");
            }
        }
    }
}
