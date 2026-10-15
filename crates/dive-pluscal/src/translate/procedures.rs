//! Bounded `PcalTranslate` procedure namespace and borrowed-scope call admission.
use super::{
    TranslationError, TranslationOptions, TranslationStage, error, lower, macros::Expanded,
};
use crate::{
    Code,
    ast::{BindingKind, Body, Span},
};
use std::collections::{BTreeMap, BTreeSet};

fn refusal(span: Span, message: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported PlusCal translation: {message}"),
    )
}

pub(super) fn normalize(
    parsed: &Expanded,
    options: &TranslationOptions,
) -> Result<lower::Model, TranslationError> {
    normalize_context(parsed, options, false)
}
pub(super) fn normalize_process(
    parsed: &Expanded,
    options: &TranslationOptions,
) -> Result<lower::Model, TranslationError> {
    normalize_context(parsed, options, true)
}
#[allow(
    clippy::too_many_lines,
    reason = "one bounded admission pass proves every namespace before call lowering"
)]
fn normalize_context(
    parsed: &Expanded,
    options: &TranslationOptions,
    process: bool,
) -> Result<lower::Model, TranslationError> {
    let Body::Sequential(_) = &parsed.algorithm.body else {
        return Err(refusal(
            parsed.algorithm.span,
            "procedure/process storage ownership",
        ));
    };
    let mut names = BTreeSet::new();
    for name in parsed.algorithm.variables.iter().map(|d| &d.name).chain(
        parsed
            .algorithm
            .procedures
            .iter()
            .flat_map(|p| p.parameters.iter().chain(&p.variables).map(|d| &d.name)),
    ) {
        if !names.insert(name.text.as_str())
            || [
                "stack",
                "pc",
                "self",
                "vars",
                "Init",
                "Next",
                "Spec",
                "Termination",
                "Terminating",
                "defaultInitValue",
            ]
            .contains(&name.text.as_str())
        {
            return Err(refusal(
                name.span,
                "procedure/global/generated-name disambiguation",
            ));
        }
    }
    let mut procedures = BTreeMap::new();
    for (index, procedure) in parsed.algorithm.procedures.iter().enumerate() {
        if procedures
            .insert(procedure.name.text.as_str(), index)
            .is_some()
            || names.contains(procedure.name.text.as_str())
            || [
                "Done",
                "Error",
                "stack",
                "pc",
                "vars",
                "Init",
                "Next",
                "Spec",
                "Termination",
                "Terminating",
            ]
            .contains(&procedure.name.text.as_str())
        {
            return Err(refusal(
                procedure.name.span,
                "procedure operator/name disambiguation",
            ));
        }
        for decl in procedure.parameters.iter().chain(&procedure.variables) {
            if decl
                .initial
                .as_ref()
                .is_some_and(|b| b.kind != BindingKind::Equal)
            {
                return Err(refusal(decl.span, "procedure membership initialization"));
            }
        }
    }
    let mut labels = BTreeSet::new();
    for statement in &parsed.statements {
        if let Some(label) = &statement.label {
            if label.modifier.is_some() && !process {
                return Err(refusal(label.span, "procedure label fairness modifiers"));
            }
            if !labels.insert(label.name.text.as_str())
                || procedures.contains_key(label.name.text.as_str())
                || names.contains(label.name.text.as_str())
            {
                return Err(refusal(label.span, "procedure/global label disambiguation"));
            }
        }
    }
    // Calls remain runtime stack transitions, including cycles. No call graph
    // is recursively expanded while translating a finite source program.
    // One private arena holds all source statements and declarations. Each
    // scope borrows it; no whole-arena clone is multiplied by procedure count.
    let mut scoped = parsed.clone();
    for procedure in &parsed.algorithm.procedures {
        scoped
            .algorithm
            .variables
            .extend(procedure.parameters.clone());
        scoped
            .algorithm
            .variables
            .extend(procedure.variables.clone());
    }
    let mut model = lower::normalize_context(&scoped, options, true, None)?;
    model.initial_pc = Some(
        model
            .actions
            .first()
            .ok_or_else(|| {
                refusal(
                    parsed.algorithm.span,
                    "empty/automatically labelled algorithm",
                )
            })?
            .name
            .clone(),
    );
    let mut used_nodes = 0;
    charge(&model, &mut used_nodes, options, parsed.algorithm.span)?;
    let mut actions = Vec::new();
    for procedure in &scoped.algorithm.procedures {
        let mut remaining = options.clone();
        remaining.max_actions = options
            .max_actions
            .min(10_000)
            .saturating_sub(model.actions.len() + actions.len());
        let mut part = lower::normalize_context(&scoped, &remaining, true, Some(procedure))?;
        charge(&part, &mut used_nodes, options, procedure.span)?;
        actions.append(&mut part.actions);
    }
    actions.append(&mut model.actions);
    model.actions = actions;
    model.procedures.clone_from(&parsed.algorithm.procedures);
    Ok(model)
}

fn charge(
    model: &lower::Model,
    used: &mut usize,
    options: &TranslationOptions,
    span: Span,
) -> Result<(), TranslationError> {
    let limit = options.parser.max_nodes.saturating_mul(4).min(200_000);
    let mut pending: Vec<_> = model.actions.iter().flat_map(|a| &a.body).collect();
    while let Some(node) = pending.pop() {
        *used = used.saturating_add(1);
        if *used > limit {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                span,
                "PlusCal normalized-node limit reached across main and procedures",
            ));
        }
        match &node.kind {
            lower::Kind::If { yes, no, .. } => {
                pending.extend(yes);
                pending.extend(no);
            }
            lower::Kind::Either(choices) => pending.extend(choices.iter().flatten()),
            lower::Kind::With { body, .. } => pending.extend(body),
            lower::Kind::Leaf => {}
        }
    }
    Ok(())
}
