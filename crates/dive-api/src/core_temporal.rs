//! Temporal properties on Core.
//!
//! Core explores and reports its native rows and CSR
//! ([`crate::core_native_graph::NativeSink`]); `dive-liveness`'s planning,
//! completed-level and final checks
//! ([`dive_liveness::analyze::periodic::external::GraphCheck`]), validation
//! and report finish the run. A safety ending is reported as the safety
//! route reports it. What Core does not reproduce exactly is refused by
//! name: `dive_runtime::capture`'s refusals, the options its route
//! excludes, other behavior-graph stops, and temporal failures whose report
//! Core does not build (planning, evaluation and validation failures).
//! Budget and resource stops in temporal analysis are reported as stops.

use std::sync::Arc;
use std::time::Instant;

use dive_ir::Ir;
use dive_liveness::analyze::periodic::Schedule;
use dive_liveness::analyze::periodic::external::{GraphCheck, NativeAnalysis};
use dive_runtime::budget::{EvaluationControl, Phase};
use dive_runtime::capture::{CsrCapture, CsrComplete, CsrPrefix};
use dive_runtime::{CheckOptions, CheckResult, Verdict};

use crate::core_engine::{CoreOutcome, CoreRun, Prepared, Search};
use crate::core_native_graph::{NativeGraph, NativeSink};
use crate::core_native_values::NativeValues;

/// The completed-level schedule and which properties it searches. `None`
/// selects a final check only.
pub(crate) type Route = Option<(Schedule, crate::TemporalMode)>;

/// The refusal of a temporal validation failure, whose report Core does not
/// build (TLC's start-up and planning errors among them).
pub(crate) const VALIDATION_FAILURE: &str =
    "a temporal validation failure (core): its report is not implemented on Core";

fn refused(feature: impl Into<String>, options: &CheckOptions, ir: &Ir) -> CoreOutcome {
    CoreOutcome::Checked(Box::new(crate::core_engine::result(
        Verdict::Unsupported {
            feature: feature.into(),
        },
        options,
        ir,
    )))
}

/// Actual privately bound native and reference domains, never a source-name
/// projection. Prepared is constructed only from IR/config via fresh binding;
/// it accepts no external bound model, value provider or callable.
pub(crate) fn fixed_name_domain(prepared: &Prepared) -> bool {
    let model = &prepared.model;
    let strings = prepared.store.strings();
    let count = model.evaluator.startup_names();
    strings.is_sealed()
        && strings.order() == dive_core::strings::StringOrder::Token
        && strings.len() == count
        && model.names.len() == count
        && model.validate_context().is_ok()
        && (0..count).all(|i| {
            u32::try_from(i).is_ok_and(|token| {
                matches!((strings.text(token), model.names.resolve(token)),
                    (Some(text), Some(atom)) if text.as_ref() == atom.as_str())
            })
        })
}

/// How a temporal capture is scheduled, decided before the search from the
/// bound model alone, so that no parallel capture is started and undone.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CaptureWidth {
    /// The workers the capture uses.
    pub(crate) workers: usize,
    /// Whether the capture runs on the concrete pool, whose admission checks
    /// [`dive_runtime::capture::refusal`] then applies.
    pub(crate) pool: bool,
    /// Why one worker captures although more were requested.
    pub(crate) one_worker: Option<String>,
}

/// The width of this temporal capture.
///
/// A model whose effects or runtime names keep its capture off the concrete
/// pool ([`dive_runtime::capture::pool_sequential`]) is captured inline with
/// one worker, exactly as `--workers 1` captures it, instead of being refused.
/// Runtime names in a proved fixed startup domain keep the requested width
/// off the pool.
pub(crate) fn capture_width(options: &CheckOptions, prepared: &Prepared) -> CaptureWidth {
    let requested = options.workers.max(1);
    let model = &prepared.model;
    if requested == 1
        || (fixed_name_domain(prepared) && dive_runtime::capture::runtime_names_only(model))
    {
        return CaptureWidth {
            workers: requested,
            pool: false,
            one_worker: None,
        };
    }
    if !dive_runtime::capture::pool_sequential(model) {
        return CaptureWidth {
            workers: requested,
            pool: true,
            one_worker: None,
        };
    }
    let what = if model.sequential.is_empty() {
        "start-up or configured effects".to_owned()
    } else {
        model.sequential.join("; ")
    };
    CaptureWidth {
        workers: 1,
        pool: false,
        one_worker: Some(format!(
            "the temporal capture has effects or runtime names ({what}): the core captures the behavior graph in order with one worker"
        )),
    }
}

/// Why Core does not run this temporal check before its search.
pub(crate) fn static_refusal(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    prepared: &Prepared,
) -> Option<String> {
    if let Some(reason) = dive_runtime::admission::route_exclusion(ir, config, options) {
        return Some(format!("temporal properties with {reason} (core)"));
    }
    let width = capture_width(options, prepared);
    // A requested width captured with one worker keeps (and extends to every
    // predicate scope) the pool's context check, so the fallback never admits
    // a model the pool would refuse as foreign.
    if width.one_worker.is_some() && prepared.model.validate_context().is_err() {
        return Some(
            "temporal properties: concrete pool capture foreign IR or name table (core)".into(),
        );
    }
    if let Some(reason) = dive_runtime::capture::refusal(&prepared.model, options, width.pool) {
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
    None
}

/// Runs the temporal check of a prepared model on Core.
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
    reason = "same owned temporal-operation boundary"
)]
fn check_configured(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    profile: (bool, dive_core::Opts),
    prepared: Prepared,
    route: Route,
    run: &mut CoreRun,
    configuration: crate::temporal::Configuration,
) -> CoreOutcome {
    // Hold the exact reference Arc used by GraphCheck and its model's scopes.
    // It is append-only; no successful branch can hide an intervening new token.
    let names = Arc::clone(&prepared.model.names);
    let guarded = fixed_name_domain(&prepared);
    let count = names.len();
    let outcome = check_configured_inner(
        ir,
        config,
        options,
        profile,
        prepared,
        route,
        run,
        configuration,
    );
    guard_outcome(guarded, &names, count, outcome, options, ir)
}

fn guard_outcome(
    guarded: bool,
    names: &dive_value::Interner,
    count: usize,
    outcome: CoreOutcome,
    options: &CheckOptions,
    ir: &Ir,
) -> CoreOutcome {
    if !guarded || names.len() == count {
        return outcome;
    }
    match &outcome {
        CoreOutcome::TemporalFailure(_) => outcome,
        CoreOutcome::Checked(result)
            if matches!(
                result.verdict,
                Verdict::Stopped(_)
                    | Verdict::ResourceLimit { .. }
                    | Verdict::BudgetMeasurement { .. }
                    | Verdict::InternalError { .. }
            ) =>
        {
            outcome
        }
        _ => refused(
            "temporal reference evaluation created a name outside the proved startup domain (core)",
            options,
            ir,
        ),
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one producer route with injectable exact caller resource controls"
)]
fn check_configured_inner(
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
    let CaptureWidth {
        workers,
        one_worker,
        ..
    } = capture_width(options, &prepared);
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
        route.map(|(schedule, _)| schedule),
    ) {
        Ok(check) if route.is_some_and(|(_, mode)| mode == crate::TemporalMode::PeriodicAll) => {
            check.with_all_prefix_properties()
        }
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
    let budget = &options.budget;
    let poll = || !matches!(budget.poll(), Ok(None));
    let explore_started = Instant::now();
    let control = match EvaluationControl::new(budget, Phase::Capture) {
        Ok(control) => control,
        Err(error) => return internal(&error.to_string(), options, ir),
    };
    let source_setup = match graph_check.source_rows_possible() {
        Ok(true) => Some(crate::core_native_graph::SourceRowSetup {
            ir: ir.clone(),
            config: config.clone(),
            options: options.clone(),
            limits: configuration.limits,
            graph_labels: configuration.graph_labels,
        }),
        Ok(false) => None,
        Err(error) => {
            let verdict = crate::core_native_graph::source_error(error);
            let safety = crate::core_engine::result(verdict.clone(), options, ir);
            return CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_capture_stop(
                verdict,
                &safety,
                false,
                started,
                graph_check.periodic_checks(),
            )));
        }
    };
    let mut atoms = match crate::core_native_atoms::NativeAtoms::prepare(
        &model,
        &store,
        &graph_check,
        &control,
        core_opts,
    ) {
        Ok(atoms) => atoms,
        Err(verdict) => {
            let safety = crate::core_engine::result(verdict.clone(), options, ir);
            return CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_capture_stop(
                verdict,
                &safety,
                false,
                started,
                graph_check.periodic_checks(),
            )));
        }
    };
    // With every atom native, values are read only for an actual fallback,
    // source-row check or report; admission waits for that read.
    if !start.has_diagnostics() {
        atoms.enable_live_emissions(&model, options);
    }
    let defer_admission = atoms.all_native();
    let mut on_level = |level: u64, graph: &NativeGraph, values: &mut NativeValues<'_>, stats| {
        // Final-only and not-yet-due levels do not scan any retained CSR.
        if !graph_check.native_level_due(stats) {
            return Ok(false);
        }
        let csr = CsrCapture::checked_targets(graph.input(), graph.limits(), &control)?;
        let mut read = crate::core_native_atoms::Reader::new(graph, values, &mut atoms);
        read.live_prefix(true);
        read.live_emission_capture(&csr);
        Ok(graph_check.level_native_with_reader(level, &csr, stats, &mut read))
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
                verdict,
                &safety,
                false,
                started,
                graph_check.periodic_checks(),
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
    if defer_admission {
        sink.defer_admission();
    }
    let exact = options.store == dive_store::StoreMode::Exact;
    if exact {
        sink.use_exact_identities();
    }
    if let Some(setup) = source_setup {
        sink.prepare_source_rows(setup);
    }
    run.searched = true;
    // Exact storage keeps full state identities, so equal 64-bit keys of
    // distinct states stay distinct vertices (ADR 0158).
    let mut out = dive_explore::explore_with_storage(
        &plan,
        &store,
        dive_explore::Config {
            opts: core_opts,
            // A ceiling stops capture before the state beyond it: the run
            // reports `stopped` in graph capture, never a holding result
            // (ADR 0158).
            max_states: options.max_states,
            max_depth: options.max_depth,
            workers,
            warn: options.eval.warnings,
            sample: None,
        },
        &poll,
        Some(&mut sink),
        Some(&crate::core_engine::search_work_control(budget)),
        &dive_explore::storage::Storage {
            exact,
            ..dive_explore::storage::Storage::default()
        },
    );
    let graph_error = sink.take_error();
    let source_error = sink.take_source_error();
    let source_rows = sink.take_source_rows();
    let (graph, mut values) = sink.finish();
    if let Some(message) = atoms.audit_mismatch() {
        return internal(message, options, ir);
    }
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
    let source_trace = source_rows
        .as_ref()
        .filter(|rows| rows.has_error())
        .and(out.trace.clone());
    let mut safety = crate::core_engine::safety_result(&search, &model, &store, &plan, start, out);
    if let Some(reason) = one_worker
        && safety.workers.used == 1
        && safety.workers.reason.is_none()
    {
        safety.workers.reason = Some(reason);
    }
    if let Some(rows) =
        source_rows.filter(dive_liveness::analyze::owned::SourceRowDriver::has_error)
    {
        if !matches!(&graph_error, Some(Verdict::Unsupported { feature }) if feature == crate::core_native_graph::SOURCE_ROW_STOP)
        {
            return internal("native source row lost its actual graph stop", options, ir);
        }
        let word = match source_trace {
            Some(Ok(trace)) => match crate::core_engine::counterexample(&model, &store, &trace) {
                Ok(word) => word,
                Err(error) => return internal(&error, options, ir),
            },
            Some(Err(error)) => return internal(&error, options, ir),
            None => {
                return internal(
                    "native source row has no reconstructed source word",
                    options,
                    ir,
                );
            }
        };
        safety.verdict = Verdict::Unsupported {
            feature: crate::core_native_graph::SOURCE_ROW_STOP.into(),
        };
        safety.stats = Some(dive_runtime::Stats {
            generated: producer_stats.generated,
            distinct: producer_stats.distinct,
            depth: producer_stats.depth,
            left_on_queue: producer_stats.left_on_queue,
            initial_generated: producer_stats.initial_generated,
        });
        safety.counterexample = Some(word);
        let state = match rows.finish(&safety) {
            Ok(Some(state)) => state,
            Ok(None) => return internal("native source row lost its sealed evidence", options, ir),
            Err(error) => return internal(&error.to_string(), options, ir),
        };
        let failure = crate::temporal::core_source_row_failure(
            ir,
            config,
            options,
            state,
            &safety,
            graph_check.periodic_checks(),
            started,
        );
        record_metrics(run, &graph, &values, &atoms);
        return CoreOutcome::TemporalFailure(Box::new(failure));
    }
    if let Some(error) = source_error {
        safety.verdict = error.clone();
        safety.stats = Some(dive_runtime::Stats {
            generated: producer_stats.generated,
            distinct: producer_stats.distinct,
            depth: producer_stats.depth,
            left_on_queue: producer_stats.left_on_queue,
            initial_generated: producer_stats.initial_generated,
        });
        let failure = crate::temporal::core_capture_stop(
            error,
            &safety,
            false,
            started,
            graph_check.periodic_checks(),
        );
        record_metrics(run, &graph, &values, &atoms);
        return CoreOutcome::TemporalFailure(Box::new(failure));
    }
    if matches!(safety.verdict, Verdict::BudgetMeasurement { .. }) {
        safety.stats = Some(dive_runtime::Stats {
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
                record_metrics(run, &graph, &values, &atoms);
                return internal(&message, options, ir);
            }
            reason => {
                record_metrics(run, &graph, &values, &atoms);
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
    dive_runtime::capture::refuse_unproved_temporal_next_error(&model, &mut safety);
    if safety.verdict != Verdict::Success {
        record_metrics(run, &graph, &values, &atoms);
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
                graph_check.periodic_checks(),
            )));
        }
        // A safety ending, a stop or a refusal: reported as a safety
        // result, never a temporal verdict, with
        // the bound model's symmetry resolution.
        safety.symmetry_liveness = Some(model.symmetry_liveness().clone());
        return CoreOutcome::Checked(Box::new(safety));
    }
    let captured_safety = safety.clone();
    let checks = graph_check.periodic_checks();
    let analysis = finish(
        &graph,
        &mut values,
        (graph_check, &mut atoms),
        safety,
        prefix,
        options,
        &control,
    );
    record_metrics(run, &graph, &values, &atoms);
    if let Some(message) = atoms.audit_mismatch() {
        return internal(message, options, ir);
    }
    let analysis = match analysis {
        Ok(a) => a,
        Err(verdict @ (Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. })) => {
            return CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_capture_stop(
                verdict,
                &captured_safety,
                prefix.is_none(),
                started,
                checks,
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
        !report_analysis_error(
            &failure.failure.failure().error,
            failure.failure.state_evaluation(),
        )
    }) {
        return refused(
            "a temporal failure (core): its report is not implemented on Core",
            options,
            ir,
        );
    }
    // Analysis owns only the selected converted word and sealed facts now.
    // Release the native capture/store before fresh independent rebinding.
    drop(atoms);
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

/// Stops retain their budget authority. A failed state label explains a
/// refusal without establishing the first error demanded by TLC.
fn report_analysis_error(
    error: &dive_liveness::analyze::AnalysisError,
    state: Option<&dive_liveness::analyze::StateEvaluation>,
) -> bool {
    use dive_liveness::analyze::{AnalysisError, StateEvaluationSite};
    match error {
        AnalysisError::Stopped(_)
        | AnalysisError::BudgetMeasurement(_)
        | AnalysisError::BudgetStartup(_) => true,
        AnalysisError::Atom { .. } => state.is_some_and(|state| {
            matches!(
                state.site(),
                StateEvaluationSite::FinalLabels | StateEvaluationSite::PeriodicLabels
            ) && state.cause().class() == dive_eval::ErrorClass::Eval
        }),
        _ => false,
    }
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
            if matches!(failure.error(), crate::temporal::Error::Analysis(error)
                if report_analysis_error(error, failure.state_evaluation())) =>
        {
            CoreOutcome::TemporalFailure(Box::new(failure))
        }
        Err(failure) => match failure.error() {
            crate::temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Planning(
                planning @ dive_liveness::plan::PlanningError::Formula {
                    error: dive_liveness::formula::FormulaError::Limit(_),
                    ..
                },
            )) => refused(planning.to_string(), options, ir),
            crate::temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Tableau(
                limit @ dive_liveness::formula::FormulaError::Limit(_),
            )) => refused(limit.to_string(), options, ir),
            _ => refused(VALIDATION_FAILURE, options, ir),
        },
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
    driver: (GraphCheck<'_>, &mut crate::core_native_atoms::NativeAtoms),
    safety: CheckResult,
    prefix: Option<u64>,
    options: &CheckOptions,
    control: &EvaluationControl,
) -> Result<Analysis, Verdict> {
    let (mut graph_check, atoms) = driver;
    if prefix.is_some()
        && let Some(failure) = graph_check.take_native_prefix_failure(&safety)
    {
        return Ok(Err(*failure));
    }
    let mut read = crate::core_native_atoms::Reader::new(graph, values, atoms);
    read.live_prefix(false);
    if let Some(level) = prefix {
        let mut read =
            |row, control: &EvaluationControl| read.values.read_retained(graph, row, control);
        let prefix = CsrPrefix::from_target_producer(
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
        let complete = CsrComplete::from_target_producer(
            graph.input(),
            safety,
            options.clone(),
            graph.limits(),
            control,
        )?;
        read.live_emission_capture(complete.graph());
        Ok(graph_check
            .finish_native_with_reader(complete, &mut read)
            .map_err(|f| *f))
    }
}

fn record_metrics(
    run: &mut CoreRun,
    graph: &NativeGraph,
    values: &NativeValues<'_>,
    atoms: &crate::core_native_atoms::NativeAtoms,
) {
    if let Some(metrics) = run.metrics.as_object_mut() {
        let memory = graph.memory();
        let conversion = values.metrics();
        metrics.insert(
            "liveness".into(),
            serde_json::json!({
                "storage": "native-csr", "graph_states": graph.state_count(),
                "retained_rows": graph.input().retained_rows,
                "convert_ms": conversion["convert_ms"],
                "converted_nodes": conversion["converted_values"],
                "memo_entries": conversion["cache_entries"],
                "values": conversion, "atoms": atoms.metrics(),
                "analysis_ms": atoms.analysis_ms(),
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

#[cfg(test)]
#[path = "core_temporal_name_tests.rs"]
mod name_tests;
