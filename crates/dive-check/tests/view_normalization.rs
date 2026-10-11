//! TLC normalizes concrete variables before VIEW, including ignored values.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{CheckOptions, Verdict};
use dive_store::StoreMode;
use std::sync::Arc;

fn run(
    value: &str,
    profile: dive_eval::Profile,
    store: StoreMode,
    width: usize,
    chaos: Option<u64>,
) -> dive_check::CheckResult {
    let source = format!(
        "---- MODULE ViewNormalize ----\nEXTENDS Naturals,TLC\nVARIABLES n,x,s\nInit == n = 0 /\\ x = 0 /\\ s = {value}\nNext == n < 3 /\\ n' = n+1 /\\ x' = n /\\ UNCHANGED s\nInv == n < 3\nProjection == n\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("ViewNormalize.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("valid IR"));
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW Projection\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    let mut options = CheckOptions::tlc(0, 7);
    options.profile = profile;
    options.eval = dive_eval::Options::for_profile(profile);
    options.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
    options.store = store;
    options.workers = width;
    options.chaos = chaos;
    dive_check::check(&ir, &cfg, &options)
}

#[test]
fn ignored_unnormalizable_variable_refuses_before_a_false_search_result() {
    // Clean pinned TLC throws EC1000/exit255 during initial fingerprinting.
    // Before the fix dive instead reported Inv with four unprintable states.
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        for width in [1, 4, 8] {
            for chaos in [None, Some(19)] {
                let result = run(
                    "{<<1>>,[a |-> 1,b |-> 1]}",
                    dive_eval::Profile::Tlc,
                    store,
                    width,
                    chaos,
                );
                assert!(
                    matches!(&result.verdict, Verdict::Unsupported { feature } if feature.contains("deep normalization")),
                    "{result:?}"
                );
                assert!(result.counterexample.is_none());
                assert_eq!(result.output, Vec::<String>::new());
                assert_eq!(
                    result
                        .stats
                        .as_ref()
                        .expect("stopped-prefix evidence")
                        .distinct,
                    0
                );
            }
        }
    }
}

#[test]
fn compound_and_approved_canonical_view_controls_keep_concrete_counts() {
    for profile in [dive_eval::Profile::Tlc, dive_eval::Profile::Dive] {
        for value in [
            "{2,1}",
            "<<{2,1},3>>",
            "[a |-> {2,1}]",
            "[i \\in 1..2 |-> {2,1}]",
            "{[a |-> 1,b |-> 1],<<1>>}",
        ] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                for width in [1, 4, 8] {
                    for chaos in [None, Some(19)] {
                        let result = run(value, profile, store, width, chaos);
                        assert!(
                            matches!(&result.verdict, Verdict::InvariantViolated { property, .. } if property == "Inv"),
                            "{value}: {result:?}"
                        );
                        let stats = result.stats.as_ref().expect("statistics");
                        assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 4, 4));
                        assert_eq!(
                            result
                                .counterexample
                                .as_ref()
                                .expect("concrete witness")
                                .states
                                .len(),
                            4
                        );
                    }
                }
            }
        }
    }
}
