//! The existing D-009 model control grants only its declared default differences.

use dive_conformance::compare::{CompareContext, FieldDiff, diff_results};
use dive_conformance::divergence::{Profile, Registry};
use dive_conformance::tlc::result::{OracleResult, Verdict};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const MODEL: &str = "specifications/ModelBadPrefix/ModelBadPrefix.cfg";
const ENTRY: &str = "d009-modelbadprefix-reflexive-membership";

fn registry() -> dive_conformance::error::Result<Registry> {
    Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))
}

fn differences() -> Result<Vec<FieldDiff>, Box<dyn std::error::Error>> {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../conformance/golden/tlc-2026.10.06.014338/construction-set-history/specifications/ModelBadPrefix/ModelBadPrefix.cfg.json"
    ))?;
    let expected: OracleResult = serde_json::from_value(golden["result"].clone())?;
    assert_eq!(expected.verdict, Verdict::EvaluationError);
    assert_eq!(expected.verdict_code, Some(2147));
    assert_eq!(expected.exit_code, Some(75));
    assert!(expected.trace.is_none());
    let mut actual = expected.clone();
    actual.verdict = Verdict::Success;
    actual.verdict_code = Some(2193);
    actual.error_class = Some("TLC_SUCCESS".into());
    actual.exit_code = Some(0);
    actual.counts.distinct_states = Some(2);
    actual.counts.states_generated = Some(2);
    actual.counts.depth = Some(2);
    actual.counts.states_left_on_queue = Some(0);
    Ok(diff_results(
        &expected,
        &actual,
        CompareContext {
            workers: 1,
            counts_comparable: true,
        },
    ))
}

#[test]
fn official_model_diff_matches_only_the_registered_default_entry() -> TestResult {
    let registry = registry()?;
    let diffs = differences()?;
    assert_eq!(
        diffs
            .iter()
            .map(|diff| diff.field.as_str())
            .collect::<Vec<_>>(),
        [
            "verdict",
            "error_class",
            "exit_code",
            "counts.distinct_states",
            "counts.states_generated",
            "counts.depth",
        ]
    );
    let entry = registry
        .excuse(Some(&Profile::Dive), &[], MODEL, Verdict::Success, &diffs)
        .ok_or("the actual documented default difference must be registered")?;
    assert_eq!(entry.id, ENTRY);
    assert_eq!(entry.pattern, MODEL);
    assert_eq!(entry.fields.len(), 4);
    assert!(
        registry
            .excuse(Some(&Profile::Dive), &[], MODEL, Verdict::Success, &[])
            .is_none(),
        "an agreement is not this expected divergence"
    );
    Ok(())
}

#[test]
fn other_models_profiles_and_verdicts_cannot_borrow_the_entry() -> TestResult {
    let registry = registry()?;
    let diffs = differences()?;
    for path in [
        "specifications/ModelBadPrefix/Other.cfg",
        "specifications/ModelRawPrefix/ModelRawPrefix.cfg",
        "specifications/ModelBadPrefix/ModelBadPrefix.tla",
    ] {
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &diffs)
                .is_none()
        );
    }
    for profile in [
        None,
        Some(Profile::Tlc),
        Some(Profile::TlcWithOverrides(vec!["comparisons=tla".into()])),
    ] {
        assert!(
            registry
                .excuse(profile.as_ref(), &[], MODEL, Verdict::Success, &diffs)
                .is_none()
        );
    }
    for verdict in [
        Verdict::EvaluationError,
        Verdict::OtherError,
        Verdict::Unclassified,
    ] {
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], MODEL, verdict, &diffs)
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn exact_exit_and_generated_pairs_are_mandatory() -> TestResult {
    let registry = registry()?;
    let diffs = differences()?;
    // Distinct/depth use the existing explicit field permissions. Only exit
    // and generated counts claim exact numeric pair authority in this schema.
    for field in ["exit_code", "counts.states_generated"] {
        for value in [
            Value::Null,
            json!(3),
            json!(-1),
            json!(true),
            json!(2.0),
            json!("2"),
        ] {
            for expected_side in [false, true] {
                let mut wrong = diffs.clone();
                let pair = wrong
                    .iter_mut()
                    .find(|diff| diff.field == field)
                    .ok_or("pair")?;
                if expected_side {
                    pair.expected = value.clone();
                } else {
                    pair.actual = value.clone();
                }
                assert!(
                    registry
                        .excuse(Some(&Profile::Dive), &[], MODEL, Verdict::Success, &wrong)
                        .is_none(),
                    "{field}/{value}/{expected_side}"
                );
            }
        }
        let missing: Vec<_> = diffs
            .iter()
            .filter(|diff| diff.field != field)
            .cloned()
            .collect();
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], MODEL, Verdict::Success, &missing)
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn traces_and_other_unlisted_observables_still_fail() -> TestResult {
    let registry = registry()?;
    for field in [
        "trace.length",
        "trace.ending",
        "trace.validated",
        "trace.invalid",
        "violated_property",
        "counts.states_left_on_queue",
    ] {
        let mut wrong = differences()?;
        wrong.push(FieldDiff {
            field: field.into(),
            expected: Value::Null,
            actual: json!(1),
        });
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], MODEL, Verdict::Success, &wrong)
                .is_none(),
            "{field}"
        );
    }
    Ok(())
}
