//! Bounded pinned post-translation AST serialization (ADR 0047).
use super::{TranslationError, TranslationOptions, TranslationStage, error, labels, lower, macros};
use crate::{
    Code,
    ast::{BindingKind, Body, Expr, Fairness, Span, StatementKind, StmtId},
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
            _ => Err(unsupported(
                statement.span,
                "control-flow, calls or nested statement blocks",
            )),
        }
    }
}

/// Serialize the proved sequential straight-line checksum subset.
/// Expressions must occupy one line and contain no junction tokens. Scoped
/// renaming, definitions, macros, processes, procedures and control-flow AST
/// mutation are refused. Native generation/admission remains a separate stage.
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
    if algorithm.variables.iter().any(|v| v.initial.is_none()) {
        return Err(unsupported(algorithm.span, "implicit initialization AST"));
    }
    let mut parsed = macros::expand(parsed, options)?;
    labels::normalize(&mut parsed, options)?;
    // The existing admission pass proves no identifier fixups are required.
    lower::normalize(&parsed, options)?;
    let Body::Sequential(block) = &parsed.algorithm.body else {
        return Err(unsupported(parsed.algorithm.span, "process checksum"));
    };
    let mut groups: Vec<(String, Vec<StmtId>)> = Vec::new();
    for id in &block.statements {
        let statement = parsed
            .statement(*id)
            .ok_or_else(|| unsupported(block.span, "invalid statement reference"))?;
        if let Some(label) = &statement.label {
            if label.modifier.is_some() {
                return Err(unsupported(label.span, "label modifier AST"));
            }
            groups.push((label.name.text.clone(), Vec::new()));
        }
        let Some((_, ids)) = groups.last_mut() else {
            return Err(unsupported(
                statement.span,
                "missing initial normalized label",
            ));
        };
        ids.push(*id);
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
        let Some(initial) = &v.initial else {
            return Err(unsupported(v.span, "implicit initialization"));
        };
        writer.push("[var |-> \"")?;
        writer.push(&v.name.text)?;
        writer.push("\", eqOrIn |-> ")?;
        writer.push(if initial.kind == BindingKind::Equal {
            "\"=\""
        } else {
            "\"\\\\in\""
        })?;
        writer.push(", val |-> ")?;
        writer.expression(&parsed, Some(initial.expression))?;
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
    w.sequence(&groups, |writer, (label, ids)| {
        writer.push("[label |-> \"")?;
        writer.push(label)?;
        writer.push("\",")?;
        writer.newline()?;
        writer.indent(" stmts |-> ")?;
        writer.sequence(ids, |writer, id| writer.statement(&parsed, *id))?;
        writer.push("]")?;
        writer.end();
        Ok(())
    })?;
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
