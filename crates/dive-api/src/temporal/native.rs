//! Native temporal reports retain selected words and sealed facts, not graphs.

use std::sync::Arc;
use std::time::Instant;

use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_liveness::analyze::periodic::Checks;
use dive_runtime::{CheckOptions, CheckResult, capture};
use dive_trace::{
    Ending, Report as ValidationReport, Trace, TraceDoc, Validation, ValidationRunError,
};

use super::{
    Error, Failure, Outcome, Report, Stage, bind_controlled, core_file_configuration, core_report,
    validate_native_document, validation_binding_failure, validation_checkpoint,
    validation_control, validation_run_error,
};
use dive_liveness::analyze::ExplorationEvidence;
use dive_liveness::analyze::periodic::external::{NativeAnalysis, NativeFinal, NativeWord};

/// An independently validated native word from complete exploration.
#[derive(Clone, Debug)]
pub struct ValidatedNativeCounterexample {
    word: NativeWord,
    exploration: capture::CompleteEvidence,
    document: TraceDoc,
    validation: ValidationReport,
}

impl ValidatedNativeCounterexample {
    /// Selected original states, edges, property names and loop position.
    #[must_use]
    pub const fn word(&self) -> &NativeWord {
        &self.word
    }

    /// Actual exhausted producer facts, without retained graph rows.
    #[must_use]
    pub const fn evidence(&self) -> &capture::CompleteEvidence {
        &self.exploration
    }

    /// Independently validated serialized native word.
    #[must_use]
    pub const fn document(&self) -> &TraceDoc {
        &self.document
    }

    /// Independent serialized replay, always valid.
    #[must_use]
    pub const fn validation(&self) -> &ValidationReport {
        &self.validation
    }
}

/// An independently validated native word on a real unfinished prefix.
#[derive(Clone, Debug)]
pub struct ValidatedNativePrefix {
    word: NativeWord,
    exploration: capture::PrefixEvidence,
    document: TraceDoc,
    validation: ValidationReport,
}

impl ValidatedNativePrefix {
    /// Selected original states, edges, property names and loop position.
    #[must_use]
    pub const fn word(&self) -> &NativeWord {
        &self.word
    }

    /// Actual successful prefix facts, which cannot supply completion.
    #[must_use]
    pub const fn evidence(&self) -> &capture::PrefixEvidence {
        &self.exploration
    }

    /// Independently validated serialized native word.
    #[must_use]
    pub const fn document(&self) -> &TraceDoc {
        &self.document
    }

    /// Independent serialized replay, always valid.
    #[must_use]
    pub const fn validation(&self) -> &ValidationReport {
        &self.validation
    }
}

pub(super) fn facts(result: &CheckResult, stats: dive_runtime::Stats) -> ExplorationEvidence {
    ExplorationEvidence {
        stats,
        workers: result.workers.clone(),
        collision: result.collision.clone(),
        output: result.output.clone(),
        warnings: result.warnings.clone(),
    }
}

fn interrupted(
    control: &dive_runtime::budget::EvaluationControl,
    error: ValidationRunError,
) -> Failure {
    let (error, progress) = validation_run_error(control, error);
    let mut failure = Failure::before(error, Stage::Validation);
    failure.validation_progress = progress.map(Box::new);
    failure
}

fn rejected(word: &NativeWord, report: ValidationReport) -> Failure {
    Failure::before(
        Error::NativeValidation {
            word: Box::new(word.clone()),
            report,
        },
        Stage::Validation,
    )
}

fn validate_word(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    word: &NativeWord,
) -> Result<(TraceDoc, ValidationReport), Failure> {
    let control =
        validation_control(options).map_err(|error| Failure::before(error, Stage::Validation))?;
    validation_checkpoint(&control).map_err(|error| interrupted(&control, error))?;
    // The source-bound model is fresh: retained native handles and its evaluator
    // are unavailable to this independent typed and serialized replay.
    let model =
        bind_controlled(ir, config, options, &control).map_err(validation_binding_failure)?;
    let trace = Trace {
        states: word.states.clone(),
        labels: vec![None; word.states.len()],
        ending: Ending::Lasso {
            back_to: word.back_to,
        },
        length: Some(word.states.len()),
        properties: word.properties.clone(),
        concrete_closure: false,
        witnesses: Vec::new(),
        draws: Vec::new(),
    };
    let report = dive_trace::validate_controlled(&model, &trace, control.clone())
        .map_err(|error| interrupted(&control, error))?;
    if report.validation != Validation::Valid {
        return Err(rejected(word, report));
    }
    let document = trace
        .to_doc(&model)
        .map_err(|error| Failure::before(Error::Document(error), Stage::Document))?;
    let report = validate_native_document(&model, &document, &control)
        .map_err(|error| interrupted(&control, error))?;
    if report.validation != Validation::Valid {
        return Err(rejected(word, report));
    }
    Ok((document, report))
}

fn outcome(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    analysis: NativeAnalysis,
) -> Result<Outcome, Failure> {
    match analysis {
        NativeAnalysis::Final(NativeFinal::Holds(evidence)) => {
            Ok(Outcome::NativeHolds(Box::new(evidence)))
        }
        NativeAnalysis::Final(NativeFinal::Candidate(word, evidence)) => {
            let (document, validation) =
                validate_word(ir, config, options, &word).map_err(|mut failure| {
                    failure.exploration =
                        Some(Box::new(facts(evidence.safety_result(), evidence.stats())));
                    failure
                })?;
            Ok(Outcome::NativeViolation(Box::new(
                ValidatedNativeCounterexample {
                    word,
                    exploration: evidence,
                    document,
                    validation,
                },
            )))
        }
        NativeAnalysis::Prefix(prefix) => {
            let (document, validation) =
                validate_word(ir, config, options, &prefix.word).map_err(|mut failure| {
                    failure.exploration = Some(Box::new(facts(
                        prefix.exploration.safety_result(),
                        prefix.exploration.stats(),
                    )));
                    failure.incomplete = true;
                    failure
                })?;
            Ok(Outcome::NativePrefixViolation(Box::new(
                ValidatedNativePrefix {
                    word: prefix.word,
                    exploration: prefix.exploration,
                    document,
                    validation,
                },
            )))
        }
    }
}

/// Finishes the real native producer through the same independent proof boundary.
pub(crate) fn core_native_report(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    result: Result<(NativeAnalysis, Checks), dive_liveness::analyze::periodic::PeriodicFailure>,
    periodic: bool,
    symmetry_liveness: dive_model::SymmetryLivenessResolution,
    started: Instant,
) -> Result<Report, Failure> {
    let (analysis, checks) = match result {
        Ok(value) => value,
        // Reuse the existing error/Safety transport. This branch constructs no
        // graph or completed result, and retains the actual failure facts.
        Err(failure) => {
            let result = core_report(
                ir,
                config,
                options,
                failure,
                periodic,
                symmetry_liveness,
                started,
            );
            return result
                .map(|mut report| {
                    report.configuration = Some(core_file_configuration());
                    report
                })
                .map_err(|mut failure| {
                    failure.configuration = Some(Box::new(core_file_configuration()));
                    failure
                });
        }
    };
    match outcome(ir, config, options, analysis) {
        Ok(outcome) => Ok(Report {
            symmetry_liveness: Some(symmetry_liveness),
            outcome,
            options: options.clone(),
            elapsed: started.elapsed(),
            periodic: periodic.then_some(checks),
            memory_samples: options.budget.memory_samples(),
            configuration: Some(core_file_configuration()),
        }),
        Err(mut failure) => {
            failure.elapsed = started.elapsed();
            failure.periodic = periodic.then(|| Box::new(checks));
            failure.configuration = Some(Box::new(core_file_configuration()));
            failure.symmetry_liveness = Some(Box::new(symmetry_liveness));
            Err(failure)
        }
    }
}

/// Retains the actual producer boundary when a checked capture factory is
/// interrupted. No completion evidence is manufactured by this failure path.
pub(crate) fn core_capture_stop(
    verdict: dive_runtime::Verdict,
    safety: &CheckResult,
    completed: bool,
    started: Instant,
    checks: Option<Checks>,
) -> Failure {
    let error = match verdict {
        dive_runtime::Verdict::Stopped(stopped) => {
            dive_liveness::analyze::AnalysisError::Stopped(stopped)
        }
        dive_runtime::Verdict::BudgetMeasurement { error } => {
            dive_liveness::analyze::AnalysisError::BudgetMeasurement(error)
        }
        dive_runtime::Verdict::ResourceLimit { .. } => {
            dive_liveness::analyze::AnalysisError::Limit(
                "native source-row allocation or evaluation resources",
            )
        }
        dive_runtime::Verdict::Unsupported { feature } => {
            dive_liveness::analyze::AnalysisError::UnsupportedValue(feature)
        }
        _ => dive_liveness::analyze::AnalysisError::Inconsistent("non-budget capture stop"),
    };
    let mut failure = Failure::before(
        Error::Analysis(error),
        Stage::Analysis(dive_liveness::analyze::Stage::Capture),
    );
    failure.exploration = safety.stats.map(|stats| Box::new(facts(safety, stats)));
    failure.incomplete = !completed;
    failure.elapsed = started.elapsed();
    failure.periodic = checks.map(Box::new);
    failure.configuration = Some(Box::new(core_file_configuration()));
    failure.symmetry_liveness = safety.symmetry_liveness.clone().map(Box::new);
    failure
}

/// A budget stop of Core's start-up work, found before the temporal route
/// plans or captures: stage and phase `check-startup`, no capture,
/// and the start-up output printed so far.
pub(crate) fn core_startup_stop(
    verdict: dive_runtime::Verdict,
    started: Instant,
    checks: Option<Checks>,
    symmetry_liveness: dive_model::SymmetryLivenessResolution,
    output: Vec<dive_model::Message>,
) -> Failure {
    let error = match verdict {
        dive_runtime::Verdict::Stopped(stopped) => {
            dive_liveness::analyze::AnalysisError::Stopped(stopped)
        }
        dive_runtime::Verdict::BudgetMeasurement { error } => {
            dive_liveness::analyze::AnalysisError::BudgetMeasurement(error)
        }
        _ => dive_liveness::analyze::AnalysisError::Inconsistent("non-budget start-up stop"),
    };
    let mut failure = Failure::before(Error::Analysis(error), Stage::Startup);
    failure.elapsed = started.elapsed();
    failure.periodic = checks.map(Box::new);
    failure.configuration = Some(Box::new(core_file_configuration()));
    failure.symmetry_liveness = Some(Box::new(symmetry_liveness));
    failure.binding_output = output;
    failure
}

#[allow(
    clippy::too_many_arguments,
    reason = "same native producer and source-validation boundary"
)]
pub(crate) fn core_source_row_failure(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    state: dive_liveness::analyze::StateEvaluation,
    safety: &CheckResult,
    checks: Option<Checks>,
    started: Instant,
) -> Failure {
    let error = dive_liveness::analyze::AnalysisError::Atom {
        property: state.property().into(),
        source: Box::new(state.source().clone()),
        detail: state.cause().to_string(),
    };
    let mut failure = Failure::before(
        Error::Analysis(error),
        Stage::Analysis(dive_liveness::analyze::Stage::Capture),
    );
    failure.exploration = safety.stats.map(|stats| Box::new(facts(safety, stats)));
    failure.state_evaluation = Some(Box::new(state));
    failure.incomplete = true;
    failure.periodic = checks.map(Box::new);
    failure.configuration = Some(Box::new(core_file_configuration()));
    failure.symmetry_liveness = safety.symmetry_liveness.clone().map(Box::new);
    let mut failure = super::finish_row_failure(ir, config, options, failure);
    failure.elapsed = started.elapsed();
    failure
}

/// Core's own temporal analysis of a model, recovered from its validated
/// report: the final analysis, or with `prefix` the first due prefix of an
/// every-level schedule. Test inputs replay the validation boundary on it.
#[cfg(test)]
#[allow(clippy::panic, reason = "test fixture producer")]
pub(crate) fn core_analysis(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    tlc: bool,
    prefix: bool,
) -> NativeAnalysis {
    use dive_liveness::analyze::periodic::Schedule;
    use dive_liveness::analyze::periodic::external::NativePrefix;
    let opts = dive_core::Opts::default();
    let prepared = crate::core_engine::prepare(ir, config, options, tlc, opts, true)
        .unwrap_or_else(|r| panic!("core refused: {:?}", r.verdict));
    let route = prefix.then_some(Schedule {
        min_states: 2,
        growth_factor: 1,
    });
    let outcome = crate::core_temporal::check(
        ir,
        config,
        options,
        (tlc, opts),
        prepared,
        route.map(|schedule| (schedule, crate::TemporalMode::Ordinary, false)),
        &mut crate::core_engine::CoreRun::default(),
    );
    let crate::core_engine::CoreOutcome::Temporal(report) = outcome else {
        panic!("core temporal report: {outcome:?}")
    };
    match report.outcome {
        Outcome::NativeHolds(evidence) => NativeAnalysis::Final(NativeFinal::Holds(*evidence)),
        Outcome::NativeViolation(word) => {
            NativeAnalysis::Final(NativeFinal::Candidate(word.word, word.exploration))
        }
        Outcome::NativePrefixViolation(word) => NativeAnalysis::Prefix(NativePrefix {
            word: word.word,
            exploration: word.exploration,
        }),
        Outcome::Safety(result) => panic!("native outcome: {result:?}"),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual finite producer and independent proof controls"
)]
mod tests {
    use super::*;
    use dive_liveness::analyze::AnalysisError;

    /// Core's own analysis of a two-state model, so the validation boundary
    /// can be replayed on it (or on a forged copy of it).
    fn input(
        tlc: bool,
        holds: bool,
        prefix: bool,
    ) -> (Arc<Ir>, ModelConfig, CheckOptions, NativeAnalysis) {
        let atom = if holds { "x<=1" } else { "x=2" };
        let source = format!(
            "---- MODULE NativeReport ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1-x\nP == []<>({atom})\n====\n"
        );
        let parsed =
            dive_sema::analyze_source("NativeReport.tla", &source, &dive_sema::Options::default());
        assert!(parsed.accepted());
        let ir = Arc::new(dive_ir::lower(&parsed).expect("IR"));
        let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config");
        let mut options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        options.workers = 1;
        let analysis = core_analysis(&ir, &config, &options, tlc, prefix);
        (ir, config, options, analysis)
    }

    #[test]
    fn actual_final_and_prefix_words_require_fresh_typed_and_serialized_proof() {
        for tlc in [false, true] {
            let (ir, config, options, analysis) = input(tlc, true, false);
            let Outcome::NativeHolds(evidence) =
                outcome(&ir, &config, &options, analysis).expect("holding")
            else {
                panic!("native holds")
            };
            assert_eq!(evidence.stats().distinct, 2);
            assert_eq!(evidence.stats().left_on_queue, 0);
            for prefix in [false, true] {
                let (ir, config, options, analysis) = input(tlc, false, prefix);
                let actual =
                    outcome(&ir, &config, &options, analysis).expect("validated native word");
                match actual {
                    Outcome::NativeViolation(word) => {
                        assert!(!prefix);
                        assert_eq!(word.validation().validation, Validation::Valid);
                        assert_eq!(word.word().target_property, "P");
                        assert_eq!(word.document().properties, ["P"]);
                        assert_eq!(word.evidence().stats().left_on_queue, 0);
                    }
                    Outcome::NativePrefixViolation(word) => {
                        assert!(prefix);
                        assert_eq!(word.validation().validation, Validation::Valid);
                        assert!(word.evidence().stats().left_on_queue > 0);
                        assert_eq!(word.word().target_property, "P");
                    }
                    _ => panic!("native violation kind"),
                }
            }
        }
    }

    #[test]
    fn forged_word_is_not_promoted_and_keeps_actual_complete_or_prefix_facts() {
        for prefix in [false, true] {
            let (ir, config, options, mut analysis) = input(true, false, prefix);
            let word = match &mut analysis {
                NativeAnalysis::Final(NativeFinal::Candidate(word, _)) => word,
                NativeAnalysis::Prefix(prefix) => &mut prefix.word,
                NativeAnalysis::Final(_) => panic!("actual candidate"),
            };
            word.states[0] = Arc::from([Some(dive_value::Value::from(999))]);
            let failure = outcome(&ir, &config, &options, analysis).expect_err("forged Init");
            assert!(
                matches!(failure.error(), Error::NativeValidation { report, .. } if matches!(report.validation, Validation::Invalid(_)))
            );
            assert_eq!(failure.incomplete, prefix);
            assert!(failure.exploration.is_some());
        }
    }

    #[test]
    fn expired_validation_cannot_certify_a_native_word() {
        for prefix in [false, true] {
            let (ir, config, mut options, analysis) = input(true, false, prefix);
            let watch = dive_runtime::budget::Budget::new(Some(std::time::Duration::ZERO), None)
                .start()
                .expect("watchdog")
                .expect("armed");
            options.budget = watch.budget().clone();
            let failure =
                outcome(&ir, &config, &options, analysis).expect_err("validation stopped");
            assert!(matches!(
                failure.error(),
                Error::Analysis(AnalysisError::Stopped(_))
            ));
            assert_eq!(failure.incomplete, prefix);
            assert_eq!(
                failure
                    .validation_progress
                    .expect("real progress")
                    .checks_passed,
                0
            );
            assert!(failure.exploration.is_some());
        }
    }
}
