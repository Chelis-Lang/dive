//! Completed-level (periodic) temporal checking on Core: prefix words carry
//! the actual prefix counts and are independently validated, holding needs
//! real exhaustion, and attribution matches final checking. These ran on the
//! retired reference periodic analysis until #791. In the TLC profile Core
//! ignores the schedule and checks once at the end (D-014).
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use dive_api::{CheckReport, PeriodicSchedule};
use dive_runtime::CheckOptions;
use serde_json::{Value, json};

/// Checks every grown completed level, so small graphs exercise prefixes.
const EVERY_LEVEL: PeriodicSchedule = PeriodicSchedule {
    min_states: 2,
    growth_factor: 1,
};
const CHAIN: &str = "Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\n";

fn source(body: &str) -> String {
    format!("---- MODULE Periodic ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n")
}

/// Core's report and JSON for `body` with `properties`, at `workers`, with
/// the every-level schedule.
fn core(body: &str, properties: &str, options: &CheckOptions) -> (CheckReport, Value) {
    let files = core_final::Files::new(
        &source(body),
        &format!("INIT Init\nNEXT Next\n{properties}"),
    );
    let request = dive_api::CheckRequest {
        periodic: EVERY_LEVEL,
        ..files.request(options)
    };
    let report = dive_api::check_file(&request).expect("Core");
    let json = report.to_json();
    (report, json)
}

fn options(base: &CheckOptions, workers: usize) -> CheckOptions {
    CheckOptions {
        workers,
        ..base.clone()
    }
}

fn counts(json: &Value) -> Value {
    let s = &json["statistics"];
    json!([
        s["generated"],
        s["distinct"],
        s["depth"],
        s["left_on_queue"]
    ])
}

#[test]
fn early_word_has_actual_prefix_counts_and_fresh_independent_validation() {
    let body = format!("{CHAIN}P == <>(x=3)");
    for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let tlc = base.profile == dive_eval::Profile::Tlc;
        for width in [1, 4, 8] {
            let (report, json) = core(&body, "PROPERTY P\n", &options(&base, width));
            assert_eq!(report.verdict(), "liveness-violation", "{json}");
            assert_eq!(json["properties"], json!(["P"]), "{json}");
            assert_eq!(json["counterexample"]["validation"], "valid", "{json}");
            if tlc {
                // One final check over the exhausted graph.
                assert_eq!(counts(&json), json!([4, 3, 3, 0]), "{json}");
                assert_eq!(json["execution"]["exhausted"], true, "{json}");
            } else {
                assert_eq!(counts(&json), json!([2, 2, 2, 1]), "{json}");
                let periodic = &json["periodic"];
                assert_eq!(
                    json!([
                        periodic["prefix_checks"],
                        periodic["last_level"],
                        periodic["last_size"]
                    ]),
                    json!([1, 1, 2]),
                    "{json}"
                );
                assert_eq!(periodic["exhausted"], false);
                assert_eq!(periodic["final_check"], false);
                assert_eq!(json["execution"]["stage"], "prefix", "{json}");
                assert_eq!(
                    json!([
                        json["options"]["workers"]["requested"],
                        json["options"]["workers"]["used"]
                    ]),
                    json!([width, width]),
                    "{json}"
                );
            }
        }
    }
}

#[test]
fn holding_requires_real_exhaustion_and_drained_levels_never_trigger_prefix_checks() {
    for width in [1, 4, 8] {
        let options = options(&CheckOptions::dive(0), width);
        for (body, expected) in [
            (format!("{CHAIN}P == []<>(x<=2)"), (4, 3, 3, 2)),
            (
                "Init == x=0\nNext == x'=x\nP == []<>(x=0)".into(),
                (2, 1, 1, 0),
            ),
        ] {
            let (report, json) = core(&body, "PROPERTY P\n", &options);
            assert_eq!(report.verdict(), "success", "{json}");
            let s = &json["statistics"];
            assert_eq!(
                json!([
                    s["generated"],
                    s["distinct"],
                    s["depth"],
                    json["periodic"]["prefix_checks"]
                ]),
                json!([expected.0, expected.1, expected.2, expected.3]),
                "{json}"
            );
            assert_eq!(s["left_on_queue"], 0);
            assert_eq!(json["periodic"]["exhausted"], true);
            assert_eq!(json["periodic"]["final_check"], true);
        }
    }
}

#[test]
fn selected_original_constructor_error_keeps_typed_cause_on_prefix() {
    // The reference periodic route reported the selected record target's
    // typed action error (General); Core refuses that failure report by name.
    let body = "Init == x=[k \\in {\"a\"} |-> 0]\nNext == x'=[a |-> 0] \\/ x'=0\nP == <>[][~(x' = <<0,0>>)]_x";
    let (report, json) = core(body, "PROPERTY P\n", &CheckOptions::tlc(0, 0));
    refused::assert_refused(&json, report.exit_code(), refused::TEMPORAL_FAILURE_REPORT);
}

#[test]
fn existing_pinned_fair_and_unfair_words_anchor_both_periodic_profiles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes");
    for name in ["LvNoFair", "LvWFIntermittent"] {
        let directory = root.join("specs").join(name);
        let oracle = root.join("tlc-2026.10.06.014338");
        assert_eq!(
            std::fs::read_to_string(oracle.join(format!("{name}.exit")))
                .expect("pinned status")
                .trim(),
            "13"
        );
        let file = directory.join(format!("{name}.tla"));
        for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for width in [1, 4, 8] {
                let request = dive_api::CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    periodic: EVERY_LEVEL,
                    ..core_final::request(&options(&base, width))
                };
                let report = dive_api::check_file(&request).expect("Core");
                let json = report.to_json();
                assert_eq!(report.verdict(), "liveness-violation", "{name}: {json}");
                assert_eq!(
                    report.exit_code_for(dive_api::ExitConvention::Tlc),
                    13,
                    "{name}: {json}"
                );
                assert_eq!(json["properties"], json!(["Prop"]), "{name}: {json}");
                assert_eq!(
                    json["counterexample"]["validation"], "valid",
                    "{name}: {json}"
                );
                if json["execution"]["stage"] == "prefix" {
                    assert!(json["statistics"]["left_on_queue"].as_u64() > Some(0));
                } else {
                    assert_eq!(json["statistics"]["left_on_queue"], 0, "{name}: {json}");
                }
            }
        }
    }
}

#[test]
fn prefixes_decide_only_the_first_property_so_attribution_matches_final_checking() {
    // P2 fails on the first prefix by stuttering at x=0; P1 fails only once
    // x reaches 5. A final-only search names P1 first, so the prefix must not
    // stop with P2's word.
    let body = "Init == x=0\nNext == x'=IF x<6 THEN x+1 ELSE x\n\
                P1 == [](x>=5 => <>(x=9))\nP2 == <>(x=9)";
    for width in [1, 4] {
        let (report, json) = core(
            body,
            "PROPERTY P1\nPROPERTY P2\n",
            &options(&CheckOptions::dive(0), width),
        );
        assert_eq!(report.verdict(), "liveness-violation", "{json}");
        // P2's word exists from level 1, but only P1's word, first reachable
        // at level 6 with x=6 still queued, may stop the run.
        assert_eq!(json["execution"]["level"], 6, "{json}");
        assert_eq!(json["periodic"]["prefix_checks"], 6, "{json}");
        assert_eq!(json["periodic"]["last_level"], 6, "{json}");
        assert_eq!(json["periodic"]["exhausted"], false);
        assert_eq!(json["properties"], json!(["P1", "P2"]), "{json}");
    }
}
