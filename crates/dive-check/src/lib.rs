//! `dive-check`: explicit-state safety model checking (ADR 0015).
//!
//! [`check`] binds a lowered specification to its TLC model configuration
//! through `dive-model` (ADR 0013 §8) and runs TLC's breadth-first search,
//! reproducing TLC's single-worker run exactly under `--compat=tlc`: the
//! same verdict and violated property, the same states generated, distinct
//! and left on the queue, the same depth, and the same shortest
//! counterexample with TLC's action labels.
//!
//! # What it checks
//!
//! - initial states and the next-state relation, through `dive-eval`'s
//!   successor generator, duplicates included;
//! - invariants on every new (or out-of-model) state, implied initial
//!   predicates on initial states, implied actions (`[][A]_v`) on every step
//!   including steps into seen states;
//! - `CONSTRAINT` and `ACTION_CONSTRAINT` (a constrained successor is still
//!   checked against the invariants, and prevents deadlock);
//! - deadlock: a state with no successor generated at all;
//! - pure safety symmetry, canonicalizing keys while retaining concrete
//!   explored states and independently checkable traces;
//! - `VIEW`, through the canonicalization hook (states are identified by the
//!   view's value; counterexamples show the concrete states explored);
//! - `ASSUME`s before the search, `POSTCONDITION`s after a complete one.
//!
//! The search stops at the first event in TLC's order (ADR 0015 §1) and
//! reports ADR 0015 §3's counts at that event; see [`mod@engine`].
//!
//! # Stores and profiles
//!
//! Fingerprint mode is the default. Under `--compat=tlc` it keys on TLC's own
//! state fingerprints (`StateFingerprinter::tlc`, TLC's variable order); the
//! default profile keys on ADR 0015 §5's cheaper two-level fingerprints
//! (`StateFingerprinter::new`), which reuse cached value fingerprints (see
//! [`CheckOptions::tlc_fingerprints`]). Exact mode (`--store exact`, D-007)
//! keys on canonical state encodings through the same engine.
//!
//! # What it refuses
//!
//! Anything not reproduced exactly is `unsupported`, naming the feature:
//! whatever `dive-model` lists in `Model::unsupported` (temporal properties,
//! unprepared symmetry, operators dive does not implement such as `IOUtils`),
//! and whatever the evaluator refuses during the search. Errors whose TLC
//! error class is not reproduced yet are reported as errors with an
//! unverified class ([`TlcClass::verified`]).
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_check::{CheckOptions, Verdict, check};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
//!            Next == x' = (x + 1) % 4\nInv == x < 3\n====\n";
//! let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
//! let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
//! let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n").into_config().expect("cfg");
//! let r = check(&ir, &cfg, &CheckOptions::tlc(0, 0));
//! assert_eq!(r.verdict, Verdict::InvariantViolated { property: "Inv".into(), initial: false });
//! let stats = r.stats.expect("statistics");
//! assert_eq!((stats.generated, stats.distinct, stats.left_on_queue, stats.depth), (4, 4, 0, 4));
//! assert_eq!(r.counterexample.expect("a trace").states.len(), 4);
//! ```
#![forbid(unsafe_code)]

pub mod budget;
pub mod capture;
pub mod checkpoint;
mod class;
pub mod dfid;
pub mod engine;
pub mod final_roles;
mod frontier;
pub use frontier::FrontierUsage;
mod host;
mod options;
mod parallel;
mod pool;
mod render;
mod result;
mod seen;
pub use seen::{SeenFailure, SeenOptions, SeenPhase, SeenReplay, SeenUsage};

use std::sync::Arc;
use std::time::Instant;

use dive_config::ModelConfig;
use dive_eval::{AssumptionOutcome, Profile};
use dive_ir::Ir;
use dive_model::{BindError, BindOptions, ErrorKind, Message};

pub use options::CheckOptions;
pub use render::{format_trace_state, render_human};
pub use result::{
    CheckResult, Collision, Counterexample, ErrorSite, PredecessorUsage, Stats, TlcClass,
    TraceState, Verdict, Workers, tlc_exit_status,
};

/// Checks the model `cfg` of the lowered specification `ir`.
///
/// Runs on its own thread with the evaluator's recommended stack.
#[must_use]
pub fn check(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> CheckResult {
    if let Err(error) = dive_eval::validate_ir(ir) {
        return bare(context_verdict(&error), opts, ir);
    }
    if opts.predecessor_directory.is_some() && opts.predecessor_memory_bytes.is_none() {
        return bare(
            Verdict::Unsupported {
                feature: "predecessor directory requires predecessor memory budget".into(),
            },
            opts,
            ir,
        );
    }
    let started = Instant::now();
    let (_watchdog, armed) = budget::arm(opts);
    let opts = armed.as_ref();
    let run = || run_check(ir, cfg, opts);
    let outcome = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, run)
            .map_err(|e| format!("cannot start the checking thread: {e}"))
            .and_then(|h| h.join().map_err(|_| "the checker panicked".to_owned()))
    });
    let mut result = match outcome {
        Ok(r) => r,
        Err(message) => bare(Verdict::InternalError { message }, opts, ir),
    };
    result.elapsed = started.elapsed();
    result
}

/// The verdict for an error of the binding stage.
pub(crate) fn model_error(e: &BindError) -> Verdict {
    Verdict::from(e)
}

impl From<&BindError> for Verdict {
    fn from(e: &BindError) -> Self {
        match &e.kind {
            ErrorKind::Tlc(ec) => Verdict::ModelError {
                code: ec.code(),
                name: ec.name().to_owned(),
                message: e.message.clone(),
            },
            ErrorKind::Eval { code } => Verdict::ModelError {
                code: *code,
                name: eval_code_name(*code).to_owned(),
                message: e.message.clone(),
            },
            ErrorKind::Unsupported { feature } => Verdict::Unsupported {
                feature: feature.clone(),
            },
            ErrorKind::Bug => Verdict::InternalError {
                message: e.message.clone(),
            },
        }
    }
}

/// The `EC` name of an evaluation error code `dive-eval` raises.
pub(crate) fn eval_code_name(code: u32) -> &'static str {
    match code {
        2215 => "TLC_EXPECTED_VALUE",
        2184 => "TLC_MODULE_APPLY_EMPTY_SEQ",
        2283 => "TLC_MODULE_ONE_ARGUMENT_ERROR",
        2169 => "TLC_MODULE_ARGUMENT_ERROR",
        2266 => "TLC_MODULE_ARGUMENT_ERROR_AN",
        2182 => "TLC_MODULE_EVALUATING",
        2183 => "TLC_MODULE_ARGUMENT_NOT_IN_DOMAIN",
        2181 => "TLC_MODULE_COMPUTING_CARDINALITY",
        2132 => "TLC_VALUE_ASSERT_FAILED",
        2280 => "TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR",
        2261 => "TLC_ENCOUNTERED_FORMULA_IN_PREDICATE",
        2179 => "TLC_MODULE_DIVISION_BY_ZERO",
        2178 => "TLC_MODULE_OVERFLOW",
        2180 => "TLC_MODULE_NULL_POWER_NULL",
        2265 => "TLC_INTEGER_TOO_BIG",
        2158 => "TLC_MODULE_CHECK_MEMBER_OF",
        2155 => "TLC_MODULE_COMPARE_VALUE",
        2154 => "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE",
        2244 => "TLC_CANT_HANDLE_REAL_NUMBERS",
        2170 => "TLC_ARGUMENT_MISMATCH",
        2176 => "TLC_MODULE_APPLYING_TO_WRONG_VALUE",
        2247 => "TLC_EXPECTED_EXPRESSION_IN_COMPUTING",
        2248 => "TLC_EXPECTED_EXPRESSION_IN_COMPUTING2",
        _ => "GENERAL",
    }
}

/// Whether evaluating `model` may have effects beyond computing a value:
/// `Print`, `TLCSet`, a live `TLCGet`, randomness, I/O or checker
/// introspection. Conservative: any definition of a user module counts,
/// whether or not it is reached, and so does a constant or definition the
/// configuration binds to such a standard operator. Caller-supplied predicate
/// and action scopes are checked for actual captured effects as well. Backends
/// that cannot reproduce TLC's evaluation order for effects refuse such models.
pub(crate) fn effectful(model: &dive_model::Model) -> bool {
    use dive_ir::{Effects, StdSupport};
    let ir = &model.ir;
    let forbidden = Effects::IMPURE.union(Effects::LIVE_TLCGET);
    let native = |def: dive_ir::DefId| {
        let def = model.evaluator.redirected(def);
        ir.defs()
            .get(def.index())
            .and_then(|d| d.std)
            .is_some_and(|op| {
                op.impure() || matches!(op.support(), StdSupport::Io | StdSupport::Introspection)
            })
    };
    let user = ir.defs().iter().enumerate().any(|(i, def)| {
        let id = dive_ir::DefId(u32::try_from(i).unwrap_or(u32::MAX));
        (!ir.module(def.module).standard && ir.node(def.body).effects.intersects(forbidden))
            || (model.evaluator.redirected(id) != id && native(id))
    });
    if user || (0..ir.consts().len()).any(|i| {
        let id = dive_ir::ConstId(u32::try_from(i).unwrap_or(u32::MAX));
        matches!(model.evaluator.constant_binding(id), Some(dive_eval::ConstBinding::Def(d)) if native(d))
    }) {
        return true;
    }
    let predicates = model
        .init
        .iter()
        .map(|init| &init.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|named| &named.pred))
        .chain(model.implied_inits.iter().map(|named| &named.pred))
        .chain(model.implied_actions.iter().map(|named| &named.pred))
        .chain(model.temporals.iter().map(|named| &named.pred))
        .chain(model.implied_temporals.iter().map(|named| &named.pred))
        .chain(model.configured_properties.iter().map(|named| &named.pred))
        .chain(model.constraints.iter().map(|named| &named.pred))
        .chain(model.action_constraints.iter().map(|named| &named.pred))
        .chain(model.view.iter().map(|named| &named.pred))
        .chain(
            model
                .alias
                .as_ref()
                .and_then(|alias| alias.as_ref().ok())
                .map(|named| &named.pred),
        )
        .chain(
            model
                .postconditions
                .as_ref()
                .into_iter()
                .flatten()
                .map(|named| &named.pred),
        );
    predicates
        .into_iter()
        .any(|pred| model.evaluator.scope_has_effects(&pred.scope))
        || model
            .init_actions
            .iter()
            .chain(&model.actions)
            .any(|action| model.evaluator.scope_has_effects(&action.scope()))
}

pub(crate) fn context_verdict(error: &dive_eval::ContextError) -> Verdict {
    Verdict::Unsupported {
        feature: error.to_string(),
    }
}

fn bare(verdict: Verdict, opts: &CheckOptions, ir: &Ir) -> CheckResult {
    // Refusal/panic reporting also receives untrusted public arenas. Preserve
    // valid concrete metadata, but never index an invalid declaration/order or
    // create a partial variable mapping that could be mistaken for a valid one.
    let names: Option<Vec<_>> = ir
        .vars()
        .iter()
        .enumerate()
        .map(|(i, d)| {
            ir.decls()
                .get(d.index())
                .filter(|decl| decl.var.is_some_and(|v| v.index() == i))
                .map(|decl| decl.name.clone())
        })
        .collect();
    let order: Vec<_> = ir.tlc_var_order().iter().map(|v| v.index()).collect();
    let valid_order = order.len() == ir.vars().len()
        && order.iter().all(|&i| i < ir.vars().len())
        && order
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == order.len();
    let (var_names, tlc_var_order) = match names {
        Some(names) if valid_order => (names, order),
        _ => (Vec::new(), Vec::new()),
    };
    CheckResult {
        frontier: crate::FrontierUsage {
            memory_bytes_per_queue: opts.frontier_memory_bytes,
            max_record_bytes: opts
                .frontier_memory_bytes
                .map(|_| dive_store::spill::MAX_RECORD_BYTES),
            ..crate::FrontierUsage::default()
        },
        seen: None,
        predecessor: crate::result::PredecessorUsage::default(),
        verdict,
        binding_model_error: false,
        stats: None,
        tlc_prints_stats: false,
        counterexample: None,
        collision: None,
        store: opts.store,
        fp_index: (opts.store == dive_store::StoreMode::Fingerprint).then_some(opts.fp_index),
        tlc_fingerprints: opts.tlc_fingerprints,
        var_names,
        tlc_var_order,
        output: Vec::new(),
        warnings: Vec::new(),
        notes: Vec::new(),
        // No search has run yet.
        workers: Workers {
            requested: opts.workers.max(1),
            used: 0,
            sequential: false,
            reason: None,
        },
        elapsed: std::time::Duration::ZERO,
    }
}

fn messages(out: &[Message], r: &mut CheckResult) {
    for m in out {
        match m {
            Message::Print(line) => r.output.push(line.clone()),
            Message::Warning { code, message } => r.warnings.push((code.code(), message.clone())),
        }
    }
}

#[allow(clippy::too_many_lines, reason = "the phases of a run, in TLC's order")]
fn run_check(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> CheckResult {
    run_check_with_checkpoint(ir, cfg, opts, None)
}

#[allow(clippy::too_many_lines, reason = "the phases of a run, in TLC order")]
pub(crate) fn run_check_with_checkpoint(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    checkpoint: Option<&mut checkpoint::Session>,
) -> CheckResult {
    if let Some(disk) = &opts.seen_disk {
        if let Err(error) = disk.limits.validate() {
            return bare(seen::verdict(&error), opts, ir);
        }
        if checkpoint.is_some() {
            return bare(Verdict::Unsupported { feature: "disk seen-state retention with native checkpoint/recovery is not implemented".into() }, opts, ir);
        }
    }
    // Refuse transformed random streams before binding can execute effects.
    // Ordinary VIEW and non-VIEW per-state RNG retain their existing routes.
    if let Some(feature) = dive_model::transformed_randomness(ir, cfg) {
        return bare(
            Verdict::Unsupported {
                feature: feature.into(),
            },
            opts,
            ir,
        );
    }
    let bound = bind_for_run(ir, cfg, opts, checkpoint.as_ref().and_then(|s| s.names()));
    let (m, host) = match bound {
        Ok(bound) => bound,
        Err(f) => return binding_failure(ir, opts, &f),
    };
    finish_bound_check(ir, cfg, opts, m, host, checkpoint)
}

fn binding_failure(
    ir: &Arc<Ir>,
    opts: &CheckOptions,
    failure: &dive_model::Failure,
) -> CheckResult {
    let mut result = bare(model_error(&failure.error), opts, ir);
    result.binding_model_error = matches!(result.verdict, Verdict::ModelError { .. });
    messages(&failure.output, &mut result);
    result
}

fn bind_for_run(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    restored_names: Option<&[String]>,
) -> Result<(dive_model::Model, Arc<dive_stdlib::host::RunHost>), Box<dive_model::Failure>> {
    let host = Arc::new(engine::run_host(opts));
    let bind_opts = BindOptions {
        eval: opts.eval,
        no_deadlock: opts.no_deadlock,
        host: Some(Arc::new(host::EngineHost::new(host.clone()))),
    };
    let bound = if let Some(names) = restored_names {
        dive_model::bind_with_names(ir.clone(), cfg, &bind_opts, names)
    } else {
        dive_model::bind(ir.clone(), cfg, &bind_opts)
    };
    bound.map(|model| (model, host))
}

#[allow(
    clippy::too_many_lines,
    reason = "the unchanged post-binding phases in TLC order"
)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "owns the original startup host through every post-binding return"
)]
fn finish_bound_check(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    mut m: dive_model::Model,
    host: Arc<dive_stdlib::host::RunHost>,
    mut checkpoint: Option<&mut checkpoint::Session>,
) -> CheckResult {
    let mut notes = Vec::new();
    let mut r = bare(Verdict::Success, opts, ir);
    messages(&m.output, &mut r);
    if m.symmetry.is_some() && m.implied_temporals.is_empty() {
        if engine::uses_random(ir) {
            r.verdict = Verdict::Unsupported {
                feature: "randomness under SYMMETRY is not supported".into(),
            };
            return r;
        }
        let mode = if opts.profile == Profile::Tlc {
            dive_model::symmetry::Mode::Tlc
        } else {
            dive_model::symmetry::Mode::Checked
        };
        match m.prepare_symmetry(cfg, mode, dive_model::symmetry::Limits::default()) {
            Ok(Some(prepared)) => {
                messages(&prepared.warnings, &mut r);
                m.symmetry_reduction = prepared.reduction;
                let symmetry = format!("SYMMETRY {}", m.symmetry.as_deref().unwrap_or_default());
                m.unsupported
                    .retain(|unsupported| unsupported.feature != symmetry);
                if opts.profile == Profile::Dive {
                    notes.push("The default profile canonicalizes symmetry keys in canonical value order (D-010). A non-invariant VIEW or asymmetric specification may differ under --compat=tlc; order relevance is not yet detected.".into());
                }
            }
            Ok(None) => {}
            Err(error) => {
                r.verdict = model_error(&error);
                r.binding_model_error = matches!(r.verdict, Verdict::ModelError { .. });
                return r;
            }
        }
    }
    if let Some(u) = m.unsupported.first() {
        r.verdict = Verdict::Unsupported {
            feature: u.feature.clone(),
        };
        r.notes = notes;
        return r;
    }
    // TLC creates its model checker here: `TLCGet("config")` and the
    // checker's names exist from now on, and the counters read 0.
    m.evaluator.intern_checker_names();
    let config = engine::run_config(opts, m.settings.check_deadlock);
    m.evaluator.intern_bfs_config(&config);
    host.set_config(Some(config));
    engine::startup_counters(&host);
    if let Some(session) = checkpoint.as_mut()
        && let Err(verdict) = session.admit_model(&m)
    {
        r.verdict = verdict;
        return r;
    }
    let assumptions = m.check_assumptions();
    r.output.extend(m.take_output());
    match assumptions {
        AssumptionOutcome::AllTrue => {}
        AssumptionOutcome::False { message, .. } => {
            r.verdict = Verdict::AssumptionFalse { message };
            r.notes = notes;
            return r;
        }
        AssumptionOutcome::Error { error, message, .. } => {
            r.verdict = class::assumption_error(&error, message);
            r.notes = notes;
            return r;
        }
    }
    let outcome = if let Some(session) = checkpoint.take() {
        engine::search(
            &m,
            opts,
            &host,
            engine::SearchHooks {
                checkpoint: Some(session),
                ..Default::default()
            },
        )
    } else {
        engine::search(&m, opts, &host, engine::SearchHooks::default())
    };
    r.seen = outcome.seen;
    r.frontier = outcome.frontier;
    r.predecessor = outcome.predecessor;
    r.verdict = outcome.verdict;
    r.stats = outcome.stats;
    r.tlc_prints_stats = outcome.tlc_prints_stats;
    r.counterexample = outcome.counterexample;
    r.collision = outcome.collision;
    r.workers = outcome.workers;
    r.output.extend(outcome.output);
    notes.extend(outcome.notes);
    if r.counterexample.is_some()
        && let Some(alias) = &cfg.alias
    {
        notes.push(format!(
            "ALIAS {} is not applied: the counterexample shows the states themselves (not implemented yet).",
            alias.text
        ));
    }
    note_canonical_stop(opts, &r.verdict, &mut notes);
    r.notes = notes;
    r
}

/// Shared default-profile qualification for an observed safety stop.
fn note_canonical_stop(opts: &CheckOptions, verdict: &Verdict, notes: &mut Vec<String>) {
    if opts.profile == Profile::Dive && verdict.is_verdict() && *verdict != Verdict::Success {
        // D-010: canonical exploration can stop at a different event than
        // TLC's order. Detecting whether it did is not implemented yet, so
        // every stop says so.
        notes.push(
            "The default profile explores sets in canonical order (D-010). When several violations are reachable at the same depth, which one is reported, and the counts at the stop, may differ under --compat=tlc; dive does not yet detect whether the order mattered in this run.".to_owned(),
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "malformed public metadata regression controls"
)]
mod metadata_controls {
    use super::*;

    #[test]
    fn panic_and_early_refusal_metadata_do_not_index_invalid_public_ids() {
        let a = dive_sema::analyze_source(
            "M.tla",
            "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\n====\n",
            &dive_sema::Options::default(),
        );
        let ir = dive_ir::lower(&a).expect("IR");
        for field in ["vars", "tlc_vars"] {
            let mut json = serde_json::to_value(&ir).expect("serialize");
            json[field] = serde_json::json!([u32::MAX]);
            let bad: Ir = serde_json::from_value(json).expect("deserialize");
            for verdict in [
                Verdict::InternalError {
                    message: "original panic".into(),
                },
                Verdict::Unsupported {
                    feature: "original option refusal".into(),
                },
            ] {
                let result = bare(verdict.clone(), &CheckOptions::tlc(0, 7), &bad);
                assert_eq!(result.verdict, verdict);
                assert!(result.var_names.is_empty() && result.tlc_var_order.is_empty());
                assert_eq!(result.workers.used, 0);
                assert!(result.stats.is_none() && result.output.is_empty());
            }
        }
    }
}
