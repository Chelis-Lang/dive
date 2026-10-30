//! Temporal properties on the core engine
//! (`docs/developer/core/11-liveness.md` §6).
//!
//! The core replaces only the producer: it explores and reports its
//! native rows and CSR ([`crate::core_native_graph::NativeSink`]), and the reference engine's
//! own planning, completed-level and final checks
//! ([`dive_liveness::analyze::periodic::external::GraphCheck`]), validation
//! and report finish the run. A safety ending is reported as the safety
//! route reports it. What the core does not reproduce exactly is refused,
//! so that `--engine auto` runs the reference engine: the reference
//! capture's own refusals, the options its route excludes, and, for now,
//! any temporal failure (planning, capture limits, evaluation errors,
//! validation), whose reporting the core does not take over yet.

use std::sync::Arc;
use std::time::Instant;

use dive_check::budget::{EvaluationControl, Phase};
use dive_check::capture::{CsrCapture, CsrComplete, CsrPrefix};
use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_ir::Ir;
use dive_liveness::analyze::periodic::Schedule;
use dive_liveness::analyze::periodic::external::{GraphCheck, NativeAnalysis};

use crate::core_engine::{CoreOutcome, CoreRun, Prepared, Search};
use crate::core_native_graph::{NativeGraph, NativeSink};
use crate::core_native_values::NativeValues;

/// How a temporal run's checks are scheduled: `Some` for the default
/// profile's completed-level checks, `None` for the final check only
/// (`--compat=tlc`).
pub(crate) type Route = Option<Schedule>;

fn refused(feature: impl Into<String>, options: &CheckOptions, ir: &Ir) -> CoreOutcome {
    CoreOutcome::Checked(Box::new(crate::core_engine::result(
        Verdict::Unsupported {
            feature: feature.into(),
        },
        options,
        ir,
    )))
}

/// Why the core does not run this temporal check before its search.
fn static_refusal(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    prepared: &Prepared,
) -> Option<String> {
    if let Some(reason) = dive_check::final_roles::route_exclusion(ir, config, options) {
        return Some(format!("temporal properties with {reason} (core)"));
    }
    if let Some(reason) =
        dive_check::capture::refusal(&prepared.model, options, options.workers > 1)
    {
        return Some(format!("temporal properties: {reason} (core)"));
    }
    if prepared.plan.effects.prints {
        return Some("Print or PrintT with temporal properties (core)".into());
    }
    if prepared.plan.effects.draws {
        return Some("RandomElement with temporal properties (core)".into());
    }
    if prepared.plan.effects.registers {
        return Some("TLCSet or TLCGet with temporal properties (core)".into());
    }
    if options.max_states.is_some() || options.max_depth.is_some() {
        return Some("--max-states or --max-depth with temporal properties (core)".into());
    }
    None
}

/// Runs the temporal check of a prepared model on the core.
#[allow(
    clippy::too_many_lines,
    reason = "the stages of a temporal core run, in order"
)]
pub(crate) fn check(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    (profile_tlc, core_opts): (bool, dive_core::Opts),
    prepared: Prepared,
    route: Route,
    run: &mut CoreRun,
) -> CoreOutcome {
    check_configured(
        ir,
        config,
        options,
        (profile_tlc, core_opts),
        prepared,
        route,
        run,
        crate::temporal::core_file_configuration(),
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one producer route with injectable exact caller resource controls"
)]
fn check_configured(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    (profile_tlc, core_opts): (bool, dive_core::Opts),
    prepared: Prepared,
    route: Route,
    run: &mut CoreRun,
    configuration: crate::temporal::Configuration,
) -> CoreOutcome {
    let started = Instant::now();
    if let Some(feature) = static_refusal(ir, config, options, &prepared) {
        return refused(feature, options, ir);
    }
    let Prepared {
        model,
        host: _,
        start,
        store,
        plan,
        compile_time,
    } = prepared;
    let mut graph_check = match GraphCheck::prepare(
        &model,
        options,
        configuration.limits,
        configuration.acceptance,
        configuration.graph_labels,
        route,
    ) {
        Ok(check) => check,
        Err(failure) => {
            return report_outcome(
                crate::temporal::core_native_report(
                    ir,
                    config,
                    options,
                    Err(*failure),
                    route.is_some(),
                    model.symmetry_liveness().clone(),
                    started,
                ),
                options,
                ir,
            );
        }
    };
    let workers = options.workers.max(1);
    let budget = &options.budget;
    let poll = || !matches!(budget.poll(), Ok(None));
    let explore_started = Instant::now();
    let control = match EvaluationControl::new(budget, Phase::Capture) {
        Ok(control) => control,
        Err(error) => return internal(&error.to_string(), options, ir),
    };
    let mut on_level = |level: u64, graph: &NativeGraph, values: &mut NativeValues<'_>, stats| {
        // Final-only and not-yet-due levels do not scan any retained CSR.
        if !graph_check.native_level_due(stats) {
            return Ok(false);
        }
        let csr = CsrCapture::checked(graph.input(), graph.limits(), &control)?;
        let mut read = |row, control: &EvaluationControl| values.read(graph.row(row)?, control);
        Ok(graph_check.level_native(level, &csr, stats, &mut read))
    };
    let mut sink = match NativeSink::new(
        &model,
        Arc::clone(&store),
        configuration.limits.capture,
        options,
        Some(&mut on_level),
    ) {
        Ok(sink) => sink,
        Err(verdict @ (Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. })) => {
            let mut safety = crate::core_engine::result(verdict.clone(), options, ir);
            safety.symmetry_liveness = Some(model.symmetry_liveness().clone());
            return CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_capture_stop(
                verdict, &safety, false, started,
            )));
        }
        Err(Verdict::InternalError { message }) => return internal(&message, options, ir),
        Err(reason) => {
            return refused(
                format!("the behavior graph on the core stopped: {reason:?}"),
                options,
                ir,
            );
        }
    };
    run.searched = true;
    let mut out = dive_explore::explore_with(
        &plan,
        &store,
        dive_explore::Config {
            opts: core_opts,
            max_states: None,
            max_depth: None,
            workers,
            warn: options.eval.warnings,
            sample: None,
        },
        &poll,
        Some(&mut sink),
    );
    let graph_error = sink.take_error();
    let (graph, mut values) = sink.finish();
    run.metrics = crate::core_engine::metrics_json(&plan, &out.metrics, compile_time);
    let prefix = match out.verdict {
        dive_explore::Verdict::PrefixStop { level } => {
            // The successful committed-prefix factory checks its own actual
            // queue/shape facts; this cannot create completed EOF evidence.
            out.verdict = dive_explore::Verdict::Success;
            Some(level)
        }
        _ => None,
    };
    let graph_stopped = out.verdict == dive_explore::Verdict::GraphStop;
    if graph_stopped
        && matches!(
            &graph_error,
            Some(Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. })
        )
    {
        // The sink's owned control actually observed this run's exhausted or
        // unmeasurable budget. Use the ordinary budget report path for real
        // counters/output/worker metadata, then retain its exact capture cause.
        out.verdict = dive_explore::Verdict::Budget;
    }
    let search = Search {
        ir,
        config,
        options,
        profile_tlc,
        workers,
        explore_started,
    };
    let producer_stats = out.stats;
    let mut safety = crate::core_engine::safety_result(&search, &model, &store, &plan, start, out);
    if matches!(safety.verdict, Verdict::BudgetMeasurement { .. }) {
        safety.stats = Some(dive_check::Stats {
            generated: producer_stats.generated,
            distinct: producer_stats.distinct,
            left_on_queue: producer_stats.left_on_queue,
            depth: producer_stats.depth,
            initial_generated: producer_stats.initial_generated,
        });
    }
    if graph_stopped {
        match graph_error {
            Some(verdict @ (Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. })) => {
                safety.verdict = verdict;
            }
            Some(Verdict::InternalError { message }) => {
                record_metrics(run, &graph, &values);
                return internal(&message, options, ir);
            }
            reason => {
                record_metrics(run, &graph, &values);
                let reason = reason.map_or_else(|| "no reason".to_owned(), |v| format!("{v:?}"));
                return refused(
                    format!("the behavior graph on the core stopped: {reason}"),
                    options,
                    ir,
                );
            }
        }
    }
    if let Verdict::Stopped(stopped) = &mut safety.verdict {
        stopped.phase = Phase::Capture;
    }
    if safety.verdict != Verdict::Success {
        record_metrics(run, &graph, &values);
        safety.symmetry_liveness = Some(model.symmetry_liveness().clone());
        if matches!(
            safety.verdict,
            Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. }
        ) {
            return CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_capture_stop(
                safety.verdict.clone(),
                &safety,
                false,
                started,
            )));
        }
        // A safety ending, a stop or a refusal: reported as the reference
        // engine's temporal route reports it, never a temporal verdict, with
        // the bound model's symmetry resolution.
        safety.symmetry_liveness = Some(model.symmetry_liveness().clone());
        return CoreOutcome::Checked(Box::new(safety));
    }
    let captured_safety = safety.clone();
    let analysis = finish(
        &graph,
        &mut values,
        graph_check,
        safety,
        prefix,
        options,
        &control,
    );
    record_metrics(run, &graph, &values);
    let analysis = match analysis {
        Ok(a) => a,
        Err(verdict @ (Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. })) => {
            return CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_capture_stop(
                verdict,
                &captured_safety,
                prefix.is_none(),
                started,
            )));
        }
        Err(Verdict::InternalError { message }) => return internal(&message, options, ir),
        Err(v) => {
            return refused(
                format!("the behavior graph on the core: {v:?}"),
                options,
                ir,
            );
        }
    };
    if analysis.as_ref().is_err_and(|failure| {
        !matches!(
            failure.failure.failure().error,
            dive_liveness::analyze::AnalysisError::Stopped(_)
                | dive_liveness::analyze::AnalysisError::BudgetMeasurement(_)
                | dive_liveness::analyze::AnalysisError::BudgetStartup(_)
        )
    }) {
        return refused(
            "a temporal failure (core): its report is the reference engine's for now",
            options,
            ir,
        );
    }
    // Analysis owns only the selected converted word and sealed facts now.
    // Release the native capture/store before fresh independent rebinding.
    drop(values);
    drop(graph);
    drop(plan);
    drop(store);
    report_outcome(
        crate::temporal::core_native_report(
            ir,
            config,
            options,
            analysis,
            route.is_some(),
            model.symmetry_liveness().clone(),
            started,
        ),
        options,
        ir,
    )
}

fn report_outcome(
    result: Result<crate::temporal::Report, crate::temporal::Failure>,
    options: &CheckOptions,
    ir: &Ir,
) -> CoreOutcome {
    match result {
        Ok(report) => match report.outcome() {
            crate::temporal::Outcome::Safety(result) => CoreOutcome::Checked(result.clone()),
            _ => CoreOutcome::Temporal(Box::new(report)),
        },
        Err(failure)
            if matches!(
                failure.error(),
                crate::temporal::Error::Analysis(
                    dive_liveness::analyze::AnalysisError::Stopped(_)
                        | dive_liveness::analyze::AnalysisError::BudgetMeasurement(_)
                        | dive_liveness::analyze::AnalysisError::BudgetStartup(_)
                )
            ) =>
        {
            CoreOutcome::TemporalFailure(Box::new(failure))
        }
        Err(_) => refused(
            "a temporal validation failure (core): its report is the reference engine's for now",
            options,
            ir,
        ),
    }
}

type Analysis = Result<
    (NativeAnalysis, dive_liveness::analyze::periodic::Checks),
    dive_liveness::analyze::periodic::PeriodicFailure,
>;

/// Completes the graph and runs the final check, or finishes the prefix
/// the level checks stopped at.
fn finish(
    graph: &NativeGraph,
    values: &mut NativeValues<'_>,
    mut graph_check: GraphCheck<'_>,
    safety: CheckResult,
    prefix: Option<u64>,
    options: &CheckOptions,
    control: &EvaluationControl,
) -> Result<Analysis, Verdict> {
    if prefix.is_some()
        && let Some(failure) = graph_check.take_native_prefix_failure(&safety)
    {
        return Ok(Err(*failure));
    }
    let mut read = |row, control: &EvaluationControl| values.read(graph.row(row)?, control);
    if let Some(level) = prefix {
        let prefix = CsrPrefix::from_producer(
            graph.input(),
            level,
            safety,
            options.clone(),
            graph.limits(),
            control,
        )?;
        Ok(graph_check
            .finish_native_prefix(prefix, &mut read)
            .map_err(|f| *f))
    } else {
        let complete = CsrComplete::from_producer(
            graph.input(),
            safety,
            options.clone(),
            graph.limits(),
            control,
        )?;
        Ok(graph_check
            .finish_native(complete, &mut read)
            .map_err(|f| *f))
    }
}

fn record_metrics(run: &mut CoreRun, graph: &NativeGraph, values: &NativeValues<'_>) {
    if let Some(metrics) = run.metrics.as_object_mut() {
        let memory = graph.memory();
        let conversion = values.metrics();
        metrics.insert(
            "liveness".into(),
            serde_json::json!({
                "storage": "native-csr", "graph_states": graph.state_count(),
                "convert_ms": conversion["convert_ms"],
                "converted_nodes": conversion["converted_values"],
                "memo_entries": conversion["cache_entries"],
                "values": conversion,
                "memory": {
                    "fixed_bytes": memory.fixed_bytes,
                    "buffer_bytes": memory.buffer_bytes,
                    "growth_overlap_upper_bound_bytes": memory.growth_overlap_upper_bound_bytes,
                    "capture_key_entries": memory.key_entries,
                    "capture_key_capacity": memory.key_capacity,
                    "key_index_retained_after_capture": false,
                    "accounting_complete": false,
                    "scope": "native collector buffers and action-label capacities; reserve overlap is an upper bound, not observed RSS; capture key index released before final analysis; opaque hash/allocator/Arc storage, Store, explorer frontiers/waves and transient conversion/analysis storage require separate accounting; no capacity or peak-memory claim",
                },
            }),
        );
    }
}

fn internal(message: &str, options: &CheckOptions, ir: &Ir) -> CoreOutcome {
    CoreOutcome::Checked(Box::new(crate::core_engine::result(
        Verdict::InternalError {
            message: format!("core temporal graph: {message}"),
        },
        options,
        ir,
    )))
}

#[cfg(test)]
#[path = "core_temporal_resource_tests.rs"]
mod resource_tests;
