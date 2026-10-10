//! Bounded sequential `PcalTranslate.ExplodeCall` / `ExplodeReturn` admission.
use super::{
    TranslationError, TranslationOptions, TranslationStage, error, lower, macros::Expanded,
};
use crate::{
    Code,
    ast::{BindingKind, Body, Span, StatementKind},
};
use std::collections::BTreeSet;

fn refusal(span: Span, message: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported PlusCal translation: {message}"),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded admission pass proves scoped names before any call lowering"
)]
pub(super) fn normalize(
    parsed: &Expanded,
    options: &TranslationOptions,
) -> Result<lower::Model, TranslationError> {
    let Body::Sequential(_) = &parsed.algorithm.body else {
        return Err(refusal(
            parsed.algorithm.span,
            "procedure/process storage ownership",
        ));
    };
    if parsed.algorithm.procedures.len() != 1 {
        return Err(refusal(
            parsed.algorithm.span,
            "multiple procedure namespace/call graph",
        ));
    }
    let procedure = &parsed.algorithm.procedures[0];
    let mut names = BTreeSet::new();
    for name in parsed
        .algorithm
        .variables
        .iter()
        .map(|d| &d.name)
        .chain(procedure.parameters.iter().map(|d| &d.name))
        .chain(procedure.variables.iter().map(|d| &d.name))
    {
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
            || name.text == procedure.name.text
        {
            return Err(refusal(
                name.span,
                "procedure/global/generated-name disambiguation",
            ));
        }
    }
    if names.contains(procedure.name.text.as_str())
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
    let mut labels = BTreeSet::new();
    for statement in &parsed.statements {
        if let Some(label) = &statement.label {
            if label.modifier.is_some() {
                return Err(refusal(label.span, "procedure label fairness modifiers"));
            }
            if !labels.insert(label.name.text.as_str())
                || label.name.text == procedure.name.text
                || names.contains(label.name.text.as_str())
            {
                return Err(refusal(label.span, "procedure/global label disambiguation"));
            }
        }
        if statement.span.start >= procedure.body.span.start
            && statement.span.end <= procedure.body.span.end
            && matches!(statement.kind, StatementKind::Call { .. })
        {
            return Err(refusal(statement.span, "nested/recursive procedure calls"));
        }
    }
    let mut scoped = parsed.clone();
    scoped
        .algorithm
        .variables
        .extend(procedure.parameters.clone());
    scoped
        .algorithm
        .variables
        .extend(procedure.variables.clone());
    let mut main_model = lower::normalize_context(&scoped, options, true, None)?;
    main_model.initial_pc = Some(main_model.actions[0].name.clone());
    scoped.algorithm.body = Body::Sequential(procedure.body.clone());
    let mut remaining = options.clone();
    remaining.max_actions = options
        .max_actions
        .min(10_000)
        .saturating_sub(main_model.actions.len());
    let mut procedure_model = lower::normalize_context(&scoped, &remaining, true, Some(procedure))?;
    charge_aggregate(
        &procedure_model,
        &main_model,
        options,
        parsed.algorithm.span,
    )?;
    procedure_model.actions.append(&mut main_model.actions);
    procedure_model.stutter = main_model.stutter;
    procedure_model.fair = main_model.fair;
    procedure_model.warnings = main_model.warnings;
    procedure_model.initial_pc = main_model.initial_pc;
    procedure_model.procedures = vec![procedure.clone()];
    // Original declaration order is global, stack, parameters, then locals.
    // The emitter inserts stack without pretending it was a source declaration.
    procedure_model
        .variables
        .clone_from(&scoped.algorithm.variables);
    Ok(procedure_model)
}

fn charge_aggregate(
    first: &lower::Model,
    second: &lower::Model,
    options: &TranslationOptions,
    span: Span,
) -> Result<(), TranslationError> {
    let limit = options.parser.max_nodes.saturating_mul(4).min(200_000);
    let mut used = 0usize;
    let mut pending: Vec<_> = first
        .actions
        .iter()
        .chain(&second.actions)
        .flat_map(|a| &a.body)
        .collect();
    while let Some(node) = pending.pop() {
        used = used.saturating_add(1);
        if used > limit {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                span,
                "PlusCal normalized-node limit reached across main and procedure",
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
