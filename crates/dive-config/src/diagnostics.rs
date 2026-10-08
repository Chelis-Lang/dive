//! Configuration diagnostics and their stable codes.
//!
//! Codes have the form `DIVE-Cnnnn` and never change meaning once published.
//! Add new codes at the end of their block; never reuse one.
//!
//! Every error also records what the pinned TLC reports for the same input
//! ([`TlcError`]): its `EC` code, exit status and, where TLC prints one, the
//! line number. Warnings never change acceptance; TLC has no equivalent.

use dive_syntax::TextRange;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The configuration is rejected, as TLC rejects it.
    Error,
    /// Accepted by TLC, but almost certainly not what was meant.
    Warning,
}

/// A stable diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Code {
    /// `DIVE-C0001`: a character that cannot start any token (TLC 5007).
    UnexpectedCharacter,
    /// `DIVE-C0002`: a string with a bad escape or no closing quote (TLC 5007).
    BadString,
    /// `DIVE-C0003`: a block comment still open at end of file (TLC 5007).
    UnterminatedComment,
    /// `DIVE-C0004` (warning): the file is not valid UTF-8.
    InvalidUtf8,
    /// `DIVE-C0010`: a token where a section keyword was expected (TLC 5006).
    ExpectedKeyword,
    /// `DIVE-C0011`: a keyword at end of file with no name after it (TLC 5003).
    MissingName,
    /// `DIVE-C0012`: a single-valued keyword given twice (TLC 5004).
    DuplicateKeyword,
    /// `DIVE-C0013`: a constant not followed by `=` or `<-` (TLC 5006).
    ExpectedAssignment,
    /// `DIVE-C0014`: end of file where a value was expected (TLC 5006).
    MissingValue,
    /// `DIVE-C0015`: a set value not closed by `}` (TLC 5006).
    UnclosedSet,
    /// `DIVE-C0016`: a `[Module]` qualifier not closed by `]` (TLC 5006).
    UnclosedModule,
    /// `DIVE-C0017`: end of file after `<-`, `<-[`, `<-[M]` or `=[` (TLC 5005).
    MissingIdentifier,
    /// `DIVE-C0018`: constant operator arguments not closed by `)` (TLC 5002).
    UnclosedArguments,
    /// `DIVE-C0019`: `CHECK_DEADLOCK` not followed by `TRUE` or `FALSE` (TLC 5006).
    ExpectedBoolean,
    /// `DIVE-C0020`: an integer TLC cannot read: above 2^31-1, or written in
    /// `\b`, `\o` or `\h` notation (TLC: `NumberFormatException`, 1000).
    BadInteger,
    /// `DIVE-C0030`: sets nested deeper than dive supports.
    NestingTooDeep,
    /// `DIVE-C0031`: input larger than 4 GiB.
    InputTooLarge,
    /// `DIVE-C0100` (warning): a name that is not an identifier. TLC looks it
    /// up as a definition and fails.
    NotAnIdentifier,
    /// `DIVE-C0101` (warning): a section keyword read as a name or value.
    KeywordAsName,
    /// `DIVE-C0102` (warning): a token that is not an identifier read as a
    /// model value, for example `-` in `N = -1`.
    OddModelValue,
}

impl Code {
    /// The stable string form, e.g. `DIVE-C0013`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedCharacter => "DIVE-C0001",
            Self::BadString => "DIVE-C0002",
            Self::UnterminatedComment => "DIVE-C0003",
            Self::InvalidUtf8 => "DIVE-C0004",
            Self::ExpectedKeyword => "DIVE-C0010",
            Self::MissingName => "DIVE-C0011",
            Self::DuplicateKeyword => "DIVE-C0012",
            Self::ExpectedAssignment => "DIVE-C0013",
            Self::MissingValue => "DIVE-C0014",
            Self::UnclosedSet => "DIVE-C0015",
            Self::UnclosedModule => "DIVE-C0016",
            Self::MissingIdentifier => "DIVE-C0017",
            Self::UnclosedArguments => "DIVE-C0018",
            Self::ExpectedBoolean => "DIVE-C0019",
            Self::BadInteger => "DIVE-C0020",
            Self::NestingTooDeep => "DIVE-C0030",
            Self::InputTooLarge => "DIVE-C0031",
            Self::NotAnIdentifier => "DIVE-C0100",
            Self::KeywordAsName => "DIVE-C0101",
            Self::OddModelValue => "DIVE-C0102",
        }
    }

    /// Every code, in table order.
    pub const ALL: [Self; 20] = [
        Self::UnexpectedCharacter,
        Self::BadString,
        Self::UnterminatedComment,
        Self::InvalidUtf8,
        Self::ExpectedKeyword,
        Self::MissingName,
        Self::DuplicateKeyword,
        Self::ExpectedAssignment,
        Self::MissingValue,
        Self::UnclosedSet,
        Self::UnclosedModule,
        Self::MissingIdentifier,
        Self::UnclosedArguments,
        Self::ExpectedBoolean,
        Self::BadInteger,
        Self::NestingTooDeep,
        Self::InputTooLarge,
        Self::NotAnIdentifier,
        Self::KeywordAsName,
        Self::OddModelValue,
    ];
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the pinned TLC reports for a rejected configuration.
///
/// TLC parses the configuration before SANY runs, so for a rejected file
/// this is TLC's whole outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TlcError {
    /// TLC's `EC` message code: 5002–5007, or 1000 (`GENERAL`) when TLC
    /// fails with an unexpected Java exception.
    pub code: u16,
    /// The `EC` constant's name, e.g. `CFG_EXPECTED_SYMBOL`.
    pub name: &'static str,
    /// TLC's process exit status: 151 (`ERROR_CONFIG_PARSE`) for `CFG_*`
    /// errors, 255 for `GENERAL`.
    pub exit_code: u8,
    /// The line TLC names (1-based), for the messages that carry one.
    pub line: Option<u32>,
    /// TLC's message parameter: the keyword (5003, 5004), the construct
    /// after which an identifier is missing (5005, e.g. `<-[`), what was
    /// expected (5006, e.g. `= or <-`), or the Java exception (1000).
    pub detail: Option<&'static str>,
}

impl TlcError {
    pub(crate) fn cfg(code: u16, line: u32, detail: Option<&'static str>) -> Self {
        let name = match code {
            5002 => "CFG_GENERAL",
            5003 => "CFG_MISSING_ID",
            5004 => "CFG_TWICE_KEYWORD",
            5005 => "CFG_EXPECT_ID",
            5006 => "CFG_EXPECTED_SYMBOL",
            5007 => "CFG_LEXICAL_ERROR",
            _ => "CFG_ERROR_READING_FILE",
        };
        Self {
            code,
            name,
            exit_code: 151,
            line: Some(line),
            detail,
        }
    }

    /// TLC's report of an unexpected Java exception.
    pub(crate) fn general(exception: &'static str) -> Self {
        Self {
            code: 1000,
            name: "GENERAL",
            exit_code: 255,
            line: None,
            detail: Some(exception),
        }
    }
}

/// One diagnostic about a configuration file.
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
    /// TLC's report for the same input. Set on every error except those
    /// that only dive has (nesting and size limits).
    pub tlc: Option<TlcError>,
}

impl Diagnostic {
    pub(crate) fn error(
        code: Code,
        range: TextRange,
        message: impl Into<String>,
        tlc: Option<TlcError>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            range,
            help: None,
            tlc,
        }
    }

    pub(crate) fn warning(code: Code, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            range,
            help: None,
            tlc: None,
        }
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Whether this diagnostic rejects the configuration.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_prefixed() {
        let mut seen = std::collections::BTreeSet::new();
        for c in Code::ALL {
            assert!(seen.insert(c.as_str()), "{c}");
            assert!(c.as_str().starts_with("DIVE-C"), "{c}");
        }
    }
}
