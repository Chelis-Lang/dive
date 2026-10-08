//! Syntax diagnostics and their stable codes.
//!
//! Codes have the form `DIVE-Snnnn` and never change meaning once
//! published. Add new codes at the end of the table; never reuse one.

use rowan::{TextRange, TextSize};

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The module is rejected, as SANY's parser would reject it.
    Error,
    /// Accepted, but worth pointing out.
    Warning,
}

/// A stable diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Code {
    /// `DIVE-S0001`: a character that cannot start any token.
    UnexpectedCharacter,
    /// `DIVE-S0002`: a string literal with a bad escape or no closing quote.
    BadString,
    /// `DIVE-S0003`: a block comment still open at end of input.
    UnterminatedComment,
    /// `DIVE-S0004`: no `---- MODULE` header in the input.
    NoModule,
    /// `DIVE-S0005`: end of input before the module's closing `====`.
    MissingModuleEnd,
    /// `DIVE-S0010`: a token the grammar does not allow here.
    UnexpectedToken,
    /// `DIVE-S0011`: an expression was expected but is missing.
    MissingExpression,
    /// `DIVE-S0012`: operators of overlapping precedence without parentheses.
    PrecedenceConflict,
    /// `DIVE-S0013`: two expressions with no operator between them.
    MissingOperator,
    /// `DIVE-S0014`: a prefix, infix or postfix operator in a position it cannot take.
    MisplacedOperator,
    /// `DIVE-S0015`: a token inside a `/\` or `\/` list item is not right of its bullet.
    JunctionIndentation,
    /// `DIVE-S0020`: a proof step number has the wrong level.
    StepLevel,
    /// `DIVE-S0021`: a step number used outside a proof, or `<+>` in an expression.
    StepNumberMisuse,
    /// `DIVE-S0022`: a proof attached to a step that cannot take one.
    UnexpectedProof,
    /// `DIVE-S0023`: proofs nested more than 100 levels (SANY's limit).
    ProofTooDeep,
    /// `DIVE-S0030`: `::` that does not follow a label.
    BadLabel,
    /// `DIVE-S0031`: a label whose removal would change how the expression parses.
    LabelChangesParse,
    /// `DIVE-S0032`: `\X` (or `\times`) used as an operator argument.
    TimesAsArgument,
    /// `DIVE-S0033`: the form `{a \in b : c \in d}`.
    SetMapWithIn,
    /// `DIVE-S0034`: `@` used as a field name in `!.@`.
    AtInExcept,
    /// `DIVE-S0035`: an ill-structured `WF_`/`SF_` expression.
    BadFairness,
    /// `DIVE-S0036`: `NEW` declares a symbol with arguments before `\in`.
    NewWithArguments,
    /// `DIVE-S0037`: a decimal number used as a structural selector (`Op!1.2`).
    BadStructuralSelector,
    /// `DIVE-S0040`: nesting deeper than this parser supports.
    NestingTooDeep,
    /// `DIVE-S0041`: input larger than 4 GiB.
    InputTooLarge,
    /// `DIVE-S0006`: the top-level module's name differs from the file name.
    ModuleNameMismatch,
}

impl Code {
    /// The stable string form, e.g. `DIVE-S0012`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedCharacter => "DIVE-S0001",
            Self::BadString => "DIVE-S0002",
            Self::UnterminatedComment => "DIVE-S0003",
            Self::NoModule => "DIVE-S0004",
            Self::MissingModuleEnd => "DIVE-S0005",
            Self::UnexpectedToken => "DIVE-S0010",
            Self::MissingExpression => "DIVE-S0011",
            Self::PrecedenceConflict => "DIVE-S0012",
            Self::MissingOperator => "DIVE-S0013",
            Self::MisplacedOperator => "DIVE-S0014",
            Self::JunctionIndentation => "DIVE-S0015",
            Self::StepLevel => "DIVE-S0020",
            Self::StepNumberMisuse => "DIVE-S0021",
            Self::UnexpectedProof => "DIVE-S0022",
            Self::ProofTooDeep => "DIVE-S0023",
            Self::BadLabel => "DIVE-S0030",
            Self::LabelChangesParse => "DIVE-S0031",
            Self::TimesAsArgument => "DIVE-S0032",
            Self::SetMapWithIn => "DIVE-S0033",
            Self::AtInExcept => "DIVE-S0034",
            Self::BadFairness => "DIVE-S0035",
            Self::NewWithArguments => "DIVE-S0036",
            Self::BadStructuralSelector => "DIVE-S0037",
            Self::NestingTooDeep => "DIVE-S0040",
            Self::InputTooLarge => "DIVE-S0041",
            Self::ModuleNameMismatch => "DIVE-S0006",
        }
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One diagnostic about the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable code.
    pub code: Code,
    /// Severity.
    pub severity: Severity,
    /// What is wrong, naming the construct.
    pub message: String,
    /// The primary source span, in bytes.
    pub range: TextRange,
    /// A likely fix, when one can be suggested.
    pub help: Option<String>,
}

impl Diagnostic {
    pub(crate) fn error(code: Code, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            range,
            help: None,
        }
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Whether this diagnostic rejects the module.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// 1-based line and column (in Unicode scalar values) of a byte offset.
#[must_use]
pub fn line_col(src: &str, offset: TextSize) -> (u32, u32) {
    let offset = usize::from(offset).min(src.len());
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, c) in src.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique() {
        let all = [
            Code::UnexpectedCharacter,
            Code::BadString,
            Code::UnterminatedComment,
            Code::NoModule,
            Code::MissingModuleEnd,
            Code::UnexpectedToken,
            Code::MissingExpression,
            Code::PrecedenceConflict,
            Code::MissingOperator,
            Code::MisplacedOperator,
            Code::JunctionIndentation,
            Code::StepLevel,
            Code::StepNumberMisuse,
            Code::UnexpectedProof,
            Code::ProofTooDeep,
            Code::BadLabel,
            Code::LabelChangesParse,
            Code::TimesAsArgument,
            Code::SetMapWithIn,
            Code::AtInExcept,
            Code::BadFairness,
            Code::NewWithArguments,
            Code::BadStructuralSelector,
            Code::NestingTooDeep,
            Code::InputTooLarge,
            Code::ModuleNameMismatch,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for c in all {
            assert!(seen.insert(c.as_str()), "{c}");
            assert!(c.as_str().starts_with("DIVE-S"));
        }
    }

    #[test]
    fn line_col_counts_chars() {
        assert_eq!(line_col("ab\ncd", TextSize::from(4)), (2, 2));
        assert_eq!(line_col("∧x", TextSize::from(3)), (1, 2));
    }
}
