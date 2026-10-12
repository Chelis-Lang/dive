//! Independent predicate admission must not execute caller-annotated Strict substitutions.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_eval::Scope;
use dive_ir::{Arg, Expr, OpRef, Pass, StdOp};
use dive_model::{BindOptions, Model, bind};
use dive_trace::{Ending, TraceDoc};
use serde_json::json;
use std::sync::Arc;
fn model(definitions: &str, operand: &str, extra: &str) -> Model {
    let source = format!(
        "---- MODULE ClosedTuple ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLES x,s\n{definitions}\nInit == x=0 /\\ s = <<0,1>>\nNext == x'=1 /\\ s'={operand}\nInv == x=0\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("ClosedTuple.tla", &source, &dive_sema::Options::default());
    assert!(
        analysis.accepted(),
        "syntax={:?}; semantic={:?}",
        analysis.syntax_diagnostics(),
        analysis.diagnostics()
    );
    bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &dive_config::parse(&format!("INIT Init\nNEXT Next\nINVARIANT Inv\n{extra}"))
            .into_config()
            .expect("cfg"),
        &BindOptions::tlc(),
    )
    .expect("binding")
}

fn names(model: &Model) -> Vec<(String, u32)> {
    (0..model.names.len())
        .map(|index| {
            let atom = model
                .names
                .resolve(u32::try_from(index).expect("token"))
                .expect("name");
            (atom.as_str().to_owned(), atom.tok())
        })
        .collect()
}

fn tail_call(model: &Model) -> (Vec<Arg>, Scope) {
    let next = model.ir.root_def("Next").expect("Next");
    let scope = model.evaluator.scope(next).expect("actual Next frame");
    let mut pending = vec![model.ir.def(next).body];
    while let Some(expr) = pending.pop() {
        let node = model.ir.expr(expr);
        if let Expr::Apply { callee, args } = node {
            let op = match callee {
                OpRef::Std(op) => Some(*op),
                OpRef::Def(id) => model.ir.def(*id).std,
                _ => None,
            };
            if op == Some(StdOp::SequencesTail) {
                return (args.to_vec(), scope);
            }
        }
        pending.extend(node.children());
    }
    panic!("actual Next Tail site");
}

#[allow(
    clippy::too_many_lines,
    clippy::items_after_statements,
    reason = "keep the serialized caller-IR construction beside its structural admission controls"
)]
fn context_model(pass: Pass, printing: bool) -> Model {
    let mut model = model(
        if printing {
            "CONSTANT C\nNoise(v) == PrintT(v)\nUnused == IF x=0 THEN Noise(\"\") ELSE TRUE"
        } else {
            "CONSTANT C\nNoise(v) == v\nUnused == IF x=0 THEN Noise(\"\") ELSE TRUE"
        },
        "Tail(s)",
        "CONSTANT C=1\n",
    );
    let (args, _) = tail_call(&model);
    let Arg::Expr { expr: operand, .. } = args[0] else {
        panic!("expression");
    };
    let noise = model.ir.root_def("Noise").expect("Noise");
    let noise_expr = model
        .ir
        .nodes()
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            matches!(&node.expr, Expr::Apply { callee: OpRef::Def(id), .. } if *id==noise)
                .then_some(dive_ir::ExprId(u32::try_from(index).expect("node")))
        })
        .expect("actual Print call");
    let declaration = model
        .ir
        .decls()
        .iter()
        .enumerate()
        .find_map(|(index, declaration)| {
            (declaration.name == "C")
                .then_some(dive_ir::DeclId(u32::try_from(index).expect("decl")))
        })
        .expect("C");
    let mut json = serde_json::to_value(&*model.ir).expect("serialize public IR");
    let next = model.ir.root_def("Next").expect("Next");
    let original_next = model.ir.def(next).body;
    // Each expression child has exactly one owner in admitted public IR.
    // Copy these small closed operands, not the definitions they call.
    fn copy_operand(
        ir: &dive_ir::Ir,
        nodes: &mut Vec<serde_json::Value>,
        expr: dive_ir::ExprId,
    ) -> dive_ir::ExprId {
        let replacement = match ir.expr(expr) {
            Expr::Num(_) | Expr::Str(_) => ir.expr(expr).clone(),
            Expr::Tuple(items) => Expr::Tuple(
                items
                    .iter()
                    .map(|&child| copy_operand(ir, nodes, child))
                    .collect(),
            ),
            Expr::Apply { callee, args } => Expr::Apply {
                callee: callee.clone(),
                args: args
                    .iter()
                    .map(|arg| match arg {
                        Arg::Expr { expr, pass } => Arg::Expr {
                            expr: copy_operand(ir, nodes, *expr),
                            pass: *pass,
                        },
                        Arg::Op { .. } => panic!("fixture contains no operator operand"),
                    })
                    .collect(),
            },
            _ => panic!("fixture copies only closed leaves, tuples and calls"),
        };
        let mut node = serde_json::to_value(&ir.nodes()[expr.index()]).expect("source node");
        node["expr"] = serde_json::to_value(replacement).expect("copied expression");
        let id = dive_ir::ExprId(u32::try_from(nodes.len()).expect("copied node"));
        nodes.push(node);
        id
    }
    let bindings = |noise_expr| {
        vec![dive_ir::SubstBinding {
            decl: declaration,
            value: Arg::Expr {
                expr: noise_expr,
                pass,
            },
        }]
        .into_boxed_slice()
    };
    let exprs = json["exprs"].as_array_mut().expect("nodes");
    let tuple = model.ir.nodes().iter().enumerate().find_map(|(i,n)|
        matches!(&n.expr, Expr::Tuple(items) if items.len()==2 && items.iter().all(|e|matches!(model.ir.expr(*e),Expr::Num(_))))
            .then_some(dive_ir::ExprId(u32::try_from(i).expect("tuple node")))).expect("Init tuple");
    let copied_operand = copy_operand(&model.ir, exprs, tuple);
    let shape_noise = copy_operand(&model.ir, exprs, noise_expr);
    let shape_substitution = dive_ir::ExprId(u32::try_from(exprs.len()).expect("shape parent"));
    let mut shape_node = exprs[operand.index()].clone();
    shape_node["expr"] = serde_json::to_value(Expr::Subst {
        body: copied_operand,
        bindings: bindings(shape_noise),
    })
    .expect("strict shape parent");
    exprs.push(shape_node);
    let role_noise = copy_operand(&model.ir, exprs, noise_expr);
    let role_substitution = dive_ir::ExprId(u32::try_from(exprs.len()).expect("role parent"));
    let mut role_node = exprs[original_next.index()].clone();
    role_node["expr"] = serde_json::to_value(Expr::Subst {
        body: original_next,
        bindings: bindings(role_noise),
    })
    .expect("strict role parent");
    exprs.push(role_node);
    let unused = model.ir.root_def("Unused").expect("Unused owner");
    let old_unused = model.ir.def(unused).body;
    let shape_owner = dive_ir::ExprId(u32::try_from(exprs.len()).expect("owned shape parent"));
    let mut owner_node = exprs[old_unused.index()].clone();
    owner_node["expr"] = serde_json::to_value(Expr::Tuple(
        vec![old_unused, shape_substitution].into_boxed_slice(),
    ))
    .expect("owned unused tuple");
    exprs.push(owner_node);
    json["defs"][next.index()]["body"] = serde_json::to_value(role_substitution).expect("body");
    json["defs"][unused.index()]["body"] = serde_json::to_value(shape_owner).expect("shape owner");
    if !printing {
        // The positive Strict binding is genuinely a closed tuple of a
        // literal, not merely a user call whose purity metadata is trusted.
        // Retain its original single child owner to keep the arena valid.
        for expr in [shape_noise, role_noise] {
            let copied: Expr = serde_json::from_value(json["exprs"][expr.index()]["expr"].clone())
                .expect("copied call");
            let Expr::Apply { args, .. } = copied else {
                panic!("copied call");
            };
            let Arg::Expr { expr: child, .. } = args[0] else {
                panic!("literal child");
            };
            json["exprs"][expr.index()]["expr"] =
                serde_json::to_value(Expr::Tuple(vec![child].into_boxed_slice()))
                    .expect("actually inert tuple");
            json["exprs"][expr.index()]["effects"] =
                serde_json::to_value(dive_ir::Effects::NONE).expect("literal tuple effects");
        }
    }
    let annotated: Arc<dive_ir::Ir> =
        Arc::new(serde_json::from_value(json.clone()).expect("annotated public IR"));
    let mut annotated_model = dive_eval::Model::new(annotated);
    annotated_model.names = model.names.clone();
    annotated_model.constants[0] = Some(dive_eval::ConstBinding::Value(dive_value::Value::Int(1)));
    let admitted = dive_eval::Evaluator::new(annotated_model, dive_eval::Options::tlc());
    if pass == Pass::Strict && printing {
        let rejected = admitted.expect_err("annotated impure Strict operands reject structurally");
        assert_eq!(rejected.kind(), dive_eval::ContextErrorKind::Structure);
        assert!(rejected.detail().contains("not inert"));
    } else {
        let evaluator = admitted.expect("lazy or actually inert Strict context");
        assert!(!evaluator.has_print_event());
    }
    assert!(!model.evaluator.has_print_event());
    assert_eq!(model.take_output(), Vec::<String>::new());
    // Public serialized metadata can claim inertness without changing the
    // actual called Noise/PrintT definitions. Admission cannot trust that claim
    // before entering a Strict substitution. This is a Rust-IR control, not a
    // claim that lowering natural TLA+ emits these forged annotations.
    for expr in [shape_noise, role_noise] {
        json["exprs"][expr.index()]["effects"] =
            serde_json::to_value(dive_ir::Effects::NONE).expect("caller annotation");
    }
    let ir: Arc<dive_ir::Ir> = Arc::new(serde_json::from_value(json).expect("public IR"));
    let mut eval_model = dive_eval::Model::new(ir.clone());
    eval_model.names = model.names.clone();
    eval_model.constants[0] = Some(dive_eval::ConstBinding::Value(dive_value::Value::Int(1)));
    let evaluator =
        dive_eval::Evaluator::new(eval_model, dive_eval::Options::tlc()).expect("admitted IR");
    model.ir = ir;
    model.evaluator = evaluator;
    for init in &mut model.init {
        init.pred.scope = model
            .evaluator
            .scope(init.def.expect("Init"))
            .expect("init scope");
    }
    model.next.as_mut().expect("Next").expr = role_substitution;
    model.next.as_mut().expect("Next").scope = model
        .evaluator
        .scope(model.ir.root_def("Next").expect("Next"))
        .expect("next scope");
    model.invariants[0].pred.scope = model
        .evaluator
        .scope(model.ir.root_def("Inv").expect("Inv"))
        .expect("inv scope");
    {
        // This test is a public predicate/model authority control, not search.
        model.actions.clear();
        model.init_actions.clear();
    }
    model
        .validate_context()
        .expect("all roles share valid rebuilt context");
    model
}

fn word() -> TraceDoc {
    TraceDoc::behavior(vec![
        json!({"x":0,"s":{"seq":[0,1]}}),
        json!({"x":1,"s":{"seq":[1]}}),
    ])
    .expect("word")
    .with_ending(Ending::Invariant("Inv".into()))
}

#[test]
fn strict_substitutions_refuse_before_independent_endpoint_effects() {
    // Fresh receiving context for every endpoint: no earlier primitive or
    // validation can seed an effect or the namespace for a later assertion.
    for route in ["validate_doc", "exists_successor", "evaluate_lasso"] {
        let model = context_model(Pass::Strict, true);
        model.validate_context().expect("owned all-role model");
        assert!(model.runtime_name_order, "source Tail(s) requires history");
        assert!(!model.evaluator.has_print_event());
        assert_eq!(model.take_output(), Vec::<String>::new());
        let before = names(&model);
        let doc = word();
        let states = dive_trace::Trace::decode(&model, &doc)
            .expect("word decoded without evaluation")
            .states;
        match route {
            "validate_doc" => assert_eq!(
                dive_trace::validate_doc(&model, &doc, &[], None)
                    .0
                    .validation,
                dive_trace::Validation::Unsupported(
                    dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned()
                )
            ),
            "exists_successor" => assert_eq!(
                dive_trace::exists::exists_successor(
                    &model,
                    model.next.as_ref().expect("Next"),
                    &states[0],
                    None,
                    dive_trace::exists::Limits::default()
                )
                .expect_err("opaque substitution does not certify enabledness"),
                dive_trace::exists::ExistsError::Unsupported(
                    "opaque substitution action context".to_owned()
                )
            ),
            "evaluate_lasso" => assert_eq!(
                dive_trace::temporal::evaluate_lasso(
                    &model,
                    model.next.as_ref().expect("Next"),
                    &states,
                    1
                )
                .expect_err("no effectful entry before temporal admission"),
                dive_trace::temporal::TemporalError::Unsupported(
                    "Strict substitution requires evaluation before independent context admission"
                        .to_owned()
                )
            ),
            _ => unreachable!(),
        }
        assert!(!model.evaluator.has_print_event(), "{route}");
        assert!(model.take_output().is_empty(), "{route}");
        assert_eq!(names(&model), before, "{route}");
    }
}

#[test]
fn parsed_tuple_history_remains_independent_of_strict_context_refusal() {
    let fresh = || model("", "Tail(s)", "");
    let model = fresh();
    assert_eq!(
        dive_trace::validate_doc(&model, &word(), &[], None)
            .0
            .validation,
        dive_trace::Validation::Valid
    );
    assert!(!model.evaluator.has_print_event());
    assert_eq!(model.take_output(), Vec::<String>::new());
    let model = fresh();
    let mutant = TraceDoc::behavior(vec![
        json!({"x":0,"s":{"seq":[0,1]}}),
        json!({"x":1,"s":{"seq":[0]}}),
    ])
    .expect("mutant")
    .with_ending(Ending::Invariant("Inv".into()));
    assert!(matches!(
        dive_trace::validate_doc(&model, &mutant, &[], None)
            .0
            .validation,
        dive_trace::Validation::Invalid(_)
    ));
    assert!(!model.evaluator.has_print_event());
    assert_eq!(model.take_output(), Vec::<String>::new());
}

#[test]
fn lazy_substitution_keeps_an_independent_valid_word_without_forcing_unused_noise() {
    let model = context_model(Pass::Lazy, true);
    let before = names(&model);
    assert_eq!(
        dive_trace::validate_doc(&model, &word(), &[], None)
            .0
            .validation,
        dive_trace::Validation::Valid
    );
    assert!(!model.evaluator.has_print_event());
    assert_eq!(model.take_output(), Vec::<String>::new());
    assert_eq!(names(&model), before);
}

#[test]
fn ordinary_evaluator_keeps_strict_execution_and_inert_bindings() {
    for printing in [false, true] {
        let model = context_model(Pass::Strict, printing);
        let before = names(&model);
        let next = model.next.as_ref().expect("Next");
        assert!(!model.evaluator.has_print_event());
        assert_eq!(model.take_output(), Vec::<String>::new());
        // Independent admission now conservatively refuses both cases; it
        // must not infer eager safety from caller expression annotations.
        assert_eq!(
            dive_trace::validate_doc(&model, &word(), &[], None)
                .0
                .validation,
            dive_trace::Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.to_owned())
        );
        assert!(!model.evaluator.has_print_event());
        assert_eq!(model.take_output(), Vec::<String>::new());
        // This deliberately calls ordinary evaluator entry *after* the proof
        // assertions. Its existing Strict execution semantics are unchanged.
        model
            .evaluator
            .scope_subst(&next.scope, next.expr)
            .expect("ordinary Strict substitution entry");
        assert_eq!(model.evaluator.has_print_event(), printing);
        assert_eq!(
            model.take_output(),
            if printing {
                vec!["\"\"".to_owned()]
            } else {
                Vec::<String>::new()
            }
        );
        assert_eq!(names(&model), before);
    }
}
