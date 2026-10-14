//! Runtime effect and retained-output limits for ordinary simulation.

use std::sync::{Arc, Mutex, PoisonError};

use dive_eval::{Host, HostError};
use dive_ir::StdOp;

use crate::Options;

#[derive(Debug)]
pub(crate) struct SimulationHost {
    max_events: usize,
    max_bytes: usize,
    output: Mutex<Output>,
    denied: Mutex<Option<String>>,
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
        })
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
        if op.impure() && !matches!(op, StdOp::TlcPrint | StdOp::TlcPrintT) {
            return Err(self.refuse(format!(
                "{}: simulation runtime effect is unsupported",
                op.qualified_name()
            )));
        }
        Ok(())
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

pub(crate) struct HostRestore<'a> {
    evaluator: &'a dive_eval::Evaluator,
    previous: Option<Arc<dyn Host>>,
}

impl<'a> HostRestore<'a> {
    pub(crate) fn install(evaluator: &'a dive_eval::Evaluator, host: Arc<dyn Host>) -> Self {
        Self {
            evaluator,
            previous: evaluator.replace_host(Some(host)),
        }
    }
}

impl Drop for HostRestore<'_> {
    fn drop(&mut self) {
        let _ = self.evaluator.replace_host(self.previous.take());
    }
}
