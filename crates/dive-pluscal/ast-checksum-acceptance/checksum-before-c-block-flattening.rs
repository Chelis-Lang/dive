//! Bounded pinned post-translation AST serialization (ADR 0047).
use super::{TranslationError, TranslationOptions, TranslationStage, error, labels, lower, macros};
use crate::{
    Code,
    ast::{BindingKind, Block, Body, Expr, Fairness, Span, StatementKind, StmtId},
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
fn groups(
    parsed: &macros::Expanded,
    block: &Block,
) -> Result<(Vec<StmtId>, Groups), TranslationError> {
    let mut unlabelled = Vec::new();
    let mut groups: Groups = Vec::new();
    for id in &block.statements {
        let statement = parsed
            .statement(*id)
            .ok_or_else(|| unsupported(block.span, "invalid statement reference"))?;
        if let Some(label) = &statement.label {
            groups.push((label.name.text.clone(), Vec::new()));
        }
        if let Some((_, ids)) = groups.last_mut() {
            ids.push(*id);
        } else {
            unlabelled.push(*id);
        }
    }
    Ok((unlabelled, groups))
}
struct Writer {
    text: String,
    indents: Vec<usize>,
    limit: usize,
    span: Span,
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
    fn expression(
        &mut self,
        parsed: &macros::Expanded,
        expr: Option<Expr>,
    ) -> Result<(), TranslationError> {
        self.push("<< ")?;
        if let Some(expr) = expr {
            let text = parsed.text(expr.span).unwrap_or_default();
            if text.contains(['\r', '\n']) {
                return Err(unsupported(expr.span, "multiline expression tokens"));
            }
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
                    expr.span,
                    "proof-step-shaped expression lexer tokens",
                ));
            }
            for (i, token) in parsed.tokens(expr).iter().enumerate() {
                if token.text == "/\\" || token.text == "\\/" {
                    return Err(unsupported(expr.span, "column-sensitive junction tokens"));
                }
                if i > 0 {
                    self.push(", ")?;
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
            } => {
                let [branch] = branches.as_slice() else {
                    return Err(unsupported(statement.span, "elsif AST nesting"));
                };
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
                if let Some(block) = otherwise {
                    self.plain_block(parsed, block)?;
                } else {
                    self.sequence::<StmtId>(&[], |_, _| Ok(()))?;
                }
                self.push("]")?;
                self.end();
                Ok(())
            }
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
            StatementKind::With { bindings, body } => {
                let [binding] = bindings.as_slice() else {
                    return Err(unsupported(
                        statement.span,
                        "multiple With binder AST nesting",
                    ));
                };
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
                self.plain_block(parsed, body)?;
                self.push("]")?;
                self.end();
                Ok(())
            }
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
                self.sequence(&unlabelled, |w, id| w.statement(parsed, *id))?;
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
    fn plain_block(
        &mut self,
        parsed: &macros::Expanded,
        block: &Block,
    ) -> Result<(), TranslationError> {
        if block
            .statements
            .iter()
            .any(|id| parsed.statement(*id).is_none_or(|s| s.label.is_some()))
        {
            return Err(unsupported(block.span, "branch-internal labelled AST"));
        }
        self.sequence(&block.statements, |w, id| w.statement(parsed, *id))
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
            writer.sequence(ids, |w, id| w.statement(parsed, *id))?;
            writer.push("]")?;
            writer.end();
            Ok(())
        })
    }
}

/// Serialize the proved bounded sequential checksum subset.
/// Expressions must occupy one line and contain no junction tokens. Scoped
/// renaming, definitions, macros, processes, procedures, multiple `with` binders,
/// `elsif` and branch-internal labels are refused. Native generation/admission
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
    let algorithm = &parsed.algorithm;
    if !algorithm.macros.is_empty()
        || !algorithm.procedures.is_empty()
        || algorithm.definitions.is_some()
        || !matches!(algorithm.body, Body::Sequential(_))
    {
        return Err(unsupported(
            algorithm.span,
            "macros, procedures, processes or definitions",
        ));
    }
    let mut parsed = macros::expand(parsed, options)?;
    labels::normalize(&mut parsed, options)?;
    // The existing admission pass proves no identifier fixups are required.
    lower::normalize(&parsed, options)?;
    let Body::Sequential(block) = &parsed.algorithm.body else {
        return Err(unsupported(parsed.algorithm.span, "process checksum"));
    };
    let (unlabelled, groups) = groups(&parsed, block)?;
    if !unlabelled.is_empty() {
        return Err(unsupported(block.span, "missing initial normalized label"));
    }
    let mut w = Writer {
        text: String::new(),
        indents: vec![0],
        limit: options.max_output_bytes.min(4_194_304),
        span: parsed.algorithm.span,
    };
    w.push("[type     |-> \"uniprocess\", ")?;
    w.newline()?;
    w.push(" name  |-> \"")?;
    w.push(&parsed.algorithm.name.text)?;
    w.push("\", ")?;
    w.newline()?;
    w.indent(" decls  |-> ")?;
    w.sequence(&parsed.algorithm.variables, |writer, v| {
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
            writer.expression(&parsed, Some(initial.expression))?;
        } else {
            writer.push("<< \"defaultInitValue\" >>")?;
        }
        writer.push("]")
    })?;
    w.push(",")?;
    w.end();
    w.newline()?;
    w.push(" defs   |-> <<  >>,")?;
    w.newline()?;
    w.push(" prcds  |-> <<>>,")?;
    w.newline()?;
    w.indent(" body  |-> ")?;
    w.labelled(&parsed, &groups)?;
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
        serialized_ast: w.text,
        checksum,
        fair_header_crc_only: fair,
    })
}
