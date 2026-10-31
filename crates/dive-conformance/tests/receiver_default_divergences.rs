//! Narrow D-009 registrations cannot authorize different models or invalid words.

use dive_conformance::compare::{CompareContext, FieldDiff, diff_results};
use dive_conformance::divergence::{Profile, Registry};
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

fn differences(
    case: &(&str, u64, u64, u64),
) -> Result<(String, Vec<FieldDiff>), Box<dyn std::error::Error>> {
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
