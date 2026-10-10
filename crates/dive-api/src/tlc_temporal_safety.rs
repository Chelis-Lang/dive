//! Bounded ordinary safety checking with independent finite witness authority.
use super::{Error, Failure, Safety, before, bind};
use dive_check::{CheckOptions, CheckResult, Verdict, capture};
use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_liveness::phase_graph::Limits;
use dive_model::Model;
use dive_trace::{Ending, Trace, Validation};
use std::sync::Arc;

fn ending(result: &CheckResult) -> Option<Ending> {
    match &result.verdict {
        Verdict::InvariantViolated { property, .. } => Some(Ending::Invariant(property.clone())),
        Verdict::PropertyViolatedInitially { property }
        | Verdict::ActionPropertyViolated { property } => Some(Ending::Property(property.clone())),
        Verdict::Deadlock => Some(Ending::Deadlock),
        Verdict::EvaluationError { cause, .. } => {
            Some(if cause.ec() == dive_eval::Ec::AssertFailed {
                Ending::Assertion
            } else {
                Ending::Error {
                    ec: Some(cause.ec().name().into()),
                }
            })
        }
        _ => None,
    }
}
fn failure(error: Error, result: &CheckResult) -> Box<Failure> {
    Box::new(Failure {
        error,
        workers: Some(result.workers.clone()),
        evidence: None,
        elapsed: std::time::Duration::ZERO,
    })
}
fn trace(cx: &dive_check::Counterexample, ending: Ending) -> Trace {
    Trace {
        states: cx
            .states
            .iter()
            .map(|s| {
                s.values
                    .iter()
                    .cloned()
                    .map(Some)
                    .collect::<Vec<_>>()
                    .into()
            })
            .collect(),
        labels: cx
            .states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 {
                    None
                } else {
                    s.action.as_ref().map(|n| dive_trace::Label {
                        name: n.clone(),
                        location: Some(s.label.clone()),
                    })
                }
            })
            .collect(),
        ending,
        length: Some(cx.states.len()),
        properties: vec![],
    }
}
pub(super) fn safety(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
    model: &Model,
) -> Result<Safety, Box<Failure>> {
    // The independent validator checks constraints at each supplied state. It
    // cannot yet replay TLC's earlier property stop on an out-of-model state.
    if (!model.implied_inits.is_empty() && !model.constraints.is_empty())
        || (!model.implied_actions.is_empty()
            && (!model.constraints.is_empty() || !model.action_constraints.is_empty()))
    {
        return Err(before(
            Error::Unsupported(
                "finite implied-property replay with state/action constraints is not certified",
            ),
            options,
        ));
    }
    let result =
        match capture::explore_concrete_parallel_with_evidence(model, options, limits.capture)
            .map_err(|e| before(Error::Thread(e), options))?
        {
            capture::Exploration::Complete(c) => Box::new(c.safety_result().clone()),
            capture::Exploration::Stopped(r) => r,
        };
    let mut receipt = Safety {
        limits,
        result,
        validation: None,
        document: None,
    };
    let Some(ending) = ending(&receipt.result) else {
        // An incomplete/resource/refused prefix is useful evidence, not a
        // violation to certify. Retain any supplied prefix without replay credit.
        return Ok(receipt);
    };
    let Some(cx) = &receipt.result.counterexample else {
        let report = dive_trace::Report {
            validation: Validation::Unsupported(
                "finite safety stop has no replayable witness".into(),
            ),
            notes: vec![],
            length: 0,
        };
        return Err(failure(
            Error::SafetyValidation {
                limits: Box::new(limits),
                result: receipt.result.clone(),
                report,
            },
            &receipt.result,
        ));
    };
    let replay = bind(ir, config, options).map_err(|e| failure(e, &receipt.result))?;
    replay
        .validate_context()
        .map_err(|e| failure(Error::Input(e), &receipt.result))?;
    let trace = trace(cx, ending);
    let report = dive_trace::validate(&replay, &trace, None);
    if report.validation != Validation::Valid {
        return Err(failure(
            Error::SafetyValidation {
                limits: Box::new(limits),
                result: receipt.result.clone(),
                report,
            },
            &receipt.result,
        ));
    }
    let document = trace
        .to_doc(&replay)
        .map_err(|e| failure(Error::Document(e), &receipt.result))?;
    let report = dive_trace::validate_doc(&replay, &document, &config.model_values, None).0;
    if report.validation != Validation::Valid {
        return Err(failure(
            Error::SafetyValidation {
                limits: Box::new(limits),
                result: receipt.result.clone(),
                report,
            },
            &receipt.result,
        ));
    }
    receipt.validation = Some(report);
    receipt.document = Some(document);
    Ok(receipt)
}
