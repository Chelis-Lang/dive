//! Fallible function-domain comparisons retain the actual evaluator control.
#![allow(clippy::expect_used, reason = "bounded fixture and typed error assertions")]

use dive_eval::{ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Stop(AtomicUsize);

impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(Interrupted::new(73))
    }
}

fn evaluator(options: Options, width: i64) -> Evaluator {
    let source = "---- MODULE FunctionSelectionControl ----\nEXTENDS Naturals\nF == TRUE\nD == {f \\in [1..12 -> {0,1}] : f[1] = 0}\nPick == F[D]\n====\n";
    let analysis = dive_sema::analyze_source(
        "FunctionSelectionControl.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let mut model = Model::new(ir.clone());
    let pairs = (0..width)
        .map(|i| {
            (
                Value::set(vec![Value::Int(i)], options.ctx).expect("singleton key"),
                Value::TRUE,
            )
        })
        .collect();
    model.def_values.insert(
        ir.root_def("F").expect("F"),
        Value::fcn(pairs, options.ctx).expect("finite set-keyed function"),
    );
    Evaluator::new(model, options).expect("evaluator")
}

fn pick(ev: &Evaluator) -> dive_eval::Result<Value> {
    let def = ev.ir().root_def("Pick").expect("Pick");
    ev.eval_in(def, ev.ir().def(def).body, &St::constant())
}

#[test]
fn a_large_function_cannot_relabel_interrupted_domain_comparison_as_unsupported() {
    for options in [Options::tlc(), Options::dive()] {
        for width in [31, 32] {
            let reference = pick(&evaluator(options, width)).expect_err("ordinary reference");
            let ev = evaluator(options, width);
            let stop = Arc::new(Stop(AtomicUsize::new(0)));
            let scope = ev.host_control(Some(stop.clone())).expect("owner scope");
            let error = pick(&ev).expect_err("deferred argument comparison interrupts");
            assert!(stop.0.load(Ordering::Relaxed) > 0, "real provider loop");
            assert_eq!(error.class(), ErrorClass::Interrupted, "width {width}");
            assert_eq!(error.interruption().expect("typed owner").token(), 73);
            drop(scope);

            // Forcing the deferred argument can itself fail before the final
            // domain lookup. Preserve the complete ordinary error, including
            // that priority, rather than inferring it from the function width.
            let ordinary = pick(&ev).expect_err("set key versus function elements");
            assert_eq!(ordinary, reference);
        }
    }
}
