//! Tiny disk-frontier budgets preserve every ordered safety observable.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::{CheckOptions, CheckResult, check};
use dive_store::StoreMode;
use std::path::Path;
use std::sync::Arc;

fn observed(r: &CheckResult) -> String {
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{}",
        r.verdict,
        r.stats,
        r.counterexample,
        r.collision,
        r.output,
        r.warnings,
        r.notes,
        r.tlc_prints_stats
    )
}

#[test]
fn tiny_spill_matches_memory_across_events_profiles_stores_and_widths() {
    for name in [
        "DuplicateSuccessors",
        "MidLevelViolation",
        "DeadlockMidLevel",
        "EvalErrorMidLevel",
        "ImpliedActionSeen",
        "ConstrainedViolation",
        "InitViolation",
    ] {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/tlc-probes/specs/{name}/{name}.tla"));
        let a = dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
        assert!(a.accepted());
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let cfg =
            dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("cfg"))
                .into_config()
                .expect("config");
        for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let mut memory = base.clone();
                memory.store = store;
                memory.sequential = true;
                let expected = check(&ir, &cfg, &memory);
                assert!(
                    expected.verdict.is_verdict(),
                    "{name}: {:?}",
                    expected.verdict
                );
                for budget in [0, 64, 1024] {
                    for (workers, sequential, chaos) in [
                        (1, true, None),
                        (1, false, None),
                        (4, false, None),
                        (8, false, None),
                        (1, false, Some(42)),
                        (4, false, Some(13)),
                        (8, false, Some(17)),
                    ] {
                        let mut options = base.clone();
                        options.store = store;
                        options.workers = workers;
                        options.sequential = sequential;
                        options.chaos = chaos;
                        options.frontier_memory_bytes = Some(budget);
                        let actual = check(&ir, &cfg, &options);
                        assert_eq!(
                            observed(&actual),
                            observed(&expected),
                            "{name} {store:?} {:?} budget{budget} workers{workers} chaos{chaos:?}",
                            base.profile
                        );
                        assert!(actual.frontier.peak_retained_bytes <= budget as u64 * 2);
                        assert_eq!(actual.frontier.retained_bytes, 0);
                        assert_eq!(actual.frontier.failures, [] as [String; 0]);
                        if budget == 0
                            && actual.stats.as_ref().is_some_and(|s| s.distinct > 0)
                            && name != "InitViolation"
                        {
                            assert!(actual.frontier.spilled_bytes > 0);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn empty_frontier_and_deferred_view_admission() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frontier");
    for name in ["EmptyFrontier", "ViewDeferred"] {
        let file = root.join(name).join(format!("{name}.tla"));
        let analysis =
            dive_sema::analyze_file(&file, &dive_sema::Options::default()).expect("source");
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg =
            dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg")).expect("cfg"))
                .into_config()
                .expect("config");
        for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
            opts.workers = 8;
            opts.chaos = Some(17);
            let memory = check(&ir, &cfg, &opts);
            opts.frontier_memory_bytes = Some(0);
            let spill = check(&ir, &cfg, &opts);
            if name == "EmptyFrontier" {
                assert_eq!(observed(&spill), observed(&memory));
                assert_eq!(spill.frontier.failures, [] as [String; 0]);
                assert_eq!(spill.frontier.spilled_bytes, 0);
            } else {
                assert!(memory.verdict.is_verdict(), "{:?}", memory.verdict);
                assert!(
                    matches!(spill.verdict, dive_check::Verdict::Unsupported { ref feature } if feature.contains("deferred")),
                    "{:?}",
                    spill.verdict
                );
                assert_eq!(spill.output, memory.output);
                assert_eq!(spill.frontier.spilled_bytes, 0);
            }
        }
    }
}
