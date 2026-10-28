//! Temporal properties on the core engine
//! (`docs/developer/core/11-liveness.md` §6).
//!
//! The core replaces only the producer: it explores and reports its
//! behavior graph ([`crate::core_graph::Graph`]), and the reference engine's
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

use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_ir::Ir;
use dive_liveness::analyze::periodic::Schedule;
use dive_liveness::analyze::periodic::external::GraphCheck;

use crate::core_engine::{CoreOutcome, CoreRun, Prepared, Search};

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
    let started = Instant::now();
    if let Some(feature) = static_refusal(ir, config, options, &prepared) {
        return refused(feature, options, ir);
    }
    let configuration = crate::temporal::file_configuration();
    let Prepared {
        model,
        host: _,
        start,
        store,
        plan,
        compile_time,
    } = prepared;
    let Ok(mut graph_check) = GraphCheck::prepare(
        &model,
        options,
        configuration.limits,
        configuration.acceptance,
        configuration.graph_labels,
        route,
    ) else {
        return refused("a temporal planning failure (core)", options, ir);
    };
    let workers = options.workers.max(1);
    let budget = &options.budget;
    let poll = || !matches!(budget.poll(), Ok(None));
    let explore_started = Instant::now();
    run.searched = true;
    let mut on_level = |level: u64, graph: &dive_check::capture::BehaviorGraph, stats| {
        graph_check.level(level, graph, stats)
    };
    let mut graph =
        crate::core_graph::Graph::new(&model, configuration.limits.capture, Some(&mut on_level));
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
        Some(&mut graph),
    );
    let graph_error = graph.take_error();
    let (builder, graph_metrics) = graph.finish();
    run.metrics = crate::core_engine::metrics_json(&plan, &out.metrics, compile_time);
    if let Some(m) = run.metrics.as_object_mut() {
        m.insert(
            "liveness".into(),
            serde_json::json!({
                "convert_ms": graph_metrics.convert.as_secs_f64() * 1e3,
                "converted_nodes": graph_metrics.converted,
                "memo_entries": graph_metrics.memo_entries,
                "graph_states": builder.graph().states.len(),
            }),
        );
    }
    let prefix = match out.verdict {
        dive_explore::Verdict::GraphStop => {
            let reason = graph_error.map_or_else(|| "no reason".to_owned(), |v| format!("{v:?}"));
            return refused(
                format!("the behavior graph on the core stopped: {reason}"),
                options,
                ir,
            );
        }
        dive_explore::Verdict::PrefixStop { level } => {
            out.verdict = dive_explore::Verdict::Success;
            Some(level)
        }
        _ => None,
    };
    let search = Search {
        ir,
        config,
        options,
        profile_tlc,
        workers,
        explore_started,
    };
    let mut safety = crate::core_engine::safety_result(&search, &model, &store, &plan, start, out);
    if safety.verdict != Verdict::Success {
        // A safety ending, a stop or a refusal: reported as the reference
        // engine's temporal route reports it, never a temporal verdict, with
        // the bound model's symmetry resolution.
        safety.symmetry_liveness = Some(model.symmetry_liveness().clone());
        return CoreOutcome::Checked(Box::new(safety));
    }
    let analysis = finish(builder, graph_check, safety, prefix, options);
    let analysis = match analysis {
        Ok(a) => a,
        Err(Verdict::InternalError { message }) => return internal(&message, options, ir),
        Err(v) => {
            return refused(
                format!("the behavior graph on the core: {v:?}"),
                options,
                ir,
            );
        }
    };
    if analysis.is_err() {
        return refused(
            "a temporal failure (core): its report is the reference engine's for now",
            options,
            ir,
        );
    }
    match crate::temporal::core_report(
        ir,
        config,
        options,
        analysis,
        route.is_some(),
        model.symmetry_liveness().clone(),
        started,
    ) {
        Ok(report) => match report.outcome() {
            crate::temporal::Outcome::Safety(result) => CoreOutcome::Checked(result.clone()),
            _ => CoreOutcome::Temporal(Box::new(report)),
        },
        Err(_) => refused(
            "a temporal validation failure (core): its report is the reference engine's for now",
            options,
            ir,
        ),
    }
}

type Analysis = Result<
    (
        dive_liveness::analyze::periodic::PeriodicAnalysis,
        dive_liveness::analyze::periodic::Checks,
    ),
    dive_liveness::analyze::periodic::PeriodicFailure,
>;

/// Completes the graph and runs the final check, or finishes the prefix
/// the level checks stopped at.
fn finish(
    builder: dive_check::capture::GraphBuilder,
    graph_check: GraphCheck<'_>,
    safety: CheckResult,
    prefix: Option<u64>,
    options: &CheckOptions,
) -> Result<Analysis, Verdict> {
    if let Some(level) = prefix {
        let prefix = builder.prefix(level, safety, options.clone())?;
        Ok(graph_check.finish_prefix(prefix).map_err(|f| *f))
    } else {
        let complete = builder.complete(safety, options.clone())?;
        Ok(graph_check.finish(complete).map_err(|f| *f))
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
