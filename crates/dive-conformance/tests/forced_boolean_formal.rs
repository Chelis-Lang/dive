//! D-047 excuses only the measured `ForcedFormal` generated-count pair.

use dive_conformance::compare::{CompareContext, FieldDiff, diff_results};
use dive_conformance::divergence::{Profile, Registry};
use dive_conformance::tlc::result::{OracleResult, Verdict};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FORCED: &str = "specifications/ForcedFormal/ForcedFormal.cfg";
const UNFORCED: &str = "specifications/ForcedFormal/UnforcedFormal.cfg";
const FORCED_GOLDEN: &str = include_str!(
    "../../../conformance/golden/tlc-2026.10.06.014338/forced-boolean-formal/specifications/ForcedFormal/ForcedFormal.cfg.json"
);
const UNFORCED_GOLDEN: &str = include_str!(
    "../../../conformance/golden/tlc-2026.10.06.014338/forced-boolean-formal/specifications/ForcedFormal/UnforcedFormal.cfg.json"
);

fn registry() -> Result<Registry, Box<dyn std::error::Error>> {
    Ok(Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?)
}

/// Parses a golden and checks TLC's one-state success with `generated` successors.
fn golden(text: &str, generated: u64) -> Result<OracleResult, Box<dyn std::error::Error>> {
    let doc: Value = serde_json::from_str(text)?;
    let expected: OracleResult = serde_json::from_value(doc["result"].clone())?;
    assert_eq!(expected.verdict, Verdict::Success);
    assert_eq!(expected.verdict_code, Some(2193));
    assert_eq!(expected.exit_code, Some(0));
    assert_eq!(expected.error_class.as_deref(), Some("TLC_SUCCESS"));
    assert_eq!(expected.counts.states_generated, Some(generated));
    assert_eq!(expected.counts.distinct_states, Some(1));
    assert_eq!(expected.counts.depth, Some(1));
    assert_eq!(expected.counts.states_left_on_queue, Some(0));
    assert!(expected.violated_property.is_none());
    assert!(expected.trace.is_none());
    Ok(expected)
}

fn diff_generated(expected: &OracleResult, native_generated: u64) -> Vec<FieldDiff> {
    let mut native = expected.clone();
    native.counts.states_generated = Some(native_generated);
    diff_results(
        expected,
        &native,
        CompareContext {
            workers: 1,
            counts_comparable: true,
        },
    )
}

#[test]
fn forced_formal_pair_is_excused_in_both_profiles_only() -> TestResult {
    let registry = registry()?;
    let expected = golden(FORCED_GOLDEN, 2)?;
    let diffs = diff_generated(&expected, 3);
    assert_eq!(
        diffs.iter().map(|d| d.field.as_str()).collect::<Vec<_>>(),
        ["counts.states_generated"]
    );
    for (profile, id, link) in [
        (
            Profile::Dive,
            "d047-forced-formal",
            "docs/divergences.md#d-047-forced-boolean-formals",
        ),
        (
            Profile::Tlc,
            "d047-forced-formal-tlc",
            "docs/tlc-differences.md#compat-gaps",
        ),
    ] {
        let entry = registry
            .excuse(Some(&profile), &[], FORCED, Verdict::Success, &diffs)
            .ok_or("missing exact D-047 pair")?;
        assert_eq!(entry.id, id);
        assert_eq!(entry.pattern, FORCED);
        assert_eq!(entry.fields, Vec::<String>::new());
        assert_eq!(entry.link, link);
        for other in [
            (FORCED, Verdict::SafetyViolation, diffs.as_slice()),
            (UNFORCED, Verdict::Success, diffs.as_slice()),
            (FORCED, Verdict::Success, &[]),
        ] {
            assert!(
                registry
                    .excuse(Some(&profile), &[], other.0, other.1, other.2)
                    .is_none()
            );
        }
    }
    assert!(
        registry
            .excuse(None, &[], FORCED, Verdict::Success, &diffs)
            .is_none()
    );
    Ok(())
}

#[test]
fn every_other_forced_formal_observable_remains_strict() -> TestResult {
    let registry = registry()?;
    let diffs = diff_generated(&golden(FORCED_GOLDEN, 2)?, 3);
    for profile in [Profile::Dive, Profile::Tlc] {
        for field in [
            "verdict",
            "error_class",
            "exit_code",
            "violated_property",
            "counts.distinct_states",
            "counts.depth",
            "counts.states_left_on_queue",
            "trace.length",
            "trace.ending",
            "trace.validated",
            "trace.invalid",
        ] {
            let mut bad = diffs.clone();
            bad.push(FieldDiff {
                field: field.into(),
                expected: json!("expected"),
                actual: json!("wrong"),
            });
            assert!(
                registry
                    .excuse(Some(&profile), &[], FORCED, Verdict::Success, &bad)
                    .is_none(),
                "{profile:?}/{field}"
            );
        }
        for (expected, actual) in [
            (3, json!(3)),
            (2, json!(2)),
            (2, json!(4)),
            (2, Value::Null),
        ] {
            let mut bad = diffs.clone();
            bad[0].expected = json!(expected);
            bad[0].actual = actual;
            assert!(
                registry
                    .excuse(Some(&profile), &[], FORCED, Verdict::Success, &bad)
                    .is_none()
            );
        }
    }
    Ok(())
}

#[test]
fn unforced_control_agrees_without_registration() -> TestResult {
    let expected = golden(UNFORCED_GOLDEN, 3)?;
    assert_eq!(diff_generated(&expected, 3), Vec::<FieldDiff>::new());
    Ok(())
}
