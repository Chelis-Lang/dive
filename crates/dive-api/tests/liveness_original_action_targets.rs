//! Constructor-alias action targets on Core, beyond the TLC-profile cases of
//! `original_action_targets.rs`: exact storage refuses by name, and the
//! default profile's value semantics hold with the pinned counts. These ran
//! on the retired reference final analysis until #791.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use dive_runtime::CheckOptions;

const SOURCE: &str = include_str!("../../dive-liveness/tests/fixtures/constructor-alias.tla");

fn cases() -> Vec<(String, String, u64)> {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-liveness/tests/fixtures/constructor-alias/observations.json"
    ))
    .expect("compact primary observations");
    let cases = oracle["expected_cases"].as_array().expect("five controls");
    assert_eq!(cases.len(), 5);
    cases
        .iter()
        .map(|case| {
            (
                case["next"].as_str().expect("Next").to_owned(),
                case["property"].as_str().expect("property").to_owned(),
                case["generated"].as_u64().expect("generated"),
            )
        })
        .collect()
}

#[test]
fn exact_storage_with_these_temporal_properties_refuses_by_name() {
    let options = CheckOptions {
        store: dive_store::StoreMode::Exact,
        ..CheckOptions::tlc(0, 0)
    };
    for (next, property, _) in cases() {
        let report = core_final::run(
            SOURCE,
            &format!("INIT Init\nNEXT {next}\nPROPERTY {property}\n"),
            &options,
        );
        refused::assert_refused(
            &report.to_json(),
            report.exit_code(),
            refused::TEMPORAL_EXACT_STORE,
        );
    }
}

#[test]
fn default_value_semantics_hold_every_case_with_the_pinned_counts() {
    // TLC's selected record target is an error (exit 255) only under its
    // literal-class semantics; the default profile compares values.
    for (next, property, generated) in cases() {
        let run = core_final::check(
            SOURCE,
            format!("INIT Init\nNEXT {next}\nPROPERTY {property}\n"),
            &CheckOptions::dive(0),
        )
        .unwrap_or_else(|stop| panic!("{next}/{property}: {stop}"));
        let core_final::Outcome::Holds(exploration) = run.outcome() else {
            panic!("{next}/{property}: {:?}", run.outcome())
        };
        let s = exploration.stats;
        assert_eq!(
            (s.generated, s.distinct, s.depth),
            (generated, 1, 1),
            "{next}/{property}"
        );
    }
}
