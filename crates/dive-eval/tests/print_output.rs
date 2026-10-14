//! Hosts may discard or refuse output without changing Print's value semantics.
#![allow(clippy::expect_used, reason = "test setup/assertions")]
use dive_eval::{Evaluator, Host, HostError, Model, Options, PrintSink, St};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
struct Sink(AtomicUsize);
impl PrintSink for Sink {
    fn print(&self, _: String) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}
#[derive(Debug)]
struct Quiet;
impl Host for Quiet {
    fn print_output(&self, _: &str) -> Result<bool, HostError> {
        Ok(false)
    }
}
#[derive(Debug)]
struct Refuse;
impl Host for Refuse {
    fn print_output(&self, _: &str) -> Result<bool, HostError> {
        Err(HostError {
            code: 0,
            message: "output denied".to_owned(),
        })
    }
}

#[test]
fn discarded_output_retains_values_and_refused_output_is_fallible() {
    let analysis = dive_sema::analyze_source(
        "PrintPolicy.tla",
        "---- MODULE PrintPolicy ----\nEXTENDS TLC\nEcho == Print(\"v\",7)\nTrue == PrintT(FALSE)\n====\n",
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let sink = Arc::new(Sink::default());
    let evaluator = Evaluator::with_sink(Model::new(ir.clone()), Options::tlc(), sink.clone())
        .expect("evaluator");
    evaluator.set_host(Arc::new(Quiet));
    assert_eq!(
        evaluator
            .eval_def(ir.root_def("Echo").expect("Echo"), &St::constant())
            .expect("return"),
        Value::Int(7)
    );
    assert_eq!(
        evaluator
            .eval_def(ir.root_def("True").expect("True"), &St::constant())
            .expect("return"),
        Value::TRUE
    );
    assert_eq!(sink.0.load(Ordering::Relaxed), 0);
    evaluator.set_host(Arc::new(Refuse));
    assert!(
        evaluator
            .eval_def(ir.root_def("Echo").expect("Echo"), &St::constant())
            .is_err()
    );
    assert_eq!(sink.0.load(Ordering::Relaxed), 0);
}

#[test]
fn retained_capture_uses_warning_history_without_sink_callbacks() {
    let analysis = dive_sema::analyze_source(
        "QuietWarnings.tla",
        "---- MODULE QuietWarnings ----\nEXTENDS TLC\nA == TRUE\n====\n",
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let sink = Arc::new(Sink::default());
    let ev = Evaluator::with_sink(Model::new(ir), Options::tlc(), sink.clone()).expect("evaluator");
    let warning = dive_eval::capture::Captured {
        text: "warning".to_owned(),
        warning: true,
    };
    let print = dive_eval::capture::Captured {
        text: "warning".to_owned(),
        warning: false,
    };
    assert_eq!(
        ev.filter_captured_warnings(vec![
            warning.clone(),
            print.clone(),
            warning.clone(),
            print.clone()
        ]),
        [warning.clone(), print.clone(), print.clone()]
    );
    assert_eq!(
        ev.filter_captured_warnings(vec![warning, print.clone()]),
        [print]
    );
    assert_eq!(sink.0.load(Ordering::Relaxed), 0);
    assert!(!ev.has_print_event(), "filtering does not evaluate events");
}
