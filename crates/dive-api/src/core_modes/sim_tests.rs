//! Simulation controls that ran on the retired reference simulator until
//! #791, now on Core's finite simulation of the same bound models.

use dive_model::Model;
use dive_sim::{Options, Report};

mod bounds;
mod cdot;
mod ownership;
mod print;
mod random_init;
mod runtime;
mod startup;

/// Simulates a bound model on Core, as `simulate_file` does after
/// binding: preflight, Core preparation (a refusal is a preflight report),
/// then Core's sampler under a fresh simulation runtime.
fn simulate(model: &Model, options: &Options) -> Report {
    simulate_in(model, options, &dive_sim::Runtime::new(options.clone()))
}

/// [`simulate`] under a caller's runtime, whose host may have bound `model`.
fn simulate_in(model: &Model, options: &Options, runtime: &dive_sim::Runtime) -> Report {
    if let Some(outcome) = dive_sim::preflight(model, options) {
        return super::preflight_report(outcome);
    }
    match super::Native::prepare(model, &dive_runtime::budget::Budget::unlimited()) {
        Ok(mut native) => native.simulate(model, options, runtime, &|| false, None),
        Err(error) => super::preflight_report(error.simulation()),
    }
}
