//! A model with no bound action ends after Init, as TLC's `modelCheck` does
//! (#231), and the reported depth counts generated initial states.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_check::{CheckOptions, Verdict, check};
use std::sync::Arc;

fn run(src: &str, cfg: &str, opts: &CheckOptions) -> dive_check::CheckResult {
    let name = src.split_whitespace().nth(2).expect("module header");
    let analysis =
        dive_sema::analyze_source(&format!("{name}.tla"), src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg");
    check(&ir, &cfg, opts)
}

fn options() -> Vec<CheckOptions> {
    let mut all = Vec::new();
    for workers in [1, 4, 8] {
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            all.push(CheckOptions { workers, ..base });
        }
    }
    all
}

const NO_ACTIONS: &str = "---- MODULE NoActions ----\nVARIABLE x\nInit == x \\in {}\nNext == \\E y \\in {}: x' = y\nPost == x /= x\n====\n";

#[test]
fn no_bound_action_skips_postconditions() {
    // Pinned TLC: 0 states generated, depth 0, exit 0.
    for opts in options() {
        let r = run(NO_ACTIONS, "INIT Init\nPOSTCONDITION Post\n", &opts);
        assert_eq!(r.verdict, Verdict::Success, "{:?}", r.verdict);
        let stats = r.stats.expect("stats");
        assert_eq!((stats.generated, stats.distinct, stats.depth), (0, 0, 0));
    }
}

#[test]
fn an_action_with_no_successors_still_checks_postconditions() {
    // Pinned TLC: "Evaluating postcondition Post ... failed", exit 10.
    for opts in options() {
        let r = run(
            NO_ACTIONS,
            "INIT Init\nNEXT Next\nPOSTCONDITION Post\n",
            &opts,
        );
        assert_eq!(
            r.verdict.tlc().map(|class| class.code),
            Some(2405),
            "{:?}",
            r.verdict
        );
    }
}

#[test]
fn initial_states_excluded_by_constraints_report_depth_one() {
    // Pinned TLC: 1 state generated, 0 distinct, depth 1, exit 0.
    let src = "---- MODULE Excluded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x\nBig == x > 5\n====\n";
    for opts in options() {
        let r = run(src, "INIT Init\nNEXT Next\nCONSTRAINT Big\n", &opts);
        assert_eq!(r.verdict, Verdict::Success, "{:?}", r.verdict);
        let stats = r.stats.expect("stats");
        assert_eq!((stats.generated, stats.distinct, stats.depth), (1, 0, 1));
    }
}
