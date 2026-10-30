//! Count policy and real owner-budget stops through the native file route.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite resource controls"
)]
use super::*;
use dive_check::budget::{Budget, Watchdog};
use dive_liveness::analyze::{AnalysisError, Stage};
use std::time::Duration;

const SOURCE: &str = "---- MODULE Capacity ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 4\nP == []<>(x = 99)\n====\n";

fn inputs(tlc: bool) -> (Arc<Ir>, dive_config::ModelConfig, CheckOptions) {
    let analysis =
        dive_sema::analyze_source("Capacity.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    let options = if tlc {
        CheckOptions::tlc(0, 0)
    } else {
        CheckOptions::dive(0)
    };
    (ir, config, options)
}

fn expired() -> Watchdog {
    let watchdog = Budget::new(Some(Duration::ZERO), None)
        .start()
        .expect("start")
        .expect("finite");
    let start = Instant::now();
    while watchdog.budget().poll().expect("poll").is_none()
        && start.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(watchdog.budget().poll().expect("poll").is_some());
    watchdog
}

#[test]
fn core_file_capacity_is_budgeted_while_library_and_reference_limits_stay_exact() {
    let core = crate::temporal::core_file_configuration();
    let legacy = crate::temporal::file_configuration();
    let defaults = dive_liveness::analyze::Limits::default();
    assert_eq!(core.limits.capture.states, usize::MAX);
    assert_eq!(core.limits.capture.edges, usize::MAX);
    assert_eq!(core.limits.capture.generated, u64::MAX);
    assert_eq!(core.limits.capture.successors_per_unit, u64::MAX);
    assert_eq!(core.limits.product.vertices, usize::MAX);
    assert_eq!(core.limits.product.edges, usize::MAX);
    assert_eq!(core.limits.product.steps, usize::MAX);
    assert_eq!(core.limits.atom_cells, usize::MAX);
    assert_eq!(
        core.acceptance,
        dive_liveness::product::AcceptancePolicy::Buffers {
            bytes: usize::MAX,
            work: usize::MAX
        }
    );
    assert_eq!(core.limits.formula, defaults.formula);
    assert_eq!(core.limits.properties, defaults.properties);
    assert_eq!(
        legacy.limits.capture,
        dive_check::capture::Limits::default()
    );
    assert_eq!(legacy.limits.product.edges, defaults.product.edges);
    assert_eq!(legacy.limits.product.steps, defaults.product.steps);
    assert_eq!(legacy.limits.atom_cells, defaults.atom_cells);
    assert_eq!(
        legacy.acceptance,
        dive_liveness::product::AcceptancePolicy::compact_default()
    );
}

#[test]
fn injected_count_ceilings_refuse_but_the_budgeted_route_returns_a_validated_word() {
    use dive_liveness::product::AcceptancePolicy;
    for tlc in [false, true] {
        for workers in [1, 2, 8] {
            let (ir, config, mut options) = inputs(tlc);
            options.workers = workers;
            let base = crate::temporal::core_file_configuration();
            for owner in 0..10 {
                let mut limits = base;
                match owner {
                    0 => limits.limits.capture.states = 1,
                    1 => limits.limits.capture.edges = 1,
                    2 => limits.limits.capture.generated = 1,
                    3 => limits.limits.capture.successors_per_unit = 0,
                    4 => limits.limits.product.vertices = 0,
                    5 => limits.limits.product.edges = 0,
                    6 => limits.limits.product.steps = 0,
                    7 => {
                        limits.acceptance = AcceptancePolicy::Buffers {
                            bytes: 0,
                            work: usize::MAX,
                        }
                    }
                    8 => {
                        limits.acceptance = AcceptancePolicy::Buffers {
                            bytes: usize::MAX,
                            work: 0,
                        };
                    }
                    _ => limits.limits.atom_cells = 0,
                }
                let prepared = crate::core_engine::prepare(
                    &ir,
                    &config,
                    &options,
                    tlc,
                    dive_core::Opts::default(),
                    true,
                )
                .expect("prepared");
                let result = check_configured(
                    &ir,
                    &config,
                    &options,
                    (tlc, dive_core::Opts::default()),
                    prepared,
                    None,
                    &mut CoreRun::default(),
                    limits,
                );
                assert!(
                    matches!(result, CoreOutcome::Checked(ref r) if matches!(r.verdict, Verdict::Unsupported { .. })),
                    "owner={owner} w={workers} tlc={tlc}: {result:?}"
                );
            }
            let prepared = crate::core_engine::prepare(
                &ir,
                &config,
                &options,
                tlc,
                dive_core::Opts::default(),
                true,
            )
            .expect("prepared");
            let result = check_configured(
                &ir,
                &config,
                &options,
                (tlc, dive_core::Opts::default()),
                prepared,
                None,
                &mut CoreRun::default(),
                base,
            );
            let CoreOutcome::Temporal(report) = result else {
                panic!("budgeted actual route: {result:?}")
            };
            let crate::temporal::Outcome::NativeViolation(word) = report.outcome() else {
                panic!("actual validated word")
            };
            assert_eq!(word.evidence().stats().distinct, 4);
            assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
            assert_eq!(word.word().properties, ["P"]);
        }
    }
}

#[test]
fn planning_budget_stop_uses_the_detailed_core_temporal_failure() {
    for tlc in [false, true] {
        let (ir, config, mut options) = inputs(tlc);
        let prepared = crate::core_engine::prepare(
            &ir,
            &config,
            &options,
            tlc,
            dive_core::Opts::default(),
            true,
        )
        .expect("prepared");
        let watchdog = expired();
        options.budget = watchdog.budget().clone();
        let result = check(
            &ir,
            &config,
            &options,
            (tlc, dive_core::Opts::default()),
            prepared,
            None,
            &mut CoreRun::default(),
        );
        let CoreOutcome::TemporalFailure(failure) = result else {
            panic!("real budget stop: {result:?}")
        };
        assert!(matches!(
            failure.error(),
            crate::temporal::Error::Analysis(AnalysisError::Stopped(_))
        ));
        assert_eq!(
            failure.stage(),
            crate::temporal::Stage::Analysis(Stage::Planning)
        );
        assert_eq!(failure.capture_completed(), Some(false));
        assert!(failure.exploration().is_none());
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "actual final and periodic owner boundary controls"
)]
fn native_final_and_due_prefix_stops_keep_actual_analysis_phase_and_counts() {
    for tlc in [false, true] {
        for periodic in [false, true] {
            let (ir, config, mut options) = inputs(tlc);
            // Bind before arming to isolate an interruption in the requested
            // temporal read, rather than a machine-dependent binding duration.
            let Prepared {
                model,
                start,
                store,
                plan,
                ..
            } = crate::core_engine::prepare(
                &ir,
                &config,
                &options,
                tlc,
                dive_core::Opts::default(),
                true,
            )
            .expect("prepared");
            let watchdog = Budget::new(Some(Duration::from_millis(200)), None)
                .start()
                .expect("start")
                .expect("finite");
            options.budget = watchdog.budget().clone();
            let configuration = crate::temporal::core_file_configuration();
            let mut driver = GraphCheck::prepare(
                &model,
                &options,
                configuration.limits,
                configuration.acceptance,
                configuration.graph_labels,
                periodic.then_some(Schedule {
                    min_states: 1,
                    growth_factor: 1,
                }),
            )
            .expect("plan before expiry");
            let control = EvaluationControl::new(&options.budget, Phase::Capture).expect("control");
            let mut atoms = crate::core_native_atoms::NativeAtoms::prepare(
                &model,
                &store,
                &driver,
                &control,
                dive_core::Opts::default(),
            )
            .expect("native predicates before expiry");
            let mut callback =
                |level, graph: &NativeGraph, values: &mut NativeValues<'_>, stats| {
                    if !driver.native_level_due(stats) {
                        return Ok(false);
                    }
                    let csr = CsrCapture::checked(graph.input(), graph.limits(), &control)?;
                    let mut read = |row, control: &EvaluationControl| {
                        wait_and_read(values, row, graph.row(row)?, control, &options.budget)
                    };
                    Ok(driver.level_native(level, &csr, stats, &mut read))
                };
            let mut sink = NativeSink::new(
                &model,
                Arc::clone(&store),
                configuration.limits.capture,
                &options,
                Some(&mut callback),
            )
            .expect("sink");
            let mut out = dive_explore::explore_with(
                &plan,
                &store,
                dive_explore::Config {
                    opts: dive_core::Opts::default(),
                    max_states: None,
                    max_depth: None,
                    workers: 1,
                    warn: true,
                    sample: None,
                },
                &|| false,
                Some(&mut sink),
            );
            assert!(sink.take_error().is_none());
            let (graph, mut values) = sink.finish();
            let prefix = match out.verdict {
                dive_explore::Verdict::PrefixStop { level } => {
                    out.verdict = dive_explore::Verdict::Success;
                    Some(level)
                }
                dive_explore::Verdict::Success => None,
                _ => panic!("actual exploration: {out:?}"),
            };
            assert_eq!(prefix.is_some(), periodic);
            let safety = crate::core_engine::safety_result(
                &Search {
                    ir: &ir,
                    config: &config,
                    options: &options,
                    profile_tlc: tlc,
                    workers: 1,
                    explore_started: Instant::now(),
                },
                &model,
                &store,
                &plan,
                start,
                out,
            );
            let stats = safety.stats.expect("actual counters");
            let analysis = if periodic {
                // This must precede the expired capture-factory poll, preserving
                // the already observed Atoms/Temporal failure at the boundary.
                finish(
                    &graph,
                    &mut values,
                    (driver, &mut atoms),
                    safety,
                    prefix,
                    &options,
                    &control,
                )
                .expect("retained level failure")
            } else {
                let complete = CsrComplete::from_producer(
                    graph.input(),
                    safety,
                    options.clone(),
                    graph.limits(),
                    &control,
                )
                .expect("complete before expiry");
                let mut read = |row, control: &EvaluationControl| {
                    wait_and_read(&mut values, row, graph.row(row)?, control, &options.budget)
                };
                driver.finish_native(complete, &mut read).map_err(|f| *f)
            };
            let result = report_outcome(
                crate::temporal::core_native_report(
                    &ir,
                    &config,
                    &options,
                    analysis,
                    periodic,
                    model.symmetry_liveness().clone(),
                    Instant::now(),
                ),
                &options,
                &ir,
            );
            let CoreOutcome::TemporalFailure(failure) = result else {
                panic!("typed temporal stop: {result:?}")
            };
            assert_eq!(
                failure.stage(),
                crate::temporal::Stage::Analysis(Stage::Atoms)
            );
            assert_eq!(failure.capture_completed(), Some(!periodic));
            assert_eq!(
                failure.exploration().expect("actual producer facts").stats,
                stats
            );
            assert!(
                matches!(failure.error(), crate::temporal::Error::Analysis(AnalysisError::Stopped(stop)) if stop.phase == Phase::Temporal)
            );
        }
    }
}

fn wait_and_read(
    values: &mut NativeValues<'_>,
    row_id: usize,
    row: &[dive_core::Handle],
    control: &EvaluationControl,
    budget: &Budget,
) -> Result<dive_eval::next::StateVals, AnalysisError> {
    let start = Instant::now();
    while budget.poll().expect("poll").is_none() && start.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(1));
    }
    values.read(row_id, row, control)
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "actual full and committed-prefix factory controls"
)]
fn interrupted_capture_factories_keep_real_complete_and_prefix_statistics() {
    for prefix in [false, true] {
        let (ir, config, options) = inputs(false);
        let Prepared {
            model,
            start,
            store,
            plan,
            ..
        } = crate::core_engine::prepare(
            &ir,
            &config,
            &options,
            false,
            dive_core::Opts::default(),
            true,
        )
        .expect("prepared");
        let configuration = crate::temporal::core_file_configuration();
        let driver = GraphCheck::prepare(
            &model,
            &options,
            configuration.limits,
            configuration.acceptance,
            configuration.graph_labels,
            None,
        )
        .expect("plans");
        let atom_control =
            EvaluationControl::new(&options.budget, Phase::Capture).expect("control");
        let mut atoms = crate::core_native_atoms::NativeAtoms::prepare(
            &model,
            &store,
            &driver,
            &atom_control,
            dive_core::Opts::default(),
        )
        .expect("native predicates before expiry");
        let mut callback =
            |level, _: &NativeGraph, _: &mut NativeValues<'_>, _| Ok(prefix && level == 1);
        let mut sink = NativeSink::new(
            &model,
            Arc::clone(&store),
            configuration.limits.capture,
            &options,
            Some(&mut callback),
        )
        .expect("sink");
        let mut out = dive_explore::explore_with(
            &plan,
            &store,
            dive_explore::Config {
                opts: dive_core::Opts::default(),
                max_states: None,
                max_depth: None,
                workers: 1,
                warn: true,
                sample: None,
            },
            &|| false,
            Some(&mut sink),
        );
        assert!(sink.take_error().is_none());
        let (graph, mut values) = sink.finish();
        let level = match out.verdict {
            dive_explore::Verdict::Success => None,
            dive_explore::Verdict::PrefixStop { level } => {
                out.verdict = dive_explore::Verdict::Success;
                Some(level)
            }
            _ => panic!("actual producer"),
        };
        assert_eq!(level.is_some(), prefix);
        let safety = crate::core_engine::safety_result(
            &Search {
                ir: &ir,
                config: &config,
                options: &options,
                profile_tlc: false,
                workers: 1,
                explore_started: Instant::now(),
            },
            &model,
            &store,
            &plan,
            start,
            out,
        );
        let stats = safety.stats.expect("actual stats");
        assert_eq!(stats.left_on_queue > 0, prefix);
        let watchdog = expired();
        let control = EvaluationControl::new(watchdog.budget(), Phase::Capture).expect("control");
        let mut stopped_options = options.clone();
        stopped_options.budget = watchdog.budget().clone();
        let verdict = finish(
            &graph,
            &mut values,
            (driver, &mut atoms),
            safety.clone(),
            level,
            &stopped_options,
            &control,
        )
        .expect_err("factory poll stops before admission");
        assert!(
            matches!(verdict, Verdict::Stopped(ref stopped) if stopped.phase == Phase::Capture)
        );
        let failure = crate::temporal::core_capture_stop(verdict, &safety, !prefix, Instant::now());
        assert_eq!(
            failure.stage(),
            crate::temporal::Stage::Analysis(Stage::Capture)
        );
        assert_eq!(failure.capture_completed(), Some(!prefix));
        assert_eq!(failure.exploration().expect("actual boundary").stats, stats);
        assert_eq!(values.metrics()["read_rows"], 0);
    }
}
