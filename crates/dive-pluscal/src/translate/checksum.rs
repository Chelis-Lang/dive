//! Bounded pinned original and post-translation AST serialization (ADR 0047).
use super::{
    FairnessOption, TranslationError, TranslationOptions, TranslationStage, error, labels, lower,
    macros,
};
use crate::{
    Code,
    ast::{
        BindingKind, Block, Body, Branch, Decl, Expr, Fairness, Label, Process, Span,
        StatementKind, StmtId, WithBinding,
    },
};
use serde::{Deserialize, Serialize};

/// Exact reference AST bytes and the corresponding algorithm checksum.
/// This is serialization evidence, not TLA admission or a model proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlgorithmChecksum {
    /// Pinned `AST.toString()` bytes of the original stored AST after generation.
    /// The separately exploded `xast` is not this algorithm-header CRC authority.
    pub serialized_ast: String,
    /// Pinned CRC, including the recorded `--fair` precedence limitation.
    pub checksum: String,
}
pub(super) fn unsupported(span: Span, detail: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported post-translation AST checksum: {detail}"),
    )
}
type Groups = Vec<(String, Vec<StmtId>)>;
pub(super) fn flatten(
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
pub(super) fn groups(
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
type TokenPositions = Vec<(usize, (usize, usize))>;
pub(super) fn token_positions(
    parsed: &macros::Expanded,
) -> Result<TokenPositions, TranslationError> {
    token_positions_parts(&parsed.source, &parsed.tokens, parsed.algorithm.span)
}
fn token_positions_parts(
    source: &str,
    tokens: &[crate::ast::Token],
    span: Span,
) -> Result<TokenPositions, TranslationError> {
    let mut positions = Vec::with_capacity(tokens.len());
    let mut tokens = tokens.iter().peekable();
    let mut line = 0;
    let mut column = 0;
    for (offset, character) in source.char_indices() {
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
            '\r' if source.as_bytes().get(offset + 1) == Some(&b'\n') => {}
            '\r' => {
                return Err(unsupported(span, "bare carriage-return line semantics"));
            }
            _ => column += character.len_utf16(),
        }
    }
    if tokens.next().is_some() {
        return Err(unsupported(span, "unknown expression token location"));
    }
    Ok(positions)
}
struct Writer {
    positions: TokenPositions,
    debug: bool,
    text: String,
    indents: Vec<usize>,
    limit: usize,
    span: Span,
}
impl Writer {
    fn location(&self, at: Option<usize>) -> Result<String, TranslationError> {
        if !self.debug {
            return Ok(String::new());
        }
        let (line, column) = if let Some(at) = at {
            let index = self
                .positions
                .binary_search_by_key(&at, |(offset, _)| *offset)
                .map_err(|_| unsupported(self.span, "debug source location"))?;
            let (line, column) = self.positions[index].1;
            (line + 1, column + 1)
        } else {
            (0, 0)
        };
        Ok(format!("(*{line}, {column}*)"))
    }
    fn located_indent(&mut self, at: Option<usize>) -> Result<(), TranslationError> {
        let prefix = self.location(at)?;
        self.indent(&prefix)
    }
    fn statement_at(parsed: &macros::Expanded, id: StmtId) -> Result<usize, TranslationError> {
        let statement = parsed
            .statement(id)
            .ok_or_else(|| unsupported(parsed.algorithm.span, "invalid debug statement"))?;
        let begin = statement
            .label
            .as_ref()
            .map_or(statement.span.start, |label| label.span.end);
        let index = parsed
            .tokens
            .partition_point(|token| token.span.start < begin);
        parsed
            .tokens
            .get(index)
            .filter(|token| token.span.start < statement.span.end)
            .map(|token| token.span.start)
            .ok_or_else(|| unsupported(statement.span, "debug statement token"))
    }
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
            writer.located_indent(Some(v.name.span.start))?;
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
            writer.push("]")?;
            writer.end();
            Ok(())
        })
    }
    fn statements(
        &mut self,
        parsed: &macros::Expanded,
        ids: &[StmtId],
    ) -> Result<(), TranslationError> {
        self.sequence(ids, |writer, id| writer.statement(parsed, *id))
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
        let process_at = parsed
            .tokens
            .iter()
            .find(|token| {
                token.span.start >= process.span.start
                    && token.span.end <= process.name.span.start
                    && token.text == "process"
            })
            .map_or(process.span.start, |token| token.span.start);
        self.located_indent(Some(process_at))?;
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
        self.push(", proceduresCalled |-><<")?;
        self.push(">>,")?;
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
        let at = if self.debug {
            Self::statement_at(parsed, id)?
        } else {
            0
        };
        let indented = matches!(
            statement.kind,
            StatementKind::Assign(_)
                | StatementKind::Await { .. }
                | StatementKind::Assert(_)
                | StatementKind::Print(_)
        );
        if indented {
            self.located_indent(Some(at))?;
        } else if self.debug {
            self.push(&self.location(Some(at))?)?;
        }
        let result = match &statement.kind {
            StatementKind::Call { .. } | StatementKind::Return => Err(unsupported(
                statement.span,
                "procedure AST checksum is unavailable",
            )),
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
                    writer.located_indent(Some(a.variable.span.start))?;
                    writer.push("[lhs |-> ")?;
                    writer.push(&writer.location(Some(a.variable.span.start))?)?;
                    writer.push("[var |-> \"")?;
                    writer.push(&a.variable.text)?;
                    writer.push("\", sub |-> ")?;
                    writer.expression(parsed, a.selectors)?;
                    writer.push("],")?;
                    writer.newline()?;
                    writer.push(" rhs |-> ")?;
                    writer.expression(parsed, Some(a.value))?;
                    writer.push("]")?;
                    writer.end();
                    Ok(())
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
                self.statements(parsed, &unlabelled)?;
                self.push("]")?;
                self.end();
                Ok(())
            }
            _ => Err(unsupported(
                statement.span,
                "calls or nested statement blocks",
            )),
        };
        if indented {
            self.end();
        }
        result
    }
    fn if_branches(
        &mut self,
        parsed: &macros::Expanded,
        branches: &[Branch],
        otherwise: Option<&Block>,
    ) -> Result<(), TranslationError> {
        self.if_branches_from(parsed, branches, otherwise, 0)
    }
    fn if_branches_from(
        &mut self,
        parsed: &macros::Expanded,
        branches: &[Branch],
        otherwise: Option<&Block>,
        start: usize,
    ) -> Result<(), TranslationError> {
        if start >= branches.len() {
            return Err(unsupported(self.span, "empty conditional AST"));
        }
        let mut opened = 0usize;
        for (index, branch) in branches.iter().enumerate().skip(start) {
            opened += 1;
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
        for index in (0..opened).rev() {
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
        self.statements(parsed, &flat)
    }
    fn labelled(
        &mut self,
        parsed: &macros::Expanded,
        groups: &Groups,
    ) -> Result<(), TranslationError> {
        self.sequence(groups, |writer, (label, ids)| {
            let id = ids
                .first()
                .copied()
                .ok_or_else(|| unsupported(writer.span, "empty labelled statement"))?;
            let at = if writer.debug {
                Some(Self::statement_at(parsed, id)?)
            } else {
                None
            };
            writer.located_indent(at)?;
            writer.push("[label |-> \"")?;
            writer.push(label)?;
            writer.push("\",")?;
            writer.newline()?;
            writer.indent(" stmts |-> ")?;
            writer.statements(parsed, ids)?;
            writer.push("]")?;
            writer.end();
            writer.end();
            Ok(())
        })
    }
}

/// Serialize exact procedure-free algorithm checksum shapes.
/// Expression layout, macros, labels and supported debug locations follow the
/// pinned serializer. Unsupported serialization returns a named diagnostic;
/// translation and rewriting use an optional checksum and proceed without it.
/// Procedure header transfer emulation is intentionally unavailable.
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
            .spawn_scoped(scope, || {
                serialize_inline(source, region, options, false).map(|(header, _, _)| header)
            })
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
fn serialize_inline(
    source: &str,
    region: std::ops::Range<usize>,
    options: &TranslationOptions,
    original_stage: bool,
) -> Result<(AlgorithmChecksum, String, super::TranslationContext), TranslationError> {
    let context = super::resolve_translation_options(source, options)?;
    let options = &context.effective;
    let mut parser = options.parser.clone();
    parser.input_version = super::options::version_weight(options.version.as_deref())?;
    let parsed =
        crate::parse_at(source, region, &parser).map_err(|diagnostic| TranslationError {
            stage: TranslationStage::Source,
            diagnostic,
        })?;
    super::options::admit_version_shape(&parsed, parser.input_version)?;
    let algorithm = &parsed.algorithm;
    if original_stage {
        return serialize_original(
            &parsed,
            options,
            context.clone(),
            parser.input_version,
            false,
        );
    }
    if !algorithm.procedures.is_empty() {
        return Err(unsupported(
            algorithm.span,
            "procedure AST checksum is unavailable",
        ));
    }
    if options.debug {
        return serialize_original(
            &parsed,
            options,
            context.clone(),
            parser.input_version,
            true,
        );
    }
    for statement in &parsed.statements {
        if !options.debug
            && let StatementKind::MacroCall { arguments, .. } = &statement.kind
        {
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
    // The exact CRC operation retains its serializer boundary. Generation and
    // rewriting do not depend on this narrower operation succeeding.
    lower::normalize(&parsed, options)?;
    let sequential = matches!(parsed.algorithm.body, Body::Sequential(_));
    let mut w = Writer {
        positions: token_positions(&parsed)?,
        debug: options.debug,
        text: String::new(),
        indents: vec![0],
        limit: options.max_output_bytes.min(4_194_304),
        span: parsed.algorithm.span,
    };
    w.located_indent(None)?;
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
    w.sequence::<StmtId>(&[], |_, _| Ok(()))?;
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
    w.end();
    let fair = parsed.algorithm.fairness == Fairness::Weak;
    // AST CRC includes newline bytes, unlike the generated-body line CRC.
    let checksum = crate::rewrite::crc32(if fair {
        "fair".as_bytes()
    } else {
        w.text.as_bytes()
    });
    let original_fairness = if sequential && (fair || parsed.algorithm.legacy_fair) {
        "wf"
    } else {
        super::fairness_name(options.fairness)
    }
    .to_owned();
    Ok((
        AlgorithmChecksum {
            serialized_ast: w.text,
            checksum,
        },
        original_fairness,
        context,
    ))
}

fn serialize_original(
    parsed: &crate::Parsed,
    options: &TranslationOptions,
    context: super::TranslationContext,
    input_version: i32,
    header: bool,
) -> Result<(AlgorithmChecksum, String, super::TranslationContext), TranslationError> {
    if input_version < 1500
        && (!parsed.algorithm.procedures.is_empty() || !parsed.algorithm.macros.is_empty())
    {
        return Err(unsupported(
            parsed.algorithm.span,
            "historical original procedure/macro AST fields",
        ));
    }
    labels::validate_root(options, parsed.algorithm.span)?;
    token_positions_parts(&parsed.source, &parsed.tokens, parsed.algorithm.span)?;
    let mut settings = options.clone();
    settings.parser.input_version = input_version;
    let serialized_ast = if header {
        super::pcal::serialize_debug_header(parsed, &settings)
    } else {
        super::pcal::serialize_original(parsed, &settings)
    }
    .map_err(|failure| {
        let span = failure.at.map_or(parsed.algorithm.span, |at| {
            parsed
                .tokens
                .iter()
                .find(|token| token.span.start >= at)
                .map_or(Span { start: at, end: at }, |token| token.span)
        });
        error(
            if failure.in_generation {
                TranslationStage::Generation
            } else {
                TranslationStage::Normalization
            },
            failure.code,
            span,
            failure.message,
        )
    })?;
    let fair = parsed.algorithm.fairness == Fairness::Weak;
    let checksum = crate::rewrite::crc32(if fair {
        b"fair"
    } else {
        serialized_ast.as_bytes()
    });
    let fairness = if matches!(parsed.algorithm.body, Body::Sequential(_))
        && (fair || parsed.algorithm.legacy_fair)
    {
        "wf"
    } else {
        super::fairness_name(options.fairness)
    }
    .to_owned();
    Ok((
        AlgorithmChecksum {
            serialized_ast,
            checksum,
        },
        fairness,
        context,
    ))
}

/// The pinned original-stage `AST.tla` contents returned in memory.
/// This artifact precedes identifier fixing and generation; it does not certify
/// an enclosing module, a model, a header CRC, or a filesystem write.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OriginalAstArtifact {
    /// Exact `AST.toString()` bytes before translation mutates shared nodes.
    pub serialized_ast: String,
    /// The reference `AST.tla` wrapper, with LF separators.
    pub module_source: String,
    /// Requested/effective option context.
    pub option_context: super::TranslationContext,
}

/// Returns a bounded normalized original AST using the pinned semantic serializer.
/// Modern procedure/macro ownership and compound locations follow the typed
/// source stages. Historical fields and broader original-stage coverage remain
/// unfinished; availability is not generated-module or model acceptance.
/// No Java or filesystem write occurs, and no translation checksum is returned.
///
/// ```
/// let source = "--algorithm A begin L:skip; end algorithm";
/// let artifact = dive_pluscal::original_ast_at(source, 0..source.len(),
///     &dive_pluscal::TranslationOptions::default())?;
/// assert!(artifact.module_source.starts_with("------ MODULE AST -------\n"));
/// # Ok::<(), dive_pluscal::TranslationError>(())
/// ```
///
/// # Errors
/// Named source, normalization, unsupported or resource diagnostics.
pub fn original_ast_at(
    source: &str,
    region: std::ops::Range<usize>,
    options: &TranslationOptions,
) -> Result<OriginalAstArtifact, TranslationError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("dive-pluscal-original-ast".into())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let (original, fairness, context) = serialize_inline(source, region, options, true).map_err(|mut failure| {
                    if let Some(detail) = failure.diagnostic.message.strip_prefix("unsupported post-translation AST checksum: ") {
                        failure.diagnostic.message = format!("unsupported original-stage AST serialization: {detail}");
                    }
                    failure
                })?;
                let prefix = format!("------ MODULE AST -------\nEXTENDS TLC\nfairness == \"{fairness}\"\n \nast == \n");
                let suffix = "\n==========================\n";
                if prefix.len().saturating_add(original.serialized_ast.len()).saturating_add(suffix.len())
                    > options.max_output_bytes.min(4_194_304) {
                    return Err(error(TranslationStage::Generation, Code::Resource, Span::default(),
                        "PlusCal AST artifact byte limit reached"));
                }
                Ok(OriginalAstArtifact {
                    module_source: format!("{prefix}{}{suffix}", original.serialized_ast),
                    serialized_ast: original.serialized_ast,
                    option_context: context,
                })
            }).map_err(|failure| error(TranslationStage::Normalization, Code::Resource, Span::default(),
                format!("cannot start PlusCal original AST worker: {failure}")))?;
        worker.join().map_err(|_| {
            error(
                TranslationStage::Generation,
                Code::Internal,
                Span::default(),
                "PlusCal original AST worker failed; report this input as a bug",
            )
        })?
    })
}
