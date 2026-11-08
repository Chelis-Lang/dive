//! A generous run budget changes nothing in a temporal check on Core. This
//! ran on the retired reference final analysis until #791; the exhausted
//! budget's stop is checked on Core by `budget.rs`
//! (`an_exhausted_time_limit_stops_graph_capture_with_no_temporal_claim`).
#![allow(clippy::expect_used, clippy::panic, reason = "test setup")]

#[path = "support/core_final.rs"]
mod core_final;

use core_final::Files;
use dive_api::{BudgetSetting, Engine};
use dive_runtime::CheckOptions;
use std::time::Duration;

#[test]
fn a_generous_budget_finds_the_same_candidate_as_the_default() {
    let files = Files::new(
        "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nP == []<>x\n====\n",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    );
    // Without fairness the stuttering lasso at x = FALSE violates P.
    let candidate = |time_limit, max_memory| {
        let request = dive_api::CheckRequest {
            time_limit,
            max_memory,
            ..files.request(&CheckOptions::tlc(0, 0))
        };
        let report = dive_api::check_file_engine(&request, Engine::Core).expect("Core");
        let json = report.to_json();
        assert_eq!(json["verdict"], "liveness-violation", "{json}");
        let temporal = report.temporal_result().expect("temporal result");
        let dive_api::temporal::Outcome::NativeViolation(v) = temporal.outcome() else {
            panic!("expected a violation: {json}")
        };
        (v.word().states.clone(), v.word().back_to)
    };
    assert_eq!(
        candidate(
            BudgetSetting::Explicit(Duration::from_secs(3600)),
            BudgetSetting::Explicit(u64::MAX)
        ),
        candidate(BudgetSetting::Default, BudgetSetting::Default)
    );
}
