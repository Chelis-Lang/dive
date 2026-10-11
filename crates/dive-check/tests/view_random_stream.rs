//! Clean pinned transformed fingerprints retain concrete random witnesses.
#![allow(clippy::expect_used, reason = "regression assertions")]

use dive_check::{CheckOptions, Verdict};
use dive_store::StoreMode;
use std::sync::Arc;

#[allow(
    clippy::too_many_arguments,
    reason = "explicit independent matrix axes"
)]
fn run(
    view: &str,
    configured: bool,
    initial: &str,
    invariant: &str,
    profile: dive_eval::Profile,
    store: StoreMode,
    width: usize,
    chaos: Option<u64>,
) -> dive_check::CheckResult {
    let call = if configured {
        "C(1..100)"
    } else {
        "RandomElement(1..100)"
    };
    let source = format!(
        "---- MODULE ViewStream ----\nEXTENDS Naturals,TLC\nCONSTANT C(_)\nVARIABLES n,x,s\nInit == n=0 /\\ x=0 /\\ s = {initial}\nNext == n<3 /\\ n'=n+1 /\\ x' = {call} /\\ UNCHANGED s\nInv == {invariant}\nProjection == {view}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("ViewStream.tla", &source, &dive_sema::Options::default());
    assert!(
        analysis.accepted(),
        "{:?} / {:?}: {source}",
        analysis.diagnostics(),
        analysis.syntax_diagnostics()
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nVIEW Projection\nCHECK_DEADLOCK FALSE\n").into_config().expect("cfg");
    let mut opts = CheckOptions::tlc(0, 7);
    opts.profile = profile;
    opts.eval = dive_eval::Options::for_profile(profile);
    opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
    opts.store = store;
    opts.workers = width;
    opts.chaos = chaos;
    dive_check::check(&ir, &cfg, &opts)
}

#[test]
fn clean_primary_view_shapes_match_concrete_draws_at_every_requested_width() {
    // These values are clean workers1/seed7/fp0 TLC outputs, captured before
    // lifting. The extra unchanged s is omitted from VIEW, as TLC's RNG is.
    for (view, expected) in [
        ("n", [0, 78, 49, 21]),
        ("<<n>>", [0, 59, 12, 21]),
        ("[phase |-> n]", [0, 75, 100, 40]),
        ("<<n,x>>", [0, 92, 31, 98]),
        ("IF n % 2 = 0 THEN n ELSE n+100", [0, 78, 13, 21]),
    ] {
        for configured in [false, true] {
            for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
                let baseline = run(
                    view,
                    configured,
                    "<<{2,1},[a |-> TRUE]>>",
                    "n<3",
                    profile,
                    StoreMode::Fingerprint,
                    1,
                    None,
                );
                for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                    for width in [1, 4, 8] {
                        for chaos in [None, Some(19)] {
                            let result = run(
                                view,
                                configured,
                                "<<{2,1},[a |-> TRUE]>>",
                                "n<3",
                                profile,
                                store,
                                width,
                                chaos,
                            );
                            assert!(
                                matches!(&result.verdict,Verdict::InvariantViolated {property,..} if property=="Inv"),
                                "{view}: {result:?}"
                            );
                            assert_eq!((result.workers.requested, result.workers.used), (width, 1));
                            assert!(result.workers.sequential);
                            let stats = result.stats.as_ref().expect("search evidence");
                            assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 4, 4));
                            let trace = result.counterexample.as_ref().expect("concrete trace");
                            assert_eq!(trace.states.len(), 4);
                            assert_eq!(
                                trace
                                    .states
                                    .iter()
                                    .map(|state| &state.values)
                                    .collect::<Vec<_>>(),
                                baseline
                                    .counterexample
                                    .as_ref()
                                    .expect("baseline witness")
                                    .states
                                    .iter()
                                    .map(|state| &state.values)
                                    .collect::<Vec<_>>()
                            );
                            if profile == dive_eval::Profile::Tlc {
                                for (state, x) in trace.states.iter().zip(expected) {
                                    assert_eq!(state.values[1], dive_value::Value::Int(x));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn transformed_first_successor_replay_boundary_stays_named_and_incomplete() {
    for configured in [false, true] {
        for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
            let result = run(
                "n",
                configured,
                "{}",
                "n<1",
                profile,
                StoreMode::Fingerprint,
                8,
                Some(19),
            );
            assert!(
                matches!(&result.verdict,Verdict::Unsupported {feature} if feature=="TLC initial-predecessor random counterexample replay is not proved"),
                "{result:?}"
            );
            let stats = result.stats.as_ref().expect("stopping prefix");
            assert_eq!((stats.generated, stats.distinct, stats.depth), (2, 2, 2));
            assert!(result.counterexample.is_none());
            assert_eq!((result.workers.requested, result.workers.used), (8, 1));
        }
    }
}

#[test]
fn unproved_concrete_shapes_do_not_turn_into_certified_random_view_traces() {
    for initial in [
        "{<<1>>}",
        "{[a |-> 1,b |-> 1],<<1>>}",
        "[i \\in 1..2 |-> i]",
        "1..2",
    ] {
        for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
            let result = run(
                "n",
                true,
                initial,
                "n<3",
                profile,
                StoreMode::Exact,
                4,
                Some(19),
            );
            assert!(
                matches!(&result.verdict,Verdict::Unsupported {feature} if feature.starts_with("random VIEW") || (initial == r"[i \in 1..2 |-> i]" && feature == "randomness under VIEW needs transformed-fingerprint seeding and independent replay")),
                "{initial}: {result:?}"
            );
            assert!(result.counterexample.is_none());
            if initial == r"[i \in 1..2 |-> i]" {
                assert_eq!(result.workers.used, 0);
                assert!(result.stats.is_none());
            } else {
                assert_eq!(result.stats.as_ref().expect("prefix").distinct, 0);
            }
        }
    }
}
