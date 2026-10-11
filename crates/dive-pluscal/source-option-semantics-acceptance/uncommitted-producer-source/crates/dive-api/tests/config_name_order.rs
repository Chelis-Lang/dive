//! Eager config strings cannot inherit their ordering from the witness.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#![allow(
    clippy::cast_possible_wrap,
    reason = "same polynomial bits as Java signed long"
)]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;
#[test]
fn stable_config_prefix_uses_seed_before_fingerprint_even_in_forged_input() {
    let src = r#"---- MODULE ConfigNames ----
EXTENDS TLC
VARIABLES a, z
Init == /\ z = TLCGet("config").seed
        /\ a = CHOOSE v \in {TLCGet("config").fingerprint, TLCGet("config").seed} : TRUE
Next == UNCHANGED <<a,z>>
Inv == a = z
====
"#;
    let fp = (dive_value::fp64::POLYS[0] as i64).to_string();
    assert_ne!(fp, "7");
    for seed in [7, 42, -1] {
        let seedstr = seed.to_string();
        let m = SpecModel::from_source(
            "ConfigNames.tla",
            src,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &TraceOptions {
                compat_tlc: true,
                run: Some((seed, 0)),
                ..Default::default()
            },
        )
        .expect("model");
        assert!(
            !m.model().runtime_name_order,
            "stable config is eager fixed prefix"
        );
        let actual = TraceDoc::behavior(vec![json!({"a":seedstr,"z":seedstr})]).expect("actual");
        assert_eq!(m.validate(&actual).validation, Validation::Valid);
        // a decodes before z. A reader must not intern this fingerprint before
        // the run's seed and thereby validate the forged CHOOSE value.
        let fresh = SpecModel::from_source(
            "ConfigNames.tla",
            src,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &TraceOptions {
                compat_tlc: true,
                run: Some((seed, 0)),
                ..Default::default()
            },
        )
        .expect("fresh model");
        let forged = TraceDoc::behavior(vec![json!({"a":fp,"z":seedstr})]).expect("forged");
        assert!(
            matches!(fresh.validate(&forged).validation, Validation::Invalid(_)),
            "seed{seed} forged prefix accepted"
        );
    }
}
