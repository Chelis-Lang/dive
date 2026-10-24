//! Result-kind evidence never replaces callback/capture effect authority.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_eval::{ConstBinding, Ec, ErrorClass, Evaluator, Model, Options, St};
use dive_ir::{Arg, DefId, Expr, Level, LocalRef, OpRef, Slot, StdOp};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn lower(definitions: &str) -> Arc<dive_ir::Ir> {
    let text = format!(
        "---- MODULE Projection ----\nEXTENDS Naturals, Sequences, FiniteSets, TLC\n{definitions}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Projection.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Arc::new(dive_ir::lower(&analysis).expect("IR"))
}

fn project(
    evaluator: &Evaluator,
    ir: &dive_ir::Ir,
    name: &str,
    work: &mut usize,
) -> dive_eval::Result<bool> {
    let def = ir.root_def(name).expect("selector");
    let Expr::Apply { callee, args } = ir.expr(ir.def(def).body) else {
        panic!("SelectSeq application")
    };
    evaluator
        .scope_selectseq_call(&evaluator.scope(def)?, callee, args, work)
        .map(|result| result.is_some())
}

fn native_concat(ir: &dive_ir::Ir) -> DefId {
    let index = ir
        .defs()
        .iter()
        .position(|def| def.std == Some(StdOp::SequencesConcat))
        .expect("canonical Sequences Concat");
    DefId(u32::try_from(index).expect("definition index"))
}

fn concat_route(ir: &dive_ir::Ir, direct: bool) -> Arc<dive_ir::Ir> {
    let mut encoded = serde_json::to_value(ir).expect("serialized IR");
    let replacement = if direct {
        OpRef::Std(StdOp::SequencesConcat)
    } else {
        OpRef::Def(native_concat(ir))
    };
    for (index, node) in ir.nodes().iter().enumerate() {
        let Expr::Apply { callee, .. } = &node.expr else {
            continue;
        };
        let concat = match callee {
            OpRef::Std(op) => *op == StdOp::SequencesConcat,
            OpRef::Def(def) => ir.def(*def).std == Some(StdOp::SequencesConcat),
            _ => false,
        };
        if concat {
            encoded["exprs"][index]["expr"]["Apply"]["callee"] =
                serde_json::to_value(&replacement).expect("operator reference");
        }
    }
    Arc::new(serde_json::from_value(encoded).expect("same IR with explicit native route"))
}

#[test]
fn constructor_families_and_owned_alias_joins_prove_nonstring_without_evaluation() {
    for left in [
        "TRUE",
        "0",
        "<<v>>",
        "{v}",
        "[a |-> v]",
        "[i \\in {0} |-> v]",
        "[{0} -> {v}]",
        "[a: {v}]",
        "{i : i \\in {v}}",
        "{i \\in {v} : i=v}",
        "Identity(<<v>>)",
        "LET F(a)==<<a>> IN F(v)",
        "IF v=0 THEN <<>> ELSE {v}",
        "CASE v=0 -> 1 [] OTHER -> [a |-> v]",
        "[i \\in 1..1000000000 |-> i]",
    ] {
        let ir = lower(&format!(
            "Identity(value)==value\nQ==SelectSeq(<<0>>,LAMBDA v: Len(({left}) \\o v)=0)"
        ));
        for options in [Options::tlc(), Options::dive()] {
            let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
            assert!(
                project(&evaluator, &ir, "Q", &mut 100_000).expect("metadata-only type fact"),
                "{left}"
            );
            assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
        }
    }
}

#[test]
fn stored_owned_tags_do_not_infer_types_from_names_or_state_invariants() {
    let ir = lower("CONSTANT C\nQ==SelectSeq(<<0>>,LAMBDA v: Len(C \\o v)=0)");
    for options in [Options::tlc(), Options::dive()] {
        for kind in 0..5 {
            let mut input = Model::new(ir.clone());
            let value = match kind {
                0 => Value::Int(1),
                1 => Value::Bool(true),
                2 => Value::model(&input.names, "owned"),
                3 => Value::tuple(vec![Value::Int(0)]),
                _ => Value::set(vec![Value::Int(0)], options.ctx).expect("stored set"),
            };
            input.constants[0] = Some(ConstBinding::Value(value));
            let evaluator = Evaluator::new(input, options).expect("evaluator");
            assert!(project(&evaluator, &ir, "Q", &mut 100_000).expect("owned outer tag"));
        }
    }
    for left in [
        "v",
        "x",
        "[i \\in {0} |-> <<i>>][0]",
        "IF TRUE THEN <<>> ELSE \"a\"",
    ] {
        let ir = lower(&format!(
            "VARIABLE x\nTypeOK==x \\in {{0}}\nQ==SelectSeq(<<0>>,LAMBDA v: Len(({left}) \\o v)=0)"
        ));
        for options in [Options::tlc(), Options::dive()] {
            let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
            let error = project(&evaluator, &ir, "Q", &mut 100_000).expect_err("unknown type");
            assert_eq!(error.class(), ErrorClass::Unsupported, "{left}");
        }
    }
}

#[test]
fn string_and_unselected_string_arms_stay_refused_after_successful_cache_warmup() {
    for definition in ["S==\"a\" \\o \"a\"", "S==IF TRUE THEN <<>> ELSE \"a\""] {
        let sequence = if definition.contains("IF") {
            "<< <<0>> >>"
        } else {
            "<<\"a\">>"
        };
        let ir = lower(&format!(
            "{definition}\nQ==SelectSeq({sequence},LAMBDA v: Len(S \\o v)>=0)"
        ));
        for options in [Options::tlc(), Options::dive()] {
            let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
            let def = ir.root_def("Q").expect("Q");
            let scope = evaluator.scope(def).expect("scope");
            for warm in [false, true] {
                if warm {
                    evaluator
                        .eval_scoped(&scope, ir.def(def).body, &St::constant())
                        .expect("ordinary result and thunk caches warm");
                }
                let error = project(&evaluator, &ir, "Q", &mut 100_000)
                    .expect_err("cache/untaken arm is not a type certificate");
                assert_eq!(error.class(), ErrorClass::Unsupported);
                assert!(error.message().contains("effect/provider/native"));
                assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
            }
        }
    }
}

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

#[test]
fn nonstring_left_does_not_hide_provider_right_or_unsafe_unused_captures() {
    for unused in [false, true] {
        let definitions = if unused {
            "CONSTANT C\nOuter(unused)==SelectSeq(<<0>>,LAMBDA v: Len(<<v>> \\o <<v>>)=2)\nQ==Outer(C)"
        } else {
            "CONSTANT C\nOuter(unused)==SelectSeq(<<0>>,LAMBDA v: Len(<<v>> \\o <<C>>)=2)\nQ==Outer(0)"
        };
        let ir = lower(definitions);
        for options in [Options::tlc(), Options::dive()] {
            let provider = Arc::new(Opaque(AtomicUsize::new(0)));
            let mut input = Model::new(ir.clone());
            input.constants[0] = Some(ConstBinding::Value(Value::deferred(
                provider.clone(),
                options.ctx,
            )));
            let evaluator = Evaluator::new(input, options).expect("evaluator");
            let q = ir.root_def("Q").expect("Q");
            let outer = ir.root_def("Outer").expect("Outer");
            let Expr::Apply { args: actuals, .. } = ir.expr(ir.def(q).body) else {
                panic!("Outer call")
            };
            let Expr::Apply { callee, args } = ir.expr(ir.def(outer).body) else {
                panic!("SelectSeq")
            };
            for warm in [false, true] {
                if warm {
                    let calls = provider.0.load(Ordering::SeqCst);
                    evaluator
                        .eval_scoped(
                            &evaluator.scope(q).expect("scope"),
                            ir.def(q).body,
                            &St::constant(),
                        )
                        .expect("ordinary unused/provider shell warmup");
                    assert_eq!(provider.0.load(Ordering::SeqCst), calls);
                }
                let scope = evaluator
                    .scope_apply(&evaluator.scope(q).expect("scope"), outer, actuals)
                    .expect("owned lazy actuals");
                let before = provider.0.load(Ordering::SeqCst);
                let error = evaluator
                    .scope_selectseq_call(&scope, callee, args, &mut 100_000)
                    .expect_err("all retained storage and both operands still checked");
                assert_eq!(error.class(), ErrorClass::Unsupported);
                assert_eq!(provider.0.load(Ordering::SeqCst), before);
                assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
            }
        }
    }
}

#[test]
fn constructor_result_kinds_do_not_authorize_print_or_unused_effect_captures() {
    for predicate in [
        "Len(<<PrintT(v)>> \\o <<v>>)=2",
        "Len(<<v>> \\o <<PrintT(v)>>)=2",
    ] {
        let ir = lower(&format!("Q==SelectSeq(<<0>>,LAMBDA v: {predicate})"));
        for options in [Options::tlc(), Options::dive()] {
            let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
            assert_eq!(
                project(&evaluator, &ir, "Q", &mut 100_000)
                    .expect_err("type is not purity")
                    .class(),
                ErrorClass::Unsupported
            );
            assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
        }
    }
    let ir = lower(
        "Outer(unused)==SelectSeq(<<0>>,LAMBDA v: Len(<<v>> \\o <<v>>)=2)\nQ==Outer(PrintT(0))",
    );
    for options in [Options::tlc(), Options::dive()] {
        let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
        let q = ir.root_def("Q").expect("Q");
        let outer = ir.root_def("Outer").expect("Outer");
        let Expr::Apply { args: actuals, .. } = ir.expr(ir.def(q).body) else {
            panic!("Outer call")
        };
        let Expr::Apply { callee, args } = ir.expr(ir.def(outer).body) else {
            panic!("selector")
        };
        for warm in [false, true] {
            if warm {
                evaluator
                    .eval_scoped(
                        &evaluator.scope(q).expect("scope"),
                        ir.def(q).body,
                        &St::constant(),
                    )
                    .expect("unused actual remains lazy");
            }
            let scope = evaluator
                .scope_apply(&evaluator.scope(q).expect("scope"), outer, actuals)
                .expect("owned capture");
            assert_eq!(
                evaluator
                    .scope_selectseq_call(&scope, callee, args, &mut 100_000)
                    .expect_err("unused captured effect")
                    .class(),
                ErrorClass::Unsupported
            );
            assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
        }
    }
}

#[test]
fn canonical_def_std_routes_reject_redirects_values_and_configured_native_origins() {
    let original = lower(
        "CONSTANT F(_,_)\nReplacement(a,b)==Print(a,b)\nQ==SelectSeq(<<0>>,LAMBDA v: Len(<<v>> \\o <<v>>)=2)",
    );
    for direct in [false, true] {
        let ir = concat_route(&original, direct);
        for options in [Options::tlc(), Options::dive()] {
            for replacement in 0..4 {
                let mut input = Model::new(ir.clone());
                let concat = native_concat(&ir);
                match replacement {
                    1 => {
                        input
                            .redirects
                            .insert(concat, ir.root_def("Replacement").expect("replacement"));
                    }
                    2 => {
                        input.def_values.insert(concat, Value::Bool(true));
                    }
                    3 => {
                        input.constants[0] = Some(ConstBinding::Def(concat));
                    }
                    _ => {}
                }
                let built = Evaluator::new(input, options);
                if replacement == 2 {
                    assert!(
                        built.is_err(),
                        "a parameterized definition cannot be value-bound"
                    );
                    continue;
                }
                let evaluator = built.expect("evaluator");
                let result = project(&evaluator, &ir, "Q", &mut 100_000);
                if replacement == 0 {
                    assert!(result.expect("canonical target"));
                } else {
                    assert_eq!(
                        result.expect_err("unproved native authority").class(),
                        ErrorClass::Unsupported
                    );
                }
                assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
            }
        }
    }
}

#[test]
fn a_stored_definition_value_never_inherits_its_source_constructor_type() {
    let ir = lower("Alias==<<0>>\nUse==Alias\nQ==SelectSeq(<<0>>,LAMBDA v: Len(Alias \\o v)=0)");
    for options in [Options::tlc(), Options::dive()] {
        let mut input = Model::new(ir.clone());
        input.def_values.insert(
            ir.root_def("Alias").expect("Alias"),
            Value::string(&input.names, "changed"),
        );
        let evaluator = Evaluator::new(input, options).expect("evaluator");
        for warm in [false, true] {
            if warm {
                let use_def = ir.root_def("Use").expect("Use");
                // This direct use reads the value replacement, not the tuple body.
                let value = evaluator
                    .eval_scoped(
                        &evaluator.scope(use_def).expect("scope"),
                        ir.def(use_def).body,
                        &St::constant(),
                    )
                    .expect("ordinary stored value use");
                assert!(matches!(value, Value::Str(_)));
            }
            assert_eq!(
                project(&evaluator, &ir, "Q", &mut 100_000)
                    .expect_err("value-shadow authority")
                    .class(),
                ErrorClass::Unsupported
            );
            assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
        }
    }
}

#[test]
fn native_parameter_concat_and_foreign_contexts_remain_outside_the_capability() {
    let ir =
        lower("Outer(F(_,_))==SelectSeq(<<0>>,LAMBDA v: Len(F(<<v>>,<<v>>))=2)\nQ==Outer(Append)");
    for options in [Options::tlc(), Options::dive()] {
        let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
        let foreign = Evaluator::new(Model::new(ir.clone()), options).expect("foreign core");
        let q = ir.root_def("Q").expect("Q");
        let outer = ir.root_def("Outer").expect("Outer");
        let args = [Arg::Op {
            op: OpRef::Std(StdOp::SequencesConcat),
            level: Level::Constant,
        }];
        let scope = evaluator
            .scope_apply(&evaluator.scope(q).expect("scope"), outer, &args)
            .expect("owned operator actual");
        assert_eq!(
            evaluator
                .scope_native_operator_analysis(
                    &scope,
                    &OpRef::Param(LocalRef {
                        up: 0,
                        slot: Slot(0)
                    }),
                    &mut 100_000
                )
                .expect("metadata"),
            None
        );
        let Expr::Apply { callee, args } = ir.expr(ir.def(outer).body) else {
            panic!("selector")
        };
        assert_eq!(
            evaluator
                .scope_selectseq_call(&scope, callee, args, &mut 100_000)
                .expect_err("native captured Concat remains refused")
                .class(),
            ErrorClass::Unsupported
        );
        assert!(
            foreign
                .scope_selectseq_call(&scope, callee, args, &mut 100_000)
                .is_err()
        );
        assert!(evaluator.take_output().is_empty() && foreign.take_output().is_empty());
    }
}

#[test]
fn discharged_own_name_gate_preserves_inner_refusal_origin_and_shared_budget() {
    let ir = lower(
        "BadRight==Nat\nQ==SelectSeq(<<0>>,LAMBDA v: Len(<<v>> \\o BadRight)=0)\nNested==SelectSeq(<<0>>,LAMBDA v: Len(SelectSeq(<<v>>,LAMBDA w: Len(<<w>> \\o <<w>>)=2))=1)",
    );
    for options in [Options::tlc(), Options::dive()] {
        let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
        let error =
            project(&evaluator, &ir, "Q", &mut 100_000).expect_err("right operand still walked");
        assert_eq!(
            error.origin(),
            Some(ir.def(ir.root_def("BadRight").expect("BadRight")).body)
        );
        assert_eq!(error.class(), ErrorClass::Unsupported);
        let mut remaining = 100_000;
        assert!(project(&evaluator, &ir, "Nested", &mut remaining).expect("nested metadata"));
        let required = 100_000 - remaining;
        assert!(required > 0);
        assert!(project(&evaluator, &ir, "Nested", &mut required.saturating_sub(1)).is_err());
        assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
    }
}

#[test]
fn admitted_type_error_operands_keep_both_eager_actuals_and_original_kernel_codes() {
    for (predicate, expected, message) in [
        (
            "Len(TRUE \\o <<v>>)=0",
            Ec::ModuleEvaluating,
            "Evaluating an expression of the form s \\o t when s is not a sequence:\nTRUE",
        ),
        (
            "Len(<<v>> \\o TRUE)=0",
            Ec::ModuleEvaluating,
            "Evaluating an expression of the form t \\o s when s is not a sequence:\nTRUE",
        ),
        (
            "Len(TRUE \\o <<1 \\div (v-v)>>)=0",
            Ec::DivisionByZero,
            "The second argument of \\div is 0.",
        ),
    ] {
        let ir = lower(&format!("Q==SelectSeq(<<0>>,LAMBDA v: {predicate})"));
        for options in [Options::tlc(), Options::dive()] {
            let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
            assert!(
                project(&evaluator, &ir, "Q", &mut 100_000)
                    .expect("type fact, not a value-validity claim")
            );
            let def = ir.root_def("Q").expect("Q");
            let error = evaluator
                .eval_scoped(
                    &evaluator.scope(def).expect("scope"),
                    ir.def(def).body,
                    &St::constant(),
                )
                .expect_err("real runtime error");
            assert_eq!(error.ec(), expected);
            assert_eq!(error.message(), message);
            assert!(error.origin().is_some());
            assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
        }
    }
}
