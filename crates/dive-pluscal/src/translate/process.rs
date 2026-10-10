//! Bounded one-process admission before existing label/control normalization.
use super::{FairnessOption, TranslationError, TranslationOptions, TranslationStage, error, lower};
use crate::{
    Code, Parsed,
    ast::{BindingKind, Body, Fairness, Process, Span},
};
use std::collections::BTreeSet;

fn unsupported(span: Span, feature: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported PlusCal translation: {feature}"),
    )
}
fn literal(word: &str) -> bool {
    !word.is_empty() && (word.bytes().all(|c| c.is_ascii_digit()) || word.starts_with('"'))
}
fn finite_id(parsed: &Parsed, process: &Process) -> bool {
    let words: Vec<_> = lower::tokens(parsed, process.id.expression)
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    if process.id.kind == BindingKind::Equal {
        return words.len() == 1 && literal(words[0]);
    }
    if words.len() == 3 && literal(words[0]) && words[1] == ".." && literal(words[2]) {
        return words[0].bytes().all(|c| c.is_ascii_digit())
            && words[2].bytes().all(|c| c.is_ascii_digit());
    }
    if words.first() != Some(&"{") || words.last() != Some(&"}") {
        return false;
    }
    let inside = &words[1..words.len() - 1];
    inside.is_empty()
        || inside.len() % 2 == 1
            && inside.iter().enumerate().all(
                |(i, w)| {
                    if i % 2 == 0 { literal(w) } else { *w == "," }
                },
            )
}
#[allow(
    clippy::too_many_lines,
    reason = "one admission pass proves process/local/global naming before reusing continuation normalization"
)]
pub(super) fn normalize(
    parsed: &Parsed,
    processes: &[Process],
    options: &TranslationOptions,
) -> Result<lower::Model, TranslationError> {
    if processes.len() != 1 {
        return Err(unsupported(
            parsed.algorithm.span,
            "multiple process blocks",
        ));
    }
    let process = &processes[0];
    if !finite_id(parsed, process) {
        return Err(unsupported(
            process.id.expression.span,
            "nonliteral or unproved finite process IDs",
        ));
    }
    let globals: BTreeSet<_> = parsed
        .algorithm
        .variables
        .iter()
        .map(|d| d.name.text.as_str())
        .collect();
    let locals: BTreeSet<_> = process
        .variables
        .iter()
        .map(|d| d.name.text.as_str())
        .collect();
    let reserved = [
        "self",
        "pc",
        "ProcSet",
        "vars",
        "Init",
        "Next",
        "Spec",
        "Terminating",
        "Termination",
        "Done",
    ];
    if reserved.contains(&process.name.text.as_str())
        || globals.contains(process.name.text.as_str())
        || locals.contains(process.name.text.as_str())
    {
        return Err(unsupported(
            process.name.span,
            "process/generated-operator name disambiguation",
        ));
    }
    for variable in &process.variables {
        if globals.contains(variable.name.text.as_str())
            || reserved.contains(&variable.name.text.as_str())
        {
            return Err(unsupported(
                variable.span,
                "process local/global/generated-name disambiguation",
            ));
        }
        if let Some(binding) = &variable.initial {
            let words = lower::tokens(parsed, binding.expression);
            if binding.kind == BindingKind::Member
                && words
                    .iter()
                    .any(|t| locals.contains(t.text.as_str()) || t.text == "self")
            {
                return Err(unsupported(
                    binding.expression.span,
                    "local membership initializer choice/subscript binding",
                ));
            }
        }
    }
    for variable in &parsed.algorithm.variables {
        if variable.name.text == "ProcSet" || variable.name.text == "self" {
            return Err(unsupported(
                variable.span,
                "process global/generated-name disambiguation",
            ));
        }
        if let Some(binding) = &variable.initial
            && lower::tokens(parsed, binding.expression)
                .iter()
                .any(|t| locals.contains(t.text.as_str()) || t.text == "self")
        {
            return Err(unsupported(
                binding.expression.span,
                "global initializer references process locals/self",
            ));
        }
    }
    if process.id.kind == BindingKind::Equal && parsed.tokens.iter().any(|t| t.text == "self") {
        return Err(unsupported(
            process.span,
            "unbound self in a constant-ID process",
        ));
    }
    for statement in &parsed.statements {
        if let Some(label) = &statement.label
            && (label.name.text == process.name.text || label.name.text == "ProcSet")
        {
            return Err(unsupported(
                label.span,
                "process/label/generated-name disambiguation",
            ));
        }
    }
    // Names are admitted before concatenating the scalar storage declarations.
    // Existing continuation/changed-variable rules then apply to this one body.
    let mut sequential = parsed.clone();
    sequential.algorithm.body = Body::Sequential(process.body.clone());
    // The reference's --fair/options warning is emitted only by its sequential
    // grammar branch. Global process fairness is assigned separately below.
    sequential.algorithm.fairness = Fairness::Unfair;
    sequential
        .algorithm
        .variables
        .extend(process.variables.clone());
    let mut model = lower::normalize(&sequential, options)?;
    let mut process = process.clone();
    process.fairness = if options.fairness == FairnessOption::None {
        Fairness::Unfair
    } else if process.fairness != Fairness::Unfair {
        process.fairness
    } else {
        match options.fairness {
            FairnessOption::Weak => Fairness::Weak,
            FairnessOption::Strong => Fairness::Strong,
            _ => Fairness::Unfair,
        }
    };
    if process.fairness != Fairness::Unfair {
        let source = parsed.text(process.id.expression.span).unwrap_or_default();
        if source.contains(['\n', '\r', '\t']) || source.contains("(*") || source.contains("\\*") {
            return Err(unsupported(
                process.id.expression.span,
                "fairness domain multiline/comment/tab normalization",
            ));
        }
    }
    model.fair = options.fairness != FairnessOption::None
        && (options.fairness == FairnessOption::Next
            || parsed.algorithm.fairness == Fairness::Weak);
    model.process = Some(process);
    Ok(model)
}
