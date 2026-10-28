//! Independent expected successors for sampled evaluation levels: comparing
//! two regenerators with the same wrong constant level must not pass.
#![allow(clippy::expect_used, reason = "test fixture failures")]

use std::sync::Arc;

use dive_core::Opts;
use dive_eval::Host;
use dive_explore::sample::{Sample, SampledState};
use dive_value::Value;

use super::{compare, key, reference};
use crate::core_engine::Prepared;

fn prepared(compat_tlc: bool) -> Prepared {
    let analysis = dive_sema::analyze_source(
        "LevelChain.tla",
        include_str!("../tests/fixtures/core-sample/LevelChain.tla"),
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(include_str!("../tests/fixtures/core-sample/LevelChain.cfg"))
        .into_config()
        .expect("config");
    let options = if compat_tlc {
        dive_check::CheckOptions::tlc(0, 0)
    } else {
        dive_check::CheckOptions::dive(0)
    };
    crate::core_engine::prepare(&ir, &config, &options, compat_tlc, Opts::default(), false)
        .expect("core prepares")
}

fn core_keys(p: &Prepared, state: &SampledState, level: i64) -> Vec<Vec<u8>> {
    let actions = dive_explore::sample::successors(
        &p.plan,
        &p.store,
        Opts::default(),
        true,
        &state.values,
        level,
    )
    .expect("sample regenerates");
    assert_eq!(actions.len(), 1);
    let mut keys: Vec<_> = actions[0]
        .as_ref()
        .expect("action evaluates")
        .iter()
        .map(|values| {
            let values =
                crate::core_ending::state(&p.model, &p.store, values).expect("state converts");
            key(&p.model, &values).expect("state key")
        })
        .collect();
    keys.sort_unstable();
    keys
}

#[test]
fn each_regenerator_uses_the_pinned_source_level_instead_of_a_shared_constant() {
    for compat in [false, true] {
        let p = prepared(compat);
        let sample = Sample {
            seed: 0,
            states: 32,
        };
        let out = dive_explore::explore(
            &p.plan,
            &p.store,
            dive_explore::Config {
                sample: Some(sample),
                opts: Opts::default(),
                max_states: None,
                max_depth: None,
                workers: 4,
                warn: true,
            },
            &|| false,
        );
        assert_eq!(out.verdict, dive_explore::Verdict::Success);
        assert_eq!(
            (out.stats.distinct, out.stats.generated, out.stats.depth),
            (5, 6, 5)
        );
        assert_eq!(out.sampled.len(), 5);
        let x = p.plan.vars.iter().position(|v| v == "x").expect("x slot");
        let y = p.plan.vars.iter().position(|v| v == "y").expect("y slot");
        let mut negative = 0;
        for state in &out.sampled {
            let n = state.values[x].as_int().expect("integer x");
            assert_eq!(state.level, n + 1, "actual expansion level");
            let current = crate::core_ending::state(&p.model, &p.store, &state.values)
                .expect("sample converts");
            let mut expected = current.to_vec();
            expected[x] = Some(Value::Int((n + 1).min(4)));
            expected[y] = Some(Value::Int((n + 1).min(4)));
            // This expectation comes from the pinned normal BFS trace's
            // y=x invariant, not from either regenerator's output.
            let expected = vec![key(&p.model, &expected).expect("expected key")];
            assert_eq!(core_keys(&p, state, state.level), expected);
            assert_eq!(
                reference((&p.model, &p.host), 0, &current, state.level)
                    .expect("reference regenerates"),
                expected
            );
            if (1..4).contains(&n) {
                assert_ne!(core_keys(&p, state, 1), expected);
                assert_ne!(
                    reference((&p.model, &p.host), 0, &current, 1)
                        .expect("wrong level still evaluates"),
                    expected
                );
                negative += 1;
            }
        }
        assert_eq!(negative, 3);
        let binding = p.model.evaluator.host_binding_stamp();
        let stream = p.host.stream();
        for previous in [None, Some(37)] {
            match previous {
                Some(level) => p.host.set_level(level),
                None => p.host.clear_level(),
            }
            let report = compare(
                (&p.model, &p.host),
                &p.plan,
                &p.store,
                Opts::default(),
                true,
                sample,
                &out.sampled,
            );
            assert_eq!(report["mismatches"], 0, "{report:#}");
            assert_eq!(p.host.level(), previous);
            assert_eq!(p.host.stream(), stream);
            assert_eq!(p.model.evaluator.host_binding_stamp(), binding);
        }
    }
}
