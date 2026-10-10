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

pub mod capture;
mod class;
pub mod dfid;
pub mod engine;
mod host;
mod options;
mod parallel;
mod pool;
mod render;
mod result;

use std::sync::Arc;
use std::time::Instant;

use dive_config::ModelConfig;
use dive_eval::{AssumptionOutcome, Profile};
use dive_ir::Ir;
use dive_model::{BindError, BindOptions, ErrorKind, Message};

pub use options::CheckOptions;
pub use render::{format_trace_state, render_human};
pub use result::{
    CheckResult, Collision, Counterexample, ErrorSite, Stats, TlcClass, TraceState, Verdict,
    Workers, tlc_exit_status,
};

/// Checks the model `cfg` of the lowered specification `ir`.
///
/// Runs on its own thread with the evaluator's recommended stack.
#[must_use]
pub fn check(ir: &Arc<Ir>, cfg: &ModelConfig, opts: &CheckOptions) -> CheckResult {
    let started = Instant::now();
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

fn bare(verdict: Verdict, opts: &CheckOptions, ir: &Ir) -> CheckResult {
    CheckResult {
        verdict,
        stats: None,
        tlc_prints_stats: false,
        counterexample: None,
        collision: None,
        store: opts.store,
        fp_index: (opts.store == dive_store::StoreMode::Fingerprint).then_some(opts.fp_index),
        tlc_fingerprints: opts.tlc_fingerprints,
        var_names: ir.vars().iter().map(|&d| ir.decl(d).name.clone()).collect(),
        tlc_var_order: ir.tlc_var_order().iter().map(|v| v.index()).collect(),
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
    let mut notes = Vec::new();
    let host = Arc::new(engine::run_host(opts));
    let bind_opts = BindOptions {
        eval: opts.eval,
        no_deadlock: opts.no_deadlock,
        host: Some(Arc::new(host::EngineHost::new(host.clone()))),
    };
    let mut m = match dive_model::bind(ir.clone(), cfg, &bind_opts) {
        Ok(m) => m,
        Err(f) => {
            let mut r = bare(model_error(&f.error), opts, ir);
            messages(&f.output, &mut r);
            r.notes = notes;
            return r;
        }
    };
    let mut r = bare(Verdict::Success, opts, ir);
    messages(&m.output, &mut r);
    if m.symmetry.is_some() && m.implied_temporals.is_empty() {
        if engine::uses_random(ir) {
            r.verdict = Verdict::Unsupported { feature: "randomness under SYMMETRY needs canonical fingerprint seeding and independent replay".into() };
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
    let outcome = engine::search(&m, opts, &host);
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
