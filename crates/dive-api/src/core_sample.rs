//! The successor-multiset sample of the shadow comparison
//! (`docs/developer/core/08-correctness.md` §4): for each sampled state, the
//! core's successors under each action, as canonical state keys, must be
//! the reference evaluator's, with the same multiplicities.

use std::sync::Arc;

use dive_eval::next::Flow;
use dive_model::Model;
use dive_value::Value;
use serde_json::{Value as Json, json};

/// At most this many mismatches are described in the report.
const SHOWN: usize = 8;

/// A state as the reference engine's canonical key: a marker per variable
/// (assigned or not), then the canonical encoding of the value.
fn key(model: &Model, s: &[Option<Value>]) -> Result<Vec<u8>, String> {
    let ctx = model.evaluator.options().ctx;
    let mut out = Vec::new();
    for v in s {
        match v {
            None => out.push(0),
            Some(v) => {
                out.push(1);
                dive_store::state::state_key(std::slice::from_ref(v), ctx, &mut out)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(out)
}

/// The reference evaluator's successors of `s0` under action `i`: sorted
/// keys, or its error.
fn reference(
    (model, host): (&Model, &dive_stdlib::host::RunHost),
    i: usize,
    s0: &dive_eval::next::StateVals,
    level: i64,
) -> Result<Vec<Vec<u8>>, String> {
    host.set_level(level);
    let a = model
        .actions
        .get(i)
        .ok_or_else(|| format!("action {i} out of range"))?;
    let mut keys = Vec::new();
    let mut failed = None;
    let r = model.evaluator.next_states(a, s0, 0, &mut |succ| {
        match key(model, succ.state) {
            Ok(k) => keys.push(k),
            Err(e) => {
                failed = Some(e);
                return Flow::Stop;
            }
        }
        Flow::Continue
    });
    if let Some(e) = failed {
        return Err(format!("key: {e}"));
    }
    r.map_err(|e| e.error.to_string())?;
    keys.sort_unstable();
    Ok(keys)
}

fn core_keys(
    model: &Model,
    heap: &mut dive_core::Heap,
    result: &dive_explore::sample::ActionSuccessors,
) -> Result<Vec<Vec<u8>>, String> {
    let ctx = model.evaluator.options().ctx;
    let states = result.as_ref().map_err(Clone::clone)?;
    let mut keys = Vec::with_capacity(states.len());
    for state in states {
        let mut vals = Vec::with_capacity(state.len());
        for &h in state {
            vals.push(if h.is_empty() {
                None
            } else {
                Some(crate::core_engine::to_value(heap, model, h, ctx)?)
            });
        }
        keys.push(key(model, &vals)?);
    }
    keys.sort_unstable();
    Ok(keys)
}

/// Compares the sampled states' successors; the report's `core_sample`.
pub(crate) fn compare(
    (model, host): (&Model, &dive_stdlib::host::RunHost),
    plan: &dive_core::Plan,
    store: &Arc<dive_core::Store>,
    opts: dive_core::Opts,
    warn: bool,
    sample: dive_explore::sample::Sample,
    sampled: &[dive_explore::sample::SampledState],
) -> Json {
    let previous_level = dive_eval::Host::level(host);
    let mut compared = 0u64;
    let mut successors = 0u64;
    let mut skipped = 0u64;
    let mut mismatches = 0u64;
    let mut shown = Vec::new();
    let mut note = |state: &str, action: usize, what: String| {
        if shown.len() < SHOWN {
            shown.push(json!({"state": state, "action": action, "difference": what}));
        }
    };
    for state in sampled {
        let Ok(t0) = crate::core_ending::state(model, store, &state.values) else {
            skipped += 1;
            continue;
        };
        let text = crate::core_ending::state_text(model, &t0);
        let core = match dive_explore::sample::successors(
            plan,
            store,
            opts,
            warn,
            &state.values,
            state.level,
        ) {
            Ok(c) => c,
            Err(e) => {
                mismatches += 1;
                note(&text, 0, format!("the core failed: {}", e.message()));
                continue;
            }
        };
        let mut heap = dive_core::Heap::new(Arc::clone(store), false, false);
        for (i, got) in core.iter().enumerate() {
            compared += 1;
            let want = reference((model, host), i, &t0, state.level);
            let got = core_keys(model, &mut heap, got);
            match (got, want) {
                (Ok(g), Ok(w)) => {
                    successors += g.len() as u64;
                    if g != w {
                        mismatches += 1;
                        let extra = g.iter().filter(|k| !w.contains(k)).count();
                        let missing = w.iter().filter(|k| !g.contains(k)).count();
                        note(
                            &text,
                            i,
                            format!(
                                "the core emits {} successors, the reference {}; {extra} not in the reference's, {missing} missing",
                                g.len(),
                                w.len()
                            ),
                        );
                    }
                }
                // Both raise an error: which one is the reference
                // engine's to classify (`core_ending.rs`).
                (Err(_), Err(_)) => {}
                (Err(e), Ok(w)) => {
                    mismatches += 1;
                    note(
                        &text,
                        i,
                        format!(
                            "the core raises `{e}`, the reference emits {} successors",
                            w.len()
                        ),
                    );
                }
                (Ok(g), Err(e)) => {
                    mismatches += 1;
                    note(
                        &text,
                        i,
                        format!(
                            "the core emits {} successors, the reference raises `{e}`",
                            g.len()
                        ),
                    );
                }
            }
        }
    }
    // Sampling is diagnostic: preserve the host used by startup and any
    // later ending classification, including an originally unavailable level.
    match previous_level {
        Some(level) => host.set_level(level),
        None => host.clear_level(),
    }
    json!({
        "seed": sample.seed,
        "requested": sample.states,
        "states": sampled.len(),
        "skipped": skipped,
        "compared": compared,
        "successors": successors,
        "mismatches": mismatches,
        "shown": shown,
    })
}

#[cfg(test)]
#[path = "core_sample_level_tests.rs"]
mod tests;
