//! `ALIAS` rendering of sampled traces.
//!
//! TLC prints a trace with the `ALIAS` value of each state in place of the
//! state (oracle cases A1 and A3). Simulation keeps the concrete states in
//! the validated document and renders the `ALIAS` beside them, evaluated on
//! the same states with the run's own startup values. `TLCGet("level")` is
//! the state's 1-based trace position, as when TLC prints the trace. Any
//! effect (output, a random draw, a register, other runtime introspection)
//! and any evaluation error leaves the trace without a rendering and with a
//! note naming the reason; rendering never changes a verdict.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dive_eval::next::StateVals;
use dive_eval::{Host, HostError, St};
use dive_ir::StdOp;
use dive_value::{Ctx, Value};
use serde_json::Value as Json;

/// Expression cache domains of rendering hosts: never 0 (ordinary
/// evaluation), so a value cached while rendering is never reused elsewhere.
fn next_domain() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1 << 62);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug)]
struct AliasHost {
    level: AtomicI64,
    config: Option<dive_eval::RunConfig>,
    refused: Mutex<Option<String>>,
    domain: u64,
}

impl AliasHost {
    fn refuse(&self, message: String) -> HostError {
        self.refused
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| message.clone());
        HostError { code: 0, message }
    }
}

impl Host for AliasHost {
    fn check_native_access(&self, op: StdOp) -> Result<(), HostError> {
        if op.impure() && op != StdOp::TlcTLCGet {
            return Err(self.refuse(format!("{} while rendering ALIAS", op.qualified_name())));
        }
        Ok(())
    }

    fn print_output(&self, _text: &str) -> Result<bool, HostError> {
        Err(self.refuse("output while rendering ALIAS".to_owned()))
    }

    fn check_tlc_access(&self, key: &str, write: bool) -> Result<(), HostError> {
        if !write && matches!(key, "level" | "config") {
            return Ok(());
        }
        Err(self.refuse(format!(
            "TLC{}({key:?}) while rendering ALIAS",
            if write { "Set" } else { "Get" }
        )))
    }

    fn random(&self, op: StdOp, _: &[Value], _: Ctx) -> Option<Result<Value, HostError>> {
        Some(Err(self.refuse(format!(
            "{} while rendering ALIAS",
            op.qualified_name()
        ))))
    }

    fn check_random_in_enabled(&self) -> Result<(), HostError> {
        Err(self.refuse("a random draw while rendering ALIAS".to_owned()))
    }

    fn check_register_access(&self) -> Result<(), HostError> {
        Err(self.refuse("register access while rendering ALIAS".to_owned()))
    }

    fn check_action_composition(&self) -> Result<(), HostError> {
        Err(self.refuse("action composition while rendering ALIAS".to_owned()))
    }

    fn expression_cache_domain(&self) -> u64 {
        self.domain
    }

    fn level(&self) -> Option<i64> {
        Some(self.level.load(Ordering::Relaxed))
    }

    fn config(&self) -> Option<dive_eval::RunConfig> {
        self.config.clone()
    }
}

/// Renders the model's `ALIAS` on each state of `states` as the text of a
/// JSON array of their values in dive's JSON form, or names why it cannot.
/// `poll` returns true once the run's budget is exhausted.
pub(crate) fn render(
    model: &dive_model::Model,
    alias: &dive_model::Named,
    states: &[StateVals],
    poll: &dyn Fn() -> bool,
) -> Result<String, String> {
    let ctx = model.evaluator.options().ctx;
    let mut rendered = String::from("[");
    for (index, value) in render_values(model, alias, states, poll)?
        .iter()
        .enumerate()
    {
        if index > 0 {
            rendered.push(',');
        }
        let json: Json = dive_trace::value_json::to_json(value, ctx)?;
        rendered.push_str(&json.to_string());
    }
    rendered.push(']');
    Ok(rendered)
}

/// The model's `ALIAS` value on each state of `states`, or why it cannot be
/// rendered ([`render`]).
pub(crate) fn render_values(
    model: &dive_model::Model,
    alias: &dive_model::Named,
    states: &[StateVals],
    poll: &dyn Fn() -> bool,
) -> Result<Vec<Value>, String> {
    let previous = model.evaluator.host();
    let host = Arc::new(AliasHost {
        level: AtomicI64::new(0),
        config: previous.as_ref().and_then(|host| host.config()),
        refused: Mutex::new(None),
        domain: next_domain(),
    });
    let installed = model.evaluator.replace_host(Some(host.clone()));
    let result = render_with(model, alias, states, &host, poll);
    let _ = model.evaluator.replace_host(installed);
    let refused = host
        .refused
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    match (result, refused) {
        (_, Some(reason)) => Err(reason),
        (result, None) => result,
    }
}

fn render_with(
    model: &dive_model::Model,
    alias: &dive_model::Named,
    states: &[StateVals],
    host: &AliasHost,
    poll: &dyn Fn() -> bool,
) -> Result<Vec<Value>, String> {
    let mut rendered = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        if poll() {
            return Err("the run budget was exhausted while rendering ALIAS".to_owned());
        }
        host.level.store(
            i64::try_from(index + 1).unwrap_or(i64::MAX),
            Ordering::Relaxed,
        );
        let value = model
            .eval(&alias.pred, &St::state(state.clone()))
            .map_err(|error| format!("evaluating ALIAS at state {}: {error}", index + 1))?;
        rendered.push(value);
    }
    Ok(rendered)
}
