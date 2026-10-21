//! Source admission never invokes a callback, domain, or opaque provider.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_eval::{ConstBinding, Evaluator, Model, Options, St};
use dive_ir::Expr;
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Opaque(AtomicUsize);
impl dive_value::DeferredSet for Opaque {
    fn kind(&self) -> dive_value::DeferredKind {
        self.0.fetch_add(1, Ordering::SeqCst);
        dive_value::DeferredKind::Map
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, Ordering::SeqCst);
        None
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Value::Int(0)])
    }
    fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(true))
    }
}
fn ir(source: &str) -> Arc<dive_ir::Ir> {
    let analysis =
        dive_sema::analyze_source("Projection.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Arc::new(dive_ir::lower(&analysis).expect("IR"))
}

#[test]
fn opaque_providers_remain_refused_without_methods_even_with_warm_caches() {
    let ir = ir(
        "---- MODULE Projection ----\nEXTENDS Sequences\nCONSTANT C\nQ==SelectSeq(<<0>>,LAMBDA v: C=C \\/ v=0)\nWarm==Q\n====\n",
    );
    for options in [Options::tlc(), Options::dive()] {
        for warm in [false, true] {
            let provider = Arc::new(Opaque(AtomicUsize::new(0)));
            let mut input = Model::new(ir.clone());
            input.constants[0] = Some(ConstBinding::Value(Value::deferred(
                provider.clone(),
                options.ctx,
            )));
            let evaluator = Evaluator::new(input, options).expect("evaluator");
            if warm {
                let def = ir.root_def("Warm").expect("Warm");
                evaluator
                    .eval_scoped(
                        &evaluator.scope(def).expect("scope"),
                        ir.def(def).body,
                        &St::constant(),
                    )
                    .expect("warm ordinary result cache");
            }
            let def = ir.root_def("Q").expect("Q");
            let Expr::Apply { callee, args } = ir.expr(ir.def(def).body) else {
                panic!("SelectSeq application")
            };
            let calls = provider.0.load(Ordering::SeqCst);
            let mut malformed = args.to_vec();
            malformed[0] = dive_ir::Arg::Expr {
                expr: dive_ir::ExprId(u32::MAX),
                pass: dive_ir::Pass::Lazy,
            };
            assert!(
                evaluator
                    .scope_selectseq_call(
                        &evaluator.scope(def).expect("scope"),
                        callee,
                        &malformed,
                        &mut 100_000
                    )
                    .is_err()
            );
            assert_eq!(provider.0.load(Ordering::SeqCst), calls);
            let error = evaluator
                .scope_selectseq_call(
                    &evaluator.scope(def).expect("scope"),
                    callee,
                    args,
                    &mut 100_000,
                )
                .expect_err("opaque storage remains refused");
            assert_eq!(error.class(), dive_eval::ErrorClass::Unsupported);
            assert!(error.message().contains("opaque/deferred"));
            assert_eq!(
                provider.0.load(Ordering::SeqCst),
                calls,
                "no provider methods during admission"
            );
        }
    }
}

#[test]
fn huge_binders_and_nested_selection_use_metadata_and_one_shared_budget() {
    let ir = ir(
        "---- MODULE Projection ----\nEXTENDS Naturals, Sequences\nQ==SelectSeq(<<0>>,LAMBDA v: \\E z \\in 1..1000000000: z=v)\nNested==SelectSeq(<<0>>,LAMBDA v: Len(SelectSeq(<<v>>,LAMBDA w: w=0))=1)\nDependent==SelectSeq(<<0>>,LAMBDA v: \\A w \\in {v}: Len(SelectSeq(<<w>>,LAMBDA z: z=w))=1)\n====\n",
    );
    for options in [Options::tlc(), Options::dive()] {
        let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
        for name in ["Q", "Nested", "Dependent"] {
            let def = ir.root_def(name).expect("definition");
            let Expr::Apply { callee, args } = ir.expr(ir.def(def).body) else {
                panic!("SelectSeq application")
            };
            let scope = evaluator.scope(def).expect("scope");
            let mut remaining = 100_000;
            assert!(
                evaluator
                    .scope_selectseq_call(&scope, callee, args, &mut remaining)
                    .expect("pure metadata")
                    .is_some()
            );
            assert!(remaining < 100_000);
            let required = 100_000 - remaining;
            assert!(
                evaluator
                    .scope_selectseq_call(&scope, callee, args, &mut required.saturating_sub(1))
                    .is_err(),
                "no nested counter reset"
            );
            assert_eq!(evaluator.take_output(), Vec::<String>::new());
        }
    }
}
