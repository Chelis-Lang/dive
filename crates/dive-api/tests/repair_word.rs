//! Short-path repair must validate both incoming and closing edges of the new word.
#![allow(
    clippy::expect_used,
    reason = "test fixtures require successful binding"
)]

use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

const CLOCK: &str = r"---- MODULE Ring ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x+1) % 4
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == <> (x = 9)
====";

fn model(src: &str) -> SpecModel {
    SpecModel::from_source(
        "Ring.tla",
        src,
        "SPECIFICATION Spec\nPROPERTY P\n",
        &TraceOptions::default(),
    )
    .expect("bind")
}
fn donor() -> TraceDoc {
    TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})])
        .expect("two anchors")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["P".into()])
}

#[test]
fn searches_each_gap_and_independently_replays_the_reconstructed_cycle() {
    let model = model(CLOCK);
    let old = donor();
    let outcome = model.repair_word(&old).expect("repair");
    assert_eq!(outcome.validation.validation, Validation::Valid);
    assert_eq!(outcome.gaps, 2);
    assert_eq!(outcome.generated, 4);
    let repaired = outcome.replacement.expect("new word");
    assert_eq!(
        repaired
            .states
            .iter()
            .map(|row| &row["x"])
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(repaired.ending, old.ending);
    assert_eq!(
        model.try_validate(&repaired).expect("fresh").validation,
        Validation::Valid
    );
    let unchanged = model.repair_word(&repaired).expect("unchanged");
    assert!(unchanged.replacement.is_none());
    assert_eq!(unchanged.validation.validation, Validation::Valid);
    assert_eq!(unchanged.generated, 0);
}

#[test]
fn a_fixed_property_and_an_unreachable_anchor_do_not_authorize_a_verdict() {
    let mut doc = donor();
    let fixed = model(&CLOCK.replace("x = 9", "x = 2"));
    let out = fixed.repair_word(&doc).expect("fixed");
    assert!(out.replacement.is_none());
    assert!(
        matches!(out.rejected.expect("candidate replay").validation,Validation::Invalid(f) if f.check==dive_api::trace::Check::Temporal)
    );
    doc.states[1].insert("x".into(), json!(12));
    let out = model(CLOCK).repair_word(&doc).expect("unreachable");
    assert!(out.replacement.is_none());
    assert_eq!(
        out.reason.as_deref(),
        Some("no path within four current Next steps")
    );
    assert!(matches!(out.validation.validation, Validation::Invalid(_)));
}

#[test]
fn a_valid_finite_behavior_cannot_be_used_as_an_unchanged_refutation() {
    let mut doc = donor();
    doc.ending = Some(Ending::Behavior);
    doc.properties.clear();
    assert!(model(CLOCK).repair_word(&doc).is_err());
}
