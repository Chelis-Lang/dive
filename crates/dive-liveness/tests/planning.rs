//! Bound pure formula planning versus direct word semantics and pinned probes.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_eval::St;
use dive_liveness::Graph;
use dive_liveness::formula::{AtomKind, Limits, Tableau, compile};
use dive_liveness::plan::{Plan, lower};
use dive_liveness::search::{Acceptance, find_lasso};
use dive_model::{Model, Pred};
use dive_value::Value;
use std::sync::Arc;

fn bound(source: &str, config: &str) -> Model {
    let name = source.split_whitespace().nth(2).expect("name");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}

fn pred(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect("definition");
    Pred::new(model.ir.def(d).body, model.evaluator.scope(d))
}

fn accepts(model: &Model, plan: &Plan, t: &Tableau, word: &[Value], back: usize) -> bool {
    let mut values = Vec::new();
    for pos in 0..word.len() {
        let next = if pos + 1 == word.len() { back } else { pos + 1 };
        let st = St::action(
            Arc::from([Some(word[pos].clone())]),
            Arc::from([Some(word[next].clone())]),
        );
        let row: Vec<bool> = plan
            .atoms
            .iter()
            .map(|a| {
                match model
                    .evaluator
                    .eval_scoped(&a.pred.scope, a.pred.expr, &st)
                    .expect("atom")
                {
                    Value::Bool(b) => b,
                    v => panic!("atom was {}", v.kind_name()),
                }
            })
            .collect();
        values.push(row);
    }
    let fits = |pos: usize, n: usize| {
        t.nodes[n]
            .literals
            .iter()
            .all(|lit| values[pos][lit.atom.index] == lit.positive)
    };
    let width = t.nodes.len();
    let mut edges = vec![vec![]; word.len() * width];
    for pos in 0..word.len() {
        let next = if pos + 1 == word.len() { back } else { pos + 1 };
        for n in 0..width {
            if fits(pos, n) {
                edges[pos * width + n] = t.nodes[n]
                    .successors
                    .iter()
                    .filter(|s| fits(next, **s))
                    .map(|s| next * width + s)
                    .collect();
            }
        }
    }
    let roots = t.roots.iter().copied().filter(|n| fits(0, *n)).collect();
    let recurrent = edges.iter().map(|es| vec![true; es.len()]).collect();
    let states = (0..t.promise_count)
        .map(|p| {
            (0..word.len() * width)
                .map(|v| t.nodes[v % width].fulfilled[p])
                .collect()
        })
        .collect();
    find_lasso(
        &Graph::new(edges, roots).expect("product"),
        &Acceptance {
            recurrent,
            states,
            actions: vec![],
        },
    )
    .expect("search")
    .is_some()
}

fn reachable(start: usize, len: usize, back: usize) -> Vec<usize> {
    let mut path = Vec::new();
    let mut at = start;
    while !path.contains(&at) {
        path.push(at);
        at = if at + 1 == len { back } else { at + 1 };
    }
    path
}

#[test]
fn bound_boolean_temporal_and_action_formulas_match_direct_words() {
    let model = bound(
        "---- MODULE Planning ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\n\
        A == []x\nE == <>x\nAE == []<>x\nEA == <>[]x\nL == x ~> ~x\n\
        I == (<>x => []x)\nQ == (<>x <=> []<>x)\nAlias == AE\n\
        Conditional == IF TRUE THEN AE ELSE EA\nAction == []<><<x' # x>>_x\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    for name in [
        "A",
        "E",
        "AE",
        "EA",
        "L",
        "I",
        "Q",
        "Alias",
        "Conditional",
        "Action",
    ] {
        let p = lower(&model, &pred(&model, name), Limits::default()).expect(name);
        let t = compile(&p.formula, Limits::default()).expect("tableau");
        if name == "Action" {
            assert!(p.atoms.iter().any(|a| a.kind == AtomKind::Action));
        }
        for len in 1..=4 {
            for bits in 0..(1_u32 << len) {
                let word: Vec<bool> = (0..len).map(|i| bits & (1 << i) != 0).collect();
                for back in 0..len {
                    let future = |i| reachable(i, len, back);
                    let always = |i| future(i).iter().all(|j| word[*j]);
                    let eventually = |i| future(i).iter().any(|j| word[*j]);
                    let ae = future(0).iter().all(|j| eventually(*j));
                    let expected = match name {
                        "A" => always(0),
                        "E" => eventually(0),
                        "AE" | "Alias" | "Conditional" => ae,
                        "EA" => future(0).iter().any(|j| always(*j)),
                        "L" => future(0)
                            .iter()
                            .all(|j| !word[*j] || future(*j).iter().any(|k| !word[*k])),
                        "I" => !eventually(0) || always(0),
                        "Q" => eventually(0) == ae,
                        "Action" => (back..len)
                            .any(|i| word[i] != word[if i + 1 == len { back } else { i + 1 }]),
                        _ => panic!("missing oracle"),
                    };
                    assert_eq!(
                        accepts(
                            &model,
                            &p,
                            &t,
                            &word.iter().map(|b| Value::Bool(*b)).collect::<Vec<_>>(),
                            back
                        ),
                        expected,
                        "{name} {word:?} back {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn unsupported_planning_never_executes_effects_or_guesses_a_scope() {
    let model = bound(
        "---- MODULE Refusal ----\nEXTENDS TLC\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\n\
        Fair == WF_x(Next)\nEffect == []Print(x,x)\nRead == [](TLCGet(\"level\") = 0)\n\
        T(p) == []p\nParam == T(x)\nBad == <>1\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let _ = model.take_output();
    for (name, reason) in [
        ("Fair", "fairness"),
        ("Effect", "effects"),
        ("Read", "TLCGet"),
        ("Param", "constructor"),
        ("Bad", "not boolean"),
    ] {
        let error = lower(&model, &pred(&model, name), Limits::default()).expect_err(name);
        assert_eq!(error.code(), "DIVE-L0020");
        let dive_liveness::plan::PlanningError::Unsupported(r) = &error else {
            panic!("expected refusal")
        };
        assert!(r.cause.origin.begin.0 > 0);
        assert!(r.request.origin.begin.0 > 0);
        assert!(r.request.location.contains("module Refusal"));
        let err = error.to_string();
        assert!(err.contains(reason), "{name}: {err}");
        assert!(
            model.take_output().is_empty(),
            "planning printed for {name}"
        );
    }
    assert!(
        lower(
            &model,
            &pred(&model, "Fair"),
            Limits {
                expansion_steps: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn pinned_counterexample_words_falsify_the_lowered_property() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-probes");
    for name in [
        "LvNoFair",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
        "LvEAPrefix",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let cfg =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("cfg");
        let model = bound(&src, &cfg);
        let raw =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.trace.json")))
                .expect("trace");
        let j: serde_json::Value = serde_json::from_str(&raw).expect("JSON");
        let word: Vec<Value> = j["counterexample"]["state"]
            .as_array()
            .expect("states")
            .iter()
            .map(|s| {
                let v = &s[1]["x"];
                if let Some(b) = v.as_bool() {
                    Value::Bool(b)
                } else {
                    Value::int(dive_value::Int::from(v.as_i64().expect("integer")))
                }
            })
            .collect();
        let actions = j["counterexample"]["action"].as_array().expect("actions");
        let back = usize::try_from(
            actions.last().expect("loop")[2][0]
                .as_u64()
                .expect("target"),
        )
        .expect("index")
            - 1;
        let plan = lower(&model, &pred(&model, "Prop"), Limits::default()).expect(name);
        let t = compile(&plan.formula, Limits::default()).expect("tableau");
        assert!(!accepts(&model, &plan, &t, &word, back), "{name}");
        // This checks property truth only, not the behavior's fairness.
        for fairness in &model.temporals {
            let refusal = lower(&model, &fairness.pred, Limits::default())
                .expect_err("fairness remains pending");
            assert!(
                refusal.to_string().contains("fairness"),
                "{name}: {refusal}"
            );
        }
    }
}
