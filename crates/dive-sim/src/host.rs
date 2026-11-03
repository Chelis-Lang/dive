//! Runtime effect and retained-output limits for ordinary simulation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dive_eval::{Host, HostError};
use dive_ir::StdOp;
use dive_stdlib::{host::RunHost, random::Streams};
use dive_value::{Ctx, Value};

use crate::Options;

#[derive(Debug)]
pub(crate) struct SimulationHost {
    max_events: usize,
    max_bytes: usize,
    output: Mutex<Output>,
    denied: Mutex<Option<String>>,
    random: RunHost,
    ownership: Mutex<Ownership>,
    binding: AtomicBool,
}

#[derive(Debug, Default)]
struct Ownership {
    caller_host: bool,
}

#[derive(Debug, Default)]
struct Output {
    events: usize,
    bytes: usize,
    limit: Option<&'static str>,
}

impl SimulationHost {
    pub(crate) fn new(options: &Options) -> Arc<Self> {
        Arc::new(Self {
            max_events: options.max_output_events,
            max_bytes: options.max_output_bytes,
            output: Mutex::new(Output::default()),
            denied: Mutex::new(None),
            random: RunHost::new(Streams::new(options.seed, options.mode), None),
            ownership: Mutex::new(Ownership::default()),
            binding: AtomicBool::new(false),
        })
    }

    pub(crate) fn binding(&self, active: bool) {
        self.binding.store(active, Ordering::Relaxed);
    }

    pub(crate) fn remaining_output(&self) -> (usize, usize) {
        let output = self.output.lock().unwrap_or_else(PoisonError::into_inner);
        (
            self.max_events.saturating_sub(output.events),
            self.max_bytes.saturating_sub(output.bytes),
        )
    }

    pub(crate) fn set_config(&self, config: Option<dive_eval::RunConfig>) {
        self.random.set_config(config);
    }

    pub(crate) fn denied(&self) -> Option<String> {
        self.denied
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn refuse(&self, message: String) -> HostError {
        self.denied
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| message.clone());
        HostError { code: 0, message }
    }

    pub(crate) fn limit(&self) -> Option<&'static str> {
        self.output
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .limit
    }
}

impl Host for SimulationHost {
    fn check_random_in_enabled(&self) -> Result<(), HostError> {
        Err(self.refuse(
            "RandomElement/Randomization inside ENABLED is unsupported in simulation".into(),
        ))
    }

    fn check_native_access(&self, op: StdOp) -> Result<(), HostError> {
        if op == StdOp::TlcRandomElement {
            let phase = self
                .ownership
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if phase.caller_host {
                return Err(self.refuse(
                    "simulation RandomElement cannot preserve a caller-provided startup stream"
                        .to_owned(),
                ));
            }
            return Ok(());
        }
        if op.impure() && !matches!(op, StdOp::TlcPrint | StdOp::TlcPrintT | StdOp::TlcTLCGet) {
            return Err(self.refuse(format!(
                "{}: simulation runtime effect is unsupported",
                op.qualified_name()
            )));
        }
        Ok(())
    }

    fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Option<Result<Value, HostError>> {
        if op != StdOp::TlcRandomElement {
            return None;
        }
        let phase = self
            .ownership
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if phase.caller_host {
            return Some(Err(self.refuse(
                "simulation RandomElement cannot preserve a caller-provided startup stream"
                    .to_owned(),
            )));
        }
        drop(phase);
        self.random.random(op, args, ctx)
    }

    fn check_register_access(&self) -> Result<(), HostError> {
        Err(self.refuse("simulation register access is unsupported".to_owned()))
    }

    fn check_tlc_access(&self, key: &str, write: bool) -> Result<(), HostError> {
        if !write && key == "config" {
            return Ok(());
        }
        Err(self.refuse(format!(
            "TLC{}({key:?}): simulation host access is unsupported",
            if write { "Set" } else { "Get" }
        )))
    }

    fn config(&self) -> Option<dive_eval::RunConfig> {
        self.random.config()
    }

    fn print_output(&self, text: &str) -> Result<bool, HostError> {
        // Existing runtime-output allowances exclude source-binding output.
        // Owning startup must not silently narrow those public limits.
        if self.binding.load(Ordering::Relaxed) {
            return Ok(true);
        }
        let mut output = self.output.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes = output.bytes.checked_add(text.len());
        let reason = if output.events >= self.max_events {
            Some("simulation output event bound")
        } else if bytes.is_none_or(|bytes| bytes > self.max_bytes) {
            Some("simulation output byte bound")
        } else {
            None
        };
        if let Some(reason) = reason {
            output.limit.get_or_insert(reason);
            return Err(HostError {
                code: 0,
                message: reason.to_owned(),
            });
        }
        output.events += 1;
        output.bytes = bytes.unwrap_or(0);
        Ok(true)
    }
}

pub(crate) struct HostRestore<'a> {
    evaluator: &'a dive_eval::Evaluator,
    previous: Option<Arc<dyn Host>>,
}

impl<'a> HostRestore<'a> {
    pub(crate) fn install(evaluator: &'a dive_eval::Evaluator, host: &Arc<SimulationHost>) -> Self {
        let previous = evaluator.replace_host(Some(host.clone()));
        host.ownership
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .caller_host = previous.as_ref().is_some_and(|previous| {
            let owned: Arc<dyn Host> = host.clone();
            !Arc::ptr_eq(previous, &owned)
        });
        Self {
            evaluator,
            previous,
        }
    }
}

impl Drop for HostRestore<'_> {
    fn drop(&mut self) {
        let _ = self.evaluator.replace_host(self.previous.take());
    }
}
