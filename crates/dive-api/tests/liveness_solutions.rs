//! Pinned strong-fairness controls on Core: every configured property is
//! kept and each violation's word is independently validated. These ran on
//! the retired reference final analysis until #791.
#![allow(clippy::expect_used, clippy::panic, reason = "fixture assertions")]
#[path = "support/core_final.rs"]
mod core_final;
use core_final::Outcome;
use dive_runtime::CheckOptions;

const SOURCE: &str = include_str!("../../dive-liveness/tests/fixtures/solutions/Solutions.tla");

#[test]
fn pinned_solution_controls_keep_all_properties_and_validated_witnesses() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for property in ["Hold", "Fail", "Hold Fail FailOther", "Residual"] {
            let cfg = format!("CONSTANT N = 3\nSPECIFICATION Spec\nPROPERTY {property}\n");
            let run = core_final::check(SOURCE, &cfg, &options)
                .unwrap_or_else(|stop| panic!("{property}: {stop}"));
            let stats = match run.outcome() {
                Outcome::Holds(exploration) => exploration.stats,
                Outcome::Violation(violation) => violation.candidate().exploration.stats,
                Outcome::Safety(result) => panic!("{property}: safety result {result:?}"),
            };
            assert_eq!(
                (stats.generated, stats.distinct, stats.depth),
                (25, 8, 4),
                "{property}"
            );
            if property == "Hold" {
                assert!(matches!(run.outcome(), Outcome::Holds(_)), "{property}");
                continue;
            }
            let Outcome::Violation(violation) = run.outcome() else {
                panic!("expected violated {property}");
            };
            if property == "Hold Fail FailOther" {
                assert_eq!(violation.candidate().properties, ["Fail", "FailOther"]);
            }
            assert_eq!(
                violation.validation().validation,
                dive_trace::Validation::Valid,
                "{property}"
            );
        }
    }
}
