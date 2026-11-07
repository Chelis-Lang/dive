//! One API-owned run budget shared by finite execution modes.

use dive_runtime::budget::{Budget, EvaluationControl, Phase, Watchdog};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct ModeBudget {
    pub(crate) budget: Budget,
    watchdog: Option<Watchdog>,
    metadata: Value,
    phase: Phase,
}

impl ModeBudget {
    pub(crate) fn start(
        compat: bool,
        phase: Phase,
        time: crate::BudgetSetting<Duration>,
        memory: crate::BudgetSetting<u64>,
    ) -> Result<Self, crate::ApiError> {
        let (time, time_source) =
            time.resolve((!compat).then_some(dive_runtime::budget::DEFAULT_TIME_LIMIT));
        let (memory, memory_source) =
            crate::check::resolve_memory(memory, dive_runtime::budget::default_memory)
                .map_err(crate::ApiError::BudgetMeasurement)?;
        let requested = Budget::new(time, memory);
        let watchdog = requested.start().map_err(|error| match error {
            dive_runtime::budget::StartError::Measurement(error) => {
                crate::ApiError::BudgetMeasurement(error)
            }
            error @ dive_runtime::budget::StartError::Thread(_) => {
                crate::ApiError::BudgetStartup(error)
            }
        })?;
        let budget = watchdog
            .as_ref()
            .map_or(requested, |owner| owner.budget().clone());
        Ok(Self {
            budget,
            watchdog,
            phase,
            metadata: json!({
                "time_limit_ms": time.map(|value| u64::try_from(value.as_millis()).unwrap_or(u64::MAX)),
                "time_limit_source": time_source, "max_memory_bytes": memory, "max_memory_source": memory_source,
                "scope": "execution after model loading, including assumptions, Init, mode steps, retained-state serialization checkpoints and independent finite-word replay; individual value encoding/decoding and initial model loading remain uninterruptible; final resource sample before publication; process memory is sampled rather than an allocation-time hard cap"
            }),
        })
    }

    pub(crate) fn control(&self) -> Result<Arc<EvaluationControl>, crate::ApiError> {
        EvaluationControl::new(&self.budget, self.phase).map_err(|error| {
            crate::ApiError::Rejected {
                message: error.to_string(),
            }
        })
    }

    pub(crate) fn finish(
        &mut self,
        document: &mut Value,
        exit: &mut u8,
        cancelled: bool,
    ) -> Result<(), crate::ApiError> {
        document["options"]["budget"] = self.metadata.clone();
        let exhausted = if let Some(owner) = &mut self.watchdog {
            owner.finish()
        } else {
            self.budget.poll()
        }
        .map_err(crate::ApiError::BudgetMeasurement)?;
        let preflight_disposition =
            document["options"]["execution_stage"] == "preflight" && !cancelled;
        if let Some(exhausted) = exhausted
            && !preflight_disposition
        {
            let stopped = exhausted.during(self.phase);
            document["result"] = json!("stopped");
            document["termination"] = json!("run-budget");
            document["message"] = json!(stopped.to_string());
            document["stopped"] = crate::check::stopped_json(&stopped);
            document["tlc_code"] = Value::Null;
            document["exit_code"] = json!(5);
            document["exhaustive"] = json!(false);
            document["model_proved"] = json!(false);
            document["violation_validated"] = json!(false);
            if document.get("counterexample").is_some() {
                document["counterexample"] = Value::Null;
            }
            *exit = 5;
        }
        Ok(())
    }
}
