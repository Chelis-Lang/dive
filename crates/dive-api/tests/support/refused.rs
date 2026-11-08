//! Named Core refusals of cases only the retired reference engine ran (#791).
//!
//! A test that measured the reference engine on a feature Core does not
//! implement now asserts that Core refuses that case by name: verdict
//! `unsupported`, exit 3 and the feature in the message. The features and
//! the tests they came from are listed on the issue that tracks restoring
//! this coverage when Core implements them.

#![allow(dead_code, reason = "each test binary uses a subset")]

use serde_json::Value;

/// `--chaos` scheduling perturbation.
pub const CHAOS: &str = "--chaos under the core engine";
/// TLC's checked comparison of values of different shapes under `--compat=tlc`.
pub const MIXED_SHAPES: &str = "of different shapes under --compat=tlc";
/// A TLC runtime name whose string order depends on evaluation order.
pub const RUNTIME_NAME: &str = "which TLC first meets after start-up";
/// An evaluation error whose TLC context Core does not reproduce.
pub const UNREPRODUCED_ERROR: &str =
    "an evaluation error the reference evaluator does not reproduce";

/// Asserts that `report` (a report's JSON) with process exit `exit` is
/// Core's named refusal of `feature`.
///
/// # Panics
///
/// Panics when the report is not that refusal.
pub fn assert_refused(report: &Value, exit: u8, feature: &str) {
    assert_eq!(report["verdict"], "unsupported", "{report}");
    assert_eq!(exit, 3, "{report}");
    assert!(
        report["message"]
            .as_str()
            .is_some_and(|message| message.contains(feature)),
        "expected a refusal naming {feature:?}: {report}"
    );
    assert!(report["counterexample"].is_null(), "{report}");
}
