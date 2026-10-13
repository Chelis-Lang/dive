//! Transformed random streams uniformly refuse before binding or search.
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

fn assert_refusal(result: &dive_check::CheckResult, width: usize) {
    assert!(
        matches!(&result.verdict, Verdict::Unsupported { feature }
        if feature == "randomness under VIEW is not supported"),
        "{result:?}"
    );
    assert_eq!((result.workers.requested, result.workers.used), (width, 0));
    assert!(
        result.stats.is_none() && result.counterexample.is_none() && result.collision.is_none()
    );
    assert!(result.output.is_empty() && result.warnings.is_empty());
}

#[test]
fn primary_view_shapes_keep_the_uniform_pre_binding_refusal() {
    // Keep the previously measured inputs; transformed RNG is explicitly
    // unsupported under the rescue policy, regardless of stored value shape.
    for view in [
        "n",
        "<<n>>",
        "[phase |-> n]",
        "<<n,x>>",
        "IF n % 2 = 0 THEN n ELSE n+100",
    ] {
        for configured in [false, true] {
            for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
                for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                    for width in [1, 4, 8] {
                        for chaos in [None, Some(19)] {
                            assert_refusal(
                                &run(
                                    view,
                                    configured,
                                    "<<{2,1},[a |-> TRUE]>>",
                                    "n<3",
                                    profile,
                                    store,
                                    width,
                                    chaos,
                                ),
                                width,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn transformed_first_successor_refuses_before_search() {
    for configured in [false, true] {
        for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
            assert_refusal(
                &run(
                    "n",
                    configured,
                    "{}",
                    "n<1",
                    profile,
                    StoreMode::Fingerprint,
                    8,
                    Some(19),
                ),
                8,
            );
        }
    }
}

#[test]
fn compound_and_mixed_state_inputs_keep_the_uniform_pre_binding_refusal() {
    for initial in ["{<<1>>}", "{[a |-> 1,b |-> 1],<<1>>}", "1..2"] {
        for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
            assert_refusal(
                &run(
                    "n",
                    true,
                    initial,
                    "n<3",
                    profile,
                    StoreMode::Exact,
                    4,
                    Some(19),
                ),
                4,
            );
        }
    }
}

#[test]
fn nonliteral_function_initial_value_keeps_the_pre_binding_refusal() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        assert_refusal(
            &run(
                "n",
                true,
                r"[i \in 1..2 |-> i]",
                "n<3",
                profile,
                StoreMode::Exact,
                4,
                Some(19),
            ),
            4,
        );
    }
}
