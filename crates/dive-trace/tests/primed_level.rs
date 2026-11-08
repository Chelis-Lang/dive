//! A primed `TLCGet("level")` is the successor's level in TLC (BFS and
//! `-simulate`), which replay does not track: words that depend on it are
//! refused by name instead of being judged at the source's level (#895).
#![allow(clippy::expect_used, reason = "fixture construction and assertions")]

use dive_model::{BindOptions, Model};
use dive_trace::{Ending, TraceDoc, Validation, validate_doc};
use serde_json::json;

/// The issue's spec. Pinned TLC reports `Inv` violated after six states:
/// `Prop` holds on every step because the successor's level is one more than
/// the source's.
const SPEC: &str = r#"---- MODULE P ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = x + 1
Prop == [][(TLCGet("level"))' = TLCGet("level") + 1]_x
L == TLCGet("level")
ViaDefinition == [][L' = L + 1]_x
ViaUnchanged == [][~UNCHANGED TLCGet("level")]_x
ShortCircuit == [][x' = x + 1 \/ (TLCGet("level"))' = 0]_x
EnabledInv == ENABLED (x' = x + 1 /\ (TLCGet("level"))' > 0)
Inv == x < 5
===="#;

fn model(property: &str, tlc: bool) -> Model {
    bind(&format!("INVARIANT Inv\nPROPERTY {property}\n"), tlc)
}

fn bind(checks: &str, tlc: bool) -> Model {
    let a = dive_sema::analyze_source("P.tla", SPEC, &dive_sema::Options::default());
    assert!(a.accepted(), "{:?}", a.diagnostics());
    let ir = std::sync::Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = format!("INIT Init\nNEXT Next\n{checks}");
    let cfg = dive_config::parse(&cfg).into_config().expect("cfg");
    let opts = if tlc {
        BindOptions::tlc()
    } else {
        BindOptions::dive()
    };
    dive_model::bind(ir, &cfg, &opts).expect("model")
}

fn word(values: &[i64], ending: Ending) -> TraceDoc {
    TraceDoc::behavior(values.iter().map(|x| json!({ "x": x })).collect())
        .expect("plain")
        .with_ending(ending)
}

fn refused_by_name(v: &Validation) -> bool {
    matches!(v, Validation::Unsupported(why) if why.contains("TLCGet(\"level\") under a prime"))
}

#[test]
fn the_wrong_action_property_counterexample_is_not_accepted() {
    for tlc in [false, true] {
        for property in ["Prop", "ViaDefinition", "ViaUnchanged"] {
            let m = model(property, tlc);
            // The wrong counterexample: at the source's level, `Prop` would
            // fail on the first step.
            let wrong = validate_doc(&m, &word(&[0, 1], Ending::Property(property.into())), &[])
                .0
                .validation;
            assert!(refused_by_name(&wrong), "{property}: {wrong:?}");
            // TLC's own counterexample also depends on the primed level along
            // the way, so replay refuses it rather than guess.
            let tlc_word = validate_doc(
                &m,
                &word(&[0, 1, 2, 3, 4, 5], Ending::Invariant("Inv".into())),
                &[],
            )
            .0
            .validation;
            assert!(refused_by_name(&tlc_word), "{property}: {tlc_word:?}");
        }
    }
}

#[test]
fn a_short_circuited_primed_level_is_never_evaluated() {
    for tlc in [false, true] {
        let m = model("ShortCircuit", tlc);
        let tlc_word = validate_doc(
            &m,
            &word(&[0, 1, 2, 3, 4, 5], Ending::Invariant("Inv".into())),
            &[],
        )
        .0
        .validation;
        assert_eq!(tlc_word, Validation::Valid);
        let wrong = validate_doc(
            &m,
            &word(&[0, 1], Ending::Property("ShortCircuit".into())),
            &[],
        )
        .0
        .validation;
        assert!(
            !matches!(wrong, Validation::Valid | Validation::Unsupported(_)),
            "{wrong:?}"
        );
    }
}

#[test]
fn a_primed_level_inside_enabled_is_refused() {
    for tlc in [false, true] {
        let m = bind("INVARIANT EnabledInv Inv\n", tlc);
        let v = validate_doc(
            &m,
            &word(&[0, 1, 2, 3, 4, 5], Ending::Invariant("Inv".into())),
            &[],
        )
        .0
        .validation;
        assert!(refused_by_name(&v), "{v:?}");
    }
}
