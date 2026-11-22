//! Source-bound rows require certified demand and fresh finite validation.
//! State-label diagnostics retain their source without certifying a row error.
#![allow(clippy::expect_used, reason = "failure transport assertions")]
#[path = "support/refused.rs"]
mod refused;

#[test]
fn ordinary_file_reports_require_certified_error_and_actual_source_row_counts() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/state-atom-error/StateCause.tla");
    for compat_tlc in [false, true] {
        let report = dive_api::check_file(&dive_api::CheckRequest {
            file: file.clone(),
            compat_tlc,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..dive_api::CheckRequest::default()
        })
        .expect("ordinary check");
        // TLC reports 2179 at a temporal row. A failed state label
        // alone supplies no certified row demand or finite word.
        refused::assert_refused(
            &report.to_json(),
            report.exit_code(),
            "TLC_MODULE_DIVISION_BY_ZERO",
        );
    }
}

#[test]
fn a_response_label_on_a_missing_lease_names_its_source_and_keeps_the_capture() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/state-atom-error");
    for compat_tlc in [false, true] {
        let request = dive_api::CheckRequest {
            file: directory.join("Lease.tla"),
            compat_tlc,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..dive_api::CheckRequest::default()
        };
        let report = dive_api::check_file(&request).expect("response check");
        let json = report.to_json();
        refused::assert_refused(&json, report.exit_code(), "liveness atom for Response");
        assert!(json["tlc"].is_null());
        assert!(json["property"].is_null());
        assert_eq!(json["diagnostics"][0]["code"], "DIVE-L0033");
        assert_eq!(
            json["diagnostics"][0]["location"],
            "line 7, col 39 to line 7, col 55 of module Lease"
        );
        let message = json["message"].as_str().expect("evaluation cause");
        assert!(message.contains("select field \"counter\" from a non-record value NoLease"));
        assert!(message.contains("line 7, col 39 to line 7, col 51 of module Lease"));
        assert_eq!(json["execution"]["stage"], "atoms");
        assert_eq!(json["execution"]["capture_completed"], true);
        assert_eq!(json["statistics"]["distinct"], 1);
        assert_eq!(json["statistics"]["generated"], 2);
        assert_eq!(json["statistics"]["depth"], 1);

        let domains = dive_api::check_file(&dive_api::CheckRequest {
            config: Some(directory.join("Domains.cfg")),
            ..request
        })
        .expect("isolated comparison");
        if compat_tlc {
            refused::assert_refused(
                &domains.to_json(),
                domains.exit_code(),
                "equality between values of different shapes",
            );
        } else {
            assert_eq!(domains.to_json()["verdict"], "success");
        }
    }
}
