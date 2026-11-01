//! `--engine=core`: the M0 spike of the new execution core (ADR 0117,
//! `docs/developer/core/`).
//!
//! The reference binder builds the model (constants, overrides, split
//! actions and startup values); the core compiles it into a plan and
//! explores it, level by level, on `--workers` threads. Anything the core does not implement is an
//! `unsupported` verdict naming it; there is no fallback to the reference
//! engine.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use dive_check::{CheckOptions, CheckResult, Stats, Verdict, Workers};
use dive_core::compile::{ConstBinding, Handoff, Role, RoleSubst};
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
    /// The new core (`dive-core` and `dive-explore`): default profile,
    /// safety only.
    Core,
    /// The new core where it implements the run, the reference checker
    /// otherwise (`docs/developer/core/08-correctness.md` §3). A refusal
    /// before the search (static) or during it (dynamic) runs the whole
    /// check again on the reference engine, and the report names the
    /// engine that ran and why.
    Auto,
}

impl Engine {
    /// Parses `reference`, `core` or `auto`.
    ///
    /// ```
    /// assert_eq!(dive_api::Engine::parse("core"), Some(dive_api::Engine::Core));
    /// assert_eq!(dive_api::Engine::parse("auto"), Some(dive_api::Engine::Auto));
    /// assert_eq!(dive_api::Engine::parse("fast"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "reference" => Some(Self::Reference),
            "core" => Some(Self::Core),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    /// The name used on the command line and in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Core => "core",
            Self::Auto => "auto",
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
    /// A temporal report, finished by the reference engine's path.
    Temporal(Box<crate::temporal::Report>),
    /// A real temporal resource stop with actual execution facts.
    TemporalFailure(Box<crate::temporal::Failure>),
}

impl CoreOutcome {
    /// The safety result, if this is one.
    pub(crate) fn checked(&self) -> Option<&CheckResult> {
        match self {
            Self::Checked(r) => Some(r),
            Self::Temporal(_) | Self::TemporalFailure(_) => None,
        }
    }
}

/// Why `--engine=auto` ran the reference engine.
#[derive(Clone, Debug)]
pub(crate) struct Fallback {
    /// `static` (refused before the search) or `dynamic` (during it).
    pub(crate) stage: &'static str,
    /// The feature the core refused.
    pub(crate) reason: String,
}

impl Fallback {
    /// The fallback for a core run that ended in `unsupported`, if it did.
    pub(crate) fn of(outcome: &CoreOutcome, run: &CoreRun) -> Option<Self> {
        let result = outcome.checked()?;
        match &result.verdict {
            Verdict::Unsupported { feature } => Some(Self {
                stage: if run.searched { "dynamic" } else { "static" },
                reason: feature.clone(),
            }),
            _ => None,
        }
    }
}

fn unsupported(feature: impl Into<String>) -> Verdict {
    Verdict::Unsupported {
        feature: feature.into(),
    }
}

pub(crate) fn result(verdict: Verdict, opts: &CheckOptions, ir: &Ir) -> CheckResult {
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
/// an instantiated definition). Values evaluated under substitutions of
/// their own (nested instances) are refused.
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
fn option_refusal(options: &CheckOptions, profile_tlc: bool) -> Option<String> {
    let e = &options.eval;
    let refused = if options.store == dive_store::StoreMode::Exact {
        "--store exact (the core keeps 64-bit state keys)"
    } else if options.tlc_fingerprints && !profile_tlc {
        "TLC fingerprints outside --compat=tlc"
    } else if e.max_depth != dive_eval::Options::dive().max_depth {
        "an evaluation depth other than the default"
    } else if e.action_composition != dive_eval::Options::dive().action_composition
        && e.action_composition != dive_eval::Options::tlc().action_composition
    {
        "--action-composition=tlc"
    } else if options.seen_disk.is_some()
        || options.frontier_memory_bytes.is_some()
        || options.frontier_directory.is_some()
        || options.predecessor_memory_bytes.is_some()
        || options.predecessor_directory.is_some()
    {
        "spilling the seen set, frontier or predecessor log to disk"
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
    if model.symmetry.is_some() && !model.implied_temporals.is_empty() {
        return Some("SYMMETRY (M0 core engine)".into());
    }
    None
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
        constants,
        redirects,
        std_redirects,
        precomputed,
        overrides,
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

/// The reference engine's note on a stop in the default profile (D-010).
/// `core_rules.rs` checks that the core's notes equal the reference
/// engine's on every fixture.
const CANONICAL_STOP_NOTE: &str = "The default profile explores sets in canonical order (D-010). When several violations are reachable at the same depth, which one is reported, and the counts at the stop, may differ under --compat=tlc; dive does not yet detect whether the order mattered in this run.";

/// The core's value semantics for the run's evaluation options: each of
/// `--compat=tlc`'s components maps to its own switch.
fn semantics(eval: &dive_eval::Options) -> dive_core::Semantics {
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
/// first fed the reference binder's start-up names, in TLC's encounter
/// order, and then sealed: a name TLC meets later gets a token that depends
/// on evaluation order, which the core refuses rather than guess.
fn store_for(
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

/// A model that fails to bind ends the run before any search, as on the
/// reference engine (for example TLC's `TLC_INTEGER_TOO_BIG` for a numeral
/// outside 32 bits under `--compat=tlc`). The core binds without the run's
/// host, so its failure is confirmed by the reference engine's own binding:
/// when that fails the same way, its result (verdict, printed output and
/// warnings) is the run's; otherwise the run is refused.
fn binding_failure(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    failure: &dive_model::Failure,
) -> CheckResult {
    let mine = Verdict::from(&failure.error);
    let reference = dive_check::check(ir, config, options);
    if reference.verdict == mine && reference.stats.is_none() {
        return reference;
    }
    result(
        unsupported(format!(
            "a binding failure the reference engine does not reproduce under the core engine: {failure}"
        )),
        options,
        ir,
    )
}

/// `SYMMETRY`, prepared as the reference engine prepares it: its maps by
/// model-value name, its start-up warnings and the default profile's
/// D-010 note.
#[derive(Default)]
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

const SYMMETRY_NOTE: &str = "The default profile canonicalizes symmetry keys in canonical value order (D-010). A non-invariant VIEW or asymmetric specification may differ under --compat=tlc; order relevance is not yet detected.";

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
    // The default profile's canonical keys may differ from TLC's (D-010);
    // under `--compat=tlc` the keys are TLC's.
    if !profile_tlc {
        out.note = Some(SYMMETRY_NOTE.into());
    }
    Ok(out)
}

/// Gives the store the run's random streams: the startup stream where the
/// binder left it, and states keyed by the reference engine's seeder.
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
fn memo_cap(ceiling: Option<u64>, workers: usize) -> usize {
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
    // The reference engine's refusal of random streams under VIEW or
    // SYMMETRY, before binding runs start-up draws.
    if let Some(feature) = dive_model::transformed_randomness(ir, config) {
        return Err(Box::new(result(unsupported(feature), options, ir)));
    }
    // The reference engine's host: constant definitions and `ASSUME`s
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
    let bind = dive_model::BindOptions {
        eval: options.eval,
        no_deadlock: options.no_deadlock,
        host: Some(Arc::clone(&host) as Arc<dyn dive_eval::Host>),
        // The temporal route reports the requested policy, as the reference
        // engine binds it; it only matters with temporal properties.
        symmetry_liveness: if temporal {
            options.symmetry_liveness
        } else {
            defaults.symmetry_liveness
        },
    };
    let mut model = match dive_model::bind(Arc::clone(ir), config, &bind) {
        Ok(m) => m,
        Err(f) => return Err(Box::new(binding_failure(ir, config, options, &f))),
    };
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
    // The ASSUMEs are startup evaluation (the binder's, `04-plan-compiler.md`
    // §2): a false or failing one ends the run as the reference engine
    // reports it, before any core work.
    let assumed = match model.check_assumptions() {
        dive_eval::AssumptionOutcome::AllTrue => None,
        dive_eval::AssumptionOutcome::False {
            property, message, ..
        } => Some(Verdict::AssumptionFalse { property, message }),
        dive_eval::AssumptionOutcome::Error { error, message, .. } => {
            Some(dive_check::classify::assumption_error(&error, message))
        }
    };
    if let Some(v) = assumed {
        // Output printed while evaluating the ASSUMEs is the reference
        // engine's to order; the core does not report it.
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
    // ASSUMEs) comes first, as the reference engine reports it.
    let mut startup: Vec<String> = model
        .output
        .iter()
        .filter_map(|m| match m {
            dive_model::Message::Print(text) => Some(text.clone()),
            dive_model::Message::Warning { .. } => None,
        })
        .collect();
    startup.extend(model.take_output());
    let h = match handoff(&model, ir, std::mem::take(&mut sym.maps)) {
        Ok(h) => h,
        Err(e) => return Err(Box::new(result(unsupported(e), options, ir))),
    };
    let store = match store_for(&model, core_opts.hashcons, semantics(&options.eval)) {
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
    let plan = match dive_core::compile(&h, Arc::clone(&store), core_opts) {
        Ok(p) => p,
        Err(e) => {
            let v = match e.into_parts() {
                (dive_core::ErrorKind::Internal, message) => Verdict::InternalError { message },
                (_, message) => unsupported(message),
            };
            return Err(Box::new(result(v, options, ir)));
        }
    };
    let compile_time = compile_started.elapsed();
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
pub(crate) struct Startup {
    sym: Symmetry,
    startup: Vec<String>,
}

/// The safety result of a core search, in the reference engine's form.
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
    let Search {
        ir,
        config,
        options,
        profile_tlc,
        workers,
        explore_started,
    } = *search;
    let Startup { mut sym, startup } = start;
    let budget = &options.budget;
    let owned_budget = out.verdict == dive_explore::Verdict::Budget;
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
        return result(v, options, ir);
    }
    if plan.effects.prints
        && !owned_budget
        && (out.verdict != dive_explore::Verdict::Success || !out.warnings.is_empty())
    {
        // TLC prints again while it rebuilds a trace or reports an error,
        // and interleaves warnings with the output; the core does not
        // reproduce either yet.
        let v = match out.verdict {
            dive_explore::Verdict::Unsupported { feature } => unsupported(feature),
            dive_explore::Verdict::Internal { message } => Verdict::InternalError { message },
            _ => unsupported(
                "Print or PrintT in a run that does not end in success, or that warns (M1 core)",
            ),
        };
        return result(v, options, ir);
    }
    let failed = out.verdict == dive_explore::Verdict::Failed;
    let verdict = match out.verdict {
        dive_explore::Verdict::Success => Verdict::Success,
        dive_explore::Verdict::InvariantViolated { property, initial } => {
            Verdict::InvariantViolated { property, initial }
        }
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
                exhausted.during(dive_check::budget::Phase::Search),
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
        } => Verdict::Stopped(Box::new(dive_check::budget::Stopped {
            resource: match limit {
                dive_explore::Limit::States => dive_check::budget::Resource::States,
                dive_explore::Limit::Depth => dive_check::budget::Resource::Depth,
            },
            limit: max,
            observed,
            elapsed_ms: u64::try_from(explore_started.elapsed().as_millis()).unwrap_or(u64::MAX),
            phase: dive_check::budget::Phase::Search,
        })),
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
    );
    let mut verdict = verdict;
    let mut in_init = matches!(
        verdict,
        Verdict::InvariantViolated { initial: true, .. }
            | Verdict::PropertyViolatedInitially { .. }
            | Verdict::ModelError { code: 2115, .. }
    ) || (matches!(verdict, Verdict::Stopped(_)) && out.in_init);
    let mut trace = out.trace.clone();
    let mut stats = out.stats;
    // Whether the trace ends in the successor a check failed on (the
    // reference engine's `Stop::then`).
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
    // The reference engine's boundary for a plan that draws: TLC rebuilds
    // a trace whose predecessor is initial from the original state object,
    // whose stream is already consumed, so its last state can go missing
    // (`docs/tlc-differences.md`, D-002). The reference engine refuses it
    // with the statistics of the stop.
    let mut draw_note = None;
    if plan.effects.draws
        && then
        && verdict.is_verdict()
        && matches!(&trace, Some(Ok(t)) if t.len() == 2)
    {
        draw_note = Some(format!(
            "The search reached {verdict:?}; its first-successor random trace has an unproved TLC replay/printing contract (D-002). Statistics record that stopping prefix, not a completed certification."
        ));
        verdict = unsupported("TLC initial-predecessor random counterexample replay is not proved");
        trace = None;
    }
    let prints = !in_init || matches!(verdict, Verdict::EvaluationError { .. });
    let mut r = result(verdict, options, ir);
    if matches!(
        &r.verdict,
        Verdict::EvaluationError {
            site: dive_check::ErrorSite::Invariant,
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
    if plan.effects.any && workers > 1 {
        r.workers.used = 1;
        r.workers.reason = Some(if plan.effects.prints {
            "the specification prints (Print or PrintT): the core keeps TLC's order of the output with one worker".into()
        } else if plan.effects.draws {
            "the specification draws (RandomElement): the core draws in TLC's order with one worker"
                .into()
        } else {
            "the specification uses TLCSet or TLCGet: the core keeps TLC's order of their effects with one worker".into()
        });
    } else {
        r.workers.used = workers;
        r.workers.reason = None;
    }
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
    // The reference engine does not apply ALIAS yet: it shows the states
    // themselves and says so (`dive_check::finish_bound_check`).
    if r.counterexample.is_some()
        && let Some(alias) = &config.alias
    {
        r.notes.push(format!(
            "ALIAS {} is not applied: the counterexample shows the states themselves (not implemented yet).",
            alias.text
        ));
    }
    // The reference engine's D-010 qualification of a stop in the default
    // profile (`dive_check::note_canonical_stop`); under `--compat=tlc`
    // TLC's order needs no note.
    if !profile_tlc && r.verdict.is_verdict() && r.verdict != Verdict::Success {
        r.notes.push(CANONICAL_STOP_NOTE.to_owned());
    }
    r
}

/// Runs the core engine.
#[allow(clippy::too_many_lines, reason = "the stages of a core run, in order")]
pub(crate) fn check(
    ir: &Arc<Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    profile_tlc: bool,
    switches: Option<&str>,
    sample: Option<usize>,
    route: Option<crate::core_temporal::Route>,
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
    let prepared = match prepare(ir, config, options, profile_tlc, core_opts, route.is_some()) {
        Ok(p) => p,
        Err(r) => return (finish(*r), run),
    };
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
    if let Some(route) = route
        && !prepared.model.implied_temporals.is_empty()
    {
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
        store,
        plan,
        compile_time,
    } = prepared;
    // The armed budget (time and memory, ADR 0107) is polled once per
    // expanded state and per initial state, as the reference engine polls
    // it; `--max-states` and `--max-depth` are admission limits.
    let budget = &options.budget;
    // Every observable result is the one-worker result
    // (`docs/developer/core/07-engine.md` §3), in both profiles: under
    // TLC's string order the table is sealed, so no worker creates a token.
    let workers = options.workers.max(1);
    let poll = || !matches!(budget.poll(), Ok(None));
    let explore_started = Instant::now();
    run.searched = true;
    let sample = sample.map(|states| dive_explore::sample::Sample {
        seed: options.seed.cast_unsigned(),
        states,
    });
    let out = dive_explore::explore_with_control(
        &plan,
        &store,
        dive_explore::Config {
            opts: core_opts,
            max_states: options.max_states,
            max_depth: options.max_depth,
            workers,
            warn: options.eval.warnings,
            sample,
        },
        &poll,
        None,
        Some(&search_work_control(budget)),
    );
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
    let search = Search {
        ir,
        config,
        options,
        profile_tlc,
        workers,
        explore_started,
    };
    let r = safety_result(&search, &model, &store, &plan, start, out);
    (finish(r), run)
}

/// Runs the temporal route on a thread with the evaluator's stack, as the
/// reference engine runs its temporal checks.
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

/// The counterexample of a core run, in the reference engine's form, so
/// that the report and its `dive-trace` document are built the same way.
pub(crate) fn counterexample(
    model: &dive_model::Model,
    store: &Arc<dive_core::Store>,
    trace: &[dive_explore::TraceState],
) -> Result<dive_check::Counterexample, String> {
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
        states.push(dive_check::TraceState {
            label,
            action,
            values,
            text,
            partial: if incomplete {
                t.action
                    .and_then(|i| model.actions.get(i))
                    .map(|action| dive_check::PartialState {
                        values: partial,
                        expression: action.expr(),
                    })
            } else {
                None
            },
        });
    }
    Ok(dive_check::Counterexample { states })
}

/// The action's name in traces, as the reference engine gives it.
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

/// Converts a persistent handle to a reference value (the reverse of the
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
    json!({
        "compile_ms": compile_time.as_secs_f64() * 1e3,
        "explore_ms": m.explore_time.as_secs_f64() * 1e3,
        "expand_ms": m.expand_time.as_secs_f64() * 1e3,
        "commit_ms": m.commit_time.saturating_sub(m.expand_time).as_secs_f64() * 1e3,
        "plan": {
            "nodes": plan.stats.nodes, "generator_nodes": plan.stats.gnodes,
            "inlined": plan.stats.inlined, "aliased_args": plan.stats.aliased,
            "thunk_args": plan.stats.thunks, "by_name": plan.stats.by_name,
            "folded": plan.stats.folded, "fold_abandoned": plan.stats.fold_abandoned,
            "fold_skipped": plan.stats.fold_skipped, "fold_fuel": plan.stats.fold_fuel,
            "fold_ms": std::time::Duration::from_nanos(plan.stats.fold_nanos).as_secs_f64() * 1e3, "cached_defs": plan.stats.cached_defs, "speculative": plan.stats.speculative, "beta": plan.stats.beta, "substituted": plan.stats.substituted, "memo_sites": plan.code.sites.len(),
        },
        "probed": m.probed,
        "discarded_before_intern": m.discarded_before_intern,
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
                "construction_history_view_bytes": m.store_memory.history_view_bytes,
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
    })
}

/// Existing armed budget shared by history work; reporting keeps the actual
/// Search/Capture owner at the ordinary explorer/temporal boundaries.
pub(crate) fn search_work_control(
    budget: &dive_check::budget::Budget,
) -> Arc<dyn dive_core::eval::WorkControl> {
    #[derive(Debug)]
    struct Control(dive_check::budget::Budget);
    impl dive_core::eval::WorkControl for Control {
        fn proceed(&self) -> bool {
            matches!(self.0.poll(), Ok(None))
        }
    }
    Arc::new(Control(budget.clone()))
}
