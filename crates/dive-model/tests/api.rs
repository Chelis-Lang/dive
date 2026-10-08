//! The binding stage's API, on small specifications.

#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use dive_eval::St;
use dive_model::{BindOptions, ErrorKind, Model, bind};
use dive_value::Value;

fn model_with(src: &str, cfg: &str, opts: &BindOptions, dump_trace: bool) -> Model {
    let mut o = dive_sema::Options::default();
    if dump_trace {
        o.extra_root_extendees = dive_model::DUMP_TRACE_JSON_MODULES
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
    }
    let analysis = dive_sema::analyze_source("M.tla", src, &o);
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg");
    bind(ir, &cfg, opts).unwrap_or_else(|f| panic!("{f}"))
}

fn model(src: &str, cfg: &str) -> Model {
    model_with(src, cfg, &BindOptions::tlc(), false)
}

fn state(vals: Vec<Value>) -> St {
    St::state(vals.into_iter().map(Some).collect::<Vec<_>>().into())
}

#[test]
fn constant_substitutions_are_evaluated_at_every_use() {
    // TLC evaluates `D` at start-up for each module that declares `K`
    // (here one), and again at every use of `K`: `K`'s tool object is `D`'s
    // node, not its value (`SpecProcessor.java:276-316`).
    let m = model(
        "---- MODULE M ----\nEXTENDS TLC\nCONSTANT K\nVARIABLE x\nD == Print(\"D\", 7)\n\
         Init == x = 0\nNext == x' = x\nInv == x = 0 /\\ K = 7\n====\n",
        "CONSTANT K <- D\nINIT Init\nNEXT Next\nINVARIANT Inv\n",
    );
    assert_eq!(
        m.output,
        [dive_model::Message::Print("\"D\"  7".to_owned())]
    );
    let inv = &m.invariants[0];
    assert_eq!(inv.name, "Inv");
    let v = m
        .eval(&inv.pred, &state(vec![Value::from(0)]))
        .expect("evaluates");
    assert_eq!(v, Value::Bool(true));
    assert_eq!(m.take_output(), ["\"D\"  7"]);
    // A use of `D` itself takes its start-up value.
    let m2 = model(
        "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nD == Print(\"D\", 7)\n\
         Init == x = 0\nNext == x' = x\nInv == x = 0 /\\ D = 7\n====\n",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    );
    let _ = m2.take_output();
    m2.eval(&m2.invariants[0].pred, &state(vec![Value::from(0)]))
        .expect("evaluates");
    assert_eq!(m2.take_output(), Vec::<String>::new());
}

#[test]
fn actions_carry_their_bindings_and_labels() {
    let m = model(
        "---- MODULE M ----\nEXTENDS Naturals\nCONSTANT Procs\nVARIABLE pc\n\
         Init == pc = [p \\in Procs |-> 0]\n\
         Step(p) == pc' = [pc EXCEPT ![p] = 1 - @]\n\
         Next == \\E p \\in Procs : Step(p)\n====\n",
        "CONSTANT Procs = {b, a}\nINIT Init\nNEXT Next\n",
    );
    let labels: Vec<&str> = m
        .actions
        .iter()
        .map(dive_eval::next::Action::label)
        .collect();
    // Model values order by intern token, and `b` is read first.
    assert_eq!(
        labels,
        [
            "<Step(b) line 6, col 12 to line 6, col 41 of module M>",
            "<Step(a) line 6, col 12 to line 6, col 41 of module M>",
        ]
    );
    // The action's scope binds `p`: evaluate it between two states.
    let names = m.names.clone();
    let f = |b: i64, a: i64| {
        Value::fcn(
            vec![
                (Value::model(&names, "b"), Value::from(b)),
                (Value::model(&names, "a"), Value::from(a)),
            ],
            dive_value::Ctx::tlc(),
        )
        .expect("fcn")
    };
    let st = St::action(vec![Some(f(0, 0))].into(), vec![Some(f(1, 0))].into());
    assert_eq!(
        m.evaluator.eval_action(&m.actions[0], &st).expect("eval"),
        Value::Bool(true)
    );
    assert_eq!(
        m.evaluator.eval_action(&m.actions[1], &st).expect("eval"),
        Value::Bool(false)
    );
}

#[test]
fn properties_split_as_tlc_splits_them() {
    let m = model(
        "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
         Next == x' = (x + 1) % 3\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\n\
         TypeOK == x \\in 0..2\n\
         Prop == x = 0 /\\ []TypeOK /\\ [][x' # x]_x /\\ []<>(x = 0)\n====\n",
        "SPECIFICATION Spec\nPROPERTY Prop\n",
    );
    let names = |v: &[dive_model::Named]| v.iter().map(|n| n.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&m.invariants), ["TypeOK"]);
    assert_eq!(
        names(&m.implied_inits),
        ["line 8, col 9 to line 8, col 13 of module M"]
    );
    assert_eq!(
        names(&m.implied_actions),
        ["line 8, col 30 to line 8, col 41 of module M"]
    );
    assert_eq!(m.implied_temporals.len(), 1);
    assert_eq!(m.temporals.len(), 1, "WF_x(Next) is fairness");
    assert_eq!(m.init.len(), 1);
    assert!(m.next.is_some());
    // Liveness is refused by name.
    assert!(
        m.unsupported
            .iter()
            .any(|u| u.feature.starts_with("temporal properties"))
    );
}

#[test]
fn configuration_errors_carry_tlc_codes() {
    let src =
        "---- MODULE M ----\nCONSTANTS A, B\nVARIABLE x\nInit == x = A\nNext == x' = x\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse("CONSTANT B = 1\nINIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    let f = bind(ir, &cfg, &BindOptions::tlc()).expect_err("A is unassigned");
    assert_eq!(f.error.code(), Some(2222));
    assert!(matches!(f.error.kind, ErrorKind::Tlc(_)));
    assert_eq!(
        f.error.message,
        "The constant parameter A is not assigned a value by the configuration file."
    );
}

#[test]
fn both_profiles_bind_the_same_roles() {
    let src = "---- MODULE M ----\nEXTENDS Naturals\nCONSTANT S\nVARIABLE x\n\
               Init == x \\in S\nNext == \\E s \\in S : x' = s\nInv == x \\in S\n====\n";
    let cfg = "CONSTANT S = {\"b\", \"a\"}\nINIT Init\nNEXT Next\nINVARIANT Inv\n";
    let tlc = model_with(src, cfg, &BindOptions::tlc(), false);
    let dive = model_with(src, cfg, &BindOptions::dive(), false);
    assert_eq!(tlc.invariants.len(), dive.invariants.len());
    assert_eq!(tlc.actions.len(), 2);
    // TLC's order splits `"b"` first; the default profile explores in the
    // canonical order (D-010).
    assert!(tlc.actions[0].label().starts_with("<Next line 6"));
    assert_eq!(dive.actions.len(), 2);
}

#[test]
fn dump_trace_modules_change_the_active_overrides() {
    let src = "---- MODULE M ----\nEXTENDS Naturals\nASSUME 1 + 1 = 2\n====\n";
    let plain = model_with(src, "", &BindOptions::tlc(), false);
    let traced = model_with(src, "", &BindOptions::tlc(), true);
    let plus = |m: &Model| {
        m.ir.defs()
            .iter()
            .find(|d| d.name == "+" && m.ir.module(d.module).name == "Naturals")
            .and_then(|d| d.std)
            .map(dive_ir::StdOp::java)
    };
    assert_eq!(plus(&plain), Some("tlc2.module.Naturals.Plus"));
    assert_eq!(plus(&traced), Some("tlc2.module.Integers.Plus"));
}

#[test]
fn model_values_get_tlc_tokens() {
    // TLC reads the configuration before it parses: its values are the first
    // names in the table, from token 1.
    let m = model(
        "---- MODULE M ----\nCONSTANT S\nVARIABLE x\nInit == x \\in S\nNext == x' = x\n====\n",
        "CONSTANT S = {m2, \"s\", m1}\nINIT Init\nNEXT Next\n",
    );
    let tok = |n: &str| m.names.get(n).map(|a| a.tok());
    assert_eq!(tok("m2"), Some(1));
    assert_eq!(tok("s"), Some(2));
    assert_eq!(tok("m1"), Some(3));
    assert_eq!(tok("STRING"), Some(4));
}
