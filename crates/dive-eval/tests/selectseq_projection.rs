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

#[test]
fn overridden_definition_sources_refuse_without_provider_calls() {
    let ir = ir(
        "---- MODULE Projection ----\nEXTENDS Sequences, FiniteSets\nD=={0}\nAlias=={0}\nQ==SelectSeq(<<0>>,LAMBDA v: Cardinality(D)=1)\nRedirected==SelectSeq(<<0>>,LAMBDA v: Cardinality(Alias)=1)\n====\n",
    );
    let target = ir.root_def("D").expect("D");
    let alias = ir.root_def("Alias").expect("Alias");
    for options in [Options::tlc(), Options::dive()] {
        for (name, redirected) in [("Q", false), ("Redirected", true)] {
            for warm in [false, true] {
                let provider = Arc::new(Opaque(AtomicUsize::new(0)));
                let mut input = Model::new(ir.clone());
                input
                    .def_values
                    .insert(target, Value::deferred(provider.clone(), options.ctx));
                if redirected {
                    // A redirected use executes D's body, ignoring D's value.
                    // Its source must not disappear merely because D is overridden.
                    input.redirects.insert(alias, target);
                }
                let evaluator = Evaluator::new(input, options).expect("evaluator");
                let def = ir.root_def(name).expect("selection");
                let scope = evaluator.scope(def).expect("scope");
                if warm {
                    evaluator
                        .eval_scoped(&scope, ir.def(def).body, &St::constant())
                        .expect("ordinary cache warmup");
                }
                let Expr::Apply { callee, args } = ir.expr(ir.def(def).body) else {
                    panic!("selection application")
                };
                let calls = provider.0.load(Ordering::SeqCst);
                let result = evaluator.scope_selectseq_call(&scope, callee, args, &mut 100_000);
                assert_eq!(
                    provider.0.load(Ordering::SeqCst),
                    calls,
                    "admission must not inspect an opaque provider"
                );
                assert_eq!(
                    result.as_ref().err().map(dive_eval::EvalError::class),
                    Some(dive_eval::ErrorClass::Unsupported),
                    "{name}/warm={warm}: overridden source must refuse"
                );
            }
        }
    }
}

#[test]
fn configured_definition_values_keep_body_evaluation_authority() {
    let ir = ir(
        "---- MODULE Projection ----\nEXTENDS Sequences, FiniteSets\nCONSTANT C\nD=={0}\nQ==SelectSeq(<<0>>,LAMBDA v: Cardinality(C)=1)\n====\n",
    );
    let target = ir.root_def("D").expect("D");
    for options in [Options::tlc(), Options::dive()] {
        for warm in [false, true] {
            let provider = Arc::new(Opaque(AtomicUsize::new(0)));
            let mut input = Model::new(ir.clone());
            input.constants[0] = Some(ConstBinding::Def(target));
            input
                .def_values
                .insert(target, Value::deferred(provider.clone(), options.ctx));
            let evaluator = Evaluator::new(input, options).expect("evaluator");
            let def = ir.root_def("Q").expect("Q");
            let scope = evaluator.scope(def).expect("scope");
            let before_warmup = provider.0.load(Ordering::SeqCst);
            if warm {
                // C <- D evaluates D's body, never its stored definition value.
                evaluator
                    .eval_scoped(&scope, ir.def(def).body, &St::constant())
                    .expect("configured definition body");
            }
            assert_eq!(
                provider.0.load(Ordering::SeqCst),
                before_warmup,
                "configured target body must ignore its stored override"
            );
            let Expr::Apply { callee, args } = ir.expr(ir.def(def).body) else {
                panic!("selection application")
            };
            let calls = provider.0.load(Ordering::SeqCst);
            assert!(
                evaluator
                    .scope_selectseq_call(&scope, callee, args, &mut 100_000)
                    .expect("body is pure")
                    .is_some()
            );
            assert_eq!(provider.0.load(Ordering::SeqCst), calls);
        }
    }
}

#[test]
fn redirected_overrides_cannot_hide_impure_target_bodies() {
    let ir = ir(
        "---- MODULE Projection ----\nEXTENDS Sequences, TLC\nD==PrintT(0)\nAlias==TRUE\nQ==SelectSeq(<<0>>,LAMBDA v: Alias)\n====\n",
    );
    for options in [Options::tlc(), Options::dive()] {
        let mut input = Model::new(ir.clone());
        let target = ir.root_def("D").expect("D");
        input
            .redirects
            .insert(ir.root_def("Alias").expect("Alias"), target);
        input.def_values.insert(target, Value::Bool(true));
        let evaluator = Evaluator::new(input, options).expect("evaluator");
        let def = ir.root_def("Q").expect("Q");
        let Expr::Apply { callee, args } = ir.expr(ir.def(def).body) else {
            panic!("selection application")
        };
        let result = evaluator.scope_selectseq_call(
            &evaluator.scope(def).expect("scope"),
            callee,
            args,
            &mut 100_000,
        );
        assert_eq!(
            result.as_ref().err().map(dive_eval::EvalError::class),
            Some(dive_eval::ErrorClass::Unsupported)
        );
        assert_eq!(evaluator.take_output(), Vec::<String>::new());
    }
}
