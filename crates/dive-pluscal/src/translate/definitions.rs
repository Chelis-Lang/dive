//! Source-owned DEFINE text and bounded ordinary-operator admission.
//!
//! Matches `PcalTLAGen.GenVarsAndDefs`/`TLAExpr.toStringVector`; helpers are
//! deliberately not inlined or substituted after earlier assignments.
use super::{TranslationError, TranslationOptions, TranslationStage, error, macros::Expanded};
use crate::{
    Code,
    ast::{Expr, Span, StatementKind},
};
use dive_syntax::{SyntaxKind, ast::Unit};
use std::collections::{BTreeMap, BTreeSet};

fn unsupported(span: Span, feature: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported PlusCal translation: DEFINE {feature}"),
    )
}
fn resource(span: Span) -> TranslationError {
    error(
        TranslationStage::Generation,
        Code::Resource,
        span,
        "PlusCal DEFINE rendered-byte limit reached",
    )
}

/// Reconstruct significant tokens at reference UTF-16/tab-expanded columns.
/// Comments are absent, but their columns and intervening lines remain.
pub(super) fn render(
    parsed: &Expanded,
    expr: Expr,
    options: &TranslationOptions,
) -> Result<String, TranslationError> {
    let positions = super::checksum::token_positions(parsed)?;
    let tokens = parsed.tokens(expr);
    let mut located = Vec::with_capacity(tokens.len());
    let mut previous = None;
    let mut minimum = usize::MAX;
    for token in tokens {
        let index = positions
            .binary_search_by_key(&token.span.start, |(offset, _)| *offset)
            .map_err(|_| unsupported(expr.span, "unknown token position"))?;
        let (line, column) = positions[index].1;
        if previous != Some(line) {
            minimum = minimum.min(column);
        }
        previous = Some(line);
        located.push((line, column));
    }
    let limit = options.max_output_bytes.min(4_194_304);
    let mut text = String::new();
    let mut previous = None;
    let mut current_column = 0;
    for (token, &(line, column)) in tokens.iter().zip(&located) {
        if let Some(before) = previous
            && line != before
        {
            let gap = line - before;
            if text.len().saturating_add(gap) > limit {
                return Err(resource(expr.span));
            }
            text.extend(std::iter::repeat_n('\n', gap));
            current_column = 0;
        }
        previous = Some(line);
        let column = column.saturating_sub(minimum);
        let gap = column
            .checked_sub(current_column)
            .ok_or_else(|| unsupported(expr.span, "overlapping token columns"))?;
        if text
            .len()
            .saturating_add(gap)
            .saturating_add(token.text.len())
            > limit
        {
            return Err(resource(expr.span));
        }
        text.extend(std::iter::repeat_n(' ', gap));
        text.push_str(&token.text);
        current_column = column + token.text.encode_utf16().count();
    }
    Ok(text)
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded namespace/body proof precedes native generation"
)]
pub(super) fn admit(
    parsed: &Expanded,
    options: &TranslationOptions,
) -> Result<(), TranslationError> {
    let Some(expr) = parsed.algorithm.definitions else {
        return Ok(());
    };
    if !parsed.algorithm.procedures.is_empty() {
        return Err(unsupported(expr.span, "procedure scopes"));
    }
    let text = render(parsed, expr, options)?;
    let module_text = format!("---- MODULE DefineAdmission ----\n{text}\n====\n");
    let syntax = dive_syntax::parse_module(&module_text);
    if !syntax.is_ok() {
        return Err(unsupported(expr.span, "unproved TLA declaration grammar"));
    }
    let Some(module) = syntax.module() else {
        return Err(unsupported(expr.span, "missing parsed declaration module"));
    };
    let mut graph = BTreeMap::new();
    let mut reserved: BTreeSet<&str> = [
        "pc",
        "stack",
        "self",
        "ProcSet",
        "vars",
        "Init",
        "Next",
        "Spec",
        "Terminating",
        "Termination",
        "defaultInitValue",
        "Done",
    ]
    .into_iter()
    .collect();
    reserved.extend(
        parsed
            .algorithm
            .variables
            .iter()
            .map(|d| d.name.text.as_str()),
    );
    if let crate::ast::Body::Processes(processes) = &parsed.algorithm.body {
        for process in processes {
            reserved.insert(process.name.text.as_str());
            reserved.extend(process.variables.iter().map(|d| d.name.text.as_str()));
        }
    }
    reserved.extend(
        parsed
            .statements
            .iter()
            .filter_map(|s| s.label.as_ref().map(|l| l.name.text.as_str())),
    );
    for unit in module.units() {
        let Unit::OperatorDef(definition) = unit else {
            return Err(unsupported(expr.span, "non-operator declaration"));
        };
        let Some(name) = definition.name() else {
            return Err(unsupported(expr.span, "unnamed operator"));
        };
        if definition.is_local()
            || definition
                .lhs()
                .is_none_or(|lhs| lhs.kind() != SyntaxKind::IDENT_LHS)
        {
            return Err(unsupported(expr.span, "LOCAL or symbolic operator"));
        }
        if reserved.contains(name.as_str()) || graph.contains_key(&name) {
            return Err(unsupported(
                expr.span,
                "operator/global/generated-name conflict",
            ));
        }
        let Some(body) = definition.body() else {
            return Err(unsupported(expr.span, "missing operator body"));
        };
        let mut references = BTreeSet::new();
        for token in body
            .syntax()
            .descendants_with_tokens()
            .filter_map(dive_syntax::SyntaxElement::into_token)
        {
            if ["'", "<>", "~>", "UNCHANGED", "ENABLED"].contains(&token.text())
                || token.text() == "[]"
                    && token
                        .parent()
                        .is_none_or(|parent| parent.kind() != SyntaxKind::CASE_EXPR)
                || token.text().starts_with("WF_")
                || token.text().starts_with("SF_")
            {
                return Err(unsupported(expr.span, "action or temporal helper"));
            }
            if token.kind() == SyntaxKind::IDENT {
                references.insert(token.text().to_string());
            }
        }
        graph.insert(name, references);
        if graph.len() > options.parser.max_nodes.min(50_000) {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                expr.span,
                "PlusCal DEFINE declaration/dependency-work limit reached",
            ));
        }
    }
    // Conservative call-graph proof: even formal/field spellings can trigger a
    // named refusal here; no unproved recursive expansion is silently admitted.
    let mut work = 0usize;
    for root in graph.keys() {
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name) {
                continue;
            }
            if let Some(references) = graph.get(name) {
                for reference in references {
                    work = work.saturating_add(1);
                    if work > options.parser.max_nodes.saturating_mul(4).min(200_000) {
                        return Err(error(
                            TranslationStage::Normalization,
                            Code::Resource,
                            expr.span,
                            "PlusCal DEFINE declaration/dependency-work limit reached",
                        ));
                    }
                    if reference == root {
                        return Err(unsupported(expr.span, "recursive operator dependencies"));
                    }
                    if let Some((name, _)) = graph.get_key_value(reference) {
                        pending.push(name);
                    }
                }
            }
        }
    }
    for statement in &parsed.statements {
        if let StatementKind::With { bindings, .. } = &statement.kind
            && bindings
                .iter()
                .any(|binding| graph.contains_key(&binding.name.text))
        {
            return Err(unsupported(
                statement.span,
                "with-binder/operator name disambiguation",
            ));
        }
    }
    Ok(())
}
