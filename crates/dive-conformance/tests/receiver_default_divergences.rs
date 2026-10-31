//! Narrow D-009 registrations cannot authorize different models or invalid words.

use dive_conformance::compare::{CompareContext, FieldDiff, diff_results};
use dive_conformance::divergence::{ObservedComparison, Profile, Registry};
use dive_conformance::tlc::result::{OracleResult, Verdict};
use serde_json::{Value, json};
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CASES: &[(&str, u64, u64, u64)] = &[
    ("InitGeneration", 1, 2, 1),
    ("InitialLiteral", 3, 3, 3),
    ("InitialStateLeft", 1, 2, 1),
    ("InitialStateRight", 1, 2, 1),
    ("InitialTupleNested", 3, 3, 3),
    ("InstanceTuple", 3, 3, 3),
    ("Issue620", 3, 3, 3),
    ("LetTuple", 3, 3, 3),
    ("NextLiteral", 3, 3, 3),
    ("RecordLiteral", 3, 3, 3),
    ("SubsequentInvariant", 3, 3, 3),
];

fn results(
    case: &(&str, u64, u64, u64),
) -> Result<(String, OracleResult, OracleResult), Box<dyn std::error::Error>> {
    let (name, distinct, generated, depth) = *case;
    let model = format!("specifications/{name}/{name}.cfg");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/golden/tlc-2026.10.06.014338/comparison-receiver-primary")
        .join(format!("{model}.json"));
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let expected: OracleResult = serde_json::from_value(golden["result"].clone())?;
    assert_eq!(expected.verdict, Verdict::EvaluationError);
    let mut actual = expected.clone();
    actual.verdict = Verdict::Success;
    actual.verdict_code = Some(2193);
    actual.error_class = Some("TLC_SUCCESS".into());
    actual.exit_code = Some(0);
    actual.counts.distinct_states = Some(distinct);
    actual.counts.states_generated = Some(generated);
    actual.counts.depth = Some(depth);
    actual.counts.states_left_on_queue = Some(0);
    actual.trace = None;
    Ok((model, expected, actual))
}

fn differences(
    case: &(&str, u64, u64, u64),
) -> Result<(String, Vec<FieldDiff>), Box<dyn std::error::Error>> {
    let (model, expected, actual) = results(case)?;
    Ok((
        model,
        diff_results(
            &expected,
            &actual,
            CompareContext {
                workers: 1,
                counts_comparable: true,
            },
        ),
    ))
}

#[test]
fn observed_count_pairs_apply_at_actual_worker_width_without_weakening_legacy_calls() -> TestResult
{
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for case in CASES {
        let (model, expected, actual) = results(case)?;
        for workers in [1, 4, 8] {
            let context = CompareContext {
                workers,
                counts_comparable: true,
            };
            let diffs = diff_results(&expected, &actual, context);
            assert_eq!(
                diffs.iter().any(|d| d.field == "counts.states_generated"),
                workers == 1
            );
            let comparison = ObservedComparison {
                expected: &expected,
                actual: &actual,
                context,
                differences: &diffs,
            };
            let entry = registry
                .excuse_compared(Some(&Profile::Dive), &[], &model, comparison)
                .ok_or_else(|| format!("missing exact count facts at W{workers} for {model}"))?;
            assert_eq!(entry.id, format!("d009-receiver-{}", case.0.to_lowercase()));
            if workers > 1 {
                assert!(
                    registry
                        .excuse(Some(&Profile::Dive), &[], &model, actual.verdict, &diffs)
                        .is_none()
                );
                assert!(
                    registry
                        .excuse_observed(
                            Some(&Profile::Dive),
                            &[],
                            &model,
                            &expected,
                            &actual,
                            &diffs
                        )
                        .is_none()
                );
            }
            for profile in [
                None,
                Some(Profile::Tlc),
                Some(Profile::TlcWithOverrides(vec![])),
            ] {
                assert!(
                    registry
                        .excuse_compared(profile.as_ref(), &[], &model, comparison)
                        .is_none()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn missing_or_wrong_count_facts_never_borrow_matching_differences() -> TestResult {
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for case in CASES {
        let (model, expected, actual) = results(case)?;
        for workers in [1, 4] {
            let context = CompareContext {
                workers,
                counts_comparable: true,
            };
            let diffs = diff_results(&expected, &actual, context);
            for mutate_expected in [true, false] {
                for wrong in [None, Some(u64::MAX), Some(999)] {
                    let mut changed_expected = expected.clone();
                    let mut changed_actual = actual.clone();
                    if mutate_expected {
                        changed_expected.counts.states_generated = wrong;
                    } else {
                        changed_actual.counts.states_generated = wrong;
                    }
                    assert!(
                        registry
                            .excuse_compared(
                                Some(&Profile::Dive),
                                &[],
                                &model,
                                ObservedComparison {
                                    expected: &changed_expected,
                                    actual: &changed_actual,
                                    context,
                                    differences: &diffs,
                                }
                            )
                            .is_none()
                    );
                }
            }
            let mut absent = diffs.clone();
            absent.retain(|diff| diff.field != "counts.states_generated");
            for context in [
                CompareContext {
                    workers: 1,
                    counts_comparable: true,
                },
                CompareContext {
                    workers: 4,
                    counts_comparable: false,
                },
                CompareContext {
                    workers: 0,
                    counts_comparable: true,
                },
            ] {
                assert!(
                    registry
                        .excuse_compared(
                            Some(&Profile::Dive),
                            &[],
                            &model,
                            ObservedComparison {
                                expected: &expected,
                                actual: &actual,
                                context,
                                differences: &absent,
                            }
                        )
                        .is_none()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn actual_count_facts_cannot_excuse_new_numeric_errors_or_invalid_words() -> TestResult {
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for case in CASES {
        let (model, expected, actual) = results(case)?;
        let context = CompareContext {
            workers: 4,
            counts_comparable: true,
        };
        let diffs = diff_results(&expected, &actual, context);
        for field in [
            "counts.states_generated",
            "trace.invalid",
            "trace.validated",
            "violated_property",
        ] {
            let mut changed = diffs.clone();
            changed.push(FieldDiff {
                field: field.into(),
                expected: json!(1),
                actual: json!(999),
            });
            assert!(
                registry
                    .excuse_compared(
                        Some(&Profile::Dive),
                        &[],
                        &model,
                        ObservedComparison {
                            expected: &expected,
                            actual: &actual,
                            context,
                            differences: &changed
                        }
                    )
                    .is_none(),
                "{model}: {field}"
            );
        }
        let mut no_exit = diffs.clone();
        no_exit.retain(|diff| diff.field != "exit_code");
        assert!(
            registry
                .excuse_compared(
                    Some(&Profile::Dive),
                    &[],
                    &model,
                    ObservedComparison {
                        expected: &expected,
                        actual: &actual,
                        context,
                        differences: &no_exit
                    }
                )
                .is_none()
        );
        assert!(
            registry
                .excuse_compared(
                    Some(&Profile::Dive),
                    &[],
                    &format!("{model}.other"),
                    ObservedComparison {
                        expected: &expected,
                        actual: &actual,
                        context,
                        differences: &diffs
                    }
                )
                .is_none()
        );
        let mut wrong_verdict = actual.clone();
        wrong_verdict.verdict = Verdict::EvaluationError;
        assert!(
            registry
                .excuse_compared(
                    Some(&Profile::Dive),
                    &[],
                    &model,
                    ObservedComparison {
                        expected: &expected,
                        actual: &wrong_verdict,
                        context,
                        differences: &diffs
                    }
                )
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn measured_default_differences_have_exact_model_exit_and_generated_authority() -> TestResult {
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for case in CASES {
        let (model, diffs) = differences(case)?;
        let entry = registry
            .excuse(Some(&Profile::Dive), &[], &model, Verdict::Success, &diffs)
            .ok_or_else(|| format!("missing measured default registration for {model}"))?;
        assert_eq!(entry.pattern, model);
        assert_eq!(entry.id, format!("d009-receiver-{}", case.0.to_lowercase()));
        assert!(entry.exit_code_pair.is_some());
        assert!(entry.generated_count_pair.is_some());
        assert!(!entry.fields.iter().any(|field| field.contains('*')));
        assert!(
            registry
                .excuse(Some(&Profile::Tlc), &[], &model, Verdict::Success, &diffs)
                .is_none()
        );
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], &model, Verdict::Success, &[])
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn changed_numeric_pairs_paths_verdicts_and_invalid_words_are_never_registered() -> TestResult {
    let registry = Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))?;
    for case in CASES {
        let (model, diffs) = differences(case)?;
        for field in ["exit_code", "counts.states_generated"] {
            for value in [Value::Null, json!(999), json!(-1), json!(true), json!("3")] {
                let mut changed = diffs.clone();
                let pair = changed
                    .iter_mut()
                    .find(|d| d.field == field)
                    .ok_or("missing numeric pair")?;
                pair.actual = value;
                assert!(
                    registry
                        .excuse(
                            Some(&Profile::Dive),
                            &[],
                            &model,
                            Verdict::Success,
                            &changed
                        )
                        .is_none()
                );
            }
        }
        for field in ["trace.validated", "trace.invalid", "violated_property"] {
            let mut changed = diffs.clone();
            changed.push(FieldDiff {
                field: field.into(),
                expected: Value::Null,
                actual: json!(1),
            });
            assert!(
                registry
                    .excuse(
                        Some(&Profile::Dive),
                        &[],
                        &model,
                        Verdict::Success,
                        &changed
                    )
                    .is_none()
            );
        }
        assert!(
            registry
                .excuse(
                    Some(&Profile::Dive),
                    &[],
                    &format!("{model}.other"),
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
                    &model,
                    Verdict::EvaluationError,
                    &diffs
                )
                .is_none()
        );
    }
    Ok(())
}
