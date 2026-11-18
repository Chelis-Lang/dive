//! Core's native final and prefix results keep the selected word and the
//! actual producer facts, and atom failures stay failures. These ran on the
//! retired reference analyses' `into_native` conversion until #791.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use dive_api::{CheckReport, PeriodicSchedule};
use dive_runtime::CheckOptions;
use serde_json::{Value, json};

fn core(
    property: &str,
    options: &CheckOptions,
    periodic: PeriodicSchedule,
) -> (CheckReport, Value) {
    let source = format!(
        "---- MODULE NativeResults ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<3 THEN x+1 ELSE x\nP == {property}\n====\n"
    );
    let files = core_final::Files::new(&source, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let request = dive_api::CheckRequest {
        periodic,
        ..files.request(options)
    };
    let report = dive_api::check_file(&request).expect("Core");
    let json = report.to_json();
    (report, json)
}

const FINAL_ONLY: PeriodicSchedule = PeriodicSchedule {
    min_states: u64::MAX,
    growth_factor: 2,
};
const EVERY_LEVEL: PeriodicSchedule = PeriodicSchedule {
    min_states: 2,
    growth_factor: 1,
};

#[test]
fn final_results_keep_the_selected_word_and_complete_facts() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for (property, holding) in [("<>[](x<4)", true), ("<>(x=4)", false)] {
            let (report, json) = core(property, &options, FINAL_ONLY);
            let s = &json["statistics"];
            assert_eq!(
                json!([
                    s["generated"],
                    s["distinct"],
                    s["depth"],
                    s["left_on_queue"]
                ]),
                json!([5, 4, 4, 0]),
                "{json}"
            );
            assert_eq!(json["execution"]["exhausted"], true, "{json}");
            if holding {
                assert_eq!(report.verdict(), "success", "{json}");
                assert!(json["counterexample"].is_null());
            } else {
                assert_eq!(report.verdict(), "liveness-violation", "{json}");
                assert_eq!(json["properties"], json!(["P"]));
                assert_eq!(json["counterexample"]["validation"], "valid");
            }
        }
    }
}

#[test]
fn periodic_final_and_real_prefix_results_preserve_distinct_producer_facts() {
    let options = CheckOptions::dive(7);
    let (report, json) = core("<>[](x<4)", &options, FINAL_ONLY);
    assert_eq!(report.verdict(), "success", "{json}");
    assert_eq!(json["options"]["seed"], 7);
    assert_eq!(json["periodic"]["exhausted"], true);
    assert_eq!(json["periodic"]["final_check"], true);

    let (report, json) = core("<>(x=4)", &options, EVERY_LEVEL);
    assert_eq!(report.verdict(), "liveness-violation", "{json}");
    assert_eq!(json["execution"]["stage"], "prefix", "{json}");
    assert!(
        json["statistics"]["left_on_queue"].as_u64() > Some(0),
        "{json}"
    );
    assert_eq!(json["periodic"]["exhausted"], false);
    assert_eq!(json["periodic"]["final_check"], false);
    assert_eq!(json["counterexample"]["validation"], "valid");
}

#[test]
fn actual_final_and_prefix_failures_keep_their_error() {
    // The reference analyses kept the typed atom error and its exploration;
    // Core refuses the temporal failure report by name in both routes.
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for periodic in [FINAL_ONLY, EVERY_LEVEL] {
            let (report, json) = core("[]<>(1 \\div (x-x)=0)", &options, periodic);
            refused::assert_refused(&json, report.exit_code(), refused::TEMPORAL_FAILURE_REPORT);
        }
    }
}
