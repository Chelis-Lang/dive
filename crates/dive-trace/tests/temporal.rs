//! Independent temporal word evaluation: exhaustive small words and pinned TLC traces.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_model::{Model, Pred};
use dive_trace::temporal::{TemporalError, evaluate_lasso};
use dive_trace::{Ending, State, TraceDoc};
use dive_value::Value;

fn bound(source: &str, config: &str) -> Model {
    let name = source.split_whitespace().nth(2).expect("module name");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}

fn predicate(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect("definition");
    Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("valid scope"),
    )
}

fn reachable(start: usize, len: usize, back: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = start;
    while !out.contains(&i) {
        out.push(i);
        i = if i + 1 == len { back } else { i + 1 };
    }
    out
}

#[test]
fn every_suffix_of_small_words_matches_direct_infinite_semantics() {
    let model = bound(
        "---- MODULE Word ----\nVARIABLE x\nInit == x = TRUE\nNext == x' \\in BOOLEAN\n\
         A == []x\nE == <>x\nAE == []<>x\nEA == <>[]x\n\
         L == x ~> ~x\nN == ~[]x\nB == []x \\/ <>~x\n\
         I == (<>x => []x)\nQ == (<>x <=> []<>x)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    for len in 1..=4 {
        for bits in 0..(1_u32 << len) {
            let word: Vec<bool> = (0..len).map(|i| bits & (1 << i) != 0).collect();
            let states: Vec<State> = word
                .iter()
                .map(|b| Arc::from([Some(Value::Bool(*b))]))
                .collect();
            for back in 0..len {
                let future = |i| reachable(i, len, back);
                let always = |i| future(i).iter().all(|j| word[*j]);
                let eventually = |i| future(i).iter().any(|j| word[*j]);
                let expected: Vec<(&str, Vec<bool>)> = vec![
                    ("A", (0..len).map(always).collect()),
                    ("E", (0..len).map(eventually).collect()),
                    (
                        "AE",
                        (0..len)
                            .map(|i| future(i).iter().all(|j| eventually(*j)))
                            .collect(),
                    ),
                    (
                        "EA",
                        (0..len)
                            .map(|i| future(i).iter().any(|j| always(*j)))
                            .collect(),
                    ),
                    (
                        "L",
                        (0..len)
                            .map(|i| {
                                future(i)
                                    .iter()
                                    .all(|j| !word[*j] || future(*j).iter().any(|k| !word[*k]))
                            })
                            .collect(),
                    ),
                    ("N", (0..len).map(|i| !always(i)).collect()),
                    (
                        "B",
                        (0..len)
                            .map(|i| always(i) || future(i).iter().any(|j| !word[*j]))
                            .collect(),
                    ),
                    ("I", (0..len).map(|i| !eventually(i) || always(i)).collect()),
                    (
                        "Q",
                        (0..len)
                            .map(|i| eventually(i) == future(i).iter().all(|j| eventually(*j)))
                            .collect(),
                    ),
                ];
                for (name, expected) in expected {
                    assert_eq!(
                        evaluate_lasso(&model, &predicate(&model, name), &states, back)
                            .expect(name),
                        expected,
                        "{name}: word {word:?}, back {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn actions_use_the_closing_successor_and_bad_words_refuse() {
    let model = bound(
        "---- MODULE Word ----\nVARIABLE x\nInit == x = TRUE\nNext == x' \\in BOOLEAN\nP == []<><<x' /= x>>_x\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let states: Vec<State> = [true, false]
        .into_iter()
        .map(|b| Arc::from([Some(Value::Bool(b))]))
        .collect();
    let p = predicate(&model, "P");
    assert_eq!(
        evaluate_lasso(&model, &p, &states, 0).expect("alternation"),
        [true, true]
    );
    assert_eq!(
        evaluate_lasso(&model, &p, &states, 1).expect("stutter"),
        [false, false]
    );
    assert!(matches!(
        evaluate_lasso(&model, &p, &[], 0),
        Err(TemporalError::Invalid(_))
    ));
    assert!(matches!(
        evaluate_lasso(&model, &p, &states, 2),
        Err(TemporalError::Invalid(_))
    ));
    assert!(matches!(
        evaluate_lasso(&model, &p, &[Arc::from([None])], 0),
        Err(TemporalError::Invalid(_))
    ));
}

#[test]
fn unfair_stutter_is_false_and_unbounded_enabledness_still_refuses() {
    let model = bound(
        "---- MODULE Word ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nFair == WF_x(Next)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let states = [Arc::from([Some(Value::Bool(true))])];
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "Fair"), &states, 0).expect("finite enabledness"),
        vec![false]
    );
    let unbounded = bound(
        "---- MODULE Unbounded ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' > x\nFair == WF_x(Next)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let states = [Arc::from([Some(0_i64.into())])];
    assert!(
        matches!(evaluate_lasso(&unbounded, &predicate(&unbounded, "Fair"), &states, 0), Err(TemporalError::Unsupported(s)) if s.contains("finite cover"))
    );
}

#[test]
fn temporal_validation_does_not_execute_side_effects() {
    let model = bound(
        "---- MODULE Word ----\nEXTENDS TLC\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nP == []Print(x, x)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let states = [Arc::from([Some(Value::Bool(true))])];
    assert_eq!(model.take_output(), Vec::<String>::new());
    assert!(
        matches!(evaluate_lasso(&model, &predicate(&model, "P"), &states, 0), Err(TemporalError::Unsupported(s)) if s.contains("side effects"))
    );
    assert_eq!(model.take_output(), Vec::<String>::new());
}

#[test]
fn inherited_search_levels_cannot_supply_temporal_truth() {
    #[derive(Debug)]
    struct SearchHost;
    impl dive_eval::Host for SearchHost {
        fn level(&self) -> Option<i64> {
            Some(100)
        }
    }
    let model = bound(
        "---- MODULE Word ----\nEXTENDS TLC\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nP == [](TLCGet(\"level\") = 100)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    model.evaluator.set_host(Arc::new(SearchHost));
    let states = [Arc::from([Some(Value::Bool(true))])];
    let result = evaluate_lasso(&model, &predicate(&model, "P"), &states, 0);
    assert!(
        matches!(&result, Err(TemporalError::Unsupported(s)) if s.contains("TLCGet")),
        "{result:?}"
    );
}

#[test]
fn pinned_tlc_liveness_words_violate_the_reported_property() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes");
    for name in [
        "LvNoFair",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
    ] {
        let dir = root.join("specs").join(name);
        let source = std::fs::read_to_string(dir.join(format!("{name}.tla"))).expect("source");
        let config = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config");
        let model = bound(&source, &config);
        let capture = root.join("tlc-2026.10.06.014338");
        let raw =
            std::fs::read_to_string(capture.join(format!("{name}.trace.json"))).expect("trace");
        let doc = TraceDoc::parse(&raw)
            .expect("document")
            .with_ending(Ending::Behavior);
        let (_, trace) = dive_trace::validate_doc(&model, &doc, &[], None);
        let trace = trace.expect("decoded trace");
        // TLC's last action pair carries the back-edge target's 1-based index,
        // including the repeated final state for a stuttering ending.
        let raw: serde_json::Value = serde_json::from_str(&raw).expect("raw json");
        let actions = raw["counterexample"]["action"].as_array().expect("actions");
        let target = actions.last().expect("closing action")[2][0]
            .as_u64()
            .expect("index");
        let back = usize::try_from(target - 1).expect("position");
        let values =
            evaluate_lasso(&model, &predicate(&model, "Prop"), &trace.states, back).expect(name);
        assert!(!values[0], "{name}: TLC's counterexample must falsify Prop");
        assert_eq!(
            std::fs::read_to_string(capture.join(format!("{name}.exit")))
                .expect("exit")
                .trim(),
            "13"
        );
    }
}

#[test]
fn recurring_action_certifies_both_fairness_kinds_on_every_small_word() {
    let model = bound(
        "---- MODULE Word ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\n\
         W == WF_x(Next)\nS == SF_x(Next)\n\
         EmptyW == WF_<<>>(Next)\nEmptyS == SF_<<>>(Next)\n\
         FalseW == WF_x(FALSE)\nFalseS == SF_x(FALSE)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    for len in 1..=4 {
        for bits in 0..(1_u32 << len) {
            let word: Vec<bool> = (0..len).map(|i| bits & (1 << i) != 0).collect();
            let states: Vec<State> = word
                .iter()
                .map(|b| Arc::from([Some(Value::Bool(*b))]))
                .collect();
            for back in 0..len {
                let recurring = (back..len).any(|i| {
                    let next = if i + 1 == len { back } else { i + 1 };
                    word[i] != word[next]
                });
                for name in ["W", "S", "EmptyW", "EmptyS", "FalseW", "FalseS"] {
                    let result = evaluate_lasso(&model, &predicate(&model, name), &states, back);
                    let expected = recurring || !matches!(name, "W" | "S");
                    assert_eq!(
                        result.expect(name),
                        vec![expected; len],
                        "{name}: {word:?}, {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn weak_and_strong_fairness_differ_on_intermittent_enabledness() {
    let model = bound(
        "---- MODULE Intermittent ----\nVARIABLES x, y\nInit == x = TRUE /\\ y = FALSE\nNext == x /\\ y' = ~y /\\ UNCHANGED x\nW == WF_y(Next)\nS == SF_y(Next)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    for len in 1..=4 {
        for bits in 0..(1_usize << (2 * len)) {
            let word: Vec<(bool, bool)> = (0..len)
                .map(|i| (bits & (1 << (2 * i)) != 0, bits & (1 << (2 * i + 1)) != 0))
                .collect();
            let states: Vec<State> = word
                .iter()
                .map(|(x, y)| Arc::from([Some(Value::Bool(*x)), Some(Value::Bool(*y))]))
                .collect();
            for back in 0..len {
                let occurs = (back..len).any(|i| {
                    let j = if i + 1 == len { back } else { i + 1 };
                    word[i].0 && word[j].0 && word[i].1 != word[j].1
                });
                let weak = occurs || word[back..].iter().any(|(x, _)| !x);
                let strong = occurs || word[back..].iter().all(|(x, _)| !x);
                for (name, expected) in [("W", weak), ("S", strong)] {
                    assert_eq!(
                        evaluate_lasso(&model, &predicate(&model, name), &states, back)
                            .expect(name),
                        vec![expected; len],
                        "{name}: {word:?}, {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn direct_enabledness_is_evaluated_independently_at_every_word_position() {
    let model = bound(
        "---- MODULE EnabledWord ----\nVARIABLE x\nInit == x = TRUE\nNext == x /\\ x' = FALSE\nE == ENABLED Next\nN == ~ENABLED Next\nP == []ENABLED Next\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let states = vec![
        Arc::from([Some(Value::Bool(true))]),
        Arc::from([Some(Value::Bool(false))]),
    ];
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "E"), &states, 0).expect("enabled"),
        vec![true, false]
    );
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "N"), &states, 0).expect("not enabled"),
        vec![false, true]
    );
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "P"), &states, 0).expect("always enabled"),
        vec![false, false]
    );
}
