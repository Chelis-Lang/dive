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
fn unfair_stutter_is_false_and_unbound_primes_in_enabled_refuse() {
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
    assert!(matches!(
        evaluate_lasso(&unbounded, &predicate(&unbounded, "Fair"), &states, 0),
        Err(TemporalError::Unsupported(_))
    ));
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
        let (_, trace) = dive_trace::validate_doc(&model, &doc, &[]);
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

#[test]
fn temporal_action_subscript_short_circuit_includes_closing_edge() {
    let model = bound(
        "---- MODULE ActionOrder ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0, 1, 2}\nBody == IF x' = 1 THEN 1 \\div 0 = 0 ELSE x' = 2\nAngle == []<><<Body>>_x\nDirectAngle == <<Body>>_x\nBox == [Body]_x\nWeak == WF_x(Body)\nStrong == SF_x(Body)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let state = |x| Arc::from([Some(Value::Int(x))]);
    let stutter = vec![state(1)];
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "Angle"), &stutter, 0)
            .expect("stutter skips body"),
        vec![false]
    );
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "Box"), &stutter, 0)
            .expect("boxed stutter skips body"),
        vec![true]
    );
    let word = vec![state(1), state(2)];
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "Angle"), &word, 1).expect("closing stutter"),
        vec![false, false]
    );
    let recurrent = vec![state(1), state(2), state(0)];
    for name in ["Weak", "Strong"] {
        assert_eq!(
            evaluate_lasso(&model, &predicate(&model, name), &recurrent, 1).expect(name),
            vec![true, true, true],
            "{name}"
        );
    }
    let captured: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-liveness/tests/fixtures/probes/temporal-action-order/capture.json"
    ))
    .expect("pinned capture");
    for before in 0..3 {
        for after in 0..3 {
            let word = vec![state(before), state(after)];
            for (name, mode) in [("DirectAngle", "angle"), ("Box", "box")] {
                let expected = captured["fields"][format!("{mode}:{before}:{after}")]
                    .as_str()
                    .expect("complete pinned row");
                let observed = evaluate_lasso(&model, &predicate(&model, name), &word, 1);
                match observed {
                    Ok(values) => assert_eq!(values[0].to_string(), expected),
                    Err(error) => assert!(
                        expected == "error:2179" && error.to_string().contains("2179"),
                        "{mode}:{before}:{after}: {error}"
                    ),
                }
            }
        }
    }
    let bad = vec![state(0), state(1)];
    assert!(evaluate_lasso(&model, &predicate(&model, "Angle"), &bad, 1).is_err());
}

#[test]
fn selectseq_independent_fresh_lasso_checks_forwarded_state_and_lexical_callbacks() {
    let source = r"---- MODULE SelectWord ----
EXTENDS Integers, Sequences
VARIABLE x
Init==x=0
Next==x'=1-x
Forward(F(_),s)==SelectSeq(s,F)
Capture(k)==LET Test(y)==y=k IN Forward(Test,<<0,1,0>>)
P==[] (Capture(x)=IF x=0 THEN <<0,0>> ELSE <<1>>)
Bad==<>[] (Capture(x) = <<0,0>>)
====";
    // Bind anew, independently of any planner or checker result/cache.
    let model = bound(source, "INIT Init\nNEXT Next\n");
    let states = vec![
        Arc::from([Some(Value::Int(0))]),
        Arc::from([Some(Value::Int(1))]),
    ];
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "P"), &states, 0).expect("fresh trace"),
        vec![true, true]
    );
    assert_eq!(
        evaluate_lasso(&model, &predicate(&model, "Bad"), &states, 0).expect("false claim"),
        vec![false, false]
    );
    let fresh = bound(source, "INIT Init\nNEXT Next\nPROPERTY Bad\n");
    let doc = TraceDoc::behavior(vec![serde_json::json!({"x":0}), serde_json::json!({"x":1})])
        .expect("word")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["Bad".into()]);
    assert_eq!(
        dive_trace::validate_doc(&fresh, &doc, &[]).0.validation,
        dive_trace::Validation::Valid
    );
    let forged = TraceDoc::behavior(vec![serde_json::json!({"x":0}), serde_json::json!({"x":2})])
        .expect("forged word")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["Bad".into()]);
    assert!(matches!(
        dive_trace::validate_doc(
            &bound(source, "INIT Init\nNEXT Next\nPROPERTY Bad\n"),
            &forged,
            &[]
        )
        .0
        .validation,
        dive_trace::Validation::Invalid(_)
    ));
    let unknown = TraceDoc::behavior(vec![serde_json::json!({"x":"post-bind-name"})])
        .expect("unknown name")
        .with_ending(Ending::Stuttering)
        .with_properties(vec!["Bad".into()]);
    assert!(matches!(
        dive_trace::validate_doc(
            &bound(source, "INIT Init\nNEXT Next\nPROPERTY Bad\n"),
            &unknown,
            &[]
        )
        .0
        .validation,
        dive_trace::Validation::Unsupported(_)
    ));
    for (callback, supported) in [
        ("LAMBDA y: y=x'", false),
        ("LAMBDA y: \\E z \\in {y}: z=x", true),
        ("LAMBDA y: y \\in Seq({x})", false),
    ] {
        let source = format!(
            "---- MODULE SelectWordRefusal ----\nEXTENDS Integers, Sequences\nVARIABLE x\nInit==x=0\nNext==x'=x\nP==[] [(SelectSeq(<<0>>, {callback}) = <<0>>)]_x\n===="
        );
        let fresh = bound(&source, "INIT Init\nNEXT Next\n");
        let result = evaluate_lasso(&fresh, &predicate(&fresh, "P"), &states, 0);
        if supported {
            assert_eq!(
                result.expect("concrete quantified callback"),
                vec![false, false],
                "{callback}"
            );
        } else {
            assert!(
                matches!(result, Err(TemporalError::Unsupported(_))),
                "{callback}"
            );
        }
    }
}

#[test]
fn selectseq_independent_enabled_negation_and_fairness_use_original_lazy_state_helpers() {
    let model = bound(
        "---- MODULE SelectTraceEnabled ----\nEXTENDS Sequences\nVARIABLE x\nInit == x = <<0>>\nNext==x'=x\nPure(S)==SelectSeq(S,LAMBDA y:y=Head(x))\nAction==(x'=x) /\\ (Pure(x) = x)\nE==[] ENABLED Action\nN==[] ~ENABLED Action\nOrdinary==[] ENABLED Next\nFair==WF_x(Action)\nStrong==SF_x(Action)\n====",
        "INIT Init\nNEXT Next\n",
    );
    let states = vec![Arc::from([Some(Value::tuple(vec![0.into()]))])];
    for (name, expected) in [
        ("Ordinary", true),
        ("E", true),
        ("N", false),
        ("Fair", true),
        ("Strong", true),
    ] {
        assert_eq!(
            evaluate_lasso(&model, &predicate(&model, name), &states, 0)
                .expect("independent original relation/finite covers"),
            vec![expected],
            "{name}"
        );
    }
}

#[test]
fn quantified_subscripts_keep_their_own_bindings_on_the_same_word() {
    let model = bound(
        r"---- MODULE SubWord ----
VARIABLES x, y
Init == x /\ y
Next == x' = ~x /\ y' = y
Vars == <<x, y>>
Sub(i) == IF i THEN x ELSE y
P == \A i \in BOOLEAN : [][FALSE]_(Sub(i))
Q == ([][FALSE]_Vars) \/ ([][TRUE]_Vars)
====",
        "INIT Init\nNEXT Next\n",
    );
    let word: Vec<State> = [true, false]
        .into_iter()
        .map(|x| Arc::from([Some(Value::Bool(x)), Some(Value::Bool(false))]))
        .collect();
    for (name, expected) in [("P", false), ("Q", true)] {
        assert_eq!(
            evaluate_lasso(&model, &predicate(&model, name), &word, 0).expect("scoped subscript"),
            vec![expected; 2]
        );
    }
}

#[test]
fn supplied_lasso_propagates_an_installed_control() {
    #[derive(Debug)]
    struct Stop;
    impl dive_eval::HostControl for Stop {
        fn check(&self) -> Result<(), dive_eval::Interrupted> {
            Err(dive_eval::Interrupted::new(17))
        }
    }
    let model = bound(
        "---- MODULE Word ----\nVARIABLE x\nInit == x\nNext == x' = x\nP == []x\n====",
        "INIT Init\nNEXT Next\n",
    );
    let pred = predicate(&model, "P");
    let states: Vec<State> = vec![Arc::from([Some(Value::Bool(true))]); 512];
    let _control = model
        .evaluator
        .host_control(Some(Arc::new(Stop)))
        .expect("control");
    assert!(matches!(
        evaluate_lasso(&model, &pred, &states, 0),
        Err(TemporalError::Interrupted(_))
    ));
}

#[test]
fn indexed_fairness_matches_ordinary_action_demand_at_each_loop_entry() {
    let model = bound(
        r"---- MODULE Dispatch ----
EXTENDS Naturals
VARIABLE x, active
Init == x = FALSE /\ active = {}
Next == UNCHANGED <<x,active>>
Vars == x
Act(k) == LET on == ~x IN /\ k \in active /\ x' = on /\ UNCHANGED active
Plain(k) == /\ TRUE /\ Act(k)
Warm == WF_Vars(x' = ~x)
SF1 == (Warm \/ ~Warm) /\ \A k \in {1,2} : SF_Vars(Act(k))
SF2 == (Warm \/ ~Warm) /\ \A k \in {1,2} : SF_Vars(Plain(k))
WF1 == (Warm \/ ~Warm) /\ \A k \in {1,2} : WF_Vars(Act(k))
WF2 == (Warm \/ ~Warm) /\ \A k \in {1,2} : WF_Vars(Plain(k))
Member == \A k \in {1,2} : (k \in active) ~> x
Other == \A k \in {1,2} : (~(k \notin active)) ~> x
Bad(k) == /\ k \in active /\ x' = (1 \div 0) /\ UNCHANGED active
Fault == (Warm \/ ~Warm) /\ \A k \in {1,2} : SF_Vars(Bad(k))
Before(k) == /\ (1 \div 0) = 4 /\ k \in active /\ x' = ~x /\ UNCHANGED active
Order == (Warm \/ ~Warm) /\ \A k \in {1,2} : SF_Vars(Before(k))
====",
        "INIT Init\nNEXT Next\n",
    );
    let make = |x, keys: &[i64]| -> State {
        let values = keys.iter().map(|n| Value::Int(*n)).collect();
        let set = Value::set_canonical(values, dive_value::Ctx::tlc()).expect("finite set");
        Arc::from([Some(Value::Bool(x)), Some(set)])
    };
    // Duplicate source members, overlapping memberships, absent and dense
    // bindings, changing first and last states, and a nonzero closing edge.
    for flags in 0..8 {
        let word = [
            make(flags & 1 != 0, &[1, 1]),
            make(flags & 2 != 0, &[1, 2]),
            make(flags & 4 != 0, &[2]),
        ];
        for back in 0..word.len() {
            for (fast, slow) in [("WF1", "WF2"), ("SF1", "SF2"), ("Member", "Other")] {
                let run = |name| {
                    evaluate_lasso(&model, &predicate(&model, name), &word, back).expect(name)
                };
                assert_eq!(run(fast), run(slow), "{fast}, flags {flags}, loop {back}");
            }
        }
    }
    let empty = [make(false, &[]), make(false, &[])];
    assert!(evaluate_lasso(&model, &predicate(&model, "Fault"), &empty, 0).is_ok());
    assert!(evaluate_lasso(&model, &predicate(&model, "Order"), &empty, 0).is_err());
    let active = [make(false, &[]), make(true, &[2]), make(false, &[])];
    assert!(evaluate_lasso(&model, &predicate(&model, "Fault"), &active, 0).is_err());
}

#[test]
fn sparse_guard_preserves_demand_for_a_strict_let_in_a_supplied_scope() {
    let model = bound(
        r"---- MODULE Demand ----
VARIABLE x, active
Init == x = FALSE /\ active = {}
Next == x' = ~x /\ UNCHANGED active
V == x
Warm == WF_V(x' = ~x)
P == \A u \in {1} : (Warm \/ ~Warm) /\ \A k \in {1} :
     SF_V(LET unused == <<u>> IN k \in active /\ x' = ~x)
====",
        "INIT Init\nNEXT Next\n",
    );
    let root = predicate(&model, "P");
    let dive_ir::Expr::Quant { body, .. } = model.ir.expr(root.expr) else {
        panic!("P must start with a quantifier")
    };
    // The library caller supplies a root frame without binding u. Building
    // the strict tuple forces u before the first membership guard.
    let inner = Pred::new(*body, root.scope);
    let empty = Value::set(vec![], dive_value::Ctx::tlc()).expect("empty set");
    let states: Vec<State> = [false, true]
        .into_iter()
        .map(|x| Arc::from([Some(Value::Bool(x)), Some(empty.clone())]))
        .collect();
    let err =
        evaluate_lasso(&model, &inner, &states, 0).expect_err("strict value must be evaluated");
    assert!(err.to_string().contains("unbound slot"), "{err}");
}

#[test]
fn supplied_foreign_atom_keeps_the_ordinary_membership_comparison() {
    let model = bound(
        r#"---- MODULE Foreign ----
VARIABLE x, active
Init == x = FALSE /\ active = {"hello"}
Next == UNCHANGED <<x,active>>
Member(key) == (key \in active) ~> x
Plain(key) == (~(key \notin active)) ~> x
===="#,
        "INIT Init\nNEXT Next\n",
    );
    let local = model.names.intern("hello");
    let foreign = dive_value::Interner::new();
    let _padding = foreign.intern("padding");
    let mut external = foreign.intern("hello");
    // Distinguish text-based hashing from the encounter-order comparison.
    if external.tok() == local.tok() {
        let other = dive_value::Interner::new();
        external = other.intern("hello");
    }
    assert_ne!(external.tok(), local.tok());
    let set = Value::set(vec![Value::Str(local)], dive_value::Ctx::tlc()).expect("word set");
    let states: Vec<State> = vec![Arc::from([Some(Value::FALSE), Some(set)])];
    for name in ["Member", "Plain"] {
        let root = predicate(&model, name);
        let scope = model
            .evaluator
            .scope_bind(&root.scope, dive_ir::Slot(0), Value::Str(external.clone()))
            .expect("supplied frame");
        let got = evaluate_lasso(&model, &Pred::new(root.expr, scope), &states, 0)
            .expect("ordinary query");
        assert_eq!(got, [true], "{name}");
    }
}
