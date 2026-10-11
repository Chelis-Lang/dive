//! Bounded pinned post-translation AST serialization (ADR 0047).
use super::{
    FairnessOption, TranslationError, TranslationOptions, TranslationStage, error, labels, lower,
    macros,
};
use crate::{
    Code,
    ast::{
        BindingKind, Block, Body, Branch, Decl, Expr, Fairness, Label, Procedure, Process, Span,
        StatementKind, StmtId, WithBinding,
    },
};
use serde::{Deserialize, Serialize};

/// Exact reference AST bytes and the corresponding algorithm checksum.
/// This is serialization evidence, not TLA admission or a model proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlgorithmChecksum {
    /// Pinned `AST.toString()` bytes after admitted normalization/translation.
    pub serialized_ast: String,
    /// Pinned CRC, including the recorded `--fair` precedence limitation.
    pub checksum: String,
    /// The reference hashes only `fair` for a `--fair algorithm` header.
    pub fair_header_crc_only: bool,
    /// Requested and effective settings used for exact AST serialization.
    pub option_context: super::TranslationContext,
}
fn unsupported(span: Span, detail: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported post-translation AST checksum: {detail}"),
    )
}
type Groups = Vec<(String, Vec<StmtId>)>;
fn flatten(
    parsed: &macros::Expanded,
    ids: &[StmtId],
    span: Span,
) -> Result<Vec<StmtId>, TranslationError> {
    let mut pending: Vec<_> = ids.iter().rev().copied().collect();
    let mut flat = Vec::new();
    let mut visited = 0usize;
    while let Some(id) = pending.pop() {
        visited = visited.saturating_add(1);
        if visited > parsed.statements.len() {
            return Err(unsupported(span, "non-tree statement references"));
        }
        let statement = parsed
            .statement(id)
            .ok_or_else(|| unsupported(span, "invalid statement reference"))?;
        if let StatementKind::Block(block) = &statement.kind {
            if statement.label.is_some() {
                return Err(unsupported(statement.span, "labelled C block wrapper"));
            }
            pending.extend(block.statements.iter().rev().copied());
        } else {
            flat.push(id);
        }
    }
    Ok(flat)
}
fn groups(
    parsed: &macros::Expanded,
    block: &Block,
) -> Result<(Vec<StmtId>, Groups), TranslationError> {
    let mut unlabelled = Vec::new();
    let mut groups: Groups = Vec::new();
    for id in flatten(parsed, &block.statements, block.span)? {
        let statement = parsed
            .statement(id)
            .ok_or_else(|| unsupported(block.span, "invalid statement reference"))?;
        if let Some(label) = &statement.label {
            groups.push((label.name.text.clone(), Vec::new()));
        }
        if let Some((_, ids)) = groups.last_mut() {
            ids.push(id);
        } else {
            unlabelled.push(id);
        }
    }
    Ok((unlabelled, groups))
}
fn procedure_checksum_admission(parsed: &macros::Expanded) -> Result<(), TranslationError> {
    let Some(procedure) = parsed.algorithm.procedures.first() else {
        return Ok(());
    };
    let Body::Sequential(main) = &parsed.algorithm.body else {
        return Err(unsupported(
            parsed.algorithm.span,
            "procedure/process checksum ownership",
        ));
    };
    let mut top_level_transfers = std::collections::BTreeSet::new();
    for (block, procedure_body) in [(main, false), (&procedure.body, true)] {
        let (_, labelled) = groups(parsed, block)?;
        for (_, ids) in labelled {
            if procedure_body
                && ids
                    .iter()
                    .filter(|id| {
                        parsed
                            .statement(**id)
                            .is_some_and(|s| matches!(s.kind, StatementKind::Assign(_)))
                    })
                    .count()
                    > 1
            {
                return Err(unsupported(
                    procedure.span,
                    "multiple sequential procedure assignments within one source label",
                ));
            }
            top_level_transfers.extend(ids.iter().map(|id| id.0));
        }
    }
    for (index, statement) in parsed.statements.iter().enumerate() {
        if matches!(
            statement.kind,
            StatementKind::Call { .. } | StatementKind::Return
        ) && !top_level_transfers.contains(&index)
        {
            return Err(unsupported(
                statement.span,
                "nested call/return shared AST mutation",
            ));
        }
    }
    Ok(())
}
type TokenPositions = Vec<(usize, (usize, usize))>;
pub(super) fn token_positions(
    parsed: &macros::Expanded,
) -> Result<TokenPositions, TranslationError> {
    let mut positions = Vec::with_capacity(parsed.tokens.len());
    let mut tokens = parsed.tokens.iter().peekable();
    let mut line = 0;
    let mut column = 0;
    for (offset, character) in parsed.source.char_indices() {
        if tokens
            .peek()
            .is_some_and(|token| token.span.start == offset)
        {
            positions.push((offset, (line, column)));
            tokens.next();
        }
        match character {
            '\n' => {
                line += 1;
                column = 0;
            }
            '\t' => column += 8 - column % 8,
            '\r' if parsed.source.as_bytes().get(offset + 1) == Some(&b'\n') => {}
            '\r' => {
                return Err(unsupported(
                    parsed.algorithm.span,
                    "bare carriage-return line semantics",
                ));
            }
            _ => column += character.len_utf16(),
        }
    }
    if tokens.next().is_some() {
        return Err(unsupported(
            parsed.algorithm.span,
            "unknown expression token location",
        ));
    }
    Ok(positions)
}
struct Writer {
    positions: TokenPositions,
    text: String,
    indents: Vec<usize>,
    limit: usize,
    span: Span,
    procedure_owner: Option<String>,
}
impl Writer {
    fn push(&mut self, text: &str) -> Result<(), TranslationError> {
        if self.text.len().saturating_add(text.len()) > self.limit {
            return Err(error(
                TranslationStage::Generation,
                Code::Resource,
                self.span,
                "PlusCal AST serialization byte limit reached",
            ));
        }
        self.text.push_str(text);
        Ok(())
    }
    fn indent(&mut self, prefix: &str) -> Result<(), TranslationError> {
        let indent = self
            .indents
            .last()
            .copied()
            .unwrap_or_default()
            .saturating_add(prefix.len());
        self.push(prefix)?;
        self.indents.push(indent);
        Ok(())
    }
    fn end(&mut self) {
        self.indents.pop();
    }
    fn newline(&mut self) -> Result<(), TranslationError> {
        self.push("\n")?;
        let indent = self.indents.last().copied().unwrap_or_default();
        for _ in 0..indent {
            self.push(" ")?;
        }
        Ok(())
    }
    fn sequence<T>(
        &mut self,
        items: &[T],
        mut item: impl FnMut(&mut Self, &T) -> Result<(), TranslationError>,
    ) -> Result<(), TranslationError> {
        self.indent("<<")?;
        for (i, value) in items.iter().enumerate() {
            if i > 0 {
                self.push(", ")?;
                self.newline()?;
            }
            item(self, value)?;
        }
        self.push(">>")?;
        self.end();
        Ok(())
    }
    fn declarations(
        &mut self,
        parsed: &macros::Expanded,
        declarations: &[Decl],
    ) -> Result<(), TranslationError> {
        self.sequence(declarations, |writer, v| {
            writer.push("[var |-> \"")?;
            writer.push(&v.name.text)?;
            writer.push("\", eqOrIn |-> ")?;
            writer.push(
                if v.initial
                    .as_ref()
                    .is_none_or(|initial| initial.kind == BindingKind::Equal)
                {
                    "\"=\""
                } else {
                    "\"\\\\in\""
                },
            )?;
            writer.push(", val |-> ")?;
            if let Some(initial) = &v.initial {
                writer.expression(parsed, Some(initial.expression))?;
            } else {
                writer.push("<< \"defaultInitValue\" >>")?;
            }
            writer.push("]")
        })
    }
    fn procedure(
        &mut self,
        parsed: &macros::Expanded,
        procedure: &Procedure,
    ) -> Result<(), TranslationError> {
        self.indent("")?;
        self.push("[name   |-> \"")?;
        self.push(&procedure.name.text)?;
        self.push("\", ")?;
        self.newline()?;
        self.push("minusLabels |-> <<>>, plusLabels |-><<>>, proceduresCalled |->")?;
        // ParseAlgorithm.GetProcedure stores the static vector by reference.
        // In this single nonrecursive procedure scope, subsequent main calls
        // append this procedure's name once, including repeated calls.
        let called = parsed.statements.iter().any(|s| {
            matches!(&s.kind,
            StatementKind::Call { name, .. } if name.text == procedure.name.text)
        });
        self.push("<<")?;
        if called {
            self.push("\"")?;
            self.push(&procedure.name.text)?;
            self.push("\"")?;
        }
        self.push(">>,")?;
        self.newline()?;
        self.indent(" params |-> ")?;
        self.declarations(parsed, &procedure.parameters)?;
        self.push(",")?;
        self.end();
        self.newline()?;
        self.indent(" decls  |-> ")?;
        self.declarations(parsed, &procedure.variables)?;
        self.push(",")?;
        self.end();
        self.newline()?;
        self.indent(" body   |-> ")?;
        let (unlabelled, labelled) = groups(parsed, &procedure.body)?;
        if !unlabelled.is_empty() {
            return Err(unsupported(procedure.span, "unlabelled procedure entry"));
        }
        if labelled.iter().any(|(_, ids)| {
            ids.iter()
                .filter(|id| {
                    parsed
                        .statement(**id)
                        .is_some_and(|s| matches!(s.kind, StatementKind::Assign(_)))
                })
                .count()
                > 1
        }) {
            return Err(unsupported(
                procedure.span,
                "multiple sequential procedure assignments within one source label",
            ));
        }
        self.procedure_owner = Some(procedure.name.text.clone());
        self.labelled(parsed, &labelled)?;
        self.procedure_owner = None;
        self.push("]")?;
        self.end();
        self.end();
        Ok(())
    }
    fn call(
        &mut self,
        parsed: &macros::Expanded,
        name: &str,
        arguments: &[Expr],
        after: Option<&str>,
    ) -> Result<(), TranslationError> {
        self.indent("")?;
        self.push(if after.is_some() {
            "[type     |-> \"callGoto\","
        } else {
            "[type     |-> \"call\","
        })?;
        self.newline()?;
        if let Some(after) = after {
            self.push(" after    |-> \"")?;
            self.push(after.trim_matches('"'))?;
        } else {
            self.push(" returnTo |-> \"")?;
        }
        self.push("\",")?;
        self.newline()?;
        self.push(" to       |-> \"")?;
        self.push(name)?;
        self.push("\",")?;
        self.newline()?;
        self.indent(if after.is_some() {
            "args     |-> "
        } else {
            " args     |-> "
        })?;
        self.sequence(arguments, |w, arg| w.expression(parsed, Some(*arg)))?;
        self.push("]")?;
        if after.is_some() {
            self.newline()?;
        }
        self.end();
        self.end();
        Ok(())
    }
    fn statements(
        &mut self,
        parsed: &macros::Expanded,
        ids: &[StmtId],
    ) -> Result<(), TranslationError> {
        self.indent("<<")?;
        let mut i = 0;
        while let Some(&id) = ids.get(i) {
            if i > 0 {
                self.push(", ")?;
                self.newline()?;
            }
            let statement = parsed
                .statement(id)
                .ok_or_else(|| unsupported(self.span, "invalid statement reference"))?;
            if let StatementKind::Call { name, arguments } = &statement.kind {
                let next = ids.get(i + 1).and_then(|id| parsed.statement(*id));
                if let Some(next) = next.filter(|s| s.label.is_none()) {
                    if let StatementKind::Goto(after) = &next.kind {
                        self.call(parsed, &name.text, arguments, Some(&after.text))?;
                        i += 2;
                        continue;
                    }
                    if matches!(next.kind, StatementKind::Return) {
                        return Err(unsupported(statement.span, "adjacent call-return AST"));
                    }
                }
            }
            self.statement(parsed, id)?;
            i += 1;
        }
        self.push(">>")?;
        self.end();
        Ok(())
    }
    fn quoted_labels(&mut self, labels: &[&Label], modifier: char) -> Result<(), TranslationError> {
        self.indent("<<")?;
        let mut first = true;
        for label in labels
            .iter()
            .filter(|label| label.modifier == Some(modifier))
        {
            if !first {
                self.push(", ")?;
            }
            first = false;
            self.push("\"")?;
            self.push(&label.name.text)?;
            self.push("\"")?;
        }
        self.push(">>")?;
        self.end();
        Ok(())
    }
    fn process(
        &mut self,
        parsed: &macros::Expanded,
        process: &Process,
        labels: &[&Label],
        options: &TranslationOptions,
    ) -> Result<(), TranslationError> {
        self.indent("")?;
        self.push("[name   |-> \"")?;
        self.push(&process.name.text)?;
        self.push("\",")?;
        self.newline()?;
        self.push(" fairness |-> \"")?;
        // ParseAlgorithm 377–394 records declared fairness even under -nof;
        // wf/sf only supply missing declarations. Generator suppression is a
        // separate contract and must not alter the serialized source AST.
        let fairness = match (process.fairness, options.fairness) {
            (Fairness::Unfair, FairnessOption::Weak) => Fairness::Weak,
            (Fairness::Unfair, FairnessOption::Strong) => Fairness::Strong,
            (declared, _) => declared,
        };
        self.push(match fairness {
            Fairness::Unfair => "unfair",
            Fairness::Weak => "wf",
            Fairness::Strong => "sf",
        })?;
        self.push("\", minusLabels |-> ")?;
        self.quoted_labels(labels, '-')?;
        self.push(", plusLabels |->")?;
        self.quoted_labels(labels, '+')?;
        self.push(", proceduresCalled |-><<>>,")?;
        self.newline()?;
        self.push(" eqOrIn |-> ")?;
        self.push(if process.id.kind == BindingKind::Equal {
            "\"=\""
        } else {
            "\"\\\\in\""
        })?;
        self.push(",")?;
        self.newline()?;
        self.push(" id     |-> ")?;
        self.expression(parsed, Some(process.id.expression))?;
        self.push(",")?;
        self.newline()?;
        self.indent(" decls  |-> ")?;
        self.declarations(parsed, &process.variables)?;
        self.push(",")?;
        self.end();
        self.newline()?;
        self.indent(" body   |-> ")?;
        let (unlabelled, groups) = groups(parsed, &process.body)?;
        if !unlabelled.is_empty() {
            return Err(unsupported(
                process.body.span,
                "missing initial process label",
            ));
        }
        self.labelled(parsed, &groups)?;
        self.push("]")?;
        self.end();
        self.end();
        Ok(())
    }
    fn processes(
        &mut self,
        parsed: &macros::Expanded,
        processes: &[Process],
        options: &TranslationOptions,
    ) -> Result<(), TranslationError> {
        // ParseAlgorithm appends modifiers in lexical encounter order. Arena
        // construction can allocate children before parents; preserve source
        // order explicitly and index each owner range without repeated walks.
        let mut labels: Vec<_> = parsed
            .statements
            .iter()
            .filter_map(|statement| statement.label.as_ref())
            .collect();
        labels.sort_by_key(|label| label.span.start);
        self.sequence(processes, |writer, process| {
            let start = labels.partition_point(|label| label.span.start < process.span.start);
            let end = labels.partition_point(|label| label.span.start < process.span.end);
            writer.process(parsed, process, &labels[start..end], options)
        })
    }
    // Keep the exact token/position serialization contract together for audit.
    #[allow(clippy::too_many_lines)]
    fn expression(
        &mut self,
        parsed: &macros::Expanded,
        expr: Option<Expr>,
    ) -> Result<(), TranslationError> {
        self.push("<< ")?;
        if let Some(expr) = expr {
            let diagnostic_span = parsed.origin(expr.span);
            let text = parsed.text(expr.span).unwrap_or_default();
            // The shared SANY lexer recognizes proof-step tokens; pinned
            // PlusCal Tokenize does not. Never invent a reference AST from
            // that broader lexical surface until its split contract is ported.
            if dive_syntax::tokens::tokenize_spec(text)
                .iter()
                .any(|token| {
                    use dive_syntax::SyntaxKind::{
                        STEP_BARE, STEP_DOT, STEP_IMPLICIT, STEP_NUMBER, STEP_UNNUMBERED,
                    };
                    matches!(
                        token.kind,
                        STEP_NUMBER | STEP_IMPLICIT | STEP_DOT | STEP_BARE | STEP_UNNUMBERED
                    )
                })
            {
                return Err(unsupported(
                    diagnostic_span,
                    "proof-step-shaped expression lexer tokens",
                ));
            }
            let tokens = parsed.tokens(expr);
            let substituted = parsed.substituted(expr);
            if substituted
                && tokens
                    .iter()
                    .any(|token| matches!(token.text.as_str(), "/\\" | "\\/"))
            {
                return Err(unsupported(
                    diagnostic_span,
                    "junction columns in substituted macro expressions",
                ));
            }
            let mut located = Vec::with_capacity(tokens.len());
            let mut previous = None;
            let mut minimum = usize::MAX;
            for token in tokens {
                if substituted {
                    // ParseAlgorithm substitutes single-line arguments in the
                    // macro expression's logical line. Provenance spans can
                    // point to different physical definition/call lines.
                    located.push((0, 0));
                    minimum = 0;
                    continue;
                }
                let index = self
                    .positions
                    .binary_search_by_key(&token.span.start, |(offset, _)| *offset)
                    .map_err(|_| {
                        unsupported(diagnostic_span, "unknown expression token location")
                    })?;
                let (line, column) = self.positions[index].1;
                if previous != Some(line) {
                    minimum = minimum.min(column);
                }
                previous = Some(line);
                located.push((line, column));
            }
            let mut previous = None;
            for (i, (token, &(line, column))) in tokens.iter().zip(&located).enumerate() {
                if let Some(before) = previous {
                    for _ in before..line {
                        self.push("\n")?;
                    }
                }
                previous = Some(line);
                if i > 0 {
                    self.push(", ")?;
                }
                if token.text == "/\\" || token.text == "\\/" {
                    let column = column.saturating_sub(minimum);
                    self.push(&format!(
                        "\"(col{column}) {}\"",
                        token.text.replace('\\', "\\\\")
                    ))?;
                    continue;
                }
                if let Some(string) = token
                    .text
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                {
                    self.push("\"\\\"\", \"")?;
                    self.push(string)?;
                    self.push("\", \"\\\"\"")?;
                } else {
                    self.push("\"")?;
                    if token.text.starts_with('\\') {
                        self.push("\\")?;
                    }
                    self.push(&token.text)?;
                    self.push("\"")?;
                }
            }
        }
        self.push(" >>")
    }
    #[allow(
        clippy::too_many_lines,
        reason = "the pinned AST templates are kept explicit for byte-level review"
    )]
    fn statement(&mut self, parsed: &macros::Expanded, id: StmtId) -> Result<(), TranslationError> {
        let statement = parsed
            .statement(id)
            .ok_or_else(|| unsupported(self.span, "invalid statement reference"))?;
        match &statement.kind {
            StatementKind::Call { name, arguments } => {
                self.call(parsed, &name.text, arguments, None)
            }
            StatementKind::Return => {
                let owner = self
                    .procedure_owner
                    .clone()
                    .ok_or_else(|| unsupported(statement.span, "return outside procedure"))?;
                self.push("[type |-> \"return\", from |-> \"")?;
                self.push(&owner)?;
                self.push("\"]")
            }
            StatementKind::Skip => self.push("[type |-> \"skip\"]"),
            StatementKind::Goto(name) => {
                self.push("[type |-> \"goto\",  to |-> \"")?;
                self.push(name.text.trim_matches('"'))?;
                self.push("\"]")
            }
            StatementKind::Await { condition, .. }
            | StatementKind::Assert(condition)
            | StatementKind::Print(condition) => {
                let kind = match statement.kind {
                    StatementKind::Await { .. } => "when",
                    StatementKind::Assert(_) => "assert",
                    _ => "print",
                };
                self.push(&format!("[type |-> \"{kind}\", "))?;
                self.newline()?;
                self.push(" exp |-> ")?;
                self.expression(parsed, Some(*condition))?;
                self.push("]")
            }
            StatementKind::Assign(assignments) => {
                self.push("[type |-> \"assignment\",")?;
                self.newline()?;
                self.indent(" ass  |-> ")?;
                self.sequence(assignments, |writer, a| {
                    writer.push("[lhs |-> [var |-> \"")?;
                    writer.push(&a.variable.text)?;
                    writer.push("\", sub |-> ")?;
                    writer.expression(parsed, a.selectors)?;
                    writer.push("],")?;
                    writer.newline()?;
                    writer.push(" rhs |-> ")?;
                    writer.expression(parsed, Some(a.value))?;
                    writer.push("]")
                })?;
                self.push("]")?;
                self.end();
                Ok(())
            }
            StatementKind::If {
                branches,
                otherwise,
            } => self.if_branches(parsed, branches, otherwise.as_ref()),
            StatementKind::Either(blocks) => {
                self.push("[type |-> \"either\", ")?;
                self.newline()?;
                self.indent(" ors  |-> ")?;
                self.indent("<< ")?;
                for (i, block) in blocks.iter().enumerate() {
                    if i > 0 {
                        self.push(", ")?;
                        self.newline()?;
                    }
                    self.plain_block(parsed, block)?;
                }
                self.push(" >>")?;
                self.end();
                self.push("]")?;
                self.end();
                Ok(())
            }
            StatementKind::With { bindings, body } => self.with_bindings(parsed, bindings, body),
            StatementKind::While { test, body } => {
                let (unlabelled, labelled) = groups(parsed, body)?;
                self.push("[type    |-> \"while\", ")?;
                self.newline()?;
                self.push(" test    |-> ")?;
                self.expression(parsed, Some(*test))?;
                self.push(",")?;
                self.newline()?;
                self.indent(" labDo   |-> ")?;
                self.labelled(parsed, &labelled)?;
                self.push(",")?;
                self.end();
                self.newline()?;
                self.indent(" unlabDo |-> ")?;
                if !parsed.algorithm.procedures.is_empty()
                    && unlabelled.iter().any(|id| {
                        parsed.statement(*id).is_some_and(|s| {
                            matches!(s.kind, StatementKind::Call { .. } | StatementKind::Return)
                        })
                    })
                {
                    return Err(unsupported(
                        body.span,
                        "nested loop call/return AST mutation",
                    ));
                }
                self.statements(parsed, &unlabelled)?;
                self.push("]")?;
                self.end();
                Ok(())
            }
            _ => Err(unsupported(
                statement.span,
                "calls or nested statement blocks",
            )),
        }
    }
    fn if_branches(
        &mut self,
        parsed: &macros::Expanded,
        branches: &[Branch],
        otherwise: Option<&Block>,
    ) -> Result<(), TranslationError> {
        if branches.is_empty() {
            return Err(unsupported(self.span, "empty conditional AST"));
        }
        for (index, branch) in branches.iter().enumerate() {
            self.push("[type    |-> \"if\", ")?;
            self.newline()?;
            self.push(" test    |-> ")?;
            self.expression(parsed, Some(branch.test))?;
            self.push(",")?;
            self.newline()?;
            self.indent(" then |-> ")?;
            self.plain_block(parsed, &branch.body)?;
            self.push(",")?;
            self.end();
            self.newline()?;
            self.indent(" else |-> ")?;
            if index + 1 < branches.len() {
                self.indent("<<")?;
            } else if let Some(block) = otherwise {
                self.plain_block(parsed, block)?;
            } else {
                self.sequence::<StmtId>(&[], |_, _| Ok(()))?;
            }
        }
        for index in (0..branches.len()).rev() {
            self.push("]")?;
            self.end();
            if index > 0 {
                self.push(">>")?;
                self.end();
            }
        }
        Ok(())
    }
    fn with_bindings(
        &mut self,
        parsed: &macros::Expanded,
        bindings: &[WithBinding],
        body: &Block,
    ) -> Result<(), TranslationError> {
        if bindings.is_empty() {
            return Err(unsupported(body.span, "empty With binder AST"));
        }
        for (index, binding) in bindings.iter().enumerate() {
            self.push("[type   |-> \"with\", ")?;
            self.newline()?;
            self.push(" var    |-> \"")?;
            self.push(&binding.name.text)?;
            self.push("\",")?;
            self.newline()?;
            self.push(" eqOrIn |-> ")?;
            self.push(if binding.binding.kind == BindingKind::Equal {
                "\"=\""
            } else {
                "\"\\\\in\""
            })?;
            self.push(",")?;
            self.newline()?;
            self.push(" exp    |-> ")?;
            self.expression(parsed, Some(binding.binding.expression))?;
            self.push(",")?;
            self.newline()?;
            self.indent(" do     |-> ")?;
            if index + 1 < bindings.len() {
                self.indent("<<")?;
            } else {
                self.plain_block(parsed, body)?;
            }
        }
        for index in (0..bindings.len()).rev() {
            self.push("]")?;
            self.end();
            if index > 0 {
                self.push(">>")?;
                self.end();
            }
        }
        Ok(())
    }
    fn plain_block(
        &mut self,
        parsed: &macros::Expanded,
        block: &Block,
    ) -> Result<(), TranslationError> {
        let flat = flatten(parsed, &block.statements, block.span)?;
        if flat
            .iter()
            .any(|id| parsed.statement(*id).is_none_or(|s| s.label.is_some()))
        {
            return Err(unsupported(block.span, "branch-internal labelled AST"));
        }
        if !parsed.algorithm.procedures.is_empty()
            && flat.iter().any(|id| {
                parsed.statement(*id).is_some_and(|s| {
                    matches!(s.kind, StatementKind::Call { .. } | StatementKind::Return)
                })
            })
        {
            // Explode mutates shared unlabelled branch vectors in the original
            // header AST. Top-level calls stay source nodes; nested transfers
            // require separately proved synthetic-expression serialization.
            return Err(unsupported(
                block.span,
                "nested call/return shared AST mutation",
            ));
        }
        self.statements(parsed, &flat)
    }
    fn labelled(
        &mut self,
        parsed: &macros::Expanded,
        groups: &Groups,
    ) -> Result<(), TranslationError> {
        self.sequence(groups, |writer, (label, ids)| {
            writer.push("[label |-> \"")?;
            writer.push(label)?;
            writer.push("\",")?;
            writer.newline()?;
            writer.indent(" stmts |-> ")?;
            writer.statements(parsed, ids)?;
            writer.push("]")?;
            writer.end();
            Ok(())
        })
    }
}

/// Serialize proved bounded sequential and finite-process checksum shapes.
/// Expression line grouping, normalized junction columns, `elsif` and nested
/// `with` binders follow the pinned serializer. Scoped renaming, definitions,
/// nested/recursive/process-owned procedures and branch-internal labels are refused.
/// One explicitly labelled disjoint sequential procedure supports top-level calls,
/// returns and adjacent call-goto, with exact original/header AST ownership.
/// Pure bounded nonrecursive
/// macros use single-line substitutions; multiline arguments and substituted
/// junction columns remain refused. Admitted process
/// fairness and automatic-label options retain the pinned parser precedence. Native generation/admission
/// remains separate. A scoped worker provides a 16 MiB stack.
///
/// # Errors
/// Named source/normalization/unsupported/resource diagnostics; no partial CRC.
#[allow(
    clippy::too_many_lines,
    reason = "ordered bounded AST admission and top-level reference serialization remain together"
)]
pub fn algorithm_checksum_at(
    source: &str,
    region: std::ops::Range<usize>,
    options: &TranslationOptions,
) -> Result<AlgorithmChecksum, TranslationError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("dive-pluscal-checksum".into())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || checksum_inline(source, region, options))
            .map_err(|failure| {
                error(
                    TranslationStage::Generation,
                    Code::Resource,
                    Span::default(),
                    format!("cannot start PlusCal checksum worker: {failure}"),
                )
            })?;
        worker.join().map_err(|_| {
            error(
                TranslationStage::Generation,
                Code::Internal,
                Span::default(),
                "PlusCal checksum worker failed; report this input as a bug",
            )
        })?
    })
}
#[allow(
    clippy::too_many_lines,
    reason = "ordered bounded AST admission and top-level reference serialization remain together"
)]
fn checksum_inline(
    source: &str,
    region: std::ops::Range<usize>,
    options: &TranslationOptions,
) -> Result<AlgorithmChecksum, TranslationError> {
    let parsed = crate::parse_at(source, region, &options.parser).map_err(|diagnostic| {
        TranslationError {
            stage: TranslationStage::Source,
            diagnostic,
        }
    })?;
    let context = super::resolve_translation_options(source, options)?;
    let options = &context.effective;
    let algorithm = &parsed.algorithm;
    if !algorithm.procedures.is_empty() {
        if algorithm.procedures.len() != 1
            || !matches!(algorithm.body, Body::Sequential(_))
            || algorithm.definitions.is_some()
            || !algorithm.macros.is_empty()
            || options.insert_missing_labels
        {
            return Err(unsupported(
                algorithm.span,
                "checksum requires one explicitly labelled sequential procedure without definitions/macros",
            ));
        }
        let procedure = &algorithm.procedures[0];
        if parsed.statements.iter().any(|s| {
            s.span.start >= procedure.body.span.start
                && s.span.end <= procedure.body.span.end
                && matches!(s.kind, StatementKind::Call { .. })
        }) {
            return Err(unsupported(
                procedure.span,
                "procedure call graph/recursion checksum",
            ));
        }
        if parsed
            .statements
            .iter()
            .any(|s| s.label.as_ref().is_some_and(|l| l.modifier.is_some()))
        {
            return Err(unsupported(
                algorithm.span,
                "procedure label fairness checksum",
            ));
        }
    }
    for statement in &parsed.statements {
        if let StatementKind::MacroCall { arguments, .. } = &statement.kind {
            for argument in arguments {
                if parsed
                    .text(argument.span)
                    .is_some_and(|text| text.contains(['\n', '\r']))
                {
                    return Err(unsupported(
                        argument.span,
                        "multiline macro argument layout",
                    ));
                }
            }
        }
    }
    let mut parsed = macros::expand(parsed, options)?;
    labels::normalize(&mut parsed, options)?;
    // Admission precedes serialization: unproved shared-vector transfers never
    // produce CRC bytes, including fair-header cases whose CRC ignores the AST.
    procedure_checksum_admission(&parsed)?;
    // The existing admission pass proves no identifier fixups are required.
    lower::normalize(&parsed, options)?;
    let sequential = matches!(parsed.algorithm.body, Body::Sequential(_));
    let mut w = Writer {
        positions: token_positions(&parsed)?,
        text: String::new(),
        indents: vec![0],
        limit: options.max_output_bytes.min(4_194_304),
        span: parsed.algorithm.span,
        procedure_owner: None,
    };
    w.push(if sequential {
        "[type     |-> \"uniprocess\", "
    } else {
        "[type    |-> \"multiprocess\", "
    })?;
    w.newline()?;
    w.push(" name  |-> \"")?;
    w.push(&parsed.algorithm.name.text)?;
    w.push("\", ")?;
    w.newline()?;
    w.indent(if sequential {
        " decls  |-> "
    } else {
        " decls |-> "
    })?;
    w.declarations(&parsed, &parsed.algorithm.variables)?;
    w.push(",")?;
    w.end();
    w.newline()?;
    w.push(if sequential {
        " defs   |-> "
    } else {
        " defs  |-> "
    })?;
    w.expression(&parsed, parsed.algorithm.definitions)?;
    w.push(",")?;
    w.newline()?;
    w.indent(if sequential {
        " prcds  |-> "
    } else {
        " prcds |-> "
    })?;
    w.sequence(&parsed.algorithm.procedures, |w, procedure| {
        w.procedure(&parsed, procedure)
    })?;
    w.push(",")?;
    w.end();
    w.newline()?;
    match &parsed.algorithm.body {
        Body::Sequential(block) => {
            let (unlabelled, groups) = groups(&parsed, block)?;
            if !unlabelled.is_empty() {
                return Err(unsupported(block.span, "missing initial normalized label"));
            }
            w.indent(" body  |-> ")?;
            w.labelled(&parsed, &groups)?;
        }
        Body::Processes(processes) => {
            w.indent(" procs |-> ")?;
            w.processes(&parsed, processes, options)?;
        }
    }
    w.push("]")?;
    w.end();
    let fair = parsed.algorithm.fairness == Fairness::Weak;
    // AST CRC includes newline bytes, unlike the generated-body line CRC.
    let checksum = crate::rewrite::crc32(if fair {
        "fair".as_bytes()
    } else {
        w.text.as_bytes()
    });
    Ok(AlgorithmChecksum {
        option_context: context,
        serialized_ast: w.text,
        checksum,
        fair_header_crc_only: fair,
    })
}
