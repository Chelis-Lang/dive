//! Public-context admission happens before evaluation, interning or callbacks.
#![allow(clippy::expect_used, reason = "fixture construction and assertions")]

use dive_eval::{ConstBinding, ContextErrorKind, Evaluator, Model, Options, PrintSink, St};
use dive_ir::{Arg, DefId, ExprId, Pass};
use dive_value::{Ctx, DeferredKind, DeferredSet, Value};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = "---- MODULE Contexts ----\nEXTENDS Naturals, TLC\nCONSTANT C, F(_)\nVARIABLE x\nP(v) == v\nQ(a,b) == a\nOp(H(_),v) == H(v)\nPure(n) == n+1\nRECURSIVE Count(_)\nCount(n) == IF n=0 THEN 0 ELSE Count(n-1)\nInit == x=0\nNext == x'=x\nSide == Print(42, TRUE)\nConstUse == C\nLiteral == \"fresh literal\"\n====\n";
fn ir() -> Arc<dive_ir::Ir> {
    let a = dive_sema::analyze_source("Contexts.tla", SOURCE, &dive_sema::Options::default());
    assert!(a.accepted(), "{:?}", a.diagnostics());
    Arc::new(dive_ir::lower(&a).expect("IR"))
}
fn id(ir: &dive_ir::Ir, name: &str) -> DefId {
    ir.root_def(name).expect(name)
}
#[derive(Default)]
struct Sink(AtomicUsize);
impl PrintSink for Sink {
    fn print(&self, _: String) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn invalid_override_maps_refuse_before_interning_or_printing() {
    let ir = ir();
    let p = id(&ir, "P");
    let q = id(&ir, "Q");
    let pure = id(&ir, "Pure");
    let op = id(&ir, "Op");
    for case in 0..9 {
        let mut model = Model::new(ir.clone());
        let kind = match case {
            0 => {
                model.redirects.insert(DefId(u32::MAX), p);
                ContextErrorKind::UnknownId
            }
            1 => {
                model.redirects.insert(p, DefId(u32::MAX));
                ContextErrorKind::UnknownId
            }
            2 => {
                model.redirects.insert(p, pure);
                model.redirects.insert(pure, p);
                ContextErrorKind::RedirectCycle
            }
            3 => {
                model.redirects.insert(p, q);
                ContextErrorKind::Arity
            }
            4 => {
                model.redirects.insert(op, q);
                ContextErrorKind::Arity
            }
            5 => {
                model.constants[0] = Some(ConstBinding::Def(p));
                ContextErrorKind::Arity
            }
            6 => {
                model.def_values.insert(DefId(u32::MAX), Value::TRUE);
                ContextErrorKind::UnknownId
            }
            7 => {
                model.constants.pop();
                ContextErrorKind::Structure
            }
            _ => {
                model.def_values.insert(p, Value::TRUE);
                ContextErrorKind::Arity
            }
        };
        let names = model.names.clone();
        let before = names.len();
        let sink = Arc::new(Sink::default());
        let error =
            Evaluator::with_sink(model, Options::tlc(), sink.clone()).expect_err("invalid model");
        assert_eq!(error.kind(), kind, "case {case}: {error}");
        assert!(error.code().starts_with("DIVE-EVAL-C"));
        assert_eq!(names.len(), before, "case {case}: no literal interning");
        assert_eq!(sink.0.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn validated_identity_chains_recursion_and_operator_arguments_still_work() {
    let ir = ir();
    let mut model = Model::new(ir.clone());
    let p = id(&ir, "P");
    let pure = id(&ir, "Pure");
    model.redirects.insert(p, p); // A legitimate fixed point, not a cycle.
    let ev = Evaluator::new(model, Options::tlc()).expect("valid identity redirect");
    let scope = ev.scope(p).expect("scope");
    let count = id(&ir, "Count");
    let call = ev
        .scope_call_values(&scope, count, vec![5.into()])
        .expect("recursive call");
    assert_eq!(
        ev.eval_scoped(&call, ir.def(count).body, &St::constant())
            .expect("recursive value"),
        0.into()
    );
    let op = id(&ir, "Op");
    let body = ir.def(id(&ir, "Init")).body;
    // Extract a numeric literal already in this IR, for the value parameter.
    let number = ir
        .nodes()
        .iter()
        .enumerate()
        .find_map(|(i, n)| {
            matches!(&n.expr, dive_ir::Expr::Num(s) if s.as_ref()=="0")
                .then_some(ExprId(u32::try_from(i).expect("small IR")))
        })
        .expect("literal");
    let applied = ev
        .scope_apply(
            &scope,
            op,
            &[
                Arg::Op {
                    op: dive_ir::OpRef::Def(pure),
                    level: dive_ir::Level::Constant,
                },
                Arg::Expr {
                    expr: number,
                    pass: Pass::Lazy,
                },
            ],
        )
        .expect("operator argument");
    assert_eq!(
        ev.eval_scoped(&applied, ir.def(op).body, &St::constant())
            .expect("higher order"),
        1.into()
    );
    assert!(ev.eval_scoped(&scope, body, &St::constant()).is_err()); // No state is invented.
}

#[test]
fn malformed_direct_lazy_and_operator_arguments_refuse_before_effects() {
    let ir = ir();
    let ev = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("valid");
    let side = id(&ir, "Side");
    let scope = ev.scope(side).expect("scope");
    let missing = ExprId(u32::MAX);
    assert!(
        ev.scope(DefId(u32::MAX))
            .expect_err("unknown def")
            .message()
            .contains("C001")
    );
    assert!(
        ev.eval_in(side, missing, &St::constant())
            .expect_err("unknown expr")
            .message()
            .contains("C001")
    );
    assert!(ev.eval_scoped(&scope, missing, &St::constant()).is_err());
    for pass in [Pass::Strict, Pass::Lazy, Pass::Direct] {
        let args = [
            Arg::Expr {
                expr: ir.def(side).body,
                pass: Pass::Strict,
            },
            Arg::Expr {
                expr: missing,
                pass,
            },
        ];
        assert!(
            ev.scope_apply(&scope, id(&ir, "Q"), &args)
                .expect_err("invalid final arg")
                .message()
                .contains("C001")
        );
    }
    let args = [
        Arg::Op {
            op: dive_ir::OpRef::Def(DefId(u32::MAX)),
            level: dive_ir::Level::Constant,
        },
        Arg::Expr {
            expr: ir.def(side).body,
            pass: Pass::Lazy,
        },
    ];
    assert!(
        ev.scope_apply(&scope, id(&ir, "Op"), &args)
            .expect_err("unknown operator")
            .message()
            .contains("C001")
    );
    assert!(
        ev.scope_apply(&scope, id(&ir, "Q"), &[])
            .expect_err("bad arity")
            .message()
            .contains("C002")
    );
    assert!(
        ev.scope_bind(&scope, dive_ir::Slot(u32::MAX), Value::TRUE)
            .is_err()
    );
    assert!(matches!(
        ev.check_assumption(DefId(u32::MAX)),
        dive_eval::AssumptionOutcome::Error { .. }
    ));
    assert_eq!(ev.take_output(), Vec::<String>::new());
}

#[test]
fn equal_number_foreign_scopes_cannot_evaluate_or_bind_values() {
    let ir = ir();
    let foreign_ir = Arc::new((*ir).clone());
    let ev = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("valid");
    let foreign = Evaluator::new(Model::new(foreign_ir), Options::tlc()).expect("valid foreign");
    let side = id(&ir, "Side");
    let scope = foreign.scope(side).expect("foreign scope");
    let error = ev
        .eval_scoped(&scope, ir.def(side).body, &St::constant())
        .expect_err("foreign scope");
    assert!(error.message().contains("C004"));
    assert!(ev.scope_apply(&scope, side, &[]).is_err());
    assert!(
        ev.scope_call_values(&scope, id(&ir, "P"), vec![Value::TRUE])
            .is_err()
    );
    assert_eq!(ev.take_output(), Vec::<String>::new());
    assert_eq!(foreign.take_output(), Vec::<String>::new());
}

#[derive(Debug)]
struct Opaque(Arc<AtomicUsize>);
impl DeferredSet for Opaque {
    fn kind(&self) -> DeferredKind {
        self.0.fetch_add(1, Ordering::SeqCst);
        DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(false))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        vec![]
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(self)
    }
}
#[test]
fn opaque_value_admission_calls_no_deferred_callbacks() {
    for nested in [false, true] {
        let mut model = Model::new(ir());
        let calls = Arc::new(AtomicUsize::new(0));
        let value = Value::deferred(Arc::new(Opaque(calls.clone())), Ctx::tlc());
        let value = if nested {
            Value::tuple(vec![value])
        } else {
            value
        };
        let baseline = calls.load(Ordering::SeqCst);
        if !nested {
            assert_eq!(baseline, 0);
        }
        model.constants[0] = Some(ConstBinding::Value(value));
        let error = Evaluator::new(model, Options::tlc()).expect_err("opaque context");
        assert_eq!(error.kind(), ContextErrorKind::OpaqueValue);
        assert_eq!(calls.load(Ordering::SeqCst), baseline);
    }
}

#[test]
fn foreign_native_deferred_captures_are_rejected_without_forcing() {
    let src = "---- MODULE Captured ----\nEXTENDS Naturals\nCONSTANT C\nLazy == { n \\in Nat : n=0 }\n====\n";
    let a = dive_sema::analyze_source("Captured.tla", src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("lower"));
    let foreign =
        Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("foreign evaluator");
    let value = foreign
        .eval_def(id(&ir, "Lazy"), &St::constant())
        .expect("native filter");
    let mut model = Model::new(Arc::new((*ir).clone()));
    model.constants[0] = Some(ConstBinding::Value(value));
    assert_eq!(
        Evaluator::new(model, Options::tlc())
            .expect_err("foreign captured IR")
            .kind(),
        ContextErrorKind::ForeignContext
    );
    assert_eq!(foreign.take_output(), Vec::<String>::new());
}

#[test]
fn captured_atoms_cannot_cross_name_tables_but_symbolic_scopes_can() {
    let ir = ir();
    let first = Model::new(ir.clone());
    first.names.intern("extra before literals");
    let first_names = first.names.clone();
    let first = Evaluator::new(first, Options::tlc()).expect("first");
    let second = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("second");
    let p = id(&ir, "P");
    let scope = first.scope(p).expect("symbolic");
    second
        .validate_scope(&scope)
        .expect("no foreign values captured");
    let atom = Value::string(&first_names, "foreign captured atom");
    let captured = first
        .scope_call_values(&scope, p, vec![atom])
        .expect("capture");
    let err = second
        .eval_scoped(&captured, ir.def(p).body, &St::constant())
        .expect_err("foreign token");
    assert!(err.message().contains("C004"));
    assert_eq!(second.take_output(), Vec::<String>::new());
}

#[test]
fn malformed_serialized_ir_refuses_before_literal_interning() {
    let ir = ir();
    for case in 0..3 {
        let mut json = serde_json::to_value(ir.as_ref()).expect("serialize IR");
        match case {
            0 => json["root"] = serde_json::json!(u32::MAX),
            1 => json["defs"][id(&ir, "P").index()]["body"] = serde_json::json!(u32::MAX),
            _ => json["tlc_vars"] = serde_json::json!([u32::MAX]),
        }
        let ir: dive_ir::Ir = serde_json::from_value(json).expect("malformed but deserializable");
        let model = Model::new(Arc::new(ir));
        let names = model.names.clone();
        let before = names.len();
        let error =
            Evaluator::new(model, Options::tlc()).expect_err(&format!("invalid arena case {case}"));
        assert!(matches!(
            error.kind(),
            ContextErrorKind::UnknownId | ContextErrorKind::Structure
        ));
        assert_eq!(names.len(), before);
    }
}

#[test]
fn cached_lazy_atoms_cannot_cross_name_tables() {
    let ir = ir();
    let model = Model::new(ir.clone());
    model.names.intern("extra before literals");
    let first = Evaluator::new(model, Options::tlc()).expect("first");
    let second = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("second");
    let p = id(&ir, "P");
    let literal = ir.def(id(&ir, "Literal")).body;
    let captured = first
        .scope_apply(
            &first.scope(p).expect("scope"),
            p,
            &[Arg::Expr {
                expr: literal,
                pass: Pass::Lazy,
            }],
        )
        .expect("lazy");
    first
        .eval_scoped(&captured, ir.def(p).body, &St::constant())
        .expect("populate lazy cache");
    let err = second
        .eval_scoped(&captured, ir.def(p).body, &St::constant())
        .expect_err("cached foreign token");
    assert!(err.message().contains("C004"));
}

#[test]
fn admission_budget_refuses_without_evaluating_large_values() {
    let mut model = Model::new(ir());
    model.constants[0] = Some(ConstBinding::Value(Value::tuple(vec![Value::TRUE; 65_537])));
    let names = model.names.clone();
    let before = names.len();
    assert_eq!(
        Evaluator::new(model, Options::tlc())
            .expect_err("bounded admission")
            .kind(),
        ContextErrorKind::ResourceLimit
    );
    assert_eq!(names.len(), before);
}

#[test]
fn cached_lazy_bindings_cannot_cross_evaluator_cores_with_equal_ir_and_names() {
    let ir = ir();
    let mut model = Model::new(ir.clone());
    model.constants[0] = Some(ConstBinding::Value(0.into()));
    let names = model.names.clone();
    let first = Evaluator::new(model, Options::tlc()).expect("first");
    let mut model = Model::new(ir.clone());
    model.names = names;
    model.constants[0] = Some(ConstBinding::Value(1.into()));
    let second = Evaluator::new(model, Options::tlc()).expect("different constants");
    let p = id(&ir, "P");
    let argument = ir.def(id(&ir, "ConstUse")).body;
    let captured = first
        .scope_apply(
            &first.scope(p).expect("scope"),
            p,
            &[Arg::Expr {
                expr: argument,
                pass: Pass::Lazy,
            }],
        )
        .expect("capture");
    assert_eq!(
        first
            .eval_scoped(&captured, ir.def(p).body, &St::constant())
            .expect("populate cache"),
        0.into()
    );
    let err = second
        .eval_scoped(&captured, ir.def(p).body, &St::constant())
        .expect_err("stale lazy cache");
    assert!(err.message().contains("C004"));
}
