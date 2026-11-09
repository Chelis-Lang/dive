//! Named Core refusals of cases only the retired reference engine ran (#791).
//!
//! A test that measured the retired Reference engine on a feature Core does not
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
/// `RandomElement` reached in finite simulation or DFID.
pub const FINITE_RANDOM: &str =
    "TLC!RandomElement: native finite mode runtime effect is unsupported";
/// TLC runtime strings in finite simulation or DFID, whose order depends on
/// TLC's encounter order.
pub const FINITE_STRING_ORDER: &str = "native finite mode runtime string encounter order";
/// `TLCGet` or `TLCSet` in finite simulation or DFID.
pub const FINITE_HOST_ACCESS: &str = "native finite mode TLCGet/TLCSet host access";
/// An effectful LET definition or operator argument used in more than one
/// conjunct, which TLC evaluates again in each.
pub const EFFECT_REUSED_IN_CONJUNCTS: &str =
    "with an effect (Print, PrintT, RandomElement) used in more than one conjunct";
/// Bounded final temporal checking (`--lncheck final --lasso short`).
pub const FINAL_SHORT: &str =
    "bounded final checking (--lncheck final --lasso short) is not implemented on Core";
/// `--max-states` or `--max-depth` with temporal properties.
pub const TEMPORAL_CAP: &str = "--max-states or --max-depth with temporal properties";
/// Exact or spilled seen storage with temporal properties.
pub const TEMPORAL_EXACT_STORE: &str =
    "exact or spilled native temporal storage under the core engine";
/// TLC action composition (`--action-composition=tlc`, TLC's `cdot`).
pub const ACTION_COMPOSITION: &str = "--action-composition=tlc under the core engine";
/// A configured `POSTCONDITION`.
pub const POSTCONDITION: &str = "POSTCONDITION (M0 core engine)";
/// A temporal validation failure (an error while checking temporal
/// properties at start-up) whose report Core does not build.
pub const TEMPORAL_VALIDATION_FAILURE_REPORT: &str =
    "a temporal validation failure (core): its report is not implemented";
/// A temporal failure whose report Core does not build.
pub const TEMPORAL_FAILURE_REPORT: &str =
    "a temporal failure (core): its report is not implemented";
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
    // Check reports name the verdict `verdict`; simulation and DFID, `result`.
    let verdict = if report["verdict"].is_null() {
        &report["result"]
    } else {
        &report["verdict"]
    };
    assert_eq!(verdict, "unsupported", "{report}");
    assert_eq!(exit, 3, "{report}");
    assert!(
        report["message"]
            .as_str()
            .is_some_and(|message| message.contains(feature)),
        "expected a refusal naming {feature:?}: {report}"
    );
    assert!(report["counterexample"].is_null(), "{report}");
}
