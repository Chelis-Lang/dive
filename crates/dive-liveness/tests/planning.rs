//! Bound pure formula planning versus direct word semantics and pinned probes.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_eval::St;
use dive_liveness::Graph;
use dive_liveness::formula::{AtomKind, Limits, Tableau, compile};
use dive_liveness::plan::{Plan, lower};
use dive_liveness::product::{Labels, Limits as ProductLimits, Product};
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
    assert!(a.accepted(), "{:?}", a.diagnostics());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}

fn pred(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect("definition");
    Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("valid scope"),
    )
}

fn accepts(model: &Model, plan: &Plan, t: &Tableau, word: &[Value], back: usize) -> bool {
    let states: Vec<dive_eval::next::StateVals> =
        word.iter().map(|v| Arc::from([Some(v.clone())])).collect();
    accepts_states(model, plan, t, &states, back)
}

fn accepts_states(
    model: &Model,
    plan: &Plan,
    t: &Tableau,
    word: &[dive_eval::next::StateVals],
    back: usize,
) -> bool {
    let mut values = Vec::new();
    for pos in 0..word.len() {
        let next = if pos + 1 == word.len() { back } else { pos + 1 };
        let st = St::action(word[pos].clone(), word[next].clone());
        let row: Vec<bool> = plan
            .atoms
            .iter()
            .map(|a| a.evaluate(model, &st).expect("atom"))
            .collect();
        values.push(row);
    }
    let edges = (0..word.len())
        .map(|pos| vec![if pos + 1 == word.len() { back } else { pos + 1 }])
        .collect();
    let labels = Labels {
        states: plan
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| {
                values
                    .iter()
                    .map(|row| a.kind == AtomKind::State && row[i])
                    .collect()
            })
            .collect(),
        actions: plan
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| {
                values
                    .iter()
                    .map(|row| vec![a.kind == AtomKind::Action && row[i]])
                    .collect()
            })
            .collect(),
    };
    Product::new(
        &Graph::new(edges, vec![0]).expect("word"),
        t,
        &labels,
        ProductLimits::default(),
    )
    .expect("product")
    .find_lasso()
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
        F(a) == WF_x(a)\nFair == F(Next)\nEffect == []Print(x,x)\nRead == [](TLCGet(\"level\") = 0)\n\
        T(p) == []p\nParam == T(x)\nH(G(_),p) == G(p)\nKnown == H(T,x)\nRECURSIVE Recur(_)\nRecur(p) == IF p THEN p ELSE Recur(p)\nHigher == H(Recur,x)\nBad == <>1\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let _ = model.take_output();
    // The exact former refusal fixture is now a proved ordinary user actual.
    // The clean pin rejects its false initial state with safety exit 12;
    // the whole-arena facade still refuses this source's unused effectful defs.
    let known = lower(&model, &pred(&model, "Known"), Limits::default()).expect("known actual");
    let tableau = compile(&known.formula, Limits::default()).expect("known tableau");
    assert!(accepts(&model, &known, &tableau, &[Value::Bool(true)], 0));
    assert!(!accepts(&model, &known, &tableau, &[Value::Bool(false)], 0));
    assert!(
        model.take_output().is_empty(),
        "known forwarding must not print"
    );
    for (name, reason) in [
        ("Fair", "action/temporal operator argument"),
        ("Effect", "effects"),
        ("Read", "TLCGet"),
        ("Higher", "recursive"),
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
    let error = lower(&model, &pred(&model, "Bad"), Limits::default()).expect_err("Bad");
    assert_eq!(error.code(), "DIVE-L0023");
    let dive_liveness::plan::PlanningError::Evaluation { error, source } = error else {
        panic!("expected TLC evaluation error for a numeric temporal operand")
    };
    assert_eq!(error.class(), dive_eval::ErrorClass::Eval);
    assert_eq!(error.ec(), dive_eval::Ec::ExpectedValue);
    assert_eq!(error.ec().code(), 2215);
    assert!(source.origin.begin.0 > 0);
    assert_eq!(error.origin(), Some(source.expr));
    assert_eq!(source.location, model.evaluator.location(source.expr));
    assert_eq!(
        error.message(),
        "TLC expected a boolean value, but did not find one.\n1"
    );
    assert!(model.take_output().is_empty(), "planning printed for Bad");
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
        for fairness in &model.temporals {
            let plan = lower(&model, &fairness.pred, Limits::default()).expect("fairness");
            let t = compile(&plan.formula, Limits::default()).expect("fair tableau");
            assert!(
                accepts(&model, &plan, &t, &word, back),
                "{name}: behavior fairness must hold"
            );
        }
    }
}

#[test]
fn fairness_language_matches_infinite_boolean_words() {
    let model = bound(
        "---- MODULE FairWords ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nW == WF_x(Next)\nS == SF_x(Next)\nN == ~WF_x(Next)\nE == []ENABLED Next\nEmptyW == WF_<<>>(Next)\nFalseS == SF_x(FALSE)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    for name in ["W", "S", "N", "E", "EmptyW", "FalseS"] {
        let plan = lower(&model, &pred(&model, name), Limits::default()).expect(name);
        let t = compile(&plan.formula, Limits::default()).expect("tableau");
        for len in 1..=4 {
            for bits in 0..(1_usize << len) {
                let word: Vec<Value> = (0..len)
                    .map(|i| Value::Bool(bits & (1 << i) != 0))
                    .collect();
                for back in 0..len {
                    let recurring = (back..len)
                        .any(|i| word[i] != word[if i + 1 == len { back } else { i + 1 }]);
                    let expected = match name {
                        "W" | "S" => recurring,
                        "N" => !recurring,
                        _ => true,
                    };
                    assert_eq!(
                        accepts(&model, &plan, &t, &word, back),
                        expected,
                        "{name}: {word:?} {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn fairness_product_distinguishes_weak_and_strong_intermittent_enabledness() {
    let model = bound(
        "---- MODULE Intermittent ----\nVARIABLES x, y\nInit == x = TRUE /\\ y = FALSE\nNext == x /\\ y' = ~y /\\ UNCHANGED x\nW == WF_y(Next)\nS == SF_y(Next)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    for name in ["W", "S"] {
        let plan = lower(&model, &pred(&model, name), Limits::default()).expect(name);
        let t = compile(&plan.formula, Limits::default()).expect("tableau");
        for len in 1..=3 {
            for bits in 0..(1_usize << (2 * len)) {
                let word: Vec<(bool, bool)> = (0..len)
                    .map(|i| (bits & (1 << (2 * i)) != 0, bits & (1 << (2 * i + 1)) != 0))
                    .collect();
                let states: Vec<dive_eval::next::StateVals> = word
                    .iter()
                    .map(|(x, y)| Arc::from([Some(Value::Bool(*x)), Some(Value::Bool(*y))]))
                    .collect();
                for back in 0..len {
                    let occurs = (back..len).any(|i| {
                        let j = if i + 1 == len { back } else { i + 1 };
                        word[i].0 && word[j].0 && word[i].1 != word[j].1
                    });
                    let disabled = if name == "W" {
                        word[back..].iter().any(|(x, _)| !x)
                    } else {
                        word[back..].iter().all(|(x, _)| !x)
                    };
                    assert_eq!(
                        accepts_states(&model, &plan, &t, &states, back),
                        occurs || disabled,
                        "{name}: {word:?} {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn temporal_nonstuttering_checks_subscript_before_body() {
    use dive_liveness::plan::{AtomTest, PredicateAtom};
    let model = bound(
        "---- MODULE ActionOrder ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0, 1, 2}\nBody == IF x' = 1 THEN 1 \\div 0 = 0 ELSE x' = 2\nSub == x\nRuntime == <<Body>>_x\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let atom = PredicateAtom {
        pred: pred(&model, "Body"),
        kind: AtomKind::Action,
        test: AtomTest::Nonstuttering {
            sub: pred(&model, "Sub").expr,
        },
    };
    let state = |x| Arc::from([Some(Value::Int(x))]);
    let stutter = St::action(state(1), state(1));
    assert!(
        !atom
            .evaluate(&model, &stutter)
            .expect("stutter skips erroneous body")
    );
    assert!(
        !atom
            .evaluate(&model, &St::action(state(1), state(0)))
            .expect("false body")
    );
    assert!(
        atom.evaluate(&model, &St::action(state(1), state(2)))
            .expect("true body")
    );
    assert_eq!(
        atom.evaluate(&model, &St::action(state(0), state(1)))
            .expect_err("nonstutter evaluates body")
            .ec()
            .code(),
        2179
    );
    // Ordinary runtime action expressions follow Tool.eval, which checks the
    // body first. Temporal LNAction evaluation has a different short circuit.
    assert_eq!(
        model
            .eval(&pred(&model, "Runtime"), &stutter)
            .expect_err("runtime body-first error")
            .ec()
            .code(),
        2179
    );
}

#[test]
fn direct_temporal_and_runtime_atoms_match_pinned_order_controls() {
    use dive_liveness::plan::{AtomTest, PredicateAtom};
    let model = bound(
        include_str!("fixtures/probes/temporal-action-order/ActionOrder.tla"),
        "INIT Init\nNEXT Next\n",
    );
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/probes/temporal-action-order/capture.json"
    ))
    .expect("pinned capture");
    let state = |x| Arc::from([Some(Value::Int(x))]);
    for mode in ["angle", "bad-sub", "runtime"] {
        for before in 0..3 {
            for after in 0..3 {
                let key = format!("{mode}:{before}:{after}");
                let expected = capture["fields"][&key]
                    .as_str()
                    .expect("complete row")
                    .to_ascii_lowercase();
                let st = St::action(state(before), state(after));
                let value = match mode {
                    "runtime" => model
                        .eval(&pred(&model, "Runtime"), &st)
                        .map(|v| v == Value::Bool(true))
                        .map_err(|e| format!("error:{}", e.ec().code())),
                    _ => PredicateAtom {
                        pred: pred(
                            &model,
                            if mode == "bad-sub" {
                                "FalseBody"
                            } else {
                                "Body"
                            },
                        ),
                        kind: AtomKind::Action,
                        test: AtomTest::Nonstuttering {
                            sub: pred(&model, if mode == "bad-sub" { "BadSub" } else { "Sub" })
                                .expr,
                        },
                    }
                    .evaluate(&model, &st)
                    .map_err(|e| format!("error:{}", e.ec().code())),
                };
                assert_eq!(
                    value.map_or_else(|e| e, |b| b.to_string()),
                    expected,
                    "{key}"
                );
            }
        }
    }
    let mut invalid = PredicateAtom {
        pred: pred(&model, "Body"),
        kind: AtomKind::Action,
        test: AtomTest::Nonstuttering {
            sub: pred(&model, "Sub").expr,
        },
    };
    invalid.pred.expr = dive_ir::ExprId(u32::MAX);
    assert_eq!(
        invalid
            .evaluate(&model, &St::action(state(1), state(1)))
            .expect_err("invalid body rejected before short circuit")
            .class(),
        dive_eval::ErrorClass::Unsupported
    );
}

#[test]
fn selectseq_user_lambda_and_forwarded_lexical_callbacks_read_actual_states() {
    let model = bound(
        r#"---- MODULE SelectCallbacks ----
EXTENDS Integers, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
Read(a) == a[2] = "read"
Write(a) == a[2] = "write"
Forward(F(_), s) == SelectSeq(s, F)
Captured(k) == LET Test(y) == y = k IN Forward(Test, <<0,1,0>>)
D == [] (SelectSeq(<< <<1,"read">>, <<2,"write">>, <<1,"read">> >>, Read)
          = << <<1,"read">>, <<1,"read">> >>)
W == [] (SelectSeq(<< <<1,"read">>, <<2,"write">> >>, Write) = << <<2,"write">> >>)
T == [] (SelectSeq(<<[Type |-> "User"],[Type |-> "Other"],[Type |-> "User"]>>,
                 LAMBDA y: y.Type="User") = <<[Type |-> "User"],[Type |-> "User"]>>)
P == [] (Captured(x) = IF x=0 THEN <<0,0>> ELSE <<1>>)
Q == [] (Captured(1-x) = IF x=0 THEN <<1>> ELSE <<0,0>>)
===="#,
        "INIT Init\nNEXT Next\n",
    );
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let default = dive_model::bind(model.ir.clone(), &cfg, &dive_model::BindOptions::default())
        .expect("default profile");
    for model in [model, default] {
        for name in ["D", "W", "T", "P", "Q"] {
            let plan = lower(&model, &pred(&model, name), Limits::default()).expect(name);
            assert!(
                !plan.atoms.is_empty(),
                "callback formal is conservatively state-level: {name}"
            );
            for x in [0, 1] {
                let st = St::state(Arc::from([Some(Value::Int(x))]));
                for atom in &plan.atoms {
                    assert!(atom.evaluate(&model, &st).expect(name), "{name}: x={x}");
                }
            }
        }
    }
}

#[test]
fn selectseq_temporal_callbacks_keep_named_capability_refusals() {
    for (callback, supported) in [
        ("LAMBDA y: y=x'", false),
        ("LAMBDA y: \\E z \\in {y}: z=x", true),
        ("LAMBDA y: [z \\in {y} |-> z] = [z \\in {y} |-> z]", true),
        ("LAMBDA y: y \\in Seq({x})", false),
        ("LAMBDA y: PrintT(y)", false),
        ("LAMBDA y: ToString(y)=\"0\"", false),
        ("LAMBDA y: ENABLED (x'=y)", false),
        ("LAMBDA y: RandomElement({TRUE,FALSE})", false),
        (
            "LAMBDA y: IF TRUE THEN y=x ELSE (\\E z \\in {y}: z=x)",
            true,
        ),
    ] {
        let source = format!(
            "---- MODULE SelectRefused ----\nEXTENDS Integers, Sequences, TLC\nVARIABLE x\nInit==x=0\nNext==x'=1-x\nP==[] [(SelectSeq(<<0>>, {callback}) = <<0>>)]_x\n===="
        );
        let model = bound(&source, "INIT Init\nNEXT Next\n");
        let predicate = pred(&model, "P");
        let result = lower(&model, &predicate, Limits::default());
        if !supported {
            assert!(result.is_err(), "{callback}");
            continue;
        }
        let plan = result.expect(callback);
        let constant_true = callback.contains("|->");
        for x in [0, 1] {
            let before: dive_trace::State = Arc::from([Some(Value::Int(x))]);
            let after: dive_trace::State = Arc::from([Some(Value::Int(1 - x))]);
            let expected = constant_true || x == 0;
            let rows = plan
                .atoms
                .iter()
                .map(|atom| {
                    atom.evaluate(&model, &St::action(before.clone(), after.clone()))
                        .expect("original binder callback")
                })
                .collect::<Vec<_>>();
            assert!(
                rows.iter().all(|actual| *actual == expected),
                "{callback}:x={x}"
            );
        }
        let states = [
            Arc::from([Some(Value::Int(0))]),
            Arc::from([Some(Value::Int(1))]),
        ];
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(&model, &predicate, &states, 0)
                .expect("independent action word"),
            vec![constant_true, constant_true],
            "{callback}"
        );
    }
}

#[test]
fn selectseq_constant_and_if_probes_lower_with_warm_enclosing_caches() {
    let mut model = bound(
        "---- MODULE SelectConstant ----\nEXTENDS Sequences\nVARIABLE x\nInit==x=TRUE\nNext==x'=x\nKeep(y)==TRUE\nSelected==SelectSeq(<<0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0>>,Keep)\nWrapper==Selected\nSame==Selected = <<0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0>>\nP==[] Same\nI==IF Same THEN <>x ELSE <>~x\n====",
        "INIT Init\nNEXT Next\n",
    );
    // Synthetic mechanism coverage: binding's authoritative precomputed values
    // are distinct from evaluator result caches. Rebuild scopes under a raw
    // evaluator over the identical IR/names; do not reuse bound-role scopes.
    let mut inputs = dive_eval::Model::new(model.ir.clone());
    inputs.names = model.names.clone();
    model.evaluator =
        dive_eval::Evaluator::new(inputs, dive_eval::Options::tlc()).expect("raw evaluator");
    let selected = pred(&model, "Wrapper");
    assert_eq!(
        model
            .evaluator
            .eval_scoped(&selected.scope, selected.expr, &St::constant())
            .expect("warm enclosing definition cache"),
        Value::tuple(vec![0.into(); 20])
    );
    for name in ["P", "I"] {
        lower(&model, &pred(&model, name), Limits::default())
            .expect("ordinary definition/IF evaluation");
    }
}

#[test]
fn selectseq_nonstuttering_reads_subscript_before_and_after() {
    use dive_liveness::plan::{AtomTest, PredicateAtom};
    let model = bound(
        "---- MODULE SelectAggregate ----\nEXTENDS Sequences\nVARIABLE x\nInit==x=0\nNext==x'=1\nKeep(y)==TRUE\nSub==SelectSeq(<<x,x,x,x,x,x,x,x,x,x,x,x,x,x,x,x,x,x,x,x>>,Keep)\nBody==TRUE\nSingle==Sub = <<0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0>>\n====",
        "INIT Init\nNEXT Next\n",
    );
    let sub = pred(&model, "Sub");
    let before: Arc<[Option<Value>]> = Arc::from([Some(Value::Int(0))]);
    let after: Arc<[Option<Value>]> = Arc::from([Some(Value::Int(1))]);
    let atom = PredicateAtom {
        pred: pred(&model, "Body"),
        kind: AtomKind::Action,
        test: AtomTest::Nonstuttering { sub: sub.expr },
    };
    let single = PredicateAtom {
        pred: pred(&model, "Single"),
        kind: AtomKind::State,
        test: AtomTest::Predicate,
    };
    let step = St::action(before.clone(), after);
    assert!(atom.evaluate(&model, &step).expect("subscript changes"));
    assert!(single.evaluate(&model, &step).expect("state atom"));
    let stutter = St::action(before.clone(), before);
    assert!(
        !atom
            .evaluate(&model, &stutter)
            .expect("subscript unchanged")
    );
}

#[test]
fn selectseq_enabled_negation_and_fairness_preserve_original_atom_semantics() {
    let model = bound(
        "---- MODULE SelectEnabled ----\nEXTENDS Sequences\nVARIABLE x\nInit == x = <<0>>\nNext==x'=x\nPure(S)==SelectSeq(S,LAMBDA y:y=Head(x))\nAction==(x'=x) /\\ (Pure(x) = x)\nE==[] ENABLED Action\nN==[] ~ENABLED Action\nOrdinary==[] ENABLED Next\nFair==WF_x(Action)\nStrong==SF_x(Action)\n====",
        "INIT Init\nNEXT Next\n",
    );
    let st = St::state(Arc::from([Some(Value::tuple(vec![0.into()]))]));
    for name in ["Ordinary", "E", "N", "Fair", "Strong"] {
        let plan =
            lower(&model, &pred(&model, name), Limits::default()).expect("pure ENABLED/fairness");
        if name == "E" || name == "N" {
            assert_eq!(
                plan.atoms[0]
                    .evaluate(&model, &st)
                    .expect("original enabled search"),
                name == "E"
            );
        }
    }
}

#[test]
fn selectseq_manual_prebound_atom_reports_the_callback_evaluation_error() {
    use dive_liveness::plan::{AtomTest, PredicateAtom};
    let model = bound(
        "---- MODULE SelectPrebound ----\nEXTENDS Sequences\nVARIABLE x\nInit==x=0\nNext==x'=x\nRun(S)==SelectSeq(S,LAMBDA y:y[1]=0) = <<>>\n====",
        "INIT Init\nNEXT Next\n",
    );
    let def = model.ir.root_def("Run").expect("run");
    let scope = model
        .evaluator
        .scope_call_values(
            &model.evaluator.scope(def).expect("scope"),
            def,
            vec![Value::tuple(vec![
                0.into(),
                Value::tuple(vec![0.into(); 30_000]),
            ])],
        )
        .expect("manually prebound direct input");
    let atom = PredicateAtom {
        pred: Pred::new(model.ir.def(def).body, scope),
        kind: AtomKind::State,
        test: AtomTest::Predicate,
    };
    // The callback applies the first element, 0, to 1: TLC's evaluation error.
    let error = atom
        .evaluate(&model, &St::state(Arc::from([Some(0.into())])))
        .expect_err("callback evaluation error");
    assert_eq!(error.class(), dive_eval::ErrorClass::Eval, "{error}");
}
