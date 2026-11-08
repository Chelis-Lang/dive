//! Primary nested ENABLED references keep their TLC counts, truth and
//! independently validated words on Core, directly and through first-order
//! temporal forwarding. They ran on the retired reference final analysis
//! until #791. Exact storage with temporal properties is a named Core refusal.
#![allow(clippy::expect_used, clippy::panic, reason = "tiny primary assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use core_final::Outcome;
use dive_runtime::CheckOptions;
use dive_trace::Validation;
use serde_json::Value;

const SOURCE: &str =
    include_str!("../../dive-liveness/tests/fixtures/nested-enabled/NestedEnabled.tla");
const HOLD: &str = include_str!("../../dive-liveness/tests/fixtures/nested-enabled/hold.cfg");
const FAIL: &str = include_str!("../../dive-liveness/tests/fixtures/nested-enabled/fail.cfg");

/// Checks `source` with `cfg` and asserts the verdict, counts and word.
fn verify(source: &str, cfg: &str, fail: bool, options: &CheckOptions, counts: (u64, u64, u64)) {
    let run = core_final::check(source, cfg, options).unwrap_or_else(|stop| panic!("{stop}"));
    match run.outcome() {
        Outcome::Holds(exploration) => {
            assert!(!fail, "{}", run.report().to_json());
            let s = exploration.stats;
            assert_eq!((s.generated, s.distinct, s.depth), counts);
        }
        Outcome::Violation(violation) => {
            assert!(fail, "{}", run.report().to_json());
            let candidate = violation.candidate();
            assert_eq!(candidate.properties, ["Fail"]);
            let s = candidate.exploration.stats;
            assert_eq!((s.generated, s.distinct, s.depth), counts);
            assert_eq!(violation.validation().validation, Validation::Valid);
        }
        Outcome::Safety(result) => panic!("{result:?}"),
    }
}

#[test]
fn two_primary_counts_truth_and_native_words_match_in_both_profiles_and_stores() {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../dive-liveness/tests/fixtures/nested-enabled/oracle.json"
    ))
    .expect("oracle");
    assert_eq!(oracle["primary_before_implementation"], true);
    let counts = (
        oracle["generated"].as_u64().expect("generated"),
        oracle["distinct"].as_u64().expect("distinct"),
        oracle["depth"].as_u64().expect("depth"),
    );
    for tlc in [true, false] {
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            let options = CheckOptions {
                store,
                ..if tlc {
                    CheckOptions::tlc(0, 0)
                } else {
                    CheckOptions::dive(0)
                }
            };
            for (cfg, fail) in [(HOLD, false), (FAIL, true)] {
                if store == dive_store::StoreMode::Exact {
                    let report = core_final::run(SOURCE, cfg, &options);
                    refused::assert_refused(
                        &report.to_json(),
                        report.exit_code(),
                        refused::TEMPORAL_EXACT_STORE,
                    );
                    continue;
                }
                verify(SOURCE, cfg, fail, &options, counts);
            }
        }
    }
}

#[test]
fn forwarded_nested_queries_keep_recurrence_candidates() {
    // Nested action queries, fairness recurrences and first-order temporal
    // forwarding together.
    let source = SOURCE
        .replace(
            "Hold == []<>(x = 1)",
            "Relay(p) == p\nHold == Relay([]<>(x = 1))",
        )
        .replace("Fail == []<>(x = 2)", "Fail == Relay([]<>(x = 2))");
    for tlc in [true, false] {
        for workers in [1, 4] {
            let options = CheckOptions {
                workers,
                ..if tlc {
                    CheckOptions::tlc(0, 0)
                } else {
                    CheckOptions::dive(0)
                }
            };
            for (cfg, fail) in [(HOLD, false), (FAIL, true)] {
                verify(&source, cfg, fail, &options, (3, 2, 2));
            }
        }
    }
}
