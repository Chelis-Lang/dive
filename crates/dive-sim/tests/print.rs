//! Print budgets are fallible runtime checks, not purity admission proofs.
#![allow(clippy::expect_used, reason = "test setup and assertions")]

use dive_sim::{Options, Outcome, simulate};
use dive_stdlib::random::RandomMode;
use std::sync::Arc;

#[derive(Debug)]
struct Caller;
impl dive_eval::Host for Caller {}

fn run(source: &str, options: &Options) -> dive_sim::Report {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                let analysis = dive_sema::analyze_source(
                    "PrintRun.tla",
                    source,
                    &dive_sema::Options::default(),
                );
                let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
                let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
                    .into_config()
                    .expect("config");
                let model =
                    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
                let caller: Arc<dyn dive_eval::Host> = Arc::new(Caller);
                model.evaluator.set_host(caller.clone());
                let report = simulate(&model, options);
                let restored = model
                    .evaluator
                    .replace_host(None)
                    .expect("restored caller host");
                assert!(Arc::ptr_eq(&caller, &restored));
                report
            })
            .expect("thread")
            .join()
            .expect("run")
    })
}

#[test]
fn retained_output_limits_stop_without_sampled_or_unrecorded_success() {
    let source = "---- MODULE PrintRun ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0\nNext == PrintT(x) /\\ x'=x+1\n====\n";
    for (events, bytes, expected_texts, generated) in [
        (0, 100, Vec::<&str>::new(), 1),
        (1, 100, vec!["0"], 2),
        (100, 0, vec![], 1),
        (100, 1, vec!["0"], 2),
    ] {
        let mut options = Options::new(0, RandomMode::Tlc, 3, 1);
        options.max_output_events = events;
        options.max_output_bytes = bytes;
        let report = run(source, &options);
        assert!(matches!(report.outcome, Outcome::Limit(_)));
        assert_eq!(report.generated, generated);
        assert_eq!(report.stage, "sampling");
        assert_eq!(
            report
                .runtime_output
                .iter()
                .map(|event| event.text.as_str())
                .collect::<Vec<_>>(),
            expected_texts
        );
    }
}

#[test]
fn lazy_argument_prints_once_and_print_preserves_the_return_value() {
    let source = "---- MODULE PrintRun ----\nEXTENDS Naturals, TLC\nVARIABLE x\nTwice(v) == v+v\nInit == x=0\nNext == x'=Twice(Print(\"lazy\",x+1))\n====\n";
    let report = run(source, &Options::new(0, RandomMode::Tlc, 2, 1));
    assert!(matches!(report.outcome, Outcome::Sampled));
    assert_eq!(
        report
            .runtime_output
            .iter()
            .map(|event| event.text.as_str())
            .collect::<Vec<_>>(),
        ["\"lazy\"  1", "\"lazy\"  3"]
    );
    assert_eq!(
        report.traces[0].states.last().expect("state")[0],
        Some(dive_value::Value::Int(6))
    );
}

#[test]
fn unicode_print_text_uses_utf8_byte_limits() {
    let source = "---- MODULE PrintRun ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0\nNext == PrintT(\"é\") /\\ x'=x+1\n====\n";
    for mode in [RandomMode::Tlc, RandomMode::Dive] {
        for (bytes, expected, generated) in [(3, vec![], 1), (4, vec!["\"é\""], 2)] {
            let mut options = Options::new(0, mode, 2, 1);
            options.max_output_bytes = bytes;
            let report = run(source, &options);
            assert!(
                matches!(report.outcome, Outcome::Limit(ref reason) if reason.contains("byte"))
            );
            assert_eq!(report.generated, generated);
            assert_eq!(
                report
                    .runtime_output
                    .iter()
                    .map(|event| event.text.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
