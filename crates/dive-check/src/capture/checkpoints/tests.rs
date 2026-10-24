#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test assertions and deliberate callback unwind"
)]

use super::*;
use dive_eval::{EvalError, St, next::StateVals};
use dive_value::Value;
use std::sync::Arc;

fn model(body: &str, config: &str, opts: &CheckOptions) -> Model {
    let source =
        format!("---- MODULE Reached ----\nEXTENDS Naturals, TLC\nVARIABLE x\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("Reached.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config)
        .into_config()
        .expect("configuration");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            symmetry_liveness: opts.symmetry_liveness,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("model")
}
struct Visit<F>(F);
impl<F> CheckpointObserver for Visit<F>
where
    F: for<'a> FnMut(ReachedCheckpoint<'a>) -> Result<(), EvalError> + Send,
{
    fn reached(&mut self, event: ReachedCheckpoint<'_>) -> Result<(), EvalError> {
        (self.0)(event)
    }
}
fn run(
    m: &Model,
    opts: &CheckOptions,
    visit: &mut dyn CheckpointObserver,
) -> CheckpointExploration {
    explore_checkpoints_with_evidence(m, opts, Limits::default(), None, visit)
        .expect("thread evidence")
}
fn stopped(run: &CheckpointExploration) -> &crate::CheckResult {
    let ControlledExploration::Stopped(result) = run.outcome() else {
        panic!("expected stopped evidence: {run:?}")
    };
    result
}
fn x(state: &StateVals) -> i64 {
    let Some(Value::Int(value)) = state[0].as_ref() else {
        panic!("integer fixture state")
    };
    *value
}
fn profiles() -> [CheckOptions; 2] {
    [CheckOptions::tlc(0, 0), CheckOptions::dive(0)]
}
const LOOP: &str = "Init == x = 0\nNext == x' = 1 - x";
const CFG: &str = "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n";

#[test]
fn init_is_really_queued_before_invariant_and_transports_original_core_cause() {
    for opts in profiles() {
        let m = model(
            &format!("{LOOP}\nFail == FALSE\nBad == x \\div 0 = 0"),
            "INIT Init\nNEXT Next\nINVARIANTS Fail Bad\n",
            &opts,
        );
        let expected = m
            .eval(
                &m.invariants[1].pred,
                &St::state(vec![Some(Value::Int(0))].into()),
            )
            .expect_err("original division cause");
        let mut calls = Vec::new();
        let result = run(
            &m,
            &opts,
            &mut Visit(|e: ReachedCheckpoint<'_>| {
                calls.push((e.phase(), x(e.state()), e.stats()));
                assert_eq!(e.edges().len(), 0);
                assert_eq!(e.concrete_successors().len(), 0);
                m.eval(&m.invariants[1].pred, &St::state(e.state().clone()))
                    .map(|_| ())
            }),
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            (
                CheckpointPhase::Init,
                0,
                Stats {
                    generated: 1,
                    distinct: 1,
                    initial_generated: 1,
                    depth: 1,
                    left_on_queue: 1
                }
            )
        );
        let decision = result.decision().expect("actual typed core failure");
        assert_eq!(decision.phase(), CheckpointPhase::Init);
        assert_eq!(decision.source(), 0);
        assert_eq!(decision.cause().class(), ErrorClass::Eval);
        assert_eq!(decision.cause().ec(), dive_eval::Ec::DivisionByZero);
        assert_eq!(decision.cause(), &expected);
        assert!(decision.cause().origin().is_some());
        assert_eq!(stopped(&result).stats, Some(decision.stats()));
        assert_eq!(
            stopped(&result)
                .counterexample
                .as_ref()
                .expect("path")
                .states[0]
                .values,
            [Value::Int(0)]
        );
        assert!(
            matches!(&stopped(&result).verdict, Verdict::Unsupported { feature } if feature == DECISION)
        );
        let allowed = run(&m, &opts, &mut Visit(|_: ReachedCheckpoint<'_>| Ok(())));
        assert!(allowed.decision().is_none());
        assert!(matches!(
            stopped(&allowed).verdict,
            Verdict::InvariantViolated { initial: true, .. }
        ));
    }
}

#[test]
fn delayed_source_decides_before_sibling_with_real_queue_depth_and_source_path() {
    for opts in profiles() {
        let m = model(
            "Init == x = 0\nNext == \\/ (x = 0 /\\ x' \\in {1, 2})\n        \\/ (x = 1 /\\ x' = 3)\n        \\/ (x > 1 /\\ x' = x)",
            CFG,
            &opts,
        );
        let mut seen = Vec::new();
        let result = run(
            &m,
            &opts,
            &mut Visit(|e: ReachedCheckpoint<'_>| {
                seen.push((e.phase(), x(e.state())));
                if e.phase() == CheckpointPhase::SourceRow {
                    assert_eq!(e.edges().last().expect("stutter").ordinal, None);
                    if x(e.state()) == 1 {
                        return Err(EvalError::general("controlled source decision"));
                    }
                }
                Ok(())
            }),
        );
        assert_eq!(
            seen,
            [
                (CheckpointPhase::Init, 0),
                (CheckpointPhase::SourceRow, 0),
                (CheckpointPhase::SourceRow, 1)
            ]
        );
        let d = result.decision().expect("reached source decision");
        assert_eq!(
            d.stats(),
            Stats {
                generated: 4,
                distinct: 4,
                left_on_queue: 2,
                depth: 3,
                initial_generated: 1
            }
        );
        let states = &stopped(&result)
            .counterexample
            .as_ref()
            .expect("checked finite source path")
            .states;
        assert_eq!(
            states.iter().map(|s| s.values.clone()).collect::<Vec<_>>(),
            [vec![Value::Int(0)], vec![Value::Int(1)]]
        );
        assert_eq!(stopped(&result).workers.used, 1);
        assert!(stopped(&result).workers.sequential);
    }
}

#[test]
fn exhausted_final_source_is_a_decision_not_graph_completion_or_level_prefix() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model("Init == x = 0\nNext == x' = x", CFG, &opts);
    let result = run(
        &m,
        &opts,
        &mut Visit(|e: ReachedCheckpoint<'_>| {
            if e.phase() == CheckpointPhase::SourceRow {
                assert_eq!(e.stats().left_on_queue, 0);
                assert_eq!(e.concrete_successors().len(), 1);
                assert_eq!(e.edges().len(), 2);
                return Err(EvalError::general("last source still decides"));
            }
            Ok(())
        }),
    );
    assert_eq!(
        result.decision().expect("decision").stats().left_on_queue,
        0
    );
    assert!(matches!(
        result.outcome(),
        ControlledExploration::Stopped(_)
    ));
}

#[test]
fn earlier_next_safety_deadlock_and_capture_admission_suppress_row_callback() {
    for opts in profiles() {
        for (body, cfg, expected) in [
            (
                "Init == x = 0\nNext == x' = 1 \\div 0",
                CFG,
                "evaluation-error",
            ),
            (
                "Init == x = 0\nNext == x' = 1\nInv == x = 0",
                "INIT Init\nNEXT Next\nINVARIANT Inv\n",
                "invariant-violation",
            ),
            (
                "Init == x = 0\nNext == x' = 1 /\\ x' = 2",
                "INIT Init\nNEXT Next\n",
                "deadlock",
            ),
        ] {
            let m = model(body, cfg, &opts);
            let mut rows = 0;
            let result = run(
                &m,
                &opts,
                &mut Visit(|e: ReachedCheckpoint<'_>| {
                    if e.phase() == CheckpointPhase::SourceRow {
                        rows += 1;
                    }
                    Ok(())
                }),
            );
            assert_eq!(rows, 0);
            assert!(result.decision().is_none());
            assert_eq!(stopped(&result).verdict.kind(), expected);
        }
        let m = model(LOOP, CFG, &opts);
        let mut calls = 0;
        let result = explore_checkpoints_with_evidence(
            &m,
            &opts,
            Limits {
                states: 0,
                ..Limits::default()
            },
            None,
            &mut Visit(|_: ReachedCheckpoint<'_>| {
                calls += 1;
                Ok(())
            }),
        )
        .expect("thread");
        assert_eq!(calls, 0);
        assert!(result.decision().is_none());
        assert!(matches!(
            stopped(&result).verdict,
            Verdict::ResourceLimit { .. }
        ));
    }
}

#[test]
fn excluded_duplicate_and_incomplete_init_candidates_do_not_invent_root_checkpoints() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model(
        "Init == x \\in {0, 1} \\/ x = 0\nNext == x' = x\nKeep == x = 0",
        "INIT Init\nNEXT Next\nCONSTRAINT Keep\n",
        &opts,
    );
    let mut roots = Vec::new();
    let result = run(
        &m,
        &opts,
        &mut Visit(|e: ReachedCheckpoint<'_>| {
            if e.phase() == CheckpointPhase::Init {
                roots.push((x(e.state()), e.stats()));
            }
            Ok(())
        }),
    );
    assert!(matches!(
        result.outcome(),
        ControlledExploration::Complete(_)
    ));
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].1.initial_generated, 1);
    assert_eq!(roots[0].1.left_on_queue, 1);
    let m = model("Init == TRUE\nNext == x' = 0", CFG, &opts);
    let mut calls = 0;
    let result = run(
        &m,
        &opts,
        &mut Visit(|_: ReachedCheckpoint<'_>| {
            calls += 1;
            Ok(())
        }),
    );
    assert_eq!(calls, 0);
    assert!(result.decision().is_none());
    assert!(matches!(
        stopped(&result).verdict,
        Verdict::NotCompletelySpecified { .. }
    ));
}

#[derive(Default)]
struct Whole {
    roots: Vec<(usize, Stats)>,
    levels: Vec<(u64, Stats)>,
    edges: usize,
}
impl Observer for Whole {
    fn init(&mut self, id: usize, _: &StateVals, stats: Stats) -> Result<(), String> {
        self.roots.push((id, stats));
        Ok(())
    }
    fn edge(&mut self, _: usize, _: &Edge, _: &StateVals, _: Stats) -> Result<(), String> {
        self.edges += 1;
        Ok(())
    }
    fn level_done(
        &mut self,
        level: u64,
        _: &super::super::BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        self.levels.push((level, stats));
        Ok(())
    }
}
#[test]
fn successful_checkpoints_preserve_whole_init_level_callbacks_and_legacy_counts() {
    for mut opts in profiles() {
        opts.sequential = true;
        let m = model(LOOP, CFG, &opts);
        let mut old = Whole::default();
        let legacy =
            super::super::explore_controlled_with_evidence(&m, &opts, Limits::default(), &mut old)
                .expect("legacy");
        let mut new = Whole::default();
        let mut rows = Vec::new();
        let result = explore_checkpoints_with_evidence(
            &m,
            &opts,
            Limits::default(),
            Some(&mut new),
            &mut Visit(|e: ReachedCheckpoint<'_>| {
                rows.push((e.phase(), x(e.state())));
                Ok(())
            }),
        )
        .expect("checkpoint");
        assert!(result.decision().is_none());
        assert_eq!(new.roots, old.roots);
        assert_eq!(new.levels, old.levels);
        assert_eq!(new.edges, old.edges);
        let ControlledExploration::Complete(a) = legacy else {
            panic!("legacy complete")
        };
        let ControlledExploration::Complete(b) = result.outcome() else {
            panic!("new complete")
        };
        assert_eq!(a.stats, b.stats);
        assert_eq!(a.graph.states, b.graph.states);
        assert_eq!(a.graph.edges, b.graph.edges);
        assert_eq!(
            rows,
            [
                (CheckpointPhase::Init, 0),
                (CheckpointPhase::SourceRow, 0),
                (CheckpointPhase::SourceRow, 1)
            ]
        );
    }
}

#[test]
fn a_partial_source_level_never_emits_legacy_edge_or_level_events() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model(LOOP, CFG, &opts);
    let mut whole = Whole::default();
    let result = explore_checkpoints_with_evidence(
        &m,
        &opts,
        Limits::default(),
        Some(&mut whole),
        &mut Visit(|e: ReachedCheckpoint<'_>| {
            if e.phase() == CheckpointPhase::SourceRow {
                Err(EvalError::general("stop before whole level"))
            } else {
                Ok(())
            }
        }),
    )
    .expect("thread");
    assert!(result.decision().is_some());
    assert_eq!(whole.roots.len(), 1);
    assert_eq!(whole.edges, 0);
    assert_eq!(whole.levels.len(), 0);
}

#[test]
fn zero_and_parallel_requested_widths_refuse_before_any_callback_or_search() {
    for width in [0, 2, 4] {
        let mut opts = CheckOptions::tlc(0, 0);
        opts.workers = width;
        let m = model(LOOP, CFG, &opts);
        let mut calls = 0;
        let result = run(
            &m,
            &opts,
            &mut Visit(|_: ReachedCheckpoint<'_>| {
                calls += 1;
                Ok(())
            }),
        );
        assert_eq!(calls, 0);
        assert!(result.decision().is_none());
        assert_eq!(stopped(&result).stats, None);
        assert_eq!(stopped(&result).workers.requested, width);
        assert_eq!(stopped(&result).workers.used, 0);
        assert!(matches!(
            stopped(&result).verdict,
            Verdict::Unsupported { .. }
        ));
    }
}

#[test]
fn callback_panic_and_foreign_interruption_leave_no_decision_or_control_leak() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model(LOOP, CFG, &opts);
    for panic_callback in [false, true] {
        let result = run(
            &m,
            &opts,
            &mut Visit(|_: ReachedCheckpoint<'_>| {
                assert!(!panic_callback, "controlled checkpoint unwind");
                Err(EvalError::interrupted(dive_eval::Interrupted::new(0)))
            }),
        );
        assert!(result.decision().is_none());
        assert!(matches!(
            stopped(&result).verdict,
            Verdict::InternalError { .. }
        ));
        let subsequent = run(&m, &opts, &mut Visit(|_: ReachedCheckpoint<'_>| Ok(())));
        assert!(subsequent.decision().is_none());
        assert!(matches!(
            subsequent.outcome(),
            ControlledExploration::Complete(_)
        ));
    }
}

#[test]
fn actual_armed_budget_keeps_stop_class_and_suppresses_callback_after_exhaustion() {
    let mut opts = CheckOptions::tlc(0, 0);
    let watchdog = crate::budget::Budget::new(Some(std::time::Duration::from_secs(3600)), None)
        .start()
        .expect("budget thread")
        .expect("armed budget");
    opts.budget = watchdog.budget().clone();
    let m = model(
        &format!("{LOOP}\nInv == TRUE"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        &opts,
    );
    let mut calls = 0;
    let result = run(
        &m,
        &opts,
        &mut Visit(|e: ReachedCheckpoint<'_>| {
            calls += 1;
            opts.budget.trip_for_test();
            m.eval(&m.invariants[0].pred, &St::state(e.state().clone()))
                .map(|_| ())
        }),
    );
    assert_eq!(calls, 1);
    assert!(result.decision().is_none());
    assert!(
        matches!(&stopped(&result).verdict, Verdict::Stopped(stop) if stop.phase == crate::budget::Phase::Capture)
    );
    let mut later_calls = 0;
    let subsequent = run(
        &m,
        &opts,
        &mut Visit(|_: ReachedCheckpoint<'_>| {
            later_calls += 1;
            Ok(())
        }),
    );
    assert_eq!(later_calls, 0);
    assert!(subsequent.decision().is_none());
    assert!(matches!(stopped(&subsequent).verdict, Verdict::Stopped(_)));
}

fn output(run: &CheckpointExploration) -> &[String] {
    match run.outcome() {
        ControlledExploration::Complete(capture) => &capture.safety_result().output,
        ControlledExploration::Stopped(result) => &result.output,
        ControlledExploration::Prefix(prefix) => &prefix.output,
    }
}

#[test]
fn init_decision_omits_invariant_print_but_same_input_allowed_callback_prints() {
    for opts in profiles() {
        let m = model(
            "Init == x = 0\nNext == x' = x\nInv == IF x = 0 THEN PrintT(\"init-invariant-sentinel\") ELSE TRUE",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &opts,
        );
        let refused = run(
            &m,
            &opts,
            &mut Visit(|e: ReachedCheckpoint<'_>| {
                assert_eq!(e.phase(), CheckpointPhase::Init);
                Err(EvalError::general("decide before the invariant effect"))
            }),
        );
        assert!(refused.decision().is_some());
        assert_eq!(output(&refused), [] as [String; 0]);
        let allowed = run(&m, &opts, &mut Visit(|_: ReachedCheckpoint<'_>| Ok(())));
        assert!(allowed.decision().is_none());
        assert!(matches!(
            allowed.outcome(),
            ControlledExploration::Complete(_)
        ));
        assert_eq!(output(&allowed), ["\"init-invariant-sentinel\"".to_owned()]);
    }
}

#[test]
fn source_decision_omits_pending_sibling_print_but_same_input_allowed_callback_prints() {
    for opts in profiles() {
        let m = model(
            "Init == x = 0\nNext == IF x = 0 THEN x' \\in {1, 2}\n        ELSE IF x = 1 THEN x' = 3\n        ELSE IF x = 2 THEN PrintT(\"pending-sibling-sentinel\") /\\ x' = x\n        ELSE x' = x",
            CFG,
            &opts,
        );
        let mut rows = Vec::new();
        let refused = run(
            &m,
            &opts,
            &mut Visit(|e: ReachedCheckpoint<'_>| {
                if e.phase() == CheckpointPhase::SourceRow {
                    rows.push(x(e.state()));
                    if x(e.state()) == 1 {
                        return Err(EvalError::general("decide before pending sibling"));
                    }
                }
                Ok(())
            }),
        );
        assert_eq!(rows, [0, 1]);
        assert_eq!(
            refused
                .decision()
                .expect("reached source")
                .stats()
                .left_on_queue,
            2
        );
        assert_eq!(output(&refused), [] as [String; 0]);
        let allowed = run(&m, &opts, &mut Visit(|_: ReachedCheckpoint<'_>| Ok(())));
        assert!(allowed.decision().is_none());
        assert!(matches!(
            allowed.outcome(),
            ControlledExploration::Complete(_)
        ));
        assert_eq!(
            output(&allowed),
            ["\"pending-sibling-sentinel\"".to_owned()]
        );
    }
}

#[test]
fn non_eval_callback_causes_keep_existing_mapping_without_a_typed_decision() {
    for opts in profiles() {
        let m = model(LOOP, CFG, &opts);
        for (cause, expected) in [
            (
                EvalError::unsupported("unimplemented callback feature"),
                Verdict::Unsupported {
                    feature: "unimplemented callback feature".into(),
                },
            ),
            (
                EvalError::resource_limit("callback resource bound"),
                Verdict::ResourceLimit {
                    message: "callback resource bound".into(),
                },
            ),
            (
                EvalError::bug("callback invariant fault"),
                Verdict::InternalError {
                    message: "callback invariant fault".into(),
                },
            ),
        ] {
            let result = run(
                &m,
                &opts,
                &mut Visit(|_: ReachedCheckpoint<'_>| Err(cause.clone())),
            );
            assert_eq!(stopped(&result).verdict, expected);
            assert!(result.decision().is_none());
            assert!(stopped(&result).counterexample.is_none());
        }
    }
}
