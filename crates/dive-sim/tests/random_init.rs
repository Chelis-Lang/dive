//! Init effects close before properties and restore the caller on every return.
#![allow(clippy::expect_used, reason = "bounded setup and assertions")]

use dive_sim::{Options, Outcome, simulate};
use dive_stdlib::{
    host::RunHost,
    random::{RandomMode, Streams},
};
use std::sync::Arc;

#[derive(Debug)]
struct Caller;
impl dive_eval::Host for Caller {}

fn model(init: &str, next: &str) -> dive_model::Model {
    bind(
        &format!(
            "---- MODULE InitCleanup ----\nEXTENDS TLC\nVARIABLE x\nInit=={init}\nNext=={next}\n====\n"
        ),
        "INIT Init\nNEXT Next\n",
        &dive_model::BindOptions::tlc(),
    )
}

fn bind(source: &str, config: &str, options: &dive_model::BindOptions) -> dive_model::Model {
    let analysis =
        dive_sema::analyze_source("InitCleanup.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        options,
    )
    .expect("model")
}

fn large_stack(run: impl FnOnce() + Send) {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, run)
            .expect("stack thread")
            .join()
            .expect("cleanup controls");
    });
}

#[test]
fn init_success_error_refusal_and_limit_restore_host_and_release_run() {
    large_stack(|| {
        for (init, next, kind, with_host) in [
            ("x=RandomElement({7})", "UNCHANGED x", "sampled", false),
            ("x=RandomElement(42)", "UNCHANGED x", "error", false),
            (
                "x=RandomElement({7})",
                "x'=RandomElement({8})",
                "unsupported",
                false,
            ),
            (
                "x=RandomElement(IF PrintT(\"init\") THEN {7} ELSE {})",
                "UNCHANGED x",
                "limit",
                false,
            ),
            ("x=RandomElement({7})", "UNCHANGED x", "unsupported", true),
            (
                "x=IF PrintT(\"init\") THEN 7 ELSE 8",
                "UNCHANGED x",
                "sampled",
                true,
            ),
            (
                "x=IF PrintT(\"init\") THEN 7 ELSE 8",
                "UNCHANGED x",
                "limit",
                true,
            ),
        ] {
            let model = model(init, next);
            let caller: Arc<dyn dive_eval::Host> = Arc::new(Caller);
            if with_host {
                model.evaluator.set_host(caller.clone());
            }
            let mut options = Options::new(0, RandomMode::Tlc, 1, 1);
            if kind == "limit" {
                options.max_output_events = 0;
            }
            for _ in 0..2 {
                let result = simulate(&model, &options);
                assert!(
                    matches!(
                        (&result.outcome, kind),
                        (Outcome::Sampled, "sampled")
                            | (Outcome::Error { .. }, "error")
                            | (Outcome::Unsupported(_), "unsupported")
                            | (Outcome::Limit(_), "limit")
                    ),
                    "{result:?}"
                );
                let restored = model.evaluator.replace_host(None);
                if with_host {
                    let restored = restored.expect("restored host");
                    assert!(Arc::ptr_eq(&caller, &restored));
                    model.evaluator.set_host(restored);
                } else {
                    assert!(restored.is_none());
                }
            }
        }
    });
}

#[test]
fn actual_binding_startup_draw_cannot_be_silently_restarted_for_init() {
    large_stack(|| {
        let streams = Streams::new(0, RandomMode::Tlc);
        let host = Arc::new(RunHost::new(streams, None));
        let caller: Arc<dyn dive_eval::Host> = host.clone();
        let options = dive_model::BindOptions {
            host: Some(caller.clone()),
            ..dive_model::BindOptions::tlc()
        };
        let model = bind(
            "---- MODULE InitCleanup ----\nEXTENDS Naturals,TLC\nCONSTANT C\nVARIABLE x\nPick==RandomElement(1..1000000)\nInit==x=RandomElement(1..1000000)\nNext==UNCHANGED x\n====\n",
            "CONSTANT C <- Pick\nINIT Init\nNEXT Next\n",
            &options,
        );
        let consumed = host.stream();
        assert_ne!(
            consumed,
            streams.startup(),
            "actual binding must consume the caller stream"
        );
        let refused = simulate(&model, &Options::new(0, RandomMode::Tlc, 1, 0));
        assert_eq!(refused.stage, "preflight");
        assert!(matches!(refused.outcome, Outcome::Unsupported(_)));
        assert_eq!(host.stream(), consumed);
        let restored = model
            .evaluator
            .replace_host(None)
            .expect("untouched caller");
        assert!(Arc::ptr_eq(&caller, &restored));
        model.evaluator.set_host(restored);
        for _ in 0..2 {
            let result = simulate(&model, &Options::new(0, RandomMode::Tlc, 1, 1));
            assert!(
                matches!(&result.outcome, Outcome::Unsupported(reason) if reason.contains("caller-provided startup stream")),
                "{result:?}"
            );
            assert_eq!(result.stage, "initialization");
            assert_eq!(result.generated, 0);
            assert!(result.traces.is_empty());
            assert_eq!(
                host.stream(),
                consumed,
                "refusal cannot consume the old host"
            );
            let restored = model.evaluator.replace_host(None).expect("restored caller");
            assert!(Arc::ptr_eq(&caller, &restored));
            model.evaluator.set_host(restored);
        }
    });
}
