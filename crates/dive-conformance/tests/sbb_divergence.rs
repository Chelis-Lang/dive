//! SBB's measured D-010 registration cannot excuse another model or field.
use dive_conformance::{
    compare::FieldDiff,
    divergence::{Profile, Registry},
    tlc::result::Verdict,
};
use serde_json::json;
const MODEL: &str = "tlatools/org.lamport.tlatools/test-model/pcal/SBB.cfg";
fn difference(field: &str, expected: serde_json::Value, actual: serde_json::Value) -> FieldDiff {
    FieldDiff {
        field: field.into(),
        expected,
        actual,
    }
}
fn registry() -> Result<Registry, dive_conformance::Error> {
    Registry::parse(include_str!(
        "../../../conformance/expected-divergences.toml"
    ))
}
fn measured() -> Vec<FieldDiff> {
    vec![
        difference("counts.distinct_states", json!(1617), json!(1581)),
        difference("counts.states_generated", json!(3126), json!(3042)),
    ]
}
#[test]
fn measured_sbb_counts_match_only_the_declared_model_and_profile()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = registry()?;
    let diffs = measured();
    assert_eq!(
        registry
            .excuse(
                Some(&Profile::Dive),
                &[],
                MODEL,
                Verdict::SafetyViolation,
                &diffs
            )
            .map(|entry| entry.id.as_str()),
        Some("d010-sbb")
    );
    for profile in [
        None,
        Some(Profile::Tlc),
        Some(Profile::TlcWithOverrides(vec!["time-limit".into()])),
    ] {
        assert!(
            registry
                .excuse(
                    profile.as_ref(),
                    &[],
                    MODEL,
                    Verdict::SafetyViolation,
                    &diffs
                )
                .is_none()
        );
    }
    for path in [
        "Other.cfg",
        "tlatools/org.lamport.tlatools/test-model/pcal/SBB2.cfg",
    ] {
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
    }
    Ok(())
}
#[test]
fn sbb_cannot_hide_other_fields_or_unmeasured_generated_counts()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = registry()?;
    let diffs = measured();
    for field in [
        "property",
        "counts.depth",
        "error_class",
        "exit_code",
        "trace.validated",
        "counts.states_left_on_queue",
    ] {
        let mut bad = diffs.clone();
        bad.push(difference(field, json!("expected"), json!("wrong")));
        assert!(
            registry
                .excuse(
                    Some(&Profile::Dive),
                    &[],
                    MODEL,
                    Verdict::SafetyViolation,
                    &bad
                )
                .is_none(),
            "{field}"
        );
    }
    let mut bad = diffs.clone();
    bad[1].actual = json!(3041);
    assert!(
        registry
            .excuse(
                Some(&Profile::Dive),
                &[],
                MODEL,
                Verdict::SafetyViolation,
                &bad
            )
            .is_none()
    );
    assert!(
        registry
            .excuse(
                Some(&Profile::Dive),
                &[],
                MODEL,
                Verdict::SafetyViolation,
                &diffs[..1]
            )
            .is_none()
    );
    assert!(
        registry
            .excuse(Some(&Profile::Dive), &[], MODEL, Verdict::Success, &diffs)
            .is_none()
    );
    Ok(())
}
