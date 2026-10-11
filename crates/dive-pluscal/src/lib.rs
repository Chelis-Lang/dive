//! Native source-mapped PlusCal grammar parsing (ADR 0022).
//!
//! [`parse`] produces a source AST, not a translated TLA+ specification.
//! [`translate`] supports bounded sequential and disjoint finite-process paths, including reference labels;
//! disjoint sequential procedures (including recursion), disjoint finite process-owned
//! shared procedures and bounded macros are admitted. Unproved process/procedure scopes,
//! scoped names, sequential procedure modifiers and branch-internal labels
//! remain separate pending stages. Expression slices retain counted nesting and are
//! neither syntax-checked as TLA+ expressions nor evaluated.
//! No Java runtime is used by this crate.
//!
//! ```
//! let source = "--algorithm A begin L: skip; end algorithm";
//! let parsed = dive_pluscal::parse(source, &dive_pluscal::Options::default())?;
//! assert_eq!(parsed.algorithm.name.text, "A");
//! assert_eq!(parsed.text(parsed.algorithm.span), Some(source));
//! # Ok::<(), dive_pluscal::Diagnostic>(())
//! ```
#![forbid(unsafe_code)]

pub mod ast;
mod lexer;
mod parser;
/// Bounded source editing and translation-body checksum interoperability.
pub mod rewrite;
#[path = "translate/mod.rs"]
mod translation;
pub use parser::{Options, parse, parse_at};
use serde::{Deserialize, Serialize};
pub use translation::{
    AlgorithmChecksum, FairnessOption, InsertedLabel, ProcessTranslation, Translation,
    TranslationError, TranslationMap, TranslationOptions, TranslationStage, TranslationWarning,
    algorithm_checksum_at, translate, translate_at,
};

/// Stable native PlusCal diagnostic categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Code {
    /// Invalid lexical token, string or comment.
    Lexical,
    /// Missing or unexpected grammar token.
    Syntax,
    /// Missing expression or unmatched delimiter.
    Expression,
    /// Illegal source label.
    Label,
    /// Parser resource bound reached.
    Resource,
    /// A parser worker failed unexpectedly; report as an implementation bug.
    Internal,
    /// Explicitly unimplemented mode.
    Unsupported,
}
impl Code {
    /// Stable machine-readable code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "DIVE-P0001",
            Self::Syntax => "DIVE-P0002",
            Self::Expression => "DIVE-P0003",
            Self::Label => "DIVE-P0004",
            Self::Resource => "DIVE-P0040",
            Self::Internal => "DIVE-P0050",
            Self::Unsupported => "DIVE-P0090",
        }
    }
}
/// Deterministic diagnostic over original source bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable category.
    pub code: Code,
    /// Original bytes, never tab-expanded.
    pub span: ast::Span,
    /// Human-readable detail.
    pub message: String,
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}..{}: {}",
            self.code.as_str(),
            self.span.start,
            self.span.end,
            self.message
        )
    }
}
impl std::error::Error for Diagnostic {}

/// Complete source grammar result; translation acceptance is not implied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    /// Complete original source retained verbatim.
    pub source: String,
    /// Algorithm structure and ranges.
    pub algorithm: ast::Algorithm,
    /// Statement arena, indexed by child blocks.
    pub statements: Vec<ast::Statement>,
    /// Significant tokens; trivia remains in `source`.
    pub tokens: Vec<ast::Token>,
}
impl Parsed {
    /// Original text, or `None` for an invalid caller range.
    #[must_use]
    pub fn text(&self, span: ast::Span) -> Option<&str> {
        self.source.get(span.start..span.end)
    }
    /// Gets a statement without trusting a caller-provided id.
    #[must_use]
    pub fn statement(&self, id: ast::StmtId) -> Option<&ast::Statement> {
        self.statements.get(id.0)
    }
    /// Converts a valid original byte boundary to a display position.
    /// Tabs advance to the next eight-column stop. Expression spans remain
    /// original bytes regardless of this optional display conversion.
    #[must_use]
    pub fn position(&self, offset: usize) -> Option<ast::Position> {
        let prefix = self.source.get(..offset)?;
        let mut line = 1;
        let mut column = 1;
        for (index, character) in prefix.char_indices() {
            match character {
                '\n' => {
                    line += 1;
                    column = 1;
                }
                '\t' => column += 8 - (column - 1) % 8,
                '\r' if self.source.as_bytes().get(index + 1) == Some(&b'\n') => {}
                _ => column += 1,
            }
        }
        Some(ast::Position { line, column })
    }
}
