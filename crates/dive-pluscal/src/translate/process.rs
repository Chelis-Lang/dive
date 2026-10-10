//! Bounded finite-process admission before existing label/control normalization.
use super::macros::Expanded as Parsed;
use super::{FairnessOption, TranslationError, TranslationOptions, TranslationStage, error, lower};
use crate::{
    Code,
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
        return normalize_multiple(parsed, processes, options);
    }
    normalize_one(parsed, &processes[0], options, false)
}
#[allow(
    clippy::too_many_lines,
    reason = "single-process admission checks binding, namespace and fairness before lowering"
)]
fn normalize_one(
    parsed: &Parsed,
    process: &Process,
    options: &TranslationOptions,
    force_pc: bool,
) -> Result<lower::Model, TranslationError> {
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
    if process.id.kind == BindingKind::Equal
        && parsed
            .semantic_tokens(process.span)
            .into_iter()
            .any(|t| t.text == "self")
    {
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
    let mut model = lower::normalize_with_pc(&sequential, options, force_pc)?;
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
    model.processes = vec![process];
    Ok(model)
}

#[derive(Clone)]
enum IdRange {
    Integer(u64, u64),
    String(String),
}
fn scalar_id(word: &str) -> Option<IdRange> {
    if word.bytes().all(|b| b.is_ascii_digit()) {
        let n = word.parse().ok()?;
        Some(IdRange::Integer(n, n))
    } else if word.starts_with('"') && word.ends_with('"') && !word.contains('\\') {
        Some(IdRange::String(word.into()))
    } else {
        None
    }
}
fn domain_ids(parsed: &Parsed, process: &Process) -> Option<Vec<IdRange>> {
    let words: Vec<_> = lower::tokens(parsed, process.id.expression)
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    if process.id.kind == BindingKind::Equal {
        return Some(vec![scalar_id(words.first().copied()?)?]);
    }
    if words.len() == 3 && words[1] == ".." {
        let (IdRange::Integer(lo, _), IdRange::Integer(hi, _)) =
            (scalar_id(words[0])?, scalar_id(words[2])?)
        else {
            return None;
        };
        return Some(if lo <= hi {
            vec![IdRange::Integer(lo, hi)]
        } else {
            vec![]
        });
    }
    if words.first() != Some(&"{") || words.last() != Some(&"}") {
        return None;
    }
    words[1..words.len() - 1]
        .iter()
        .step_by(2)
        .map(|w| scalar_id(w))
        .collect()
}
fn intersects(left: &IdRange, right: &IdRange) -> bool {
    match (left, right) {
        (IdRange::Integer(a, b), IdRange::Integer(c, d)) => a <= d && c <= b,
        (IdRange::String(a), IdRange::String(b)) => a == b,
        _ => false,
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "the bounded multi-process pass proves disjoint domains and namespace ownership before lowering each body"
)]
fn normalize_multiple(
    parsed: &Parsed,
    processes: &[Process],
    options: &TranslationOptions,
) -> Result<lower::Model, TranslationError> {
    if processes.is_empty() {
        return Err(unsupported(
            parsed.algorithm.span,
            "empty process collection",
        ));
    }
    if processes.len() > options.max_actions.min(10_000) {
        return Err(error(
            TranslationStage::Normalization,
            Code::Resource,
            parsed.algorithm.span,
            "PlusCal process count exceeds the total action budget",
        ));
    }
    let mut ids: Vec<Vec<IdRange>> = Vec::new();
    let mut names: BTreeSet<&str> = parsed
        .algorithm
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
    let all_locals: BTreeSet<_> = processes
        .iter()
        .flat_map(|p| &p.variables)
        .map(|v| v.name.text.as_str())
        .collect();
    for process in processes {
        if !finite_id(parsed, process) {
            return Err(unsupported(
                process.id.expression.span,
                "nonliteral or unproved finite process IDs",
            ));
        }
        let domain = domain_ids(parsed, process).ok_or_else(|| {
            unsupported(
                process.id.expression.span,
                "multiple-process ID disjointness outside bounded integers/unescaped strings",
            )
        })?;
        if ids.iter().any(|previous| {
            previous
                .iter()
                .any(|a| domain.iter().any(|b| intersects(a, b)))
        }) {
            return Err(unsupported(
                process.id.expression.span,
                "overlapping process ID domains",
            ));
        }
        ids.push(domain);
        for (name, span) in std::iter::once((&process.name.text, process.name.span))
            .chain(process.variables.iter().map(|v| (&v.name.text, v.span)))
        {
            if reserved.contains(&name.as_str()) || !names.insert(name.as_str()) {
                return Err(unsupported(
                    span,
                    "multiple-process local/process/global name disambiguation",
                ));
            }
        }
    }
    for statement in &parsed.statements {
        if let Some(label) = &statement.label
            && !names.insert(label.name.text.as_str())
        {
            return Err(unsupported(
                label.span,
                "multiple-process label/name disambiguation",
            ));
        }
    }
    for process in processes {
        let labels: BTreeSet<_> = parsed
            .statements
            .iter()
            .filter(|s| s.span.start >= process.span.start && s.span.end <= process.span.end)
            .filter_map(|s| s.label.as_ref().map(|l| l.name.text.as_str()))
            .collect();
        let locals: BTreeSet<_> = process
            .variables
            .iter()
            .map(|v| v.name.text.as_str())
            .collect();
        for token in parsed.semantic_tokens(process.span) {
            if all_locals.contains(token.text.as_str()) && !locals.contains(token.text.as_str()) {
                return Err(unsupported(
                    token.span,
                    "cross-process local-variable scope",
                ));
            }
        }
        for statement in parsed
            .statements
            .iter()
            .filter(|s| s.span.start >= process.span.start && s.span.end <= process.span.end)
        {
            if let crate::ast::StatementKind::Goto(target) = &statement.kind
                && !["Done", "\"Done\""].contains(&target.text.as_str())
                && !labels.contains(target.text.as_str())
            {
                return Err(error(
                    TranslationStage::Normalization,
                    Code::Label,
                    target.span,
                    "goto target is outside the current process's labels",
                ));
            }
        }
    }
    for global in &parsed.algorithm.variables {
        if let Some(binding) = &global.initial
            && lower::tokens(parsed, binding.expression)
                .iter()
                .any(|t| all_locals.contains(t.text.as_str()))
        {
            return Err(unsupported(
                binding.expression.span,
                "global initializer references process locals",
            ));
        }
    }
    let mut parts = normalize_parts(parsed, processes, options, false)?;
    // The pinned parser omits pc only when every process has a single infinite
    // loop with no internal labels, jumps or label modifiers.
    let use_pc = parts.iter().any(|p| p.use_pc);
    if parts.iter().any(|p| p.use_pc != use_pc) {
        parts.clear();
        parts = normalize_parts(parsed, processes, options, true)?;
    }
    let mut model = parts.remove(0);
    for part in parts {
        model.actions.extend(part.actions);
        model.variables.extend(part.variables);
        model.processes.extend(part.processes);
        model.stutter &= part.stutter;
        model.warnings.extend(part.warnings);
    }
    if model.actions.len() > options.max_actions.min(10_000) {
        return Err(error(
            TranslationStage::Normalization,
            Code::Resource,
            parsed.algorithm.span,
            "PlusCal action limit reached across all processes",
        ));
    }
    Ok(model)
}

fn normalize_parts(
    parsed: &Parsed,
    processes: &[Process],
    options: &TranslationOptions,
    force_pc: bool,
) -> Result<Vec<lower::Model>, TranslationError> {
    let mut parts = Vec::new();
    let mut actions = 0usize;
    let mut nodes = 0usize;
    for process in processes {
        let mut part = normalize_one(parsed, process, options, force_pc)?;
        actions = actions.checked_add(part.actions.len()).ok_or_else(|| {
            error(
                TranslationStage::Normalization,
                Code::Resource,
                process.span,
                "PlusCal aggregate action count overflow",
            )
        })?;
        if actions > options.max_actions.min(10_000) {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                process.span,
                "PlusCal action limit reached across all processes",
            ));
        }
        charge_nodes(parsed, &part, &mut nodes, options)?;
        // Retain global declarations once, not once per admitted process.
        // Each body's lowering still sees the complete globals while proving
        // assignments and binders; only the stored aggregate is deduplicated.
        if !parts.is_empty() {
            part.variables.drain(..parsed.algorithm.variables.len());
        }
        parts.push(part);
    }
    Ok(parts)
}

fn charge_nodes(
    parsed: &Parsed,
    part: &lower::Model,
    used: &mut usize,
    options: &TranslationOptions,
) -> Result<(), TranslationError> {
    let limit = options.parser.max_nodes.saturating_mul(4).min(200_000);
    let mut pending: Vec<_> = part.actions.iter().flat_map(|a| &a.body).collect();
    while let Some(node) = pending.pop() {
        *used = used.saturating_add(1);
        if *used > limit {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                parsed.algorithm.span,
                "PlusCal normalized-node limit reached across all processes",
            ));
        }
        match &node.kind {
            lower::Kind::If { yes, no, .. } => {
                pending.extend(yes);
                pending.extend(no);
            }
            lower::Kind::Either(choices) => pending.extend(choices.iter().flatten()),
            lower::Kind::With { body, .. } => pending.extend(body),
            _ => {}
        }
    }
    Ok(())
}
