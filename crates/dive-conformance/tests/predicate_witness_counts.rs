//! D-029 excuses only the two measured default generated-count pairs.

use dive_conformance::compare::{CompareContext, FieldDiff, diff_results};
use dive_conformance::divergence::{Profile, Registry};
use dive_conformance::tlc::result::{OracleResult, Verdict};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CASES: &[(&str, &str, u64, u64, u64, u64)] = &[
    (
        "specifications/ActionCount/ActionCount.cfg",
        include_str!(
            "../../../conformance/golden/tlc-2026.10.06.014338/predicate-witness-counts/specifications/ActionCount/ActionCount.cfg.json"
        ),
        3,
        2,
        1,
        1,
    ),
    (
        "specifications/byzpaxos/VoteProof.cfg",
        include_str!(
            "../../../conformance/golden/tlc-2026.10.06.014338/examples/specifications/byzpaxos/VoteProof.cfg.json"
        ),
        318_877,
        27_727,
        6_962,
        14,
    ),
];

fn differences(
    golden: &str,
    expected_count: u64,
    native_count: u64,
    distinct: u64,
    depth: u64,
) -> Result<Vec<FieldDiff>, Box<dyn std::error::Error>> {
    let doc: Value = serde_json::from_str(golden)?;
    let expected: OracleResult = serde_json::from_value(doc["result"].clone())?;
    assert_eq!(expected.verdict, Verdict::Success);
    assert_eq!(expected.verdict_code, Some(2193));
    assert_eq!(expected.exit_code, Some(0));
    assert_eq!(expected.error_class.as_deref(), Some("TLC_SUCCESS"));
    assert_eq!(expected.counts.states_generated, Some(expected_count));
    assert_eq!(expected.counts.distinct_states, Some(distinct));
    assert_eq!(expected.counts.depth, Some(depth));
    assert_eq!(expected.counts.states_left_on_queue, Some(0));
    assert!(expected.violated_property.is_none());
    assert!(expected.trace.is_none());
    let mut native = expected.clone();
    native.counts.states_generated = Some(native_count);
    Ok(diff_results(
        &expected,
        &native,
        CompareContext {
            workers: 1,
            counts_comparable: true,
        },
    ))
}

#[test]
fn exact_measured_pairs_are_default_divergences_only() -> TestResult {
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for &(path, golden, expected, actual, distinct, depth) in CASES {
        let diffs = differences(golden, expected, actual, distinct, depth)?;
        assert_eq!(
            diffs.iter().map(|d| d.field.as_str()).collect::<Vec<_>>(),
            ["counts.states_generated"]
        );
        let entry = registry
            .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &diffs)
            .ok_or("missing exact D-029 pair")?;
        assert!(entry.id.starts_with("d029-"));
        assert_eq!(entry.pattern, path);
        assert_eq!(entry.fields, Vec::<String>::new());
        for profile in [
            None,
            Some(Profile::Tlc),
            Some(Profile::TlcWithOverrides(vec!["counts".into()])),
        ] {
            assert!(
                registry
                    .excuse(profile.as_ref(), &[], path, Verdict::Success, &diffs)
                    .is_none()
            );
        }
        assert!(
            registry
                .excuse(
                    Some(&Profile::Dive),
                    &[],
                    "specifications/Other/Other.cfg",
                    Verdict::Success,
                    &diffs
                )
                .is_none()
        );
        assert!(
            registry
                .excuse(
                    Some(&Profile::Dive),
                    &[],
                    path,
                    Verdict::SafetyViolation,
                    &diffs
                )
                .is_none()
        );
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &[])
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn every_other_observable_and_unmeasured_count_pair_remains_strict() -> TestResult {
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for &(path, golden, expected, actual, distinct, depth) in CASES {
        let diffs = differences(golden, expected, actual, distinct, depth)?;
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
                    .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &bad)
                    .is_none(),
                "{path}/{field}"
            );
        }
        for (expected, actual) in [
            (expected + 1, json!(actual)),
            (expected, json!(actual + 1)),
            (expected, Value::Null),
        ] {
            let mut bad = diffs.clone();
            bad[0].expected = json!(expected);
            bad[0].actual = actual;
            assert!(
                registry
                    .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &bad)
                    .is_none()
            );
        }
    }
    Ok(())
}

#[test]
fn already_true_recursive_or_has_only_its_precise_registered_default_differences() -> TestResult {
    let path = "specifications/ActionCount/OrError.cfg";
    let other = "specifications/ActionCount/Other.cfg";
    let doc: Value = serde_json::from_str(include_str!(
        "../../../conformance/golden/tlc-2026.10.06.014338/predicate-witness-counts/specifications/ActionCount/OrError.cfg.json"
    ))?;
    let expected: OracleResult = serde_json::from_value(doc["result"].clone())?;
    assert_eq!(expected.verdict, Verdict::EvaluationError);
    assert_eq!(expected.verdict_code, Some(2179));
    assert_eq!(
        expected.error_class.as_deref(),
        Some("TLC_MODULE_DIVISION_BY_ZERO")
    );
    assert_eq!(expected.exit_code, Some(255));
    assert_eq!(expected.counts.distinct_states, Some(1));
    assert_eq!(expected.counts.states_generated, Some(2));
    assert_eq!(expected.counts.depth, Some(1));
    let mut actual = expected.clone();
    actual.verdict = Verdict::Success;
    actual.verdict_code = Some(2193);
    actual.error_class = Some("TLC_SUCCESS".into());
    actual.exit_code = Some(0);
    actual.trace = None;
    let diffs = diff_results(
        &expected,
        &actual,
        CompareContext {
            workers: 1,
            counts_comparable: true,
        },
    );
    assert_eq!(
        diffs.iter().map(|d| d.field.as_str()).collect::<Vec<_>>(),
        [
            "verdict",
            "error_class",
            "exit_code",
            "trace.length",
            "trace.ending"
        ]
    );
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    assert_eq!(
        registry
            .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &diffs)
            .ok_or("exact OrError default divergence")?
            .id,
        "d029-or-error"
    );
    for profile in [
        None,
        Some(Profile::Tlc),
        Some(Profile::TlcWithOverrides(vec!["counts".into()])),
    ] {
        assert!(
            registry
                .excuse(profile.as_ref(), &[], path, Verdict::Success, &diffs)
                .is_none()
        );
    }
    for field in [
        "counts.distinct_states",
        "counts.states_generated",
        "counts.depth",
        "violated_property",
        "trace.validated",
        "trace.invalid",
    ] {
        let mut bad = diffs.clone();
        bad.push(FieldDiff {
            field: field.into(),
            expected: json!(1),
            actual: json!(2),
        });
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &bad)
                .is_none(),
            "{field}"
        );
    }
    let mut bad = diffs.clone();
    bad[2].actual = json!(1);
    assert!(
        registry
            .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &bad)
            .is_none()
    );
    assert!(
        registry
            .excuse(Some(&Profile::Dive), &[], other, Verdict::Success, &diffs)
            .is_none()
    );
    assert!(
        registry
            .excuse(Some(&Profile::Dive), &[], path, Verdict::Deadlock, &diffs)
            .is_none()
    );
    Ok(())
}
