//! Human-readable output, in TLC's conventions (its output without
//! `-tool`): the error, the behavior up to it, then the statistics.

use std::fmt::Write as _;

use dive_value::{Ctx, print};

use crate::result::{CheckResult, TraceState, Verdict};

/// A state as TLC prints it in a counterexample (`TLCStateMut.toString`):
/// `/\ x = v` per variable in TLC's variable order, or `x = v` for a single
/// variable.
#[must_use]
pub fn format_trace_state(r: &CheckResult, s: &TraceState, ctx: Ctx) -> String {
    if let Some(text) = &s.text {
        return text.clone();
    }
    let vars: Vec<(&str, &dive_value::Value)> = r
        .tlc_var_order
        .iter()
        .filter_map(|&i| Some((r.var_names.get(i)?.as_str(), s.values.get(i)?)))
        .collect();
    print::format_state(&vars, ctx).unwrap_or_else(|_| {
        let mut out = String::new();
        for (n, v) in &vars {
            let _ = writeln!(out, "/\\ {n} = {}", print::to_tlc_string_lenient(v, ctx));
        }
        out
    })
}

/// TLC's error message for a verdict.
fn headline(v: &Verdict) -> String {
    match v {
        Verdict::Success => "Model checking completed. No error has been found.".to_owned(),
        Verdict::InvariantViolated {
            property,
            initial: true,
        } => format!("Error: Invariant {property} is violated by the initial state:"),
        Verdict::InvariantViolated { property, .. } => {
            format!("Error: Invariant {property} is violated.")
        }
        Verdict::PropertyViolatedInitially { property } => {
            format!("Error: Property {property} is violated by the initial state:")
        }
        Verdict::ActionPropertyViolated { property } => {
            format!("Error: Action property {property} is violated.")
        }
        Verdict::Deadlock => "Error: Deadlock reached.".to_owned(),
        Verdict::AssumptionFalse { message } | Verdict::ModelError { message, .. } => {
            format!("Error: {message}")
        }
        Verdict::PostconditionFalse { message, .. }
        | Verdict::NotCompletelySpecified { message, .. }
        | Verdict::EvaluationError { message, .. } => format!("Error: {message}"),
        Verdict::Unsupported { feature } => {
            format!("unsupported: {feature}; no verdict produced")
        }
        Verdict::ResourceLimit { message } => {
            format!("resource limit: {message}; no verdict produced")
        }
        Verdict::InternalError { message } => {
            format!("internal error: {message}; no verdict produced (please report this)")
        }
    }
}

/// The whole human-readable report.
#[must_use]
pub fn render_human(r: &CheckResult, ctx: Ctx) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", headline(&r.verdict));
    if let Some(cx) = &r.counterexample {
        let initial_only = matches!(
            r.verdict,
            Verdict::InvariantViolated { initial: true, .. }
                | Verdict::PropertyViolatedInitially { .. }
        );
        if initial_only {
            for s in &cx.states {
                let _ = write!(out, "{}", format_trace_state(r, s, ctx));
            }
        } else {
            let _ = writeln!(out, "Error: The behavior up to this point is:");
            for (i, s) in cx.states.iter().enumerate() {
                let _ = writeln!(out, "State {}: {}", i + 1, s.label);
                let _ = writeln!(out, "{}", format_trace_state(r, s, ctx));
            }
        }
    }
    if r.verdict == Verdict::Success
        && let Some(c) = &r.collision
    {
        let _ = writeln!(
            out,
            "The probability of a fingerprint collision is {} (calculated optimistically){}.",
            c.optimistic,
            c.actual.as_ref().map_or_else(String::new, |a| format!(
                "; {a} based on the actual fingerprints"
            ))
        );
    }
    if let Some(s) = &r.stats
        && r.tlc_prints_stats
    {
        let _ = writeln!(
            out,
            "{} states generated, {} distinct states found, {} states left on queue.",
            s.generated, s.distinct, s.left_on_queue
        );
        let _ = writeln!(
            out,
            "The depth of the complete state graph search is {}.",
            s.depth
        );
    }
    if let Some(seen) = &r.seen {
        let cleanup = seen
            .after_cleanup
            .is_some_and(|stats| stats.cleanup_completed);
        let _ = writeln!(
            out,
            "Disk seen-state retention: {} admitted keys; actual inline worker {}; explicit cleanup completed: {cleanup}. This is not a total-memory budget.",
            seen.admitted_keys, r.workers.used
        );
        if let Some(failure) = &seen.first_failure {
            let _ = writeln!(
                out,
                "Seen storage {:?} failure: {}",
                failure.phase, failure.error
            );
        }
    }
    for n in &r.notes {
        let _ = writeln!(out, "note: {n}");
    }
    out
}
