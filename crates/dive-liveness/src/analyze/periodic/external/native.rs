//! CSR analysis uses the same planning, labels, product, attribution and finish.

use dive_runtime::{CheckResult, Stats, capture};

use super::{GraphCheck, NativeAnalysis, NativeRead, NativeReader};
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
        let mut reader =
            |row, control: &dive_runtime::budget::EvaluationControl| read(row, control);
        self.level_native_with_reader(level, graph, stats, &mut reader)
    }

    /// Checks a committed boundary with optional source-bound native predicates.
    /// The original reader-only entrypoint retains its callback contract.
    pub fn level_native_with_reader(
        &mut self,
        level: u64,
        graph: &capture::CsrCapture<'_>,
        stats: Stats,
        read: &mut dyn NativeReader,
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
                    self.d.prefix_properties,
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

    /// Takes an already observed level-check failure before any post-stop CSR
    /// factory polls. This returns failure facts only, never prefix/completion
    /// authority or a candidate. The actual producer supplies its stopped stats.
    ///
    /// ```no_run
    /// # use dive_runtime::CheckResult;
    /// # use dive_liveness::analyze::periodic::external::GraphCheck;
    /// # fn stopped(driver: &mut GraphCheck<'_>, actual: &CheckResult) {
    /// if let Some(failure) = driver.take_native_prefix_failure(actual) {
    ///     assert_eq!(failure.capture_completed, Some(false));
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn take_native_prefix_failure(
        &mut self,
        result: &CheckResult,
    ) -> Option<Box<PeriodicFailure>> {
        if !matches!(self.d.decision, Some(Err(_))) {
            return None;
        }
        let stats = result.stats?;
        let error = self.d.decision.take()?.err()?;
        Some(Box::new(budget_failure(
            failure(
                error,
                self.d.stage,
                Some(Box::new(facts(result, stats))),
                std::mem::take(&mut self.d.evaluations),
                Some(false),
                self.d.checks,
            ),
            &self.options.budget,
        )))
    }

    /// Finishes a real exhausted native capture with owned selected-word values
    /// and sealed producer facts. No `BehaviorGraph` is built or retained.
    ///
    /// # Errors
    /// The existing analysis failures, with actual complete exploration facts.
    pub fn finish_native(
        self,
        complete: capture::CsrComplete<'_>,
        read: &mut NativeRead<'_>,
    ) -> Result<(NativeAnalysis, Checks), Box<PeriodicFailure>> {
        let mut reader =
            |row, control: &dive_runtime::budget::EvaluationControl| read(row, control);
        self.finish_native_with_reader(complete, &mut reader)
    }

    /// Finishes exhausted capture with optional source-bound native predicates.
    ///
    /// # Errors
    /// Existing analysis and original owned resource failures.
    pub fn finish_native_with_reader(
        mut self,
        complete: capture::CsrComplete<'_>,
        read: &mut dyn NativeReader,
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
    watchdog: Option<&mut dive_runtime::budget::Watchdog>,
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
                    exhausted.during(dive_runtime::budget::Phase::Finalization),
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
