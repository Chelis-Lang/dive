//! Compare arithmetic against the unchanged pinned Java updater observations.
#![allow(
    clippy::expect_used,
    reason = "fixed primary fixture contains valid elapsed inputs"
)]

use dive_liveness::periodic::{RatioError, calculate_runtime_ratio};

#[path = "fixtures/runtime_ratio_cases.rs"]
mod fixtures;

fn canonical_bits(value: f64) -> u64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        value.to_bits()
    }
}

#[test]
fn all_primary_numeric_rows_match_within_the_observed_clock_bracket() {
    assert_eq!(fixtures::CASES.len(), 196);
    for &(previous, start, interval, delta, before, after, expected) in fixtures::CASES {
        assert!(after >= before && after - before <= 1_000);
        assert!((before..=after).any(|now| {
            canonical_bits(
                calculate_runtime_ratio(f64::from_bits(previous), start, now, interval, delta)
                    .expect("nonnegative primary elapsed time"),
            ) == expected
        }));
    }
}

#[test]
fn negative_elapsed_is_refused_like_the_primary_assertion_controls() {
    for elapsed in [-1, -60_000, i64::MIN] {
        assert_eq!(
            calculate_runtime_ratio(0.2, 0, 120_000, 60_000, elapsed),
            Err(RatioError::NegativeElapsed)
        );
    }
}
