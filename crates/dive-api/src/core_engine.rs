//! `--engine=core`: the M0 spike of the new execution core (ADR 0117,
//! `docs/developer/core/`).
//!
//! The reference binder builds the model (constants, overrides, split
//! actions and startup values); the core compiles it into a plan and
//! explores it single-threaded. Anything the core does not implement is an
//! `unsupported` verdict naming it; there is no fallback to the reference
//! engine.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use dive_check::{CheckOptions, CheckResult, Stats, Verdict, Workers};
use dive_core::compile::{ConstBinding, Handoff, Role};
use dive_eval::{SlotView, St};
use dive_ir::{DefId, ExprId, Ir, LocalRef, Slot};
use dive_value::Value;
use serde_json::{Value as Json, json};

/// Which execution engine runs `dive check`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Engine {
    /// The reference checker (`dive-check`).
    #[default]
    Reference,
    /// The new core (`dive-core` and `dive-explore`), M0: default profile,
    /// one worker, safety only.
    Core,
}

impl Engine {
    /// Parses `reference` or `core`.
    ///
    /// ```
    /// assert_eq!(dive_api::Engine::parse("core"), Some(dive_api::Engine::Core));
    /// assert_eq!(dive_api::Engine::parse("fast"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "reference" => Some(Self::Reference),
            "core" => Some(Self::Core),
            _ => None,
        }
    }

    /// The name used on the command line and in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Core => "core",
        }
    }
}

/// What a core run reports beyond TLC's statistics.
#[derive(Clone, Debug, Default)]
pub(crate) struct CoreRun {
    /// The disabled strategy switches.
    pub(crate) opts: String,
    /// Strategy metrics, as JSON.
    pub(crate) metrics: Json,
}

fn unsupported(feature: impl Into<String>) -> Verdict {
    Verdict::Unsupported {
        feature: feature.into(),
    }
}

fn result(verdict: Verdict, opts: &CheckOptions, ir: &Ir) -> CheckResult {
    CheckResult {
        symmetry_liveness: None,
        seen: None,
        frontier: dive_check::FrontierUsage::default(),
        predecessor: dive_check::PredecessorUsage::default(),
        verdict,
        binding_model_error: false,
        stats: None,
        tlc_prints_stats: false,
        counterexample: None,
        collision: None,
        store: opts.store,
        fp_index: None,
        tlc_fingerprints: false,
        var_names: ir.vars().iter().map(|&d| ir.decl(d).name.clone()).collect(),
        tlc_var_order: ir.tlc_var_order().iter().map(|v| v.index()).collect(),
        output: Vec::new(),
        warnings: Vec::new(),
        notes: Vec::new(),
        workers: Workers {
            requested: opts.workers.max(1),
            used: 1,
            sequential: false,
            reason: (opts.workers > 1).then(|| "the M0 core engine is single-threaded".to_owned()),
        },
        elapsed: std::time::Duration::ZERO,
    }
}

/// Resolves a split slot to a value: chases lazy arguments, evaluating a
/// constant-level one.
fn slot_value(model: &dive_model::Model, view: SlotView) -> Result<Option<Value>, String> {
    match view {
        SlotView::Unset => Ok(None),
        SlotView::Value(v) => Ok(Some(v)),
        SlotView::Lazy(expr, scope) => {
            if let dive_ir::Expr::Local(r) = &model.ir.node(expr).expr {
                let inner = model.evaluator.scope_slot(&scope, *r);
                if !matches!(inner, SlotView::Unset) {
                    return slot_value(model, inner);
                }
            }
            model
                .evaluator
                .eval_scoped(&scope, expr, &St::constant())
                .map(Some)
                .map_err(|e| format!("a split argument that is not constant ({e})"))
        }
        _ => Err("a split binding the M0 core cannot read".into()),
    }
}

fn role(
    model: &dive_model::Model,
    name: String,
    expr: ExprId,
    scope: &dive_eval::Scope,
) -> Result<Role, String> {
    let def = scope.def();
    let n = model.ir.def(def).frame.len();
    let mut slots = Vec::new();
    for i in 0..n {
        #[allow(clippy::cast_possible_truncation, reason = "frames are small")]
        let slot = Slot(i as u32);
        let view = model.evaluator.scope_slot(scope, LocalRef { up: 0, slot });
        if let Some(v) = slot_value(model, view)? {
            slots.push((slot, v));
        }
    }
    Ok(Role {
        name,
        expr,
        def,
        slots,
    })
}

fn named_roles(
    model: &dive_model::Model,
    named: &[dive_model::Named],
) -> Result<Vec<Role>, String> {
    named
        .iter()
        .map(|n| role(model, n.name.clone(), n.pred.expr, &n.pred.scope))
        .collect()
}

fn action_roles(
    model: &dive_model::Model,
    actions: &[dive_eval::next::Action],
) -> Result<Vec<Role>, String> {
    actions
        .iter()
        .map(|a| role(model, a.label().to_owned(), a.expr(), &a.scope()))
        .collect()
}

/// Refuses what M0 does not model, before any work.
fn refusal(model: &dive_model::Model) -> Option<String> {
    if let Some(u) = model.unsupported.first() {
        return Some(u.feature.clone());
    }
    if !model.implied_temporals.is_empty() || !model.configured_properties.is_empty() {
        return Some("temporal properties (M0 core engine)".into());
    }
    if !model.implied_inits.is_empty() || !model.implied_actions.is_empty() {
        return Some("implied initial or action properties (M0 core engine)".into());
    }
    if model.view.is_some() || model.alias.is_some() {
        return Some("VIEW or ALIAS (M0 core engine)".into());
    }
    if model
        .postconditions
        .as_ref()
        .map_or(true, |p| !p.is_empty())
    {
        return Some("POSTCONDITION (M0 core engine)".into());
    }
    if model.symmetry.is_some() {
        return Some("SYMMETRY (M0 core engine)".into());
    }
    None
}

fn handoff<'a>(model: &'a dive_model::Model, ir: &'a Ir) -> Result<Handoff<'a>, String> {
    let ev = &model.evaluator;
    let constants = (0..ir.consts().len())
        .map(|i| {
            #[allow(clippy::cast_possible_truncation, reason = "dense ids")]
            let c = dive_ir::ConstId(i as u32);
            ev.constant_binding(c).map(|b| match b {
                dive_eval::ConstBinding::Value(v) => ConstBinding::Value(v),
                dive_eval::ConstBinding::Def(d) => ConstBinding::Def(d),
            })
        })
        .collect();
    let mut redirects = HashMap::new();
    let mut std_redirects = HashMap::new();
    let mut precomputed = HashMap::new();
    for (i, def) in ir.defs().iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, reason = "dense ids")]
        let d = DefId(i as u32);
        let r = ev.redirected(d);
        if r != d {
            redirects.insert(d, r);
            if let Some(op) = def.std {
                std_redirects.insert(op, r);
            }
        }
        if let Some(v) = ev.precomputed(d) {
            precomputed.insert(d, v);
        } else if ev.is_value_override(d) {
            return Err(format!(
                "the value override of {} (M0 core engine)",
                def.name
            ));
        }
    }
    Ok(Handoff {
        ir,
        ctx: ev.options().ctx,
        constants,
        redirects,
        std_redirects,
        precomputed,
        init: action_roles(model, &model.init_actions)?,
        next: action_roles(model, &model.actions)?,
        invariants: named_roles(model, &model.invariants)?,
        constraints: named_roles(model, &model.constraints)?,
        action_constraints: named_roles(model, &model.action_constraints)?,
        check_deadlock: model.settings.check_deadlock,
    })
}

/// Runs the core engine.
#[allow(clippy::too_many_lines, reason = "the stages of a core run, in order")]
pub(crate) fn check(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    time_limit: Option<std::time::Duration>,
    profile_tlc: bool,
    switches: Option<&str>,
) -> (CheckResult, CoreRun) {
    let started = Instant::now();
    let text = switches
        .map(str::to_owned)
        .or_else(|| std::env::var("DIVE_CORE_OPT").ok());
    let core_opts = match text {
        Some(text) => match dive_core::Opts::parse(&text) {
            Ok(o) => o,
            Err(e) => {
                return (
                    result(unsupported(format!("DIVE_CORE_OPT: {e}")), options, ir),
                    CoreRun::default(),
                );
            }
        },
        None => dive_core::Opts::default(),
    };
    let mut run = CoreRun {
        opts: core_opts.describe(),
        metrics: Json::Null,
    };
    let finish = |mut r: CheckResult| {
        r.elapsed = started.elapsed();
        r
    };
    if profile_tlc {
        return (
            finish(result(
                unsupported(
                    "--compat=tlc with --engine=core (M0 implements the default profile only)",
                ),
                options,
                ir,
            )),
            run,
        );
    }
    let bind = dive_model::BindOptions {
        eval: options.eval,
        no_deadlock: options.no_deadlock,
        host: None,
        ..dive_model::BindOptions::dive()
    };
    let model = match dive_model::bind(Arc::clone(ir), config, &bind) {
        Ok(m) => m,
        Err(f) => {
            return (
                finish(result(
                    unsupported(format!("a binding failure under the M0 core engine: {f}")),
                    options,
                    ir,
                )),
                run,
            );
        }
    };
    if let Some(feature) = refusal(&model) {
        return (finish(result(unsupported(feature), options, ir)), run);
    }
    match model.check_assumptions() {
        dive_eval::AssumptionOutcome::AllTrue => {}
        _ => {
            return (
                finish(result(
                    unsupported("a false or failing ASSUME (M0 core engine)"),
                    options,
                    ir,
                )),
                run,
            );
        }
    }
    let h = match handoff(&model, ir) {
        Ok(h) => h,
        Err(e) => return (finish(result(unsupported(e), options, ir)), run),
    };
    let store = Arc::new(dive_core::Store::new(core_opts.hashcons));
    let compile_started = Instant::now();
    let plan = match dive_core::compile(&h, Arc::clone(&store), core_opts) {
        Ok(p) => p,
        Err(e) => {
            let v = match e.into_parts() {
                (dive_core::ErrorKind::Internal, message) => Verdict::InternalError { message },
                (_, message) => unsupported(message),
            };
            return (finish(result(v, options, ir)), run);
        }
    };
    let compile_time = compile_started.elapsed();
    let deadline = time_limit.map(|t| started + t);
    let out = dive_explore::explore(
        &plan,
        &store,
        dive_explore::Config {
            opts: core_opts,
            deadline,
            warn: options.eval.warnings,
        },
    );
    let verdict = match out.verdict {
        dive_explore::Verdict::Success => Verdict::Success,
        dive_explore::Verdict::InvariantViolated { property, initial } => {
            Verdict::InvariantViolated { property, initial }
        }
        dive_explore::Verdict::Deadlock => Verdict::Deadlock,
        dive_explore::Verdict::Unsupported { feature } => unsupported(feature),
        dive_explore::Verdict::TimeLimit => Verdict::ResourceLimit {
            message: "the time limit was reached (core engine)".into(),
        },
        dive_explore::Verdict::Internal { message } => Verdict::InternalError { message },
    };
    let counted = matches!(
        verdict,
        Verdict::Success | Verdict::InvariantViolated { .. } | Verdict::Deadlock
    );
    let mut r = result(verdict, options, ir);
    for (var, at) in &out.warnings {
        let name = plan.vars.get(usize::from(*var)).map_or("?", String::as_str);
        r.output.push(format!(
            "@!@!@STARTMSG 2143:3 @!@!@\nThe variable {name} was changed while it is specified as UNCHANGED at\n{}\n@!@!@ENDMSG 2143 @!@!@",
            model.evaluator.location(ExprId(*at))
        ));
    }
    if counted {
        r.stats = Some(Stats {
            generated: out.stats.generated,
            distinct: out.stats.distinct,
            left_on_queue: out.stats.left_on_queue,
            depth: out.stats.depth,
            initial_generated: out.stats.initial_generated,
        });
        r.tlc_prints_stats = true;
    }
    run.metrics = metrics_json(&plan, &out.metrics, compile_time);
    (finish(r), run)
}

fn metrics_json(
    plan: &dive_core::Plan,
    m: &dive_explore::Metrics,
    compile_time: std::time::Duration,
) -> Json {
    let memo: Vec<Json> = m
        .memo
        .iter()
        .filter(|(_, s)| s.lookups > 0 || s.bypass > 0)
        .map(|(n, s)| json!({"site": n, "lookups": s.lookups, "hits": s.hits, "bypass": s.bypass, "off": s.off}))
        .collect();
    json!({
        "compile_ms": compile_time.as_secs_f64() * 1e3,
        "explore_ms": m.explore_time.as_secs_f64() * 1e3,
        "plan": {
            "nodes": plan.stats.nodes, "generator_nodes": plan.stats.gnodes,
            "inlined": plan.stats.inlined, "aliased_args": plan.stats.aliased,
            "thunk_args": plan.stats.thunks, "by_name": plan.stats.by_name,
            "folded": plan.stats.folded, "cached_defs": plan.stats.cached_defs, "memo_sites": plan.code.sites.len(),
        },
        "probed": m.probed,
        "discarded_before_intern": m.discarded_before_intern,
        "conjuncts": {"total": m.conjuncts_total, "parent_skipped": m.conjuncts_skipped},
        "store": {"hits": m.store.hits, "misses": m.store.misses, "nodes": m.store.nodes, "handles": m.store.handles},
        "arena_peak": {"nodes": m.arena_peak.0, "handles": m.arena_peak.1},
        "thunks": {"forced": m.thunks.0, "cached": m.thunks.1},
        "memo": memo,
    })
}
