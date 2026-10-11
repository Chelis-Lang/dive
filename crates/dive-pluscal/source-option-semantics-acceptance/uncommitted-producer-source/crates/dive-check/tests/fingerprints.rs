//! The default profile's state fingerprints (ADR 0015 §5, issue #42).
//!
//! dive's former two-level fingerprint (FP64 over FP64 value fingerprints)
//! merged 64 of `MCWriteThroughCache`'s 5,196 states: the same model value
//! changed in two variables cancelled. The default profile now keys the
//! fingerprint set on the non-linear combination, and must find TLC's counts;
//! so must `--compat=tlc` (TLC's own fingerprints) and exact mode.
//!
//! The model is vendored in `tests/fixtures/examples/` (Examples `6a7b26a`,
//! MIT; see the `LICENSE.md` there).

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::PathBuf;
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Stats, Verdict, check};
use dive_store::StoreMode;

const MODEL: &str = "specifications/SpecifyingSystems/CachingMemory/MCWriteThroughCache";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(opts: CheckOptions) -> CheckResult {
    let dir = crate_dir().join("tests/fixtures/examples");
    let tla = dir.join(format!("{MODEL}.tla"));
    let cfg = dir.join(format!("{MODEL}.cfg"));
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || {
            let analysis =
                dive_sema::analyze_file(&tla, &dive_sema::Options::default()).expect("readable");
            assert!(analysis.accepted(), "{} rejected", tla.display());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
            let cfg = dive_config::parse(&std::fs::read_to_string(&cfg).expect("cfg"))
                .into_config()
                .expect("cfg parses");
            check(&ir, &cfg, &opts)
        })
        .expect("thread")
        .join()
        .expect("no panic")
}

/// TLC's result: `conformance/golden/tlc-2026.10.06.014338/examples/`.
fn tlc_stats() -> Stats {
    let golden = crate_dir().join(format!(
        "../../conformance/golden/tlc-2026.10.06.014338/examples/{MODEL}.cfg.json"
    ));
    let text = std::fs::read_to_string(golden).expect("the golden");
    for field in [
        "\"distinct_states\": 5196,",
        "\"states_generated\": 28170,",
        "\"depth\": 18",
    ] {
        assert!(text.contains(field), "the golden records {field}");
    }
    Stats {
        generated: 28_170,
        distinct: 5_196,
        left_on_queue: 0,
        depth: 18,
        initial_generated: 2,
    }
}

#[test]
fn default_profile_finds_every_state_of_mc_write_through_cache() {
    let want = tlc_stats();
    let r = run(CheckOptions::dive(0));
    assert_eq!(r.verdict, Verdict::Success);
    assert!(
        !r.tlc_fingerprints,
        "the default profile uses dive's scheme"
    );
    assert_eq!(r.stats, Some(want));
    assert!(
        r.collision.is_some(),
        "fingerprint mode reports the estimates"
    );
}

#[test]
fn compat_and_exact_agree_on_mc_write_through_cache() {
    let want = tlc_stats();
    let r = run(CheckOptions::tlc(0, 0));
    assert!(
        r.tlc_fingerprints,
        "--compat=tlc keys on TLC's fingerprints"
    );
    assert_eq!((r.verdict, r.stats), (Verdict::Success, Some(want)));
    let mut exact = CheckOptions::dive(0);
    exact.store = StoreMode::Exact;
    let r = run(exact);
    assert_eq!((r.verdict, r.stats), (Verdict::Success, Some(want)));
}
