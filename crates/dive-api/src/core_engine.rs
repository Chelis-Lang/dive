//! The Core execution engine behind `dive check` (ADR 0117, ADR 0153,
//! `docs/developer/core/`).
//!
//! `dive-model`'s binder builds the model (constants, overrides, split
//! actions and startup values); the core compiles it into a plan and
//! explores it, level by level, on `--workers` threads. Anything the core does not implement is an
//! `unsupported` verdict naming it; there is no other engine to fall back
//! to.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use dive_core::compile::{ConstBinding, Handoff, Role, RoleSubst};
use dive_eval::{SlotView, St};
use dive_ir::{DefId, ExprId, Ir, LocalRef, Slot};
use dive_runtime::{CheckOptions, CheckResult, Stats, Verdict, Workers};
use dive_value::Value;
use serde_json::{Value as Json, json};

/// What a core run reports beyond TLC's statistics.
#[derive(Clone, Debug, Default)]
pub(crate) struct CoreRun {
    /// The disabled strategy switches.
    pub(crate) opts: String,
    /// Strategy metrics, as JSON.
    pub(crate) metrics: Json,
    /// Whether the search started: a refusal before it is static, one
    /// during it dynamic.
    pub(crate) searched: bool,
    /// The successor sample (`--core-sample`), as JSON; `Null` without one.
    pub(crate) sample: Json,
}

/// What a core run produced: a safety result, or a temporal report.
#[derive(Debug)]
pub(crate) enum CoreOutcome {
    /// A safety result (also a refusal, or the safety ending of a temporal
    /// run).
    Checked(Box<CheckResult>),
    /// A temporal report, finished by `crate::temporal`'s report path.
    Temporal(Box<crate::temporal::Report>),
    /// A real temporal resource stop with actual execution facts.
    TemporalFailure(Box<crate::temporal::Failure>),
}

/// Default completed-level Store reclamation trigger for pure fingerprint
/// safety BFS (ADR 0149). Reclamation is result-preserving; below this measured
/// Store size it never runs, so small and medium searches pay nothing.
pub(crate) const DEFAULT_STORE_COMPACT_BYTES: usize = 256 << 20;

fn unsupported(feature: impl Into<String>) -> Verdict {
    Verdict::Unsupported {
        feature: feature.into(),
    }
}

/// Names the first model feature that distributed safety does not admit.
pub(crate) fn distributed_model_refusal(
    plan: &dive_core::Plan,
    model: &dive_model::Model,
) -> Option<&'static str> {
    Some(if !model.implied_temporals.is_empty() {
        "distributed safety with temporal properties is not implemented"
    } else if plan.symmetry.is_some() {
        "distributed safety with SYMMETRY is not implemented"
    } else if plan.view.is_some() {
        "distributed safety with VIEW is not implemented"
    } else if plan.effects.any {
        "distributed safety with runtime effects (Print, PrintT, RandomElement, TLCGet or TLCSet) is not implemented"
    } else {
        return None;
    })
}

pub(crate) fn result(verdict: Verdict, opts: &CheckOptions, ir: &Ir) -> CheckResult {
    CheckResult {
        symmetry_liveness: None,
        seen: None,
        frontier: dive_runtime::FrontierUsage::default(),
        predecessor: dive_runtime::PredecessorUsage::default(),
        verdict,
        binding_model_error: false,
        stats: None,
        tlc_prints_stats: false,
        counterexample: None,
        diagnostic_witness: None,
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
            // The search sets it; 0 for a run that ends before it.
            used: 0,
            sequential: false,
            reason: None,
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
                .map_err(|e| format!("a split argument that is not constant (M1 core: {e})"))
        }
        _ => Err("a split binding the M0 core cannot read".into()),
    }
}

pub(crate) fn role(
    model: &dive_model::Model,
    name: String,
    expr: ExprId,
    scope: &dive_eval::Scope,
) -> Result<Role, String> {
    let substs = role_substs(model, &name, scope)?;
    let def = scope.def();
    let n = model.ir.def(def).frame.len();
    let mut slots = Vec::new();
    for i in 0..n {
        #[allow(clippy::cast_possible_truncation, reason = "frames are small")]
        let slot = Slot(i as u32);
        // The core binds `LET` values itself, lazily (`bind_role_lets`).
        if is_let_slot(model, def, slot) {
            continue;
        }
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
        substs,
    })
}

/// Whether a frame slot holds a `LET` value.
fn is_let_slot(model: &dive_model::Model, def: dive_ir::DefId, slot: Slot) -> bool {
    model
        .ir
        .def(def)
        .frame
        .get(slot.index())
        .is_some_and(|s| matches!(s.kind, dive_ir::SlotKind::LetValue(_)))
}

/// The bound slots of a scope's frame.
fn frame_slots(
    model: &dive_model::Model,
    scope: &dive_eval::Scope,
) -> Result<Vec<(Slot, Value)>, String> {
    let n = model.ir.def(scope.def()).frame.len();
    let mut slots = Vec::new();
    for i in 0..n {
        #[allow(clippy::cast_possible_truncation, reason = "frames are small")]
        let slot = Slot(i as u32);
        // The core binds `LET` values itself, lazily (`bind_role_lets`).
        if is_let_slot(model, scope.def(), slot) {
            continue;
        }
        let view = model.evaluator.scope_slot(scope, LocalRef { up: 0, slot });
        if let Some(v) = slot_value(model, view)? {
            slots.push((slot, v));
        }
    }
    Ok(slots)
}

/// The `INSTANCE` substitutions active at a split role (splitting entered
/// an instantiated definition). Uncaptured top-level operator definitions
/// retain their identity. Values evaluated under substitutions of their own
/// (nested instances) and captured operator closures remain named refusals.
fn role_substs(
    model: &dive_model::Model,
    name: &str,
    scope: &dive_eval::Scope,
) -> Result<Vec<RoleSubst>, String> {
    if !scope.has_substitutions() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (i, d) in model.ir.decls().iter().enumerate() {
        if !d.substituted {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, reason = "dense ids")]
        let decl = dive_ir::DeclId(i as u32);
        match model.evaluator.scope_subst_slot(scope, decl) {
            SlotView::Unset => {}
            SlotView::Value(value) => out.push(RoleSubst::Value { decl, value }),
            SlotView::Lazy(expr, s) if !s.has_substitutions() => out.push(RoleSubst::Expr {
                decl,
                expr,
                def: s.def(),
                slots: frame_slots(model, &s)?,
            }),
            SlotView::Other => {
                let Some(def) = model
                    .evaluator
                    .scope_subst_operator(scope, decl)
                    .map_err(|error| error.to_string())?
                else {
                    return Err(format!(
                        "the split action {name} with a captured or opaque operator substitution (core)"
                    ));
                };
                out.push(RoleSubst::Operator { decl, def });
            }
            _ => {
                return Err(format!(
                    "the split action {name} inside nested INSTANCE substitutions (M1 core)"
                ));
            }
        }
    }
    Ok(out)
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
/// A run option the core does not implement, which would change the
/// result if the core ignored it.
pub(crate) fn option_refusal(options: &CheckOptions, profile_tlc: bool) -> Option<String> {
    let e = &options.eval;
    let refused = if options.tlc_fingerprints && !profile_tlc {
        "TLC fingerprints outside --compat=tlc"
    } else if e.max_depth != dive_eval::Options::dive().max_depth {
        "an evaluation depth other than the default"
    } else if e.action_composition != dive_eval::Options::dive().action_composition
        && e.action_composition != dive_eval::Options::tlc().action_composition
    {
        "--action-composition=tlc"
    } else if options.chaos.is_some() {
        "--chaos"
    } else {
        return None;
    };
    Some(format!("{refused} under the core engine"))
}

fn refusal(model: &dive_model::Model, temporal: bool) -> Option<String> {
    // The liveness route checks the temporal properties the binder lists.
    if let Some(u) = model
        .unsupported
        .iter()
        .find(|u| !(temporal && matches!(u.kind, dive_model::UnsupportedKind::TemporalProperty)))
    {
        return Some(u.feature.clone());
    }
    // Implied initial predicates and actions run on the core; a genuine
    // temporal property needs `dive-liveness`, on the liveness route.
    if !temporal && !model.implied_temporals.is_empty() {
        return Some("temporal properties (M0 core engine)".into());
    }
    if model
        .postconditions
        .as_ref()
        .map_or(true, |p| !p.is_empty())
    {
        return Some("POSTCONDITION (M0 core engine)".into());
    }
    // With genuine temporal properties, the default policy (D-005) checks
    // the whole run unreduced: the start-up prepares no symmetry maps, and
    // the report names the policy. Other policies are refused by the binder.
    if model.symmetry.is_some()
        && !model.implied_temporals.is_empty()
        && !(temporal && model.unreduced_symmetry_liveness())
    {
        return Some("SYMMETRY (M0 core engine)".into());
    }
    None
}

/// The binder's start-up evaluation of a constant definition it left to the
/// core: `Evaluator::precompute`, as `processConstantDefns` runs it. A
/// failure leaves no value, as the binder swallows it.
#[derive(Debug)]
struct BinderStartup(dive_eval::Evaluator);

impl dive_core::compile::StartupEvaluator for BinderStartup {
    fn evaluate(&self, def: DefId) -> Option<dive_value::Value> {
        let value = self.0.precompute(def).ok().flatten()?;
        Some(crate::core_record_types::compact(&value, &self.0).unwrap_or(value))
    }
}

pub(crate) fn handoff<'a>(
    model: &'a dive_model::Model,
    ir: &'a Ir,
    symmetry: Vec<Vec<(String, String)>>,
) -> Result<Handoff<'a>, String> {
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
    let mut overrides = std::collections::HashSet::new();
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
            if ev.is_value_override(d) {
                overrides.insert(d);
            }
            precomputed.insert(d, v);
        } else if let Some(v) = ev.value_override(d) {
            // `Op = v` for a definition the binder did not precompute (for
            // example `NoVal = NoVal`, a model value).
            overrides.insert(d);
            precomputed.insert(d, v);
        }
    }
    Ok(Handoff {
        ir,
        ctx: ev.options().ctx,
        action_composition: ev.options().action_composition == dive_eval::ActionComposition::Tla,
        constants,
        redirects,
        std_redirects,
        precomputed,
        overrides,
        deferred: model.deferred_constants.clone(),
        startup_evaluator: (!model.deferred_constants.is_empty()).then(|| {
            Arc::new(BinderStartup(model.evaluator.clone()))
                as Arc<dyn dive_core::compile::StartupEvaluator>
        }),
        assumptions: model.deferred_assumptions.clone(),
        init: action_roles(model, &model.init_actions)?,
        next: action_roles(model, &model.actions)?,
        invariants: named_roles(model, &model.invariants)?,
        constraints: named_roles(model, &model.constraints)?,
        action_constraints: named_roles(model, &model.action_constraints)?,
        implied_inits: named_roles(model, &model.implied_inits)?,
        implied_actions: named_roles(model, &model.implied_actions)?,
        check_deadlock: model.settings.check_deadlock,
        symmetry,
        view: model
            .view
            .as_ref()
            .map(|n| role(model, n.name.clone(), n.pred.expr, &n.pred.scope))
            .transpose()?,
    })
}

/// The note on a stop in the default profile: the same text as
/// `dive_runtime::note_canonical_stop`'s, kept in step by hand. The
/// report-schema fixtures in `crates/dive-cli/tests/fixtures/report-schema/`
/// pin this copy, which is the one Core reports.
const CANONICAL_STOP_NOTE: &str = "The default profile explores sets in canonical order. When several violations are reachable at the same depth, which one is reported, and the counts at the stop, may differ under --compat=tlc; dive does not yet detect whether the order mattered in this run.";

/// The core's value semantics for the run's evaluation options: each of
/// `--compat=tlc`'s components maps to its own switch.
pub(crate) fn semantics(eval: &dive_eval::Options) -> dive_core::Semantics {
    dive_core::Semantics {
        string_order: if eval.ctx.order == dive_value::ValueOrder::Tlc {
            dive_core::strings::StringOrder::Token
        } else {
            dive_core::strings::StringOrder::Text
        },
        int_mode: match eval.ctx.int_mode {
            dive_value::IntMode::Tlc => dive_core::int::IntMode::Tlc,
            dive_value::IntMode::Big => dive_core::int::IntMode::Big,
        },
        tlc_comparisons: eval.comparisons == dive_eval::Comparisons::Tlc,
        tlc_exploration: eval.exploration == dive_eval::ExplorationOrder::Tlc,
    }
}

/// A store with the run's semantics. Under TLC's string order its table is
/// first fed the binder's start-up names, in TLC's encounter
/// order, and then sealed: a name TLC meets later gets a token that depends
/// on evaluation order, which the core refuses rather than guess.
pub(crate) fn store_for(
    model: &dive_model::Model,
    hashcons: bool,
    semantics: dive_core::Semantics,
) -> Result<dive_core::Store, String> {
    let store = dive_core::Store::with_semantics(hashcons, semantics);
    store.set_printer(dive_core::print::Printer {
        names: Arc::clone(&model.names),
        ctx: model.evaluator.options().ctx,
    });
    if semantics.string_order == dive_core::strings::StringOrder::Token {
        let n = model.evaluator.startup_names();
        let mut texts = Vec::with_capacity(n);
        for tok in 0..n {
            let atom = u32::try_from(tok)
                .ok()
                .and_then(|t| model.names.resolve(t))
                .ok_or_else(|| format!("start-up name {tok} is missing from the name table"))?;
            texts.push(atom);
        }
        store
            .strings()
            .preload(texts.iter().map(dive_value::Atom::as_str))
            .map_err(|e| e.to_string())?;
        store.strings().seal();
    }
    Ok(store)
}

/// A model that fails to bind ends the run before any search (for example
/// TLC's `TLC_INTEGER_TOO_BIG` for a numeral outside 32 bits under
/// `--compat=tlc`). The core binds with the run's own host and binding
/// options, so its failure is the run's result: the verdict, what the binder
/// printed before it and its warnings, in the form every engine reports a
/// binding failure (`dive_runtime::bare`).
///
/// In the default profile the core's binder leaves some pure constant
/// definitions unevaluated (`dive_model::bind_deferring_constants`). That
/// cannot change a binding failure: TLC swallows the failure of a start-up
/// evaluation, and a pure definition prints nothing and draws nothing, so
/// the failing stage, its error and the output before it are those of the
/// full binding.
fn binding_failure(
    ir: &Arc<Ir>,
    options: &CheckOptions,
    failure: &dive_model::Failure,
) -> CheckResult {
    let mut r = dive_runtime::bare(Verdict::from(&failure.error), options, ir);
    r.binding_model_error = matches!(r.verdict, Verdict::ModelError { .. });
    dive_runtime::messages(&failure.output, &mut r);
    r
}

/// `SYMMETRY`, prepared by `dive-model`'s binder: its maps by
/// model-value name, its start-up warnings and the default profile's
/// canonical-order note.
#[derive(Clone, Default)]
struct Symmetry {
    maps: Vec<Vec<(String, String)>>,
    warnings: Vec<(u32, String)>,
    note: Option<String>,
}

/// Why `symmetry` ended the run before the search.
enum SymmetryEnd {
    Refused(String),
    Model(Verdict),
}

impl SymmetryEnd {
    fn into_result(self, options: &CheckOptions, ir: &Ir) -> CheckResult {
        match self {
            Self::Refused(feature) => result(unsupported(feature), options, ir),
            Self::Model(v) => {
                let mut r = result(v, options, ir);
                r.binding_model_error = matches!(r.verdict, Verdict::ModelError { .. });
                r
            }
        }
    }
}

const SYMMETRY_NOTE: &str = "The default profile canonicalizes symmetry keys in canonical value order. A non-invariant VIEW or asymmetric specification may differ under --compat=tlc; order relevance is not yet detected.";

fn symmetry(
    model: &mut dive_model::Model,
    config: &dive_config::ModelConfig,
    profile_tlc: bool,
) -> Result<Symmetry, SymmetryEnd> {
    let prepared = model
        .prepare_symmetry(
            config,
            dive_model::symmetry::Mode::Checked,
            dive_model::symmetry::Limits::default(),
        )
        .map_err(|e| SymmetryEnd::Model(Verdict::from(&e)))?;
    let mut out = Symmetry::default();
    let Some(prepared) = prepared else {
        return Ok(out);
    };
    for w in &prepared.warnings {
        match w {
            dive_model::Message::Warning { code, message } => {
                out.warnings.push((code.code(), message.clone()));
            }
            dive_model::Message::Print(_) => {
                return Err(SymmetryEnd::Refused(
                    "printed output while preparing SYMMETRY (core)".into(),
                ));
            }
        }
    }
    if let Some(reduction) = &prepared.reduction {
        let universe = reduction.universe();
        for map in reduction.maps() {
            let mut pairs = Vec::new();
            for (from, &to) in map.images().iter().enumerate() {
                if from != to {
                    let (Some(a), Some(b)) = (universe.get(from), universe.get(to)) else {
                        return Err(SymmetryEnd::Refused(
                            "a SYMMETRY map outside its universe (core)".into(),
                        ));
                    };
                    pairs.push((a.as_str().to_owned(), b.as_str().to_owned()));
                }
            }
            out.maps.push(pairs);
        }
    }
    let symmetry = format!("SYMMETRY {}", model.symmetry.as_deref().unwrap_or_default());
    model.unsupported.retain(|u| u.feature != symmetry);
    // The default profile's canonical keys may differ from TLC's;
    // under `--compat=tlc` the keys are TLC's.
    if !profile_tlc {
        out.note = Some(SYMMETRY_NOTE.into());
    }
    Ok(out)
}

/// Gives the store the run's random streams: the startup stream where the
/// binder left it, and states keyed by `dive-store`'s TLC fingerprinter.
fn arm_draws(
    store: &dive_core::Store,
    host: &dive_stdlib::host::RunHost,
    ir: &Ir,
    options: &CheckOptions,
) -> Result<(), String> {
    let streams = host.streams();
    let startup = host.stream();
    let tlc_order: Vec<usize> = ir.tlc_var_order().iter().map(|v| v.index()).collect();
    let seeder = dive_store::state::StateFingerprinter::tlc(options.fp_index, &tlc_order)
        .ok_or_else(|| {
            format!(
                "fingerprint polynomial index {} out of range",
                options.fp_index
            )
        })?;
    let Some(printer) = store.printer() else {
        return Err("RandomElement without the model's names (core)".into());
    };
    store.set_draws(Arc::new(crate::core_random::CoreDraws {
        streams,
        startup,
        startup_drew: startup != streams.startup(),
        seeder,
        names: Arc::clone(&printer.names),
        ctx: printer.ctx,
    }));
    Ok(())
}

/// A model compiled for the core: what `check` explores.
pub(crate) struct Prepared {
    pub(crate) model: dive_model::Model,
    pub(crate) host: Arc<dive_stdlib::host::RunHost>,
    pub(crate) start: Startup,
    pub(crate) store: Arc<dive_core::Store>,
    pub(crate) plan: dive_core::Plan,
    pub(crate) compile_time: std::time::Duration,
}

/// The byte cap of each worker's memo tables: a sixty-fourth of the run's
/// memory ceiling, shared by the workers, between 1 MiB and
/// [`dive_core::memo::DEFAULT_CAP`] (`06-memoization.md` §6). The cap only
/// bounds a cache, so it never changes a result.
pub(crate) fn memo_cap(ceiling: Option<u64>, workers: usize) -> usize {
    let most = dive_core::memo::DEFAULT_CAP;
    ceiling.map_or(most, |bytes| {
        let share = bytes / 64 / u64::try_from(workers).unwrap_or(u64::MAX).max(1);
        usize::try_from(share).unwrap_or(most).clamp(1 << 20, most)
    })
}

/// Binds, checks the ASSUMEs and compiles the plan, as `check` does before
/// its search; `temporal` admits genuine temporal properties (the liveness
/// route). The `Err` is the run's result.
#[allow(
    clippy::too_many_lines,
    reason = "the start-up stages of a core run, in order"
)]
pub(crate) fn prepare(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    profile_tlc: bool,
    core_opts: dive_core::Opts,
    temporal: bool,
) -> Result<Prepared, Box<CheckResult>> {
    prepare_observed(ir, config, options, profile_tlc, core_opts, temporal, None)
}

#[allow(
    clippy::too_many_lines,
    reason = "existing ordered startup and compilation with explicit producer-only observation"
)]
fn prepare_observed(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    profile_tlc: bool,
    core_opts: dive_core::Opts,
    temporal: bool,
    debugger: Option<Arc<dyn dive_core::debug::Observer>>,
) -> Result<Prepared, Box<CheckResult>> {
    // The binder's refusal of random streams under VIEW or SYMMETRY,
    // before binding runs start-up draws.
    if let Some(feature) = dive_model::transformed_randomness(ir, config) {
        return Err(Box::new(result(unsupported(feature), options, ir)));
    }
    // The binder's host: constant definitions and `ASSUME`s
    // draw from the startup stream, which `Init` continues
    // (`03-value-model.md` §6.1).
    let mode = if profile_tlc {
        dive_stdlib::random::RandomMode::Tlc
    } else {
        dive_stdlib::random::RandomMode::Dive
    };
    let host = Arc::new(dive_stdlib::host::RunHost::new(
        dive_stdlib::random::Streams::new(options.seed, mode),
        None,
    ));
    let defaults = dive_model::BindOptions::dive();
    let bind_options = |host: &Arc<dive_stdlib::host::RunHost>| dive_model::BindOptions {
        eval: options.eval,
        no_deadlock: options.no_deadlock,
        host: Some(Arc::clone(host) as Arc<dyn dive_eval::Host>),
        // The temporal route reports the requested policy, as the binder
        // binds it; it only matters with temporal properties.
        symmetry_liveness: if temporal {
            options.symmetry_liveness
        } else {
            defaults.symmetry_liveness
        },
    };
    // In the default profile the core evaluates the pure constant
    // definitions its roles use only as values, once, when it compiles its
    // plan (issue #846). Under `--compat=tlc` the startup evaluation stays
    // the binder's: the names it meets feed TLC's string order.
    let bound = if profile_tlc {
        dive_model::bind(Arc::clone(ir), config, &bind_options(&host))
    } else {
        dive_model::bind_deferring_constants(Arc::clone(ir), config, &bind_options(&host))
    };
    let mut model = match bound {
        Ok(m) => m,
        Err(f) => return Err(Box::new(binding_failure(ir, options, &f))),
    };
    // On the temporal route the binder still evaluates the start-up
    // definitions the temporal properties and their checking reach
    // (`dive_model::bind_deferring_constants`); the core keeps the others.
    // TLC creates its model checker after binding, and so does the core:
    // `TLCGet("config")` and the checker's names exist from here on,
    // and the ASSUMEs read the start-up counters.
    model.evaluator.intern_checker_names();
    let run_config = dive_runtime::run::run_config(options, model.settings.check_deadlock);
    model.evaluator.intern_run_config(&run_config);
    host.set_config(Some(run_config));
    dive_runtime::run::startup_counters(&host);
    if debugger.is_some() && !model.implied_temporals.is_empty() {
        return Err(Box::new(result(
            unsupported("compiled debugger with temporal checking"),
            options,
            ir,
        )));
    }
    if options.seen_disk.is_some() && !model.implied_temporals.is_empty() {
        return Err(Box::new(result(
            unsupported("disk seen-state retention with temporal checking"),
            options,
            ir,
        )));
    }
    let mut sym = Symmetry::default();
    if model.symmetry.is_some() && model.implied_temporals.is_empty() {
        match symmetry(&mut model, config, profile_tlc) {
            Ok(s) => sym = s,
            Err(r) => return Err(Box::new(r.into_result(options, ir))),
        }
    }
    if let Some(feature) = refusal(&model, temporal) {
        return Err(Box::new(result(unsupported(feature), options, ir)));
    }
    // TLC normalizes the temporal properties before it evaluates any ASSUME,
    // and stops with its tautology error (2253) when the normalization has no
    // solutions. Core does not build that report; it refuses here, before an
    // ASSUME could report another error first. Planning is effect-free.
    if temporal
        && dive_liveness::analyze::startup_tautology(
            &model,
            options,
            crate::temporal::core_file_configuration().limits,
        )
    {
        return Err(Box::new(result(
            unsupported(crate::core_temporal::VALIDATION_FAILURE),
            options,
            ir,
        )));
    }
    // The core compiles its plan, evaluating the pure ASSUMEs the binding
    // left to it (`Model::deferred_assumptions`, issue #846), before the
    // remaining ASSUMEs are checked. Compiling is pure start-up work (it
    // prints nothing, draws nothing and reads no counter), so a compile
    // failure is reported only after the ASSUMEs pass, as before.
    // The plan is compiled from the bound IR: binding installs the local
    // Java overrides TLC loads (the pinned Hanoi `Bits.class`) on it, and
    // the source IR would evaluate the module's TLA+ body instead.
    let bound_ir = Arc::clone(&model.ir);
    let compiled = compile_plan(
        &model,
        &bound_ir,
        options,
        core_opts,
        &mut sym,
        debugger.as_ref(),
    );
    // The core evaluates a leading run of the ASSUMEs, stopping at the first
    // that is not TRUE (TLC's order: an ASSUME may guard the next). One it
    // found FALSE is the first that is not TRUE, and is reported without
    // evaluating it again.
    let (proved, refuted): (&[DefId], _) = match &compiled {
        Ok((_, plan, _)) => (&plan.startup_assumptions, plan.startup_assumption_false),
        Err(_) => (&[], None),
    };
    // The other ASSUMEs are startup evaluation (the binder's,
    // `04-plan-compiler.md` §2): a false or failing one ends the run as the
    // binder reports it, before any search.
    let outcome = match refuted {
        Some(a) => model.evaluator.false_assumption(a),
        None => model.check_assumptions_except(proved),
    };
    let assumed = match outcome {
        dive_eval::AssumptionOutcome::AllTrue => None,
        dive_eval::AssumptionOutcome::False {
            property, message, ..
        } => Some(Verdict::AssumptionFalse { property, message }),
        dive_eval::AssumptionOutcome::Error { error, message, .. } => {
            Some(dive_runtime::classify::assumption_error(&error, message))
        }
    };
    // From `Init` on the live counters are not provided: reading one is
    // refused rather than answered with a count the core does not keep.
    dive_runtime::run::search_counters(&host);
    if let Some(v) = assumed {
        // Output printed while evaluating the ASSUMEs has an order the core
        // does not reproduce, so the core does not report it.
        let printed = !model.take_output().is_empty()
            || model
                .output
                .iter()
                .any(|m| matches!(m, dive_model::Message::Print(_)));
        let v = if printed {
            unsupported("output printed at startup before a false or failing ASSUME (core engine)")
        } else {
            v
        };
        return Err(Box::new(result(v, options, ir)));
    }
    // What the binder printed at start-up (constant definitions, then the
    // ASSUMEs) comes first.
    let mut startup: Vec<String> = model
        .output
        .iter()
        .filter_map(|m| match m {
            dive_model::Message::Print(text) => Some(text.clone()),
            dive_model::Message::Warning { .. } => None,
        })
        .collect();
    startup.extend(model.take_output());
    let (store, mut plan, compile_time) = compiled?;
    if let Some(observer) = debugger {
        if let Some(map) = &plan.code.source_map
            && let Err(error) = observer.prepared(map)
        {
            return Err(Box::new(result(
                unsupported(error.message().to_owned()),
                options,
                ir,
            )));
        }
        plan.code.debugger = Some(observer);
    }
    // Only simulation draws these (`core_modes::sim_random`); every search
    // here keeps its refusal before exploring.
    if let Some(op) = plan.effects.sampled_draw {
        return Err(Box::new(result(
            unsupported(format!(
                "the standard operator {} (M0 core)",
                op.qualified_name()
            )),
            options,
            ir,
        )));
    }
    if plan.effects.draws
        && let Err(feature) = arm_draws(&store, &host, ir, options)
    {
        return Err(Box::new(result(unsupported(feature), options, ir)));
    }
    if plan.effects.registers {
        store.set_registers(Arc::new(crate::core_random::CoreRegisters(
            model.evaluator.clone(),
        )));
    }
    Ok(Prepared {
        model,
        host,
        start: Startup { sym, startup },
        store,
        plan,
        compile_time,
    })
}

/// The stop of a budget that start-up work (binding, constant folding,
/// compiling the plan, the ASSUMEs) has already spent: phase
/// `check-startup`, before `Init` is evaluated, rather than at the search's
/// first poll (#968). `None` while the budget holds.
pub(crate) fn startup_stop(options: &CheckOptions) -> Option<Verdict> {
    match options.budget.poll() {
        Ok(None) => None,
        Ok(Some(exhausted)) => Some(Verdict::Stopped(Box::new(
            exhausted.during(dive_runtime::budget::Phase::Startup),
        ))),
        Err(error) => Some(Verdict::BudgetMeasurement { error }),
    }
}

/// The hand-off, the store and the compiled plan, with the compile time; the
/// `Err` is the run's result, reported after the ASSUMEs pass.
fn compile_plan(
    model: &dive_model::Model,
    ir: &Arc<Ir>,
    options: &CheckOptions,
    core_opts: dive_core::Opts,
    sym: &mut Symmetry,
    debugger: Option<&Arc<dyn dive_core::debug::Observer>>,
) -> Result<(Arc<dive_core::Store>, dive_core::Plan, std::time::Duration), Box<CheckResult>> {
    let mut h = match handoff(model, ir, std::mem::take(&mut sym.maps)) {
        Ok(h) => h,
        Err(e) => return Err(Box::new(result(unsupported(e), options, ir))),
    };
    for value in h.precomputed.values_mut() {
        if let Some(compact) = crate::core_record_types::compact(value, &model.evaluator) {
            *value = compact;
        }
    }
    let store = match store_for(model, core_opts.hashcons, semantics(&options.eval)) {
        Ok(s) => Arc::new(s),
        Err(message) => {
            return Err(Box::new(result(
                Verdict::InternalError { message },
                options,
                ir,
            )));
        }
    };
    let compile_started = Instant::now();
    let compiled = if debugger.is_some() {
        dive_core::compile::compile_debug(&h, Arc::clone(&store), core_opts)
    } else {
        dive_core::compile(&h, Arc::clone(&store), core_opts)
    };
    match compiled {
        Ok(plan) => Ok((store, plan, compile_started.elapsed())),
        Err(e) => {
            let v = match e.into_parts() {
                (dive_core::ErrorKind::Internal, message) => Verdict::InternalError { message },
                (_, message) => unsupported(message),
            };
            Err(Box::new(result(v, options, ir)))
        }
    }
}

/// What a core search ran with, for its report.
pub(crate) struct Search<'a> {
    pub(crate) ir: &'a Arc<Ir>,
    pub(crate) config: &'a dive_config::ModelConfig,
    pub(crate) options: &'a CheckOptions,
    pub(crate) profile_tlc: bool,
    pub(crate) workers: usize,
    pub(crate) explore_started: Instant,
}

/// What the start-up adds to a report: `SYMMETRY`'s warnings and note, and
/// the binder's output.
#[derive(Clone)]
pub(crate) struct Startup {
    sym: Symmetry,
    startup: Vec<String>,
}

impl Startup {
    pub(crate) fn has_diagnostics(&self) -> bool {
        !self.startup.is_empty() || !self.sym.warnings.is_empty()
    }
}

/// Ordinary file checks honor the process-level strategy override. Reuse
/// receipts in this API carry only the unmodified compiled default policy.
pub(crate) fn edit_policy() -> Result<(), &'static str> {
    if std::env::var_os("DIVE_CORE_OPT").is_some() {
        return Err("DIVE_CORE_OPT overrides the fixed reuse policy");
    }
    Ok(())
}

/// Whole-source admission before binding can evaluate a constant or assumption.
/// A file module with a standard module's name can activate native overrides
/// even when its literal TLA+ body appears pure.
pub(crate) fn edit_input_refusal(
    ir: &Arc<Ir>,
    cfg: &dive_config::ModelConfig,
) -> Option<&'static str> {
    use dive_ir::{Effects, OpRef, StdOp, std_op::Phase};
    fn outside(op: StdOp) -> bool {
        op.impure()
            || !matches!(
                op.module(),
                "Naturals" | "Integers" | "FiniteSets" | "Sequences"
            )
    }
    if let Err(reason) = edit_policy() {
        return Some(reason);
    }
    if ir.modules().iter().any(|m| {
        !m.standard
            && StdOp::ALL
                .iter()
                .any(|op| op.module() == m.name && op.phase() != Phase::Native)
    }) {
        return Some("source module shadows a native standard module");
    }
    if ir
        .defs()
        .iter()
        .any(|d| d.effects.contains(Effects::IMPURE) || d.std.is_some_and(outside))
        || ir.nodes().iter().any(|n| {
            n.effects.contains(Effects::IMPURE)
                || n.expr
                    .op_refs()
                    .into_iter()
                    .any(|op| matches!(op,OpRef::Std(s) if outside(*s)))
        })
    {
        return Some("source includes an impure or unadmitted native operation");
    }
    let recipe =
        dive_model::Recipe::new(ir.clone(), cfg.clone(), dive_eval::Options::dive(), false);
    recipe
        .has_randomness()
        .then_some("source or configuration may invoke randomness")
}

/// The safety result of a core search, as a `dive_runtime::CheckResult`.
#[allow(
    clippy::too_many_lines,
    reason = "the endings of a core search, in order"
)]
pub(crate) fn safety_result(
    search: &Search<'_>,
    model: &dive_model::Model,
    store: &Arc<dive_core::Store>,
    plan: &dive_core::Plan,
    start: Startup,
    out: dive_explore::Outcome,
) -> CheckResult {
    let expanded = out.expanded;
    let Search {
        ir,
        config,
        options,
        profile_tlc,
        workers,
        explore_started,
    } = *search;
    let inline_roots = out.metrics.inline_graph_roots;
    let ordered_width = out.metrics.ordered_print_workers;
    let wave_width = out.metrics.max_wave_workers;
    let initial_width = out.metrics.initial_workers;
    let thread_shortage = out.metrics.wave_thread_shortage;
    let searched_result = |verdict| {
        let mut report = result(verdict, options, ir);
        if options.sequential && workers == 1 {
            report.workers.used = 1;
            report.workers.sequential = true;
            report.workers.reason = Some("--sequential requested inline checking".into());
        } else if thread_shortage {
            report.workers.used = wave_width;
            report.workers.reason = Some(
                "the operating system limited expansion worker threads in at least one wave".into(),
            );
        } else if workers > 1 && expanded && ordered_width == Some(1) {
            report.workers.used = 1;
            report.workers.reason = Some(
                "each expanded level had only one source for ordered diagnostic output".into(),
            );
        } else if plan.effects.any && ordered_width.is_none() && workers > 1 {
            report.workers.used = 1;
            report.workers.reason = Some(if plan.effects.prints {
                "the specification prints (Print or PrintT): the core keeps TLC's order of the output with one worker".into()
            } else if plan.effects.draws {
                "the specification draws (RandomElement): the core draws in TLC's order with one worker"
                .into()
            } else {
                "the specification uses TLCSet or TLCGet: the core keeps TLC's order of their effects with one worker".into()
            });
        } else if let Some(roots) = inline_roots {
            report.workers.sequential = true;
            report.workers.used = 1;
            report.workers.reason = Some(format!(
                "temporal capture has {roots} initial states and short, non-enumerating actions. One worker expands and commits each source inline"
            ));
        } else if initial_width > 1 && !expanded {
            report.workers.used = initial_width;
            report.workers.reason = Some(
                "the run used parallel Init and ended before expanding successor states".into(),
            );
        } else if workers > 1 && !expanded {
            report.workers.used = 1;
            report.workers.reason = Some(
                "the run ended in Init, which one worker evaluates, before any parallel level"
                    .into(),
            );
        } else {
            report.workers.used = workers;
            report.workers.reason = None;
        }
        report
    };
    let Startup { mut sym, startup } = start;
    let budget = &options.budget;
    let storage_failed = !out.metrics.storage.candidates.failures.is_empty()
        || !out.metrics.storage.reclamation.failures.is_empty()
        || out
            .metrics
            .distributed
            .as_ref()
            .is_some_and(|facts| !facts.failures.is_empty())
        || !out.metrics.storage.predecessor_failures.is_empty()
        || !out.metrics.storage.frontier.failures.is_empty()
        || out
            .metrics
            .storage
            .seen
            .as_ref()
            .is_some_and(|usage| usage.first_failure.is_some());
    let owned_budget = out.verdict == dive_explore::Verdict::Budget || storage_failed;
    // A reached owned budget publishes observed prefix facts, not a model
    // verdict or an effect/replay parity claim. Preserve that stop and output.
    if plan.effects.registers && !owned_budget && out.verdict != dive_explore::Verdict::Success {
        // TLC writes and reads its registers again while it rebuilds a
        // trace or replays an error; the core does not reproduce that yet.
        let v = match out.verdict {
            dive_explore::Verdict::Unsupported { feature } => unsupported(feature),
            dive_explore::Verdict::Internal { message } => Verdict::InternalError { message },
            _ => unsupported("TLCSet or TLCGet in a run that does not end in success (M1 core)"),
        };
        return searched_result(v);
    }
    if plan.effects.prints
        && !owned_budget
        && !out.printed.is_empty()
        && (out.verdict != dive_explore::Verdict::Success || !out.warnings.is_empty())
    {
        // TLC prints again while it rebuilds a trace or reports an error,
        // and interleaves warnings with the output; the core does not
        // reproduce either yet. A search that printed nothing is exempt:
        // TLC's rebuild and error report re-evaluate only what the search
        // evaluated, so they print nothing either (a `Print` in a disjunct
        // reached only after a violation, as in a trace export).
        let v = match out.verdict {
            dive_explore::Verdict::Unsupported { feature } => unsupported(feature),
            dive_explore::Verdict::Internal { message } => Verdict::InternalError { message },
            _ => unsupported(
                "Print or PrintT in a run that does not end in success, or that warns (M1 core)",
            ),
        };
        return searched_result(v);
    }
    let failed = out.verdict == dive_explore::Verdict::Failed;
    let verdict = match out.verdict {
        dive_explore::Verdict::Success => Verdict::Success,
        dive_explore::Verdict::InvariantViolated { property, initial } => {
            Verdict::InvariantViolated { property, initial }
        }
        dive_explore::Verdict::CheckpointPaused { next_level } => Verdict::ResourceLimit {
            message: format!(
                "native checkpoint paused before source level {next_level}; search is incomplete"
            ),
        },
        dive_explore::Verdict::CheckpointError { message } => Verdict::ResourceLimit { message },
        dive_explore::Verdict::Deadlock => Verdict::Deadlock,
        dive_explore::Verdict::NoNextAction => Verdict::ModelError {
            code: 2115,
            name: "TLC_STATES_AND_NO_NEXT_ACTION".into(),
            message: "No next state actions defined to generate successor states from.".into(),
        },
        dive_explore::Verdict::PropertyViolatedInitially { property } => {
            Verdict::PropertyViolatedInitially { property }
        }
        dive_explore::Verdict::ActionPropertyViolated { property } => {
            Verdict::ActionPropertyViolated { property }
        }
        dive_explore::Verdict::Failed => Verdict::InternalError {
            message: "a core failure without its site".into(),
        },
        dive_explore::Verdict::Unsupported { feature } => unsupported(feature),
        dive_explore::Verdict::Budget => match budget.poll() {
            Ok(Some(exhausted)) => Verdict::Stopped(Box::new(
                exhausted.during(dive_runtime::budget::Phase::Search),
            )),
            Err(error) => Verdict::BudgetMeasurement { error },
            Ok(None) => Verdict::InternalError {
                message: "the core stopped for a budget that is not exhausted".into(),
            },
        },
        dive_explore::Verdict::Admission {
            limit,
            max,
            observed,
        } => Verdict::Stopped(Box::new(dive_runtime::budget::Stopped {
            resource: match limit {
                dive_explore::Limit::States => dive_runtime::budget::Resource::States,
                dive_explore::Limit::Depth => dive_runtime::budget::Resource::Depth,
            },
            limit: max,
            observed,
            elapsed_ms: u64::try_from(explore_started.elapsed().as_millis()).unwrap_or(u64::MAX),
            phase: dive_runtime::budget::Phase::Search,
        })),
        dive_explore::Verdict::Internal { message } if storage_failed => {
            Verdict::ResourceLimit { message }
        }
        dive_explore::Verdict::Internal { message } => Verdict::InternalError { message },
        // The safety route has no graph sink.
        dive_explore::Verdict::GraphStop | dive_explore::Verdict::PrefixStop { .. } => {
            Verdict::InternalError {
                message: "a graph stop without a graph sink".into(),
            }
        }
    };
    let mut counted = matches!(
        verdict,
        Verdict::Success
            | Verdict::InvariantViolated { .. }
            | Verdict::PropertyViolatedInitially { .. }
            | Verdict::ActionPropertyViolated { .. }
            | Verdict::Deadlock
            | Verdict::ModelError { code: 2115, .. }
            | Verdict::Stopped(_)
            | Verdict::ResourceLimit { .. }
    );
    let mut verdict = verdict;
    let mut in_init = matches!(
        verdict,
        Verdict::InvariantViolated { initial: true, .. }
            | Verdict::PropertyViolatedInitially { .. }
            | Verdict::ModelError { code: 2115, .. }
    ) || (matches!(verdict, Verdict::Stopped(_) | Verdict::ResourceLimit { .. })
        && out.in_init);
    let mut trace = out.trace.clone();
    let mut stats = out.stats;
    // Whether the trace ends in the successor a check failed on.
    let mut then = matches!(
        verdict,
        Verdict::InvariantViolated { initial: false, .. } | Verdict::ActionPropertyViolated { .. }
    );
    if let Some(f) = out.failure.as_ref().filter(|_| failed) {
        match crate::core_ending::ending(model, store, f, out.replayed.as_ref()) {
            Ok(end) => {
                verdict = end.verdict;
                in_init = end.in_init;
                counted = true;
                then = end.with && f.state.is_some() && !in_init;
                if !end.with
                    && f.state.is_some()
                    && let Some(Ok(t)) = &mut trace
                {
                    t.pop();
                }
                if let Some(rp) = &out.replayed {
                    stats.distinct += rp.distinct;
                    stats.left_on_queue += rp.queued;
                }
                if in_init {
                    // After an error in the initial states TLC prints its
                    // statistics: every state it inserted is on the queue,
                    // and the depth is 1 once a state was generated.
                    stats.left_on_queue = stats.distinct;
                    stats.depth = u64::from(stats.generated > 0);
                }
            }
            Err(refusal) => verdict = refusal,
        }
    }
    // The boundary for a plan that draws: TLC rebuilds
    // a trace whose predecessor is initial from the original state object,
    // whose stream is already consumed, so its last state can go missing
    // (see `docs/tlc-differences.md`). The core refuses it by name,
    // with the stop's statistics and a note, and drops the trace.
    let mut draw_note = None;
    if plan.effects.draws
        && then
        && verdict.is_verdict()
        && matches!(&trace, Some(Ok(t)) if t.len() == 2)
    {
        draw_note = Some(format!(
            "The search reached {verdict:?}; its first-successor random trace has an unproved TLC replay/printing contract. Statistics record that stopping prefix, not a completed certification."
        ));
        verdict = unsupported("TLC initial-predecessor random counterexample replay is not proved");
        trace = None;
    }
    let prints = !in_init || matches!(verdict, Verdict::EvaluationError { .. });
    let mut r = searched_result(verdict);
    r.predecessor = dive_runtime::PredecessorUsage {
        storage: out.metrics.storage.predecessor,
        failures: out.metrics.storage.predecessor_failures.clone(),
        deciding_event_before_failure: out.metrics.storage.predecessor_deciding_event.clone(),
        cleanup_completed: out.metrics.storage.predecessor_cleanup_completed,
    };
    let frontier = &out.metrics.storage.frontier;
    r.frontier = dive_runtime::FrontierUsage {
        memory_bytes_per_queue: frontier.memory_bytes_per_queue,
        max_record_bytes: frontier
            .memory_bytes_per_queue
            .map(|_| dive_store::spill::MAX_RECORD_BYTES),
        peak_retained_bytes: frontier.peak_retained_bytes,
        retained_bytes: frontier.retained_bytes,
        spilled_bytes: frontier.spilled_bytes,
        failures: frontier.failures.clone(),
        deciding_event_before_failure: frontier.deciding_event_before_failure.clone(),
    };
    if matches!(
        &r.verdict,
        Verdict::EvaluationError {
            site: dive_runtime::ErrorSite::Invariant,
            code: 2103,
            reported: 2102,
            verified: false,
            ..
        }
    ) {
        r.notes.push("The initial invariant's diagnostic context has no proved deciding class or formal behavior word; its original cause/context, physical exit and stopping counts are retained.".into());
    }
    r.notes.extend(draw_note);
    r.warnings.append(&mut sym.warnings);
    r.notes.extend(sym.note.take());
    r.output = startup;
    r.output.extend(out.printed.iter().cloned());
    for (var, at) in &out.warnings {
        let name = plan.vars.get(usize::from(*var)).map_or("?", String::as_str);
        r.output.push(format!(
            "@!@!@STARTMSG 2143:3 @!@!@\nThe variable {name} was changed while it is specified as UNCHANGED at\n{}\n@!@!@ENDMSG 2143 @!@!@",
            model.evaluator.location(ExprId(*at))
        ));
    }
    if counted {
        r.stats = Some(Stats {
            generated: stats.generated,
            distinct: stats.distinct,
            left_on_queue: stats.left_on_queue,
            depth: stats.depth,
            initial_generated: stats.initial_generated,
        });
        r.tlc_prints_stats = prints;
    }
    if counted
        && let Some(trace) = trace
            .as_ref()
            .filter(|t| !matches!(t, Ok(t) if t.is_empty()))
    {
        match trace
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|t| counterexample(model, store, t))
        {
            Ok(cx) => r.counterexample = Some(cx),
            Err(message) => {
                r.verdict = Verdict::InternalError {
                    message: format!("core trace reconstruction: {message}"),
                };
                r.stats = None;
                r.tlc_prints_stats = false;
            }
        }
    }
    // ALIAS is not applied: the counterexample shows the states
    // themselves and the report says so.
    if r.counterexample.is_some()
        && let Some(alias) = &config.alias
    {
        r.notes.push(format!(
            "ALIAS {} is not applied: the counterexample shows the states themselves (not implemented yet).",
            alias.text
        ));
    }
    // The canonical-order qualification of a stop in the default
    // profile (`dive_runtime::note_canonical_stop`); under `--compat=tlc`
    // TLC's order needs no note.
    if !profile_tlc && r.verdict.is_verdict() && r.verdict != Verdict::Success {
        r.notes.push(CANONICAL_STOP_NOTE.to_owned());
    }
    r
}

/// Runs the core engine.
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "the shared backend stages include explicit identity-bound distributed policy"
)]
pub(crate) fn check(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    profile_tlc: bool,
    switches: Option<&str>,
    sample: Option<usize>,
    route: Option<crate::core_temporal::Route>,
    distributed: Option<(&crate::DistributedOptions, &str)>,
    debugger: Option<&Arc<dyn dive_core::debug::Observer>>,
) -> (CoreOutcome, CoreRun) {
    let started = Instant::now();
    let text = switches
        .map(str::to_owned)
        .or_else(|| std::env::var("DIVE_CORE_OPT").ok());
    let core_opts = match text {
        Some(text) => match dive_core::Opts::parse(&text) {
            Ok(o) => o,
            Err(e) => {
                return (
                    CoreOutcome::Checked(Box::new(result(
                        unsupported(format!("DIVE_CORE_OPT: {e}")),
                        options,
                        ir,
                    ))),
                    CoreRun::default(),
                );
            }
        },
        None => dive_core::Opts::default(),
    };
    let core_opts = dive_core::Opts {
        memo_cap: memo_cap(options.budget.memory, options.workers.max(1)),
        ..core_opts
    };
    let mut run = CoreRun {
        opts: core_opts.describe(),
        metrics: Json::Null,
        searched: false,
        sample: Json::Null,
    };
    let finish = |mut r: CheckResult| {
        r.elapsed = started.elapsed();
        CoreOutcome::Checked(Box::new(r))
    };
    if let Some(feature) = option_refusal(options, profile_tlc) {
        return (finish(result(unsupported(feature), options, ir)), run);
    }
    let prepared = match prepare_observed(
        ir,
        config,
        options,
        profile_tlc,
        core_opts,
        route.is_some(),
        debugger.cloned(),
    ) {
        Ok(p) => p,
        Err(r) => return (finish(*r), run),
    };
    if (options.store_compact_bytes.is_some() || options.independent_frontier)
        && (prepared.plan.effects.any || !prepared.model.implied_temporals.is_empty())
    {
        return (
            finish(result(
                unsupported("Store reclamation with runtime effects or native temporal capture"),
                options,
                ir,
            )),
            run,
        );
    }
    if distributed.is_some()
        && let Some(feature) = distributed_model_refusal(&prepared.plan, &prepared.model)
    {
        return (finish(result(unsupported(feature), options, ir)), run);
    }
    if !prepared.model.implied_temporals.is_empty()
        && (options.frontier_memory_bytes.is_some()
            || options.frontier_directory.is_some()
            || options.candidate_memory_bytes.is_some()
            || options.candidate_directory.is_some()
            || options.store_compact_bytes.is_some()
            || options.predecessor_memory_bytes.is_some()
            || options.predecessor_directory.is_some())
    {
        return (
            finish(result(
                unsupported("spilled or reclaimed native temporal storage under the core engine"),
                options,
                ir,
            )),
            run,
        );
    }
    if prepared.plan.effects.registers && sample.is_some() {
        return (
            finish(result(
                unsupported("--core-sample with TLCSet or TLCGet (M1 core)"),
                options,
                ir,
            )),
            run,
        );
    }
    if prepared.plan.effects.draws && sample.is_some() {
        return (
            finish(result(
                unsupported("--core-sample with RandomElement (M1 core)"),
                options,
                ir,
            )),
            run,
        );
    }
    if route
        .flatten()
        .is_some_and(|(_, _, roots)| roots != dive_explore::storage::InitialStates::All)
        && (prepared.model.implied_temporals.is_empty()
            || dive_runtime::admission::effectful(&prepared.model))
    {
        return (
            finish(result(
                unsupported(
                    "cold fair search requires pure source with a configured temporal property",
                ),
                options,
                ir,
            )),
            run,
        );
    }
    if let Some(route) = route
        && !prepared.model.implied_temporals.is_empty()
    {
        // A start-up stop is reported before the temporal route plans
        // (#968), but after its refusals, which hold at any budget.
        if let Some(feature) = crate::core_temporal::static_refusal(ir, config, options, &prepared)
        {
            return (finish(result(unsupported(feature), options, ir)), run);
        }
        if let Some(verdict) = startup_stop(options) {
            let checks = route.map(
                |(schedule, _, _)| dive_liveness::analyze::periodic::Checks {
                    schedule,
                    ..Default::default()
                },
            );
            // The binder's messages, then what Core's start-up printed after
            // them (`Startup::startup` holds both, as plain text).
            let mut output = prepared.model.output.clone();
            let binder = output
                .iter()
                .filter(|m| matches!(m, dive_model::Message::Print(_)))
                .count();
            output.extend(
                prepared.start.startup[binder.min(prepared.start.startup.len())..]
                    .iter()
                    .cloned()
                    .map(dive_model::Message::Print),
            );
            return (
                CoreOutcome::TemporalFailure(Box::new(crate::temporal::core_startup_stop(
                    verdict,
                    started,
                    checks,
                    prepared.model.symmetry_liveness().clone(),
                    output,
                ))),
                run,
            );
        }
        let outcome = temporal_on_stack(
            ir,
            config,
            options,
            (profile_tlc, core_opts),
            prepared,
            route,
            &mut run,
        );
        return match outcome {
            CoreOutcome::Checked(r) => (finish(*r), run),
            t @ (CoreOutcome::Temporal(_) | CoreOutcome::TemporalFailure(_)) => (t, run),
        };
    }
    let Prepared {
        model,
        host,
        start,
        mut store,
        plan,
        compile_time,
    } = prepared;
    // A distributed run's worker identity, and its refusals (state-variable
    // arity, source admission, local Java classes), hold at any budget, so
    // they precede the start-up stop (#973).
    let identity = match &distributed {
        Some((_, config_hash)) => {
            match crate::core_worker::identity(ir, config, options, config_hash, core_opts) {
                Ok(identity) => Some(identity),
                Err(verdict) => return (finish(result(verdict, options, ir)), run),
            }
        }
        None => None,
    };
    if let Some(verdict) = startup_stop(options) {
        // The start-up output and symmetry diagnostics are retained, as a
        // stop in the search retains them.
        let Startup { mut sym, startup } = start;
        let mut stopped = result(verdict, options, ir);
        stopped.warnings.append(&mut sym.warnings);
        stopped.notes.extend(sym.note.take());
        stopped.output = startup;
        return (finish(stopped), run);
    }
    // The armed budget (time and memory, ADR 0107) is polled once per
    // expanded state and per initial state; `--max-states` and `--max-depth` are admission limits.
    let budget = &options.budget;
    // Every observable result is the one-worker result
    // (`docs/developer/core/07-engine.md` §3), in both profiles: under
    // TLC's string order the table is sealed, so no worker creates a token.
    let workers = if let Some((remote, _)) = &distributed {
        remote.workers.len()
    } else if options.seen_disk.as_ref().is_some_and(|s| !s.partitioned)
        || options.sequential
        || debugger.is_some()
    {
        1
    } else {
        options.workers.max(1)
    };
    let poll = || !matches!(budget.poll(), Ok(None));
    let explore_started = Instant::now();
    let sample = sample.map(|states| dive_explore::sample::Sample {
        seed: options.seed.cast_unsigned(),
        states,
    });
    let coordinator = if let (Some((remote, _)), Some(identity)) = (&distributed, &identity) {
        match dive_explore::distributed::Coordinator::new((*remote).clone(), identity, &store) {
            Ok(coordinator) => Some(coordinator),
            Err(error) => {
                return (
                    finish(result(
                        Verdict::ResourceLimit {
                            message: error.message().into(),
                        },
                        options,
                        ir,
                    )),
                    run,
                );
            }
        }
    } else {
        None
    };
    let explore_config = dive_explore::Config {
        opts: core_opts,
        max_states: options.max_states,
        max_depth: options.max_depth,
        workers,
        warn: options.eval.warnings,
        sample,
    };
    let default_compact = (options.store_compact_bytes.is_none()
        && options.store == dive_store::StoreMode::Fingerprint
        && sample.is_none()
        && debugger.is_none()
        && coordinator.is_none()
        && !plan.effects.any
        && model.implied_temporals.is_empty()
        && options.frontier_memory_bytes.is_none()
        && options.frontier_directory.is_none()
        && options.candidate_memory_bytes.is_none()
        && options.candidate_directory.is_none()
        && options.predecessor_memory_bytes.is_none()
        && options.predecessor_directory.is_none()
        && options.seen_disk.is_none())
    .then_some(DEFAULT_STORE_COMPACT_BYTES)
    .or(options.independent_frontier.then_some(64 << 20));
    let storage = dive_explore::storage::Storage {
        exact: options.store == dive_store::StoreMode::Exact,
        initial_states: dive_explore::storage::InitialStates::All,
        predecessor_memory_bytes: options.predecessor_memory_bytes,
        predecessor_directory: options.predecessor_directory.clone(),
        frontier_memory_bytes: options.frontier_memory_bytes,
        independent_frontier: options.independent_frontier,
        frontier_directory: options.frontier_directory.clone(),
        candidate_memory_bytes: options.candidate_memory_bytes,
        candidate_directory: options.candidate_directory.clone(),
        store_compact_bytes: options.store_compact_bytes.or(default_compact),
        store_compact_default: options.store_compact_bytes.is_none() && default_compact.is_some(),
        seen_disk: options
            .seen_disk
            .as_ref()
            .map(|options| dive_explore::storage::DiskOptions {
                partitioned: options.partitioned,
                limits: options.limits,
                directory: options.directory.clone(),
            }),
    };
    run.searched = true;
    let out = if let Some(coordinator) = &coordinator {
        dive_explore::explore_distributed(
            &plan,
            &store,
            explore_config,
            &poll,
            Some(&search_work_control(budget)),
            &storage,
            coordinator,
        )
    } else {
        dive_explore::explore_owned(
            &plan,
            &mut store,
            explore_config,
            &poll,
            Some(&search_work_control(budget)),
            &storage,
            None,
            None,
        )
    };
    if let Some(sample) = sample {
        run.sample = crate::core_sample::compare(
            (&model, &host),
            &plan,
            &store,
            core_opts,
            options.eval.warnings,
            sample,
            &out.sampled,
        );
    }
    run.metrics = metrics_json(&plan, &out.metrics, compile_time);
    if let Some(Err(reason)) = &out.metrics.orbit_census {
        run.metrics["orbit_census"] = json!({"status": "unavailable", "reason": reason});
    } else if out.verdict == dive_explore::Verdict::Success
        && out.stats.left_on_queue == 0
        && let Some(Ok(census)) = &out.metrics.orbit_census
        && census.sizes.values().sum::<u64>() == out.stats.distinct
    {
        let labeled: u128 = census
            .sizes
            .iter()
            .map(|(&size, &n)| u128::from(size) * u128::from(n))
            .sum();
        run.metrics["orbit_census"] = json!({
            "representatives": out.stats.distinct,
            "orbit_sizes": census.sizes,
            "labeled_orbit_union": labeled.to_string(),
            "weighted_generated": (census.successors + u128::from(out.stats.initial_generated)).to_string(),
            "scope": "full orbits of retained mathematical states; reachable labeled count requires equivariance of Init, Next and constraints; generated count also requires equivariant successor multiplicities"
        });
    }
    let search = Search {
        ir,
        config,
        options,
        profile_tlc,
        workers,
        explore_started,
    };
    let disk_usage = out.metrics.storage.seen.clone();
    let observed_stats = out.stats;
    let mut observed_output = start.startup.clone();
    observed_output.extend(out.printed.iter().cloned());
    let disk_print_replay =
        disk_usage.is_some() && out.replayed.is_some() && !observed_output.is_empty();
    let mut r = safety_result(&search, &model, &store, &plan, start, out);
    if let Some(coordinator) = coordinator {
        let facts = coordinator.usage();
        r.workers.used = facts.workers.len();
        r.workers.reason=Some("authenticated remote Core source evaluation with coordinator-owned ordered commit; Init and selected-word/diagnostic replay remain local".into());
    }
    if let Some(usage) = disk_usage {
        r.seen = Some(native_seen_usage(&usage));
        let creation_failed = usage
            .first_failure
            .as_ref()
            .is_some_and(|failure| failure.phase == "Creation");
        if !options.seen_disk.as_ref().is_some_and(|s| s.partitioned) {
            r.workers.used = usize::from(!creation_failed);
            r.workers.reason = Some("fallible Core disk membership executes inline; the requested worker width is retained separately".into());
        } else if creation_failed {
            r.workers.used = 0;
        } else if options.sequential {
            r.workers.used = 1;
            r.workers.sequential = true;
            r.workers.reason = Some("--sequential requested inline ordered admission".into());
        }
        if r.stats.is_none() && !creation_failed {
            r.stats = Some(dive_runtime::Stats {
                generated: observed_stats.generated,
                distinct: observed_stats.distinct,
                left_on_queue: observed_stats.left_on_queue,
                depth: observed_stats.depth,
                initial_generated: observed_stats.initial_generated,
            });
        }
        if r.output.is_empty() {
            r.output = observed_output;
        }
        if disk_print_replay && usage.first_failure.is_none() {
            r.verdict =
                unsupported("Core disk invariant-error replay after actual Print/PrintT execution");
            r.counterexample = None;
            r.diagnostic_witness = None;
        }
        if let Some(event) = usage.deciding_event_before_failure {
            r.notes.push(format!("Core disk storage failed after the observed search event {event:?}; it supplies no certified model verdict."));
        }
    }
    (finish(r), run)
}

pub(crate) fn native_seen_usage(
    usage: &dive_explore::storage::DiskUsage,
) -> dive_runtime::SeenUsage {
    use dive_runtime::SeenPhase;
    dive_runtime::SeenUsage {
        audited_fingerprint_gap: usage.audited_fingerprint_gap,
        limits: usage.limits,
        admitted_keys: usage.admitted_keys,
        before_cleanup: usage.before_cleanup,
        after_cleanup: usage.after_cleanup,
        first_failure: usage
            .first_failure
            .as_ref()
            .map(|failure| dive_runtime::SeenFailure {
                phase: match failure.phase {
                    "Creation" => SeenPhase::Creation,
                    "Insertion" | "RecoveryInsertion" => SeenPhase::Insertion,
                    "AdmissionMembership" => SeenPhase::AdmissionMembership,
                    "ReplayMembership" => SeenPhase::ReplayMembership,
                    "ReplayInsertion" => SeenPhase::ReplayInsertion,
                    _ => SeenPhase::Cleanup,
                },
                error: failure.error.clone(),
                generated: (!matches!(failure.phase, "Creation" | "Cleanup"))
                    .then_some(failure.generated),
                predecessor: failure.predecessor,
                successor_ordinal: (!matches!(failure.phase, "Creation" | "Cleanup"))
                    .then_some(failure.successor_ordinal),
            }),
        deciding_event_before_failure: match &usage.deciding_event_before_failure {
            Some(dive_explore::Verdict::Success) => Some(Verdict::Success),
            Some(dive_explore::Verdict::InvariantViolated { property, initial }) => {
                Some(Verdict::InvariantViolated {
                    property: property.clone(),
                    initial: *initial,
                })
            }
            Some(dive_explore::Verdict::Deadlock) => Some(Verdict::Deadlock),
            _ => None,
        },
        replay: None,
    }
}

/// Runs the temporal route on a thread with the evaluator's stack.
fn temporal_on_stack(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    (profile_tlc, core_opts): (bool, dive_core::Opts),
    prepared: Prepared,
    route: crate::core_temporal::Route,
    run: &mut CoreRun,
) -> CoreOutcome {
    let joined = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                crate::core_temporal::check(
                    ir,
                    config,
                    options,
                    (profile_tlc, core_opts),
                    prepared,
                    route,
                    run,
                )
            })
            .map(std::thread::ScopedJoinHandle::join)
    });
    match joined {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) => CoreOutcome::Checked(Box::new(result(
            Verdict::InternalError {
                message: "the core temporal thread panicked".into(),
            },
            options,
            ir,
        ))),
        Err(e) => CoreOutcome::Checked(Box::new(result(
            Verdict::InternalError {
                message: format!("the core temporal thread did not start: {e}"),
            },
            options,
            ir,
        ))),
    }
}

/// The counterexample of a core run, as a `dive_runtime` counterexample, so
/// that the report and its `dive-trace` document are built the same way.
pub(crate) fn counterexample(
    model: &dive_model::Model,
    store: &Arc<dive_core::Store>,
    trace: &[dive_explore::TraceState],
) -> Result<dive_runtime::Counterexample, String> {
    let ctx = model.evaluator.options().ctx;
    let mut heap = dive_core::Heap::new(Arc::clone(store), false, false);
    let mut states = Vec::with_capacity(trace.len());
    for t in trace {
        let mut values = Vec::with_capacity(t.values.len());
        let mut partial = Vec::with_capacity(t.values.len());
        for &h in &t.values {
            partial.push(if h.is_empty() {
                None
            } else {
                Some(to_value(&mut heap, model, h, ctx)?)
            });
        }
        // Keep display text and partial bindings separate from complete rows.
        let incomplete = partial.iter().any(Option::is_none);
        let text = if incomplete {
            Some(crate::core_ending::state_text(model, &partial))
        } else {
            values.extend(partial.iter().flatten().cloned());
            None
        };
        let (label, action) = match t.action {
            None => ("<Initial predicate>".to_owned(), None),
            Some(i) => {
                let a = model
                    .actions
                    .get(i)
                    .ok_or_else(|| format!("trace action {i} out of range"))?;
                (a.label().to_owned(), action_name(model, a))
            }
        };
        states.push(dive_runtime::TraceState {
            label,
            action,
            values,
            text,
            partial: if incomplete {
                t.action.and_then(|i| model.actions.get(i)).map(|action| {
                    dive_runtime::PartialState {
                        values: partial,
                        expression: action.expr(),
                    }
                })
            } else {
                None
            },
        });
    }
    Ok(dive_runtime::Counterexample { states })
}

/// The action's name in traces.
pub(crate) fn action_name(
    model: &dive_model::Model,
    a: &dive_eval::next::Action,
) -> Option<String> {
    use dive_ir::actions::ActionName;
    match a.name() {
        ActionName::Def(d) => Some(model.ir.def(d).name.clone()),
        ActionName::Std(op) => Some(op.name().to_owned()),
        ActionName::Builtin(name) => Some(name.to_owned()),
        ActionName::Unnamed => None,
    }
}

/// Converts a persistent handle to a `dive_value::Value` (the reverse of the
/// binder boundary, `dive_core::convert`), for reports and validation.
pub(crate) fn to_value(
    heap: &mut dive_core::Heap,
    model: &dive_model::Model,
    h: dive_core::Handle,
    ctx: dive_value::Ctx,
) -> Result<Value, String> {
    dive_core::print::to_value(
        heap,
        &model.names,
        h,
        ctx,
        dive_core::print::Conversion::Report,
    )
}

pub(crate) fn metrics_json(
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
    let mut report = json!({
        "compile_ms": compile_time.as_secs_f64() * 1e3,
        "distributed":m.distributed,
        "explore_ms": m.explore_time.as_secs_f64() * 1e3,
        "expand_ms": m.expand_time.as_secs_f64() * 1e3,
        "commit_ms": m.commit_time.as_secs_f64() * 1e3,
        "plan": {
            "nodes": plan.stats.nodes, "generator_nodes": plan.stats.gnodes,
            "inlined": plan.stats.inlined, "aliased_args": plan.stats.aliased,
            "thunk_args": plan.stats.thunks, "by_name": plan.stats.by_name,
            "folded": plan.stats.folded, "fold_abandoned": plan.stats.fold_abandoned,
            "fold_skipped": plan.stats.fold_skipped, "fold_fuel": plan.stats.fold_fuel,
            "fold_ms": std::time::Duration::from_nanos(plan.stats.fold_nanos).as_secs_f64() * 1e3, "cached_defs": plan.stats.cached_defs, "speculative": plan.stats.speculative, "beta": plan.stats.beta, "substituted": plan.stats.substituted, "memo_sites": plan.code.sites.len(),
            "startup_values": plan.stats.startup_values, "startup_ms": std::time::Duration::from_nanos(plan.stats.startup_nanos).as_secs_f64() * 1e3, "startup_call_hits": plan.stats.startup_call_hits, "startup_fallbacks": plan.stats.startup_fallbacks,
        },
        "probed": m.probed,
        "discarded_before_intern": m.discarded_before_intern,
        "candidate_storage": {
            "memory_bytes_per_worker": m.storage.candidates.memory_bytes_per_worker,
            "scope": "encoded occurrence/DAG queues; active Arena roots, per-record codec scratch, names, other caches, accepted Store and total RSS excluded",
            "max_record_bytes": dive_store::spill::MAX_RECORD_BYTES,
            "identity_close_entries_limit": dive_core::heap::MAX_LOCAL_CLOSE_ENTRIES,
            "identity_close_scratch_bytes_limit": dive_core::heap::MAX_LOCAL_CLOSE_SCRATCH_BYTES,
            "peak_retained_bytes": m.storage.candidates.peak_retained_bytes,
            "retained_bytes": m.storage.candidates.retained_bytes,
            "committed_spill_bytes": m.storage.candidates.spilled_bytes,
            "write_accounting_complete": m.storage.candidates.failures.is_empty(),
            "failures": m.storage.candidates.failures,
            "search_event_before_storage_failure": m.storage.candidates.deciding_event_before_failure,
        },
        "store_reclamation": {
            "failures": m.storage.reclamation.failures,
            "trigger_bytes": m.storage.reclamation.trigger_bytes,
            "trigger_source": m.storage.reclamation.trigger_bytes.map(|_| if m.storage.reclamation.default_trigger { "default" } else { "requested" }),
            "skipped": m.storage.reclamation.skipped,
            "scope": if m.storage.reclamation.between_waves {
                "measured Store trigger between committed waves and levels. Only partial chunk roots, permanent startup and names need remapping. Total RSS and disk are outside the trigger"
            } else { "completed-level measured Store trigger; live frontier, permanent startup and names are retained; not a hard cap or total RSS bound" },
            "collections": m.storage.reclamation.collections,
            "reclaimed_nodes": m.storage.reclamation.reclaimed_nodes,
            "peak_before_bytes_lower_bound": m.storage.reclamation.peak_before_bytes,
            "peak_after_bytes_lower_bound": m.storage.reclamation.peak_after_bytes,
            "peak_after_nodes": m.storage.reclamation.peak_after_nodes,
        },
        "exact_states": m.storage.exact_states,
        "conjuncts": {"total": m.conjuncts_total, "parent_skipped": m.conjuncts_skipped},
        "store": {
            "hits": m.store.hits, "misses": m.store.misses, "nodes": m.store.nodes, "handles": m.store.handles,
            "memory": {
                "known_retained_bytes_lower_bound": m.store_memory.known_retained_bytes(),
                "accounting_complete": false,
                "fixed_bytes": m.store_memory.fixed_bytes,
                "slab_bytes": m.store_memory.slab_bytes,
                "boxed_payload_bytes": m.store_memory.boxed_payload_bytes,
                "current_table_bytes": m.store_memory.current_table_bytes,
                "retired_table_bytes": m.store_memory.retired_table_bytes,
                "retired_tables": m.store_memory.retired_tables,
                "retirement_tracking_bytes": m.store_memory.retirement_tracking_bytes,
                "rehash_scratch_bytes_cumulative": m.store_memory.rehash_scratch_bytes,
                "largest_individual_rehash_scratch_bytes": m.store_memory.largest_rehash_scratch_bytes,
                "strings": {
                    "unique_payload_bytes": m.store_memory.strings.payload_bytes,
                    "text_slots_bytes": m.store_memory.strings.text_slots_bytes,
                    "index_entries_bytes_lower_bound": m.store_memory.strings.index_entries_bytes,
                    "index_capacity": m.store_memory.strings.index_capacity,
                    "count": m.store_memory.strings.strings,
                },
                "external_shared_owners": {
                    "printer_names": m.store_memory.has_printer,
                    "random_streams": m.store_memory.has_draws,
                    "registers": m.store_memory.has_registers,
                },
                "scope": "requested persistent-store component storage before explorer worker teardown; lower bound, not RSS or peak; excludes allocator and Arc metadata, opaque string-index bucket overhead/spare capacity, external shared objects, worker arenas/caches, graph buffers and in-progress growth; rehash scratch fields are allocation traffic and largest individual temporary, not retained or concurrent peak bytes",
            },
        },
        "arena_peak": {"nodes": m.arena_peak.0, "handles": m.arena_peak.1},
        "thunks": {"forced": m.thunks.0, "cached": m.thunks.1},
        "memo": memo,
    });
    if let Some(roots) = &m.initial_roots {
        report["initial_roots"] = json!(roots);
    }
    if m.initial_workers > 0 {
        report["parallel_init_workers"] = json!(m.initial_workers);
    }
    report
}

/// Existing armed budget shared by history work; reporting keeps the actual
/// Search/Capture owner at the ordinary explorer/temporal boundaries.
pub(crate) fn search_work_control(
    budget: &dive_runtime::budget::Budget,
) -> Arc<dyn dive_core::eval::WorkControl> {
    #[derive(Debug)]
    struct Control(dive_runtime::budget::Budget);
    impl dive_core::eval::WorkControl for Control {
        fn proceed(&self) -> bool {
            matches!(self.0.poll(), Ok(None))
        }
    }
    Arc::new(Control(budget.clone()))
}
