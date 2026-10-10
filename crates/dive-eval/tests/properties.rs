//! Property tests over generated constant expressions:
//! - evaluation is deterministic: two evaluators of the same specification
//!   give the same value or the same error, with the same output;
//! - the integer modes agree wherever TLC's 32-bit arithmetic does not
//!   overflow: same value, or the same error with the same message.

#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

mod harness;

use std::fmt::Write as _;
use std::sync::Arc;

use dive_eval::{Evaluator, Model, Options, St};
use dive_value::{Ctx, IntMode, ValueOrder, print::print_value};
use proptest::prelude::*;

/// A generated integer expression in TLA+ syntax.
fn int_expr() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        (-40i64..40).prop_map(|n| if n < 0 {
            format!("({n})")
        } else {
            n.to_string()
        }),
        Just("2147483647".to_owned()),
        Just("65536".to_owned()),
    ];
    // No binders below the top: SANY rejects a bound name that shadows another.
    let arith = leaf.prop_recursive(4, 24, 3, |inner| {
        prop_oneof![
            (
                inner.clone(),
                inner.clone(),
                prop::sample::select(vec!["+", "-", "*", "\\div", "%"])
            )
                .prop_map(|(a, b, op)| format!("({a} {op} {b})")),
            (inner.clone(), 0u32..4).prop_map(|(a, e)| format!("({a} ^ {e})")),
            (inner.clone(), inner.clone(), inner.clone())
                .prop_map(|(c, a, b)| format!("(IF {c} > 0 THEN {a} ELSE {b})")),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("Cardinality({{{a}, {b}}})")),
            (inner.clone(), inner).prop_map(|(a, b)| format!("Len(<<{a}, {b}>> \\o <<{a}>>)")),
        ]
    });
    prop_oneof![
        arith.clone(),
        (arith.clone(), arith.clone(), arith.clone())
            .prop_map(|(a, b, c)| format!("(CHOOSE x \\in {{{a}, {b}}} : x > {c})")),
        (arith.clone(), arith)
            .prop_map(|(a, b)| format!("(LET k == {a} IN [i \\in 1..3 |-> i * k][({b} % 3) + 1])")),
    ]
}

fn spec(exprs: &[String]) -> String {
    let mut src =
        String::from("---- MODULE P ----\nEXTENDS Integers, FiniteSets, Sequences, TLC\n");
    for (i, e) in exprs.iter().enumerate() {
        let _ = writeln!(src, "E{i} == {e}");
    }
    src.push_str("====\n");
    src
}

/// Evaluates every `E<i>` and renders the result or the error.
fn eval_all(src: &str, n: usize, opts: Options) -> Vec<String> {
    let analysis = dive_sema::analyze_source("P.tla", src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "front end rejects:\n{src}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let mut model = Model::new(ir.clone());
    model.names = Arc::new(harness::seed_interner(src));
    let ev = Evaluator::new(model, opts).expect("valid evaluator context");
    (0..n)
        .map(|i| {
            let d = ir.root_def(&format!("E{i}")).expect("defined");
            match ev.eval_def(d, &St::constant()) {
                Ok(v) => print_value(&v, opts.ctx).unwrap_or_default(),
                Err(e) => format!("error: {e}"),
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    #[test]
    fn evaluation_is_deterministic(exprs in prop::collection::vec(int_expr(), 1..8)) {
        let src = spec(&exprs);
        for opts in [Options::tlc(), Options::dive()] {
            let a = eval_all(&src, exprs.len(), opts);
            let b = eval_all(&src, exprs.len(), opts);
            prop_assert_eq!(a, b);
        }
    }

    #[test]
    fn integer_modes_agree_without_overflow(exprs in prop::collection::vec(int_expr(), 1..8)) {
        let src = spec(&exprs);
        let big = Options {
            ctx: Ctx { int_mode: IntMode::Big, order: ValueOrder::Tlc },
            ..Options::tlc()
        };
        let small = eval_all(&src, exprs.len(), Options::tlc());
        let wide = eval_all(&src, exprs.len(), big);
        for (s, w) in small.iter().zip(&wide) {
            if s.contains("Overflow") || s.contains("TLC_INTEGER_TOO_BIG") {
                continue;
            }
            prop_assert_eq!(s, w);
        }
    }
}
