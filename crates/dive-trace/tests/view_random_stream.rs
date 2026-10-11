//! Independent concrete replay follows the transformed fingerprint authority.
#![allow(clippy::expect_used, reason = "regression assertions")]

use dive_stdlib::random::{RandomMode, Streams};
use dive_trace::{Ending, Replay, Trace, Validation};
use dive_value::Value;
use std::sync::Arc;

#[test]
fn pinned_view_random_words_replay_and_altered_initial_words_fail() {
    for (view, xs) in [
        ("n", [0, 78, 49, 21]),
        ("<<n>>", [0, 59, 12, 21]),
        ("[phase |-> n]", [0, 75, 100, 40]),
        ("<<n,x>>", [0, 92, 31, 98]),
        ("IF n % 2 = 0 THEN n ELSE n+100", [0, 78, 13, 21]),
    ] {
        for configured in [false, true] {
            let call = if configured {
                "C(1..100)"
            } else {
                "RandomElement(1..100)"
            };
            let source = format!(
                "---- MODULE ViewStream ----\nEXTENDS Naturals,TLC\nCONSTANT C(_)\nVARIABLES n,x\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'={call}\nInv == n<3\nProjection == {view}\n====\n"
            );
            let analysis = dive_sema::analyze_source(
                "ViewStream.tla",
                &source,
                &dive_sema::Options::default(),
            );
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let cfg=dive_config::parse("CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nVIEW Projection\nCHECK_DEADLOCK FALSE\n").into_config().expect("cfg");
            let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
            let mut trace = Trace {
                states: xs
                    .into_iter()
                    .enumerate()
                    .map(|(n, x)| {
                        Arc::from([
                            Some(Value::Int(i64::try_from(n).expect("small"))),
                            Some(Value::Int(x)),
                        ])
                    })
                    .collect(),
                labels: vec![None; 4],
                ending: Ending::Invariant("Inv".into()),
                length: Some(4),
                properties: vec![],
            };
            let replay = Replay::new(Streams::new(7, RandomMode::Tlc), 0).expect("fp0");
            assert!(
                matches!(
                    dive_trace::validate(&model, &trace, Some(&replay)).validation,
                    Validation::Valid
                ),
                "{view}"
            );
            trace.states[0] = Arc::from([Some(Value::Int(0)), Some(Value::Int(1))]);
            assert!(matches!(
                dive_trace::validate(&model, &trace, Some(&replay)).validation,
                Validation::Invalid(_)
            ));
        }
    }
}
