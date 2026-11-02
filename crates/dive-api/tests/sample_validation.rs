//! A sampled word is stronger than a behavior prefix and weaker than a proof.
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const SOURCE: &str = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..1\nNext == x' \\in 0..3\nInv == x<2\nC == x<=2\nA == [][x'<=1]_x\nInitial == x=0\nLive == <>(x=2)\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\n====\n";

fn word(last: i64) -> Result<TraceDoc> {
    Ok(TraceDoc::behavior(vec![json!({"x":0}), json!({"x":last})])?.with_ending(Ending::Behavior))
}

#[test]
fn sample_operation_checks_roles_while_legacy_behavior_stays_unchanged() -> Result {
    for tlc in [false, true] {
        for (role, bad) in [("INVARIANT Inv", 2), ("CONSTRAINT C", 3), ("PROPERTY A", 2)] {
            let model = SpecModel::from_source(
                "M.tla",
                SOURCE,
                &format!("INIT Init\nNEXT Next\n{role}\n"),
                &TraceOptions {
                    compat_tlc: tlc,
                    ..Default::default()
                },
            )?;
            let good = word(1)?;
            let bad = word(bad)?;
            assert!(matches!(model.validate(&bad).validation, Validation::Valid));
            assert!(matches!(
                model.try_validate_sample(&good)?.validation,
                Validation::Valid
            ));
            assert!(
                matches!(
                    model.try_validate_sample(&bad)?.validation,
                    Validation::Invalid(_)
                ),
                "{role}"
            );
            let serialized = TraceDoc::parse(&bad.to_json().to_string())?;
            assert!(matches!(
                model.try_validate_sample(&serialized)?.validation,
                Validation::Invalid(_)
            ));
        }
        let initial = SpecModel::from_source(
            "M.tla",
            SOURCE,
            "INIT Init\nNEXT Next\nPROPERTY Initial\n",
            &TraceOptions {
                compat_tlc: tlc,
                ..Default::default()
            },
        )?;
        let bad = TraceDoc::behavior(vec![json!({"x":1})])?.with_ending(Ending::Behavior);
        assert!(matches!(
            initial.validate(&bad).validation,
            Validation::Valid
        ));
        assert!(matches!(
            initial.try_validate_sample(&bad)?.validation,
            Validation::Invalid(_)
        ));
    }
    Ok(())
}

#[test]
fn finite_sampling_ignores_fairness_but_cannot_claim_temporal_properties() -> Result {
    for tlc in [false, true] {
        let options = TraceOptions {
            compat_tlc: tlc,
            ..Default::default()
        };
        let fair = SpecModel::from_source("M.tla", SOURCE, "SPECIFICATION Spec\n", &options)?;
        assert!(matches!(
            fair.try_validate_sample(&word(1)?)?.validation,
            Validation::Valid
        ));
        let temporal = SpecModel::from_source(
            "M.tla",
            SOURCE,
            "SPECIFICATION Spec\nPROPERTY Live\n",
            &options,
        )?;
        assert!(matches!(
            temporal.try_validate_sample(&word(1)?)?.validation,
            Validation::Unsupported(_)
        ));
        let end = word(1)?.with_ending(Ending::Invariant("Inv".to_owned()));
        assert!(matches!(
            fair.try_validate_sample(&end)?.validation,
            Validation::Invalid(_)
        ));
    }
    Ok(())
}
