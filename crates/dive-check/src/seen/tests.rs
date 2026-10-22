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
thread_local! {
    static REPLAY_PLAN: RefCell<Option<(u64, bool, Mutation)>> = const { RefCell::new(None) };
}
pub(super) fn before_replay(parent: &Path, ordinal: u64, insertion: bool) {
    REPLAY_PLAN.with(|plan| {
        let mut plan = plan.borrow_mut();
        if let Some((wanted, at_insert, action)) = *plan
            && wanted == ordinal
            && insertion == at_insert
        {
            *plan = None;
            mutate(parent, action);
        }
    });
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
    let family = if name.starts_with("Er") {
        "errors/specs"
    } else {
        "disk-seen"
    };
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/{family}/{name}/{name}.tla"));
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
    REPLAY_PLAN.with(|plan| *plan.borrow_mut() = None);
    (result, parent)
}

#[test]
fn replay_identity_mismatch_retains_typed_position_without_fake_library_poison() {
    let parent = temporary();
    let mut run = Run::open(
        StoreMode::Fingerprint,
        &SeenOptions {
            directory: Some(parent.clone()),
            ..SeenOptions::default()
        },
    )
    .expect("owned store");
    assert!(
        run.insert(Key::Fingerprint(7), 3, Some(0), 0)
            .expect("first insert")
    );
    let error = run
        .replay_insert(Key::Fingerprint(7), 3, 0, 1)
        .expect_err("protocol requires prior absence");
    assert_eq!(error.operation, Operation::Corruption);
    let failure = run
        .usage
        .first_failure
        .as_ref()
        .expect("positioned protocol failure");
    assert_eq!(failure.phase, SeenPhase::ReplayInsertion);
    assert_eq!(failure.generated, Some(3));
    assert_eq!(failure.predecessor, Some(0));
    assert_eq!(failure.successor_ordinal, Some(1));
    assert_eq!(failure.error, error);
    assert_eq!(run.usage.admitted_keys, 1);
    assert!(
        !run.set.stats().poisoned,
        "engine protocol failure is not library I/O poison"
    );
    mutate(&parent, Mutation::CleanupBlocker);
    let mut outcome = crate::engine::Outcome {
        prefix_level: None,
        seen: None,
        graph: None,
        frontier: crate::FrontierUsage::default(),
        predecessor: crate::PredecessorUsage::default(),
        verdict: super::verdict(&error),
        stats: None,
        tlc_prints_stats: false,
        counterexample: None,
        collision: None,
        output: Vec::new(),
        notes: Vec::new(),
        workers: crate::Workers::one(),
    };
    let usage = run.close(&mut outcome);
    let failure = usage.first_failure.expect("first protocol cause retained");
    assert_eq!(failure.phase, SeenPhase::ReplayInsertion);
    assert_eq!(failure.error.operation, Operation::Corruption);
    assert_eq!(failure.error.message, error.message);
    assert!(failure.error.cleanup_failure.is_some());
    assert!(matches!(outcome.verdict, Verdict::ResourceLimit { .. }));
    assert!(
        !usage
            .after_cleanup
            .expect("cleanup facts")
            .cleanup_completed
    );
    let directory = directory(&parent);
    std::fs::remove_file(directory.join("caller-added-file")).expect("caller blocker");
    std::fs::remove_dir(directory).expect("private directory");
    std::fs::remove_dir(parent).expect("parent");
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
            assert!(matches!(
                result
                    .seen
                    .as_ref()
                    .expect("usage")
                    .replay
                    .as_ref()
                    .expect("original positioned error")
                    .original_error,
                Verdict::EvaluationError {
                    site: crate::ErrorSite::Invariant,
                    ..
                }
            ));
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
            assert!(matches!(
                usage.deciding_event_before_failure,
                Some(Verdict::EvaluationError {
                    site: crate::ErrorSite::Invariant,
                    ..
                })
            ));
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
                    assert!(matches!(
                        result.verdict,
                        Verdict::EvaluationError {
                            site: crate::ErrorSite::Invariant,
                            ..
                        }
                    ));
                    assert!(usage.first_failure.is_none());
                    assert_eq!(usage.admitted_keys, 3);
                    let replay = usage.replay.as_ref().expect("typed original error");
                    assert!(matches!(
                        replay.original_error,
                        Verdict::EvaluationError {
                            site: crate::ErrorSite::Invariant,
                            ..
                        }
                    ));
                    assert_eq!((replay.admitted_keys, replay.queued), (0, 0));
                    assert!(replay.halted_by.is_none());
                }
                assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
                std::fs::remove_dir(parent).expect("empty parent");
            }
        }
    }
}

#[test]
fn replay_adds_real_keys_and_logical_queue_without_advancing_generated() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let (result, parent) = run_named("ReplayNew", store, None, 16, profile);
            assert!(matches!(
                result.verdict,
                Verdict::EvaluationError {
                    site: crate::ErrorSite::Invariant,
                    ..
                }
            ));
            let stats = result.stats.expect("completed error replay counters");
            assert_eq!(
                (
                    stats.generated,
                    stats.distinct,
                    stats.left_on_queue,
                    stats.depth
                ),
                (2, 4, 2, 2)
            );
            let usage = result.seen.expect("usage");
            let replay = usage.replay.expect("replay evidence");
            assert_eq!(
                (
                    replay.generated,
                    replay.predecessor,
                    replay.stopped_successor_ordinal
                ),
                (2, 0, 0)
            );
            assert_eq!((replay.admitted_keys, replay.queued), (2, 2));
            assert!(replay.halted_by.is_none());
            assert_eq!(usage.admitted_keys, 4);
            assert_eq!(usage.before_cleanup.expect("records").physical_entries, 4);
            assert!(usage.first_failure.is_none());
            assert!(usage.after_cleanup.expect("close").cleanup_completed);
            std::fs::remove_dir(parent).expect("parent");
        }
    }
}

#[test]
fn replay_membership_and_insertion_corruption_keep_original_error_and_position() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for insertion in [false, true] {
                REPLAY_PLAN.with(|plan| {
                    *plan.borrow_mut() = Some((1, insertion, Mutation::TruncateIndex));
                });
                let (result, parent) = run_named("ReplayNew", store, None, 16, profile);
                assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
                let stats = result.stats.expect("prefix");
                assert_eq!(
                    (stats.generated, stats.distinct, stats.left_on_queue),
                    (2, 2, 0)
                );
                let usage = result.seen.expect("usage");
                let replay = usage.replay.expect("original error");
                assert!(matches!(
                    replay.original_error,
                    Verdict::EvaluationError {
                        site: crate::ErrorSite::Invariant,
                        ..
                    }
                ));
                assert!(matches!(
                    replay.halted_by,
                    Some(Verdict::ResourceLimit { .. })
                ));
                assert_eq!((replay.admitted_keys, replay.queued), (0, 0));
                let failure = usage.first_failure.expect("positioned failure");
                assert_eq!(
                    failure.phase,
                    if insertion {
                        SeenPhase::ReplayInsertion
                    } else {
                        SeenPhase::ReplayMembership
                    }
                );
                assert_eq!(
                    (
                        failure.generated,
                        failure.predecessor,
                        failure.successor_ordinal
                    ),
                    (Some(2), Some(0), Some(1))
                );
                assert_eq!(failure.error.operation, Operation::Corruption);
                assert!(usage.before_cleanup.expect("poison").poisoned);
                assert!(usage.after_cleanup.expect("close").cleanup_completed);
                assert!(
                    result
                        .notes
                        .iter()
                        .any(|note| note.contains("diagnostic evidence")
                            && note.contains("EvaluationError"))
                );
                std::fs::remove_dir(parent).expect("parent");
            }
        }
    }
}

#[test]
fn replay_entry_bound_retains_completed_prefix_and_original_error() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let (result, parent) = run_named("ReplayNew", store, None, 3, profile);
            assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
            let stats = result.stats.expect("positioned prefix");
            assert_eq!(
                (stats.generated, stats.distinct, stats.left_on_queue),
                (2, 3, 1)
            );
            let usage = result.seen.expect("usage");
            let replay = usage.replay.expect("original error");
            assert!(matches!(
                replay.original_error,
                Verdict::EvaluationError {
                    site: crate::ErrorSite::Invariant,
                    ..
                }
            ));
            assert_eq!((replay.admitted_keys, replay.queued), (1, 1));
            assert!(matches!(
                replay.halted_by,
                Some(Verdict::ResourceLimit { .. })
            ));
            let failure = usage.first_failure.expect("entry limit");
            assert_eq!(failure.phase, SeenPhase::ReplayInsertion);
            assert_eq!(
                (
                    failure.generated,
                    failure.predecessor,
                    failure.successor_ordinal
                ),
                (Some(2), Some(0), Some(2))
            );
            assert_eq!(failure.error.operation, Operation::Admission);
            assert!(usage.after_cleanup.expect("close").cleanup_completed);
            std::fs::remove_dir(parent).expect("parent");
        }
    }
}

#[test]
fn replay_secondary_error_changes_class_but_retains_original_typed_error() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let (result, parent) = run_named("ErInvReplayError", store, None, 16, profile);
            assert!(matches!(
                result.verdict,
                Verdict::EvaluationError {
                    site: crate::ErrorSite::Invariant,
                    code: 2103,
                    reported: 2111,
                    ..
                }
            ));
            let stats = result.stats.expect("pinned second-error counters");
            assert_eq!(
                (
                    stats.generated,
                    stats.distinct,
                    stats.left_on_queue,
                    stats.depth
                ),
                (4, 5, 1, 3)
            );
            let usage = result.seen.expect("usage");
            let replay = usage.replay.expect("original error");
            assert!(matches!(
                replay.original_error,
                Verdict::EvaluationError {
                    site: crate::ErrorSite::Invariant,
                    code: 2111,
                    reported: 2111,
                    ..
                }
            ));
            assert_eq!(
                (replay.generated, replay.admitted_keys, replay.queued),
                (4, 1, 0)
            );
            assert!(matches!(
                replay.halted_by,
                Some(Verdict::EvaluationError { .. })
            ));
            assert!(usage.first_failure.is_none());
            assert!(usage.after_cleanup.expect("close").cleanup_completed);
            std::fs::remove_dir(parent).expect("parent");
        }
    }
}

#[test]
fn replay_read_fault_outranks_late_cleanup_without_losing_original_error() {
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        REPLAY_PLAN.with(|plan| {
            *plan.borrow_mut() = Some((1, false, Mutation::TruncateIndex));
        });
        let (result, parent) = run_named(
            "ReplayNew",
            store,
            Some((u64::MAX, Mutation::CleanupBlocker)),
            16,
            dive_eval::Profile::Tlc,
        );
        assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
        let usage = result.seen.expect("usage");
        let replay = usage.replay.expect("original error survives both failures");
        assert!(matches!(
            replay.original_error,
            Verdict::EvaluationError {
                site: crate::ErrorSite::Invariant,
                code: 2111,
                ..
            }
        ));
        let failure = usage.first_failure.expect("original read cause");
        assert_eq!(failure.phase, SeenPhase::ReplayMembership);
        assert_eq!(failure.error.operation, Operation::Corruption);
        assert_eq!(
            (
                failure.generated,
                failure.predecessor,
                failure.successor_ordinal
            ),
            (Some(2), Some(0), Some(1))
        );
        assert!(failure.error.cleanup_failure.is_some());
        assert!(
            !usage
                .after_cleanup
                .expect("partial cleanup")
                .cleanup_completed
        );
        let directory = directory(&parent);
        assert_eq!(
            std::fs::read(directory.join("caller-added-file")).expect("preserved caller bytes"),
            b"keep"
        );
        std::fs::remove_file(directory.join("caller-added-file")).expect("caller blocker");
        std::fs::remove_dir(directory).expect("private directory");
        std::fs::remove_dir(parent).expect("parent");
    }
}

fn assert_public_property_replay(public: &crate::CheckResult, memory: &crate::CheckResult) {
    assert_eq!(public.verdict, memory.verdict);
    assert!(matches!(
        public.verdict,
        Verdict::EvaluationError {
            site: crate::ErrorSite::Invariant,
            code: 2111,
            ..
        }
    ));
    assert_eq!(public.verdict.tlc(), memory.verdict.tlc());
    let class = public.verdict.tlc().expect("captured TLC class");
    assert_eq!((class.code, class.exit, class.reported), (2111, 76, 2111));
    assert!(class.verified);
    assert_eq!(public.stats, memory.stats);
    assert_eq!(public.tlc_prints_stats, memory.tlc_prints_stats);
    assert_eq!(public.collision, memory.collision);
    assert_eq!(public.output, memory.output);
    assert_eq!(public.warnings, memory.warnings);
    assert_eq!(public.notes, memory.notes);
    assert_eq!(public.var_names, memory.var_names);
    assert_eq!(public.tlc_var_order, memory.tlc_var_order);
    assert_eq!(public.store, memory.store);
    assert_eq!(public.fp_index, memory.fp_index);
    assert_eq!(public.tlc_fingerprints, memory.tlc_fingerprints);
    assert_eq!(
        format!("{:?}", public.counterexample),
        format!("{:?}", memory.counterexample)
    );
    assert_eq!(
        public
            .counterexample
            .as_ref()
            .expect("diagnostic word")
            .states
            .len(),
        3
    );
    let word = public.counterexample.as_ref().expect("diagnostic word");
    for (index, state) in word.states.iter().enumerate() {
        assert_eq!(format!("{:?}", state.values), format!("[{index}, 0]"));
        assert!(state.text.is_none());
        if index == 0 {
            assert_eq!(state.label, "<Initial predicate>");
            assert!(state.action.is_none());
        } else {
            assert_eq!(
                state.label,
                "<Step line 6, col 12 to line 6, col 40 of module ErInvReplayProp>"
            );
            assert_eq!(state.action.as_deref(), Some("Step"));
        }
    }
}

#[test]
fn public_pure_action_property_replay_matches_memory_and_private_disk() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/errors/specs/ErInvReplayProp/ErInvReplayProp.tla");
    let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("config"))
            .into_config()
            .expect("cfg");
    for mut options in [crate::CheckOptions::tlc(0, 7), crate::CheckOptions::dive(7)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            options.store = store;
            options.sequential = true;
            let memory = crate::check(&ir, &cfg, &options);
            let parent = temporary();
            options.seen_disk = Some(SeenOptions {
                directory: Some(parent.clone()),
                ..SeenOptions::default()
            });
            let public = crate::check(&ir, &cfg, &options);
            assert_public_property_replay(&public, &memory);
            assert_eq!(
                std::fs::read_dir(&parent)
                    .expect("closed public store")
                    .count(),
                0
            );
            // The independently invoked private engine must preserve the same
            // source transaction, not merely agree on its final error class.
            let host = Arc::new(crate::engine::run_host(&options));
            let bind = dive_model::BindOptions {
                eval: options.eval,
                symmetry_liveness: options.symmetry_liveness,
                no_deadlock: options.no_deadlock,
                host: Some(Arc::new(crate::host::EngineHost::new(host.clone()))),
            };
            let model = dive_model::bind(ir.clone(), &cfg, &bind).expect("bound model");
            model.evaluator.intern_checker_names();
            let config = crate::engine::run_config(&options, model.settings.check_deadlock);
            host.set_config(Some(config.clone()));
            model.evaluator.intern_bfs_config(&config);
            let disk = crate::engine::search(
                &model,
                &options,
                &host,
                crate::engine::SearchHooks::default(),
            );
            assert_eq!(disk.verdict, public.verdict);
            assert_eq!(disk.stats, public.stats);
            assert_eq!(disk.output, public.output);
            assert_eq!(disk.tlc_prints_stats, public.tlc_prints_stats);
            assert_eq!(disk.collision, public.collision);
            assert_eq!(disk.frontier, public.frontier);
            assert_eq!(disk.predecessor, public.predecessor);
            assert_eq!(disk.workers, public.workers);
            assert_eq!(public.workers.requested, 1);
            assert_eq!(public.workers.used, 1);
            assert!(public.workers.sequential);
            assert_eq!(format!("{:?}", disk.seen), format!("{:?}", public.seen));
            assert_eq!(disk.verdict, memory.verdict);
            assert_eq!(disk.stats, memory.stats);
            assert_eq!(
                format!("{:?}", disk.counterexample),
                format!("{:?}", memory.counterexample)
            );
            let stats = disk.stats.expect("error replay counters");
            assert_eq!(
                (
                    stats.generated,
                    stats.distinct,
                    stats.left_on_queue,
                    stats.depth
                ),
                (4, 4, 1, 3)
            );
            let usage = disk.seen.expect("disk evidence");
            assert!(usage.replay.is_some());
            assert!(usage.first_failure.is_none());
            assert!(usage.after_cleanup.expect("close").cleanup_completed);
            std::fs::remove_dir(parent).expect("parent");
            options.seen_disk = None;
        }
    }
}

#[test]
fn printed_error_replay_keeps_earlier_positioned_read_failure() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        REPLAY_PLAN.with(|plan| *plan.borrow_mut() = Some((1, false, Mutation::TruncateIndex)));
        let (out, parent) = run_named(
            "ReplayPrint",
            dive_store::StoreMode::Exact,
            None,
            100,
            profile,
        );
        assert!(matches!(out.verdict, Verdict::ResourceLimit { .. }));
        let usage = out.seen.expect("seen facts");
        let replay = usage.replay.expect("replay");
        assert!(replay.unproved_print_output);
        assert!(matches!(
            replay.original_error,
            Verdict::EvaluationError { .. }
        ));
        assert!(matches!(
            replay.halted_by,
            Some(Verdict::ResourceLimit { .. })
        ));
        let failure = usage.first_failure.expect("positioned failure");
        assert_eq!(failure.phase, SeenPhase::ReplayMembership);
        assert_eq!(failure.successor_ordinal, Some(1));
        assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
        std::fs::remove_dir_all(parent).expect("test parent cleanup");
    }
}
