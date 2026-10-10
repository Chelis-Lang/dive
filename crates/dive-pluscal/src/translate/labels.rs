//! Bounded sequential `ParseAlgorithm.InnerAddLabels` normalization.
use super::{InsertedLabel, TranslationError, TranslationOptions, TranslationStage, error};
use crate::{
    Code, Parsed,
    ast::{Block, Body, Label, Name, Span, StatementKind, StmtId},
};
use std::collections::{BTreeMap, BTreeSet};

struct Labels<'a> {
    parsed: &'a mut Parsed,
    options: &'a TranslationOptions,
    typed: BTreeSet<String>,
    next: usize,
    added: Vec<InsertedLabel>,
    positions: BTreeMap<usize, crate::ast::Position>,
}
fn failure(span: Span, message: &str) -> TranslationError {
    error(TranslationStage::Normalization, Code::Label, span, message)
}
/// Work on a clone: generated names must not masquerade as source AST spelling.
pub(super) fn normalize(
    parsed: &mut Parsed,
    options: &TranslationOptions,
) -> Result<Vec<InsertedLabel>, TranslationError> {
    let root = &options.label_root;
    let mut chars = root.chars();
    if root.len() > 256 {
        return Err(error(
            TranslationStage::Normalization,
            Code::Resource,
            parsed.algorithm.span,
            "PlusCal label-root byte limit (256) reached",
        ));
    }
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(failure(
            parsed.algorithm.span,
            "label root must start with an ASCII letter and contain only ASCII letters, digits or underscore (maximum 256 bytes)",
        ));
    }
    let blocks = match &parsed.algorithm.body {
        Body::Sequential(block) => vec![block.clone()],
        Body::Processes(processes) => processes.iter().map(|p| p.body.clone()).collect(),
    };
    // These families have separate expansion/normalization and stay refused by
    // the existing admission pass rather than assigning invented labels.
    if !parsed.algorithm.procedures.is_empty() || !parsed.algorithm.macros.is_empty() {
        return Ok(Vec::new());
    }
    let typed: BTreeSet<_> = parsed
        .statements
        .iter()
        .filter_map(|s| s.label.as_ref().map(|l| l.name.text.clone()))
        .collect();
    // Unlike the sequential source form, process bodies always require the
    // explicit LabelFlag when the reference inserts any missing label.
    let has_typed = !typed.is_empty() || matches!(parsed.algorithm.body, Body::Processes(_));
    let positions = positions(parsed);
    let mut labels = Labels {
        parsed,
        options,
        typed,
        next: 1,
        added: Vec::new(),
        positions,
    };
    for block in &blocks {
        labels.sequence(block, true, false, &BTreeSet::new(), 0)?;
    }
    if has_typed && !options.insert_missing_labels && !labels.added.is_empty() {
        return Err(failure(
            labels.added[0].source,
            "missing label; explicit labels require --label to insert additional labels",
        ));
    }
    Ok(labels.added)
}
// One pass instead of rescanning the entire source for every generated label.
fn positions(parsed: &Parsed) -> BTreeMap<usize, crate::ast::Position> {
    let mut targets = parsed
        .tokens
        .iter()
        .map(|token| token.span.start)
        .peekable();
    let mut result = BTreeMap::new();
    let mut line = 1;
    let mut column = 1;
    for (offset, character) in parsed.source.char_indices() {
        if targets.peek() == Some(&offset) {
            result.insert(offset, crate::ast::Position { line, column });
            targets.next();
        }
        match character {
            '\n' => {
                line += 1;
                column = 1;
            }
            '\t' => column += 8 - (column - 1) % 8,
            '\r' if parsed.source.as_bytes().get(offset + 1) == Some(&b'\n') => {}
            _ => column += 1,
        }
    }
    result
}

impl Labels<'_> {
    fn flattened(&self, block: &Block) -> Result<Vec<StmtId>, TranslationError> {
        let mut result = Vec::new();
        for id in &block.statements {
            let statement = &self.parsed.statements[id.0];
            if let StatementKind::Block(child) = &statement.kind {
                if statement.label.is_some() {
                    return Err(error(
                        TranslationStage::Normalization,
                        Code::Unsupported,
                        statement.span,
                        "unsupported PlusCal translation: labelled C grouping block normalization",
                    ));
                }
                result.extend(self.flattened(child)?);
            } else {
                result.push(*id);
            }
        }
        Ok(result)
    }
    fn needs(&mut self, id: StmtId) -> Result<(), TranslationError> {
        if self.parsed.statements[id.0].label.is_some() {
            return Ok(());
        }
        if self.added.len() >= self.options.max_actions.min(10_000) {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                self.parsed.statements[id.0].span,
                "PlusCal inserted-label limit reached",
            ));
        }
        let name = loop {
            let name = format!("{}{}", self.options.label_root, self.next);
            self.next += 1;
            if !self.typed.contains(&name) {
                break name;
            }
        };
        let source = self.parsed.statements[id.0].span;
        // An empty anchor distinguishes generated names from source spelling.
        let anchor = Span {
            start: source.start,
            end: source.start,
        };
        self.parsed.statements[id.0].label = Some(Label {
            name: Name {
                text: name.clone(),
                span: anchor,
            },
            modifier: None,
            span: anchor,
        });
        // InnerGetWith records the last consumed token: C syntax has consumed
        // its opening parenthesis, while P syntax has only consumed `with`.
        let position_start = if self.parsed.algorithm.syntax == crate::ast::Syntax::C
            && matches!(
                self.parsed.statements[id.0].kind,
                StatementKind::With { .. }
            ) {
            super::lower::tokens(self.parsed, crate::ast::Expr { span: source })
                .iter()
                .find(|token| token.text == "(")
                .map_or(source.start, |token| token.span.start)
        } else {
            source.start
        };
        let position = self
            .positions
            .get(&position_start)
            .copied()
            .ok_or_else(|| {
                error(
                    TranslationStage::Normalization,
                    Code::Internal,
                    source,
                    "inserted label has invalid original position",
                )
            })?;
        self.added.push(InsertedLabel {
            name,
            source,
            position,
        });
        Ok(())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one source-kind dispatcher preserves the reference assigned-variable and next-label recurrence"
    )]
    fn sequence(
        &mut self,
        block: &Block,
        first: bool,
        in_with: bool,
        input: &BTreeSet<String>,
        depth: usize,
    ) -> Result<(bool, BTreeSet<String>), TranslationError> {
        if depth > self.options.parser.max_depth.min(256) {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                block.span,
                "PlusCal label-normalization depth limit reached",
            ));
        }
        let mut assigned = input.clone();
        let mut next = first;
        let mut had = false;
        for id in self.flattened(block)? {
            let stmt = self.parsed.statements[id.0].clone();
            if stmt.label.is_some() {
                had = true;
                assigned.clear();
                if in_with {
                    return Err(failure(stmt.span, "label in with statement"));
                }
            }
            if next {
                if in_with {
                    return Err(failure(
                        stmt.span,
                        "statement follows control transfer inside with",
                    ));
                }
                self.needs(id)?;
                next = false;
                had = true;
                assigned.clear();
            }
            match stmt.kind {
                StatementKind::If {
                    branches,
                    otherwise,
                } => {
                    let mut union = BTreeSet::new();
                    for branch in branches {
                        let (child, output) =
                            self.sequence(&branch.body, false, in_with, &assigned, depth + 1)?;
                        next |= child;
                        union.extend(output);
                    }
                    if let Some(body) = otherwise {
                        let (child, output) =
                            self.sequence(&body, false, in_with, &assigned, depth + 1)?;
                        next |= child;
                        union.extend(output);
                    } else {
                        union.extend(assigned.iter().cloned());
                    }
                    assigned = union;
                }
                StatementKind::Either(branches) => {
                    let mut union = BTreeSet::new();
                    for body in branches {
                        let (child, output) =
                            self.sequence(&body, false, in_with, &assigned, depth + 1)?;
                        next |= child;
                        union.extend(output);
                    }
                    assigned = union;
                }
                StatementKind::While { body, .. } => {
                    if in_with {
                        return Err(failure(stmt.span, "while inside a with statement"));
                    }
                    self.needs(id)?;
                    had = true;
                    assigned.clear();
                    self.sequence(&body, false, false, &assigned, depth + 1)?;
                    next = false;
                }
                StatementKind::With { body, .. } => {
                    // The pinned implementation uses this call's input, not
                    // the current assigned set. Retain that observable rule.
                    let inner = if in_with {
                        input.clone()
                    } else {
                        BTreeSet::new()
                    };
                    let (child, output) = self.sequence(&body, false, true, &inner, depth + 1)?;
                    next = child;
                    assigned = output;
                    if !in_with && !input.is_disjoint(&assigned) {
                        self.needs(id)?;
                        had = true;
                        assigned.clear();
                    }
                }
                StatementKind::Assign(items) => {
                    let new: BTreeSet<_> = items.into_iter().map(|a| a.variable.text).collect();
                    if assigned.is_disjoint(&new) {
                        assigned.extend(new);
                    } else {
                        if in_with {
                            return Err(failure(
                                stmt.span,
                                "second assignment to same variable inside a with statement",
                            ));
                        }
                        self.needs(id)?;
                        had = true;
                        assigned = new;
                    }
                    next = false;
                }
                StatementKind::Goto(_) => next = true,
                StatementKind::Call { .. }
                | StatementKind::Return
                | StatementKind::MacroCall { .. } => {
                    return Err(error(
                        TranslationStage::Normalization,
                        Code::Unsupported,
                        stmt.span,
                        "unsupported PlusCal translation: procedure/macro label normalization",
                    ));
                }
                _ => next = false,
            }
        }
        Ok((had || next, assigned))
    }
}
