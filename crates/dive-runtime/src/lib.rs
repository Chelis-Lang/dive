//! `dive-runtime`: the run contract every checking engine shares.
//!
//! The execution core and temporal analysis (`dive-liveness`) agree on these
//! pieces, so neither needs the other's search runtime:
//!
//! - [`CheckOptions`] and the [`CheckResult`] it produces, with its
//!   [`Verdict`], statistics, counterexample and storage evidence;
//! - TLC's error classes ([`classify`]) and human rendering;
//! - time and memory budgets and evaluation control ([`budget`]);
//! - behavior-graph capture types, evidence and admission ([`capture`]);
//! - native checkpoint framing, requests and Core's archive identity
//!   ([`checkpoint`]);
//! - the run host's configuration and counters ([`run`]) and pure model
//!   admission analysis ([`admission`]).
//!
//! The searches themselves belong to the execution core.
//!
//! # Example
//!
//! ```
//! use dive_runtime::{CheckOptions, Verdict};
//! let opts = CheckOptions::tlc(0, 7);
//! assert_eq!(opts.seed, 7);
//! assert!(Verdict::Success.is_verdict());
//! ```
#![forbid(unsafe_code)]

pub mod admission;
pub mod budget;
pub mod capture;
pub mod checkpoint;
#[doc(hidden)]
pub mod class;
pub mod classify;
#[doc(hidden)]
pub mod diagnostic;
mod options;
mod render;
mod result;
pub mod run;
pub mod usage;

use dive_ir::Ir;
use dive_model::{BindError, ErrorKind, Message};

pub use options::CheckOptions;
pub use render::{format_trace_state, render_human};
pub use result::{
    CheckResult, Collision, Counterexample, DiagnosticWitness, ErrorSite, PartialState,
    PredecessorUsage, Stats, TlcClass, TraceState, Verdict, Workers, tlc_exit_status,
};
pub use usage::{FrontierUsage, SeenFailure, SeenOptions, SeenPhase, SeenReplay, SeenUsage};

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
#[doc(hidden)]
#[must_use]
pub fn eval_code_name(code: u32) -> &'static str {
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

/// A result with `verdict` and no search: the run's metadata only.
#[doc(hidden)]
#[must_use]
pub fn bare(verdict: Verdict, opts: &CheckOptions, ir: &Ir) -> CheckResult {
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
        symmetry_liveness: None,
        frontier: crate::FrontierUsage {
            memory_bytes_per_queue: opts.frontier_memory_bytes,
            max_record_bytes: opts
                .frontier_memory_bytes
                .map(|_| dive_store::spill::MAX_RECORD_BYTES),
            ..crate::FrontierUsage::default()
        },
        seen: None,
        predecessor: crate::PredecessorUsage::default(),
        verdict,
        binding_model_error: false,
        stats: None,
        tlc_prints_stats: false,
        counterexample: None,
        diagnostic_witness: None,
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

/// Appends binding output and warnings to `r`.
#[doc(hidden)]
pub fn messages(out: &[Message], r: &mut CheckResult) {
    for m in out {
        match m {
            Message::Print(line) => r.output.push(line.clone()),
            Message::Warning { code, message } => r.warnings.push((code.code(), message.clone())),
        }
    }
}

/// Shared default-profile qualification for an observed safety stop.
#[doc(hidden)]
pub fn note_canonical_stop(opts: &CheckOptions, verdict: &Verdict, notes: &mut Vec<String>) {
    if opts.profile == dive_eval::Profile::Dive
        && verdict.is_verdict()
        && *verdict != Verdict::Success
    {
        // Canonical exploration can stop at a different event than
        // TLC's order. Detecting whether it did is not implemented yet, so
        // every stop says so.
        notes.push(
            "The default profile explores sets in canonical order. When several violations are reachable at the same depth, which one is reported, and the counts at the stop, may differ under --compat=tlc; dive does not yet detect whether the order mattered in this run.".to_owned(),
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
