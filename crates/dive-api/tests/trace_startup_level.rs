//! Public API replay distinguishes Init level 0 from generated-state level 1.
#![allow(clippy::expect_used, reason = "fixture construction and assertions")]
use dive_api::trace::{Check, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

const SPEC: &str = include_str!("../../dive-trace/tests/fixtures/startup-level/Audit.tla");
const CFG: &str = include_str!("../../dive-trace/tests/fixtures/startup-level/Audit.cfg");

fn doc(word: &[i64], lossy: bool, ending: Ending) -> TraceDoc {
    if lossy {
        TraceDoc::from_json(&json!({"vars":["x"],"counterexample":{"action":[],"state":word.iter().enumerate().map(|(k,x)| json!([k+1,{"x":x}])).collect::<Vec<_>>()}})).expect("lossy")
    } else {
        TraceDoc::behavior(word.iter().map(|x|json!({"x":x})).collect()).expect("plain")
    }.with_ending(ending)
}

#[test]
fn primary_init_word_and_forged_negatives_through_public_api() {
    let primary: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-trace/tests/fixtures/startup-level/primary.json"
    ))
    .expect("primary");
    let word: Vec<i64> = primary["states"]
        .as_array()
        .expect("states")
        .iter()
        .map(|v| v["x"].as_i64().expect("x"))
        .collect();
    for compat_tlc in [false, true] {
        let opts = TraceOptions {
            compat_tlc,
            ..TraceOptions::default()
        };
        let m = SpecModel::from_source("Audit.tla", SPEC, CFG, &opts).expect("model");
        for lossy in [false, true] {
            assert_eq!(
                m.validate(&doc(&word, lossy, Ending::Invariant("Inv".into())))
                    .validation,
                Validation::Valid
            );
            for (word, check) in [
                (&[1, 2, 3][..], Check::Init),
                (&[0, 2][..], Check::Next),
                (&[0, 1][..], Check::Ending),
            ] {
                assert!(
                    matches!(m.validate(&doc(word,lossy,Ending::Invariant("Inv".into()))).validation,Validation::Invalid(f) if f.check==check)
                );
            }
        }
    }
}

#[test]
fn primary_initial_invariant_and_implied_property_failures_are_level_one() {
    for compat_tlc in [false, true] {
        let opts = TraceOptions {
            compat_tlc,
            ..TraceOptions::default()
        };
        let invariant = SPEC.replace("Inv == TLCGet(\"level\")<3", "Inv == TLCGet(\"level\")#1");
        let implied = SPEC.replace("====", "InitialLevel == TLCGet(\"level\")#1 \\/ x#0\n====");
        for (src, cfg, ending) in [
            (&invariant, CFG.to_owned(), Ending::Invariant("Inv".into())),
            (
                &implied,
                CFG.to_owned() + "PROPERTY InitialLevel\n",
                Ending::Property("InitialLevel".into()),
            ),
        ] {
            let m = SpecModel::from_source("Audit.tla", src, &cfg, &opts).expect("model");
            for lossy in [false, true] {
                assert_eq!(
                    m.validate(&doc(&[0], lossy, ending.clone())).validation,
                    Validation::Valid
                );
                assert!(matches!(
                    m.validate(&doc(&[0, 1], lossy, ending.clone())).validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
