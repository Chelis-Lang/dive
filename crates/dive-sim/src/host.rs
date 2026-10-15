//! Runtime effect and retained-output limits for ordinary simulation.

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
    init_random: Mutex<InitRandom>,
}

#[derive(Debug, Default)]
struct InitRandom {
    active: bool,
    calls: usize,
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
            init_random: Mutex::new(InitRandom::default()),
        })
    }

    pub(crate) fn initial(&self) -> Initial<'_> {
        let mut phase = self
            .init_random
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        phase.active = true;
        phase.calls = 0;
        Initial { host: self }
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
    fn check_native_access(&self, op: StdOp) -> Result<(), HostError> {
        if op == StdOp::TlcRandomElement {
            let phase = self
                .init_random
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if phase.active {
                if phase.caller_host {
                    return Err(self.refuse(
                        "simulation Init RandomElement cannot preserve a caller-provided startup stream"
                            .to_owned(),
                    ));
                }
                // Arguments still run normally. Only the actual native invocation
                // below consumes the allowance, including nested domain calls.
                return Ok(());
            }
        }
        if op.impure() && !matches!(op, StdOp::TlcPrint | StdOp::TlcPrintT) {
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
        let mut phase = self
            .init_random
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if phase.caller_host {
            return Some(Err(self.refuse(
                "simulation Init RandomElement cannot preserve a caller-provided startup stream"
                    .to_owned(),
            )));
        }
        if !phase.active || phase.calls != 0 {
            return Some(Err(self.refuse(
                "simulation supports only one actual RandomElement call during Init".to_owned(),
            )));
        }
        phase.calls += 1;
        drop(phase);
        self.random.random(op, args, ctx)
    }

    fn check_register_access(&self) -> Result<(), HostError> {
        Err(self.refuse("simulation register access is unsupported".to_owned()))
    }

    fn check_tlc_access(&self, key: &str, write: bool) -> Result<(), HostError> {
        Err(self.refuse(format!(
            "TLC{}({key:?}): simulation host access is unsupported",
            if write { "Set" } else { "Get" }
        )))
    }

    fn print_output(&self, text: &str) -> Result<bool, HostError> {
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

pub(crate) struct Initial<'a> {
    host: &'a SimulationHost,
}

impl Drop for Initial<'_> {
    fn drop(&mut self) {
        self.host
            .init_random
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active = false;
    }
}

pub(crate) struct HostRestore<'a> {
    evaluator: &'a dive_eval::Evaluator,
    previous: Option<Arc<dyn Host>>,
}

impl<'a> HostRestore<'a> {
    pub(crate) fn install(evaluator: &'a dive_eval::Evaluator, host: &Arc<SimulationHost>) -> Self {
        let previous = evaluator.replace_host(Some(host.clone()));
        host.init_random
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .caller_host = previous.is_some();
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
