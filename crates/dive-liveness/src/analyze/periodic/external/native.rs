//! CSR analysis uses the same planning, labels, product, attribution and finish.

use dive_check::{CheckResult, Stats, capture};

use super::{GraphCheck, NativeAnalysis, NativeRead};
use crate::analyze::periodic::{Checks, PeriodicFailure, budget_failure, failure};
use crate::analyze::{
    AnalysisError, EvaluationErrors, ExplorationEvidence, Stage, analyze_native, finish,
};

fn facts(result: &CheckResult, stats: Stats) -> ExplorationEvidence {
    ExplorationEvidence {
        stats,
        workers: result.workers.clone(),
        collision: result.collision.clone(),
        output: result.output.clone(),
        warnings: result.warnings.clone(),
    }
}

impl GraphCheck<'_> {
    /// Whether this committed level needs temporal analysis. Producers can
    /// avoid constructing a checked CSR view when this returns false.
    /// This scheduling query supplies no graph or completion authority.
    #[must_use]
    pub fn native_level_due(&self, stats: Stats) -> bool {
        self.periodic
            && self.d.decision.is_none()
            && stats.left_on_queue != 0
            && self
                .d
                .checks
                .schedule
                .due(stats.distinct, self.d.checks.last_size)
    }

    /// Checks a committed native CSR boundary without reconstructing a value
    /// graph. The reader receives only checked retained-row ids.
    pub fn level_native(
        &mut self,
        level: u64,
        graph: &capture::CsrCapture<'_>,
        stats: Stats,
        read: &mut NativeRead<'_>,
    ) -> bool {
        if self.native_level_due(stats) {
            self.d.checks.prefixes += 1;
            self.d.checks.last_level = Some(level);
            self.d.checks.last_size = stats.distinct;
            let _budget = crate::budget::enter(&self.options.budget);
            let word = match self.control.enter(&self.model.evaluator) {
                Ok(_registration) => analyze_native(
                    self.model,
                    &self.plans,
                    1,
                    graph,
                    read,
                    self.limits,
                    self.policy,
                    self.graph_labels,
                    &mut self.d.stage,
                    &mut self.d.evaluations,
                    &self.control,
                ),
                Err(_) => Err(AnalysisError::Inconsistent(
                    "another driver owns evaluator control",
                )),
            };
            match word {
                Ok(None) => {}
                Ok(Some(word)) => self.d.decision = Some(Ok(word)),
                Err(error) => self.d.decision = Some(Err(error)),
            }
        }
        self.d.decision.is_some()
    }

    /// Finishes a real exhausted native capture with owned selected-word values
    /// and sealed producer facts. No `BehaviorGraph` is built or retained.
    ///
    /// # Errors
    /// The existing analysis failures, with actual complete exploration facts.
    pub fn finish_native(
        mut self,
        complete: capture::CsrComplete<'_>,
        read: &mut NativeRead<'_>,
    ) -> Result<(NativeAnalysis, Checks), Box<PeriodicFailure>> {
        let _budget = crate::budget::enter(&self.options.budget);
        let exploration = facts(
            complete.evidence().safety_result(),
            complete.evidence().stats(),
        );
        self.d.checks.exhausted = true;
        self.d.checks.final_check = true;
        self.d.evaluations.periodic = false;
        let word = match self.control.enter(&self.model.evaluator) {
            Ok(_registration) => analyze_native(
                self.model,
                &self.plans,
                usize::MAX,
                complete.graph(),
                read,
                self.limits,
                self.policy,
                self.graph_labels,
                &mut self.d.stage,
                &mut self.d.evaluations,
                &self.control,
            ),
            Err(_) => Err(AnalysisError::Inconsistent(
                "another driver owns evaluator control",
            )),
        };
        let result = finish::complete(word, complete.into_evidence())
            .map(|analysis| (NativeAnalysis::Final(analysis), self.d.checks))
            .map_err(|error| {
                failure(
                    error,
                    self.d.stage,
                    Some(Box::new(exploration.clone())),
                    self.d.evaluations,
                    Some(true),
                    self.d.checks,
                )
            })
            .map_err(|error| budget_failure(error, &self.options.budget));
        close(self.watchdog.as_mut(), result, exploration, true).map_err(Box::new)
    }

    /// Finishes a successful committed native prefix. A prefix can produce a
    /// candidate or failure, and can never produce a holding result.
    ///
    /// # Errors
    /// The retained level-check failure or a prefix without an analysis decision.
    pub fn finish_native_prefix(
        mut self,
        prefix: capture::CsrPrefix<'_>,
        _read: &mut NativeRead<'_>,
    ) -> Result<(NativeAnalysis, Checks), Box<PeriodicFailure>> {
        let _budget = crate::budget::enter(&self.options.budget);
        let evidence = prefix.into_evidence();
        let exploration = facts(evidence.safety_result(), evidence.stats());
        let word = self.d.decision.take().unwrap_or_else(|| {
            Err(AnalysisError::Inconsistent(
                "prefix stopped without an analysis decision",
            ))
        });
        let result = finish::prefix(word, evidence)
            .map(|analysis| (NativeAnalysis::Prefix(analysis), self.d.checks))
            .map_err(|error| {
                failure(
                    error,
                    self.d.stage,
                    Some(Box::new(exploration.clone())),
                    self.d.evaluations,
                    Some(false),
                    self.d.checks,
                )
            })
            .map_err(|error| budget_failure(error, &self.options.budget));
        close(self.watchdog.as_mut(), result, exploration, false).map_err(Box::new)
    }
}

fn close(
    watchdog: Option<&mut dive_check::budget::Watchdog>,
    result: Result<(NativeAnalysis, Checks), PeriodicFailure>,
    exploration: ExplorationEvidence,
    completed: bool,
) -> Result<(NativeAnalysis, Checks), PeriodicFailure> {
    if let Some(watchdog) = watchdog {
        let finished = watchdog.finish();
        if let Ok((_, checks)) = &result {
            let error = match finished {
                Err(error) => Some(AnalysisError::BudgetMeasurement(error)),
                Ok(Some(exhausted)) => Some(AnalysisError::Stopped(Box::new(
                    exhausted.during(dive_check::budget::Phase::Finalization),
                ))),
                Ok(None) => None,
            };
            if let Some(error) = error {
                return Err(failure(
                    error,
                    Stage::Finalization,
                    Some(Box::new(exploration)),
                    EvaluationErrors::default(),
                    Some(completed),
                    *checks,
                ));
            }
        }
    }
    result
}
