//! Source-bound rows require certified demand and fresh finite validation.
//! Borrowed-model legacy refusal coverage remains in dive-liveness. The typed
//! state-cause transport of the retired reference final route (#791) is not
//! exposed by Core, which refuses the temporal failure report by name.
#![allow(clippy::expect_used, reason = "failure transport assertions")]
#[path = "support/refused.rs"]
mod refused;

#[test]
fn ordinary_file_reports_require_certified_error_and_actual_source_row_counts() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/state-atom-error/StateCause.tla");
    for compat_tlc in [false, true] {
        let report = dive_api::check_file_engine(
            &dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..dive_api::CheckRequest::default()
            },
            dive_api::Engine::Core,
        )
        .expect("ordinary check");
        // Only the retired reference engine reported this liveness-atom
        // error (TLC 2179 from a temporal state row, exit 255, a certified
        // one-state word) (#791, #910); Core refuses to build the report rather than
        // claim the property.
        refused::assert_refused(
            &report.to_json(),
            report.exit_code(),
            refused::TEMPORAL_FAILURE_REPORT,
        );
    }
}
