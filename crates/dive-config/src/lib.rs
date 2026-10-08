//! `dive-config`: the TLC model configuration (`.cfg`) format.
//!
//! [`parse`] reads a cfg file exactly as the pinned TLC does
//! (`tlc2.tool.impl.ModelConfig` at upstream commit `94d0c50`, see
//! `docs/provenance.md`):
//!
//! - **Tokens** come from SANY's TLA+ lexer in its `SPEC` state, shared with
//!   `dive-syntax` ([`dive_syntax::tokens`]). Comments are TLA+ comments.
//! - **Keywords** are TLC's full set ([`KEYWORDS`]), including the
//!   undocumented `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE`.
//! - **Values** follow TLC's narrow grammar ([`ConfigValue`]): 32-bit
//!   non-negative decimal integers, strings kept verbatim, `TRUE`/`FALSE`,
//!   sets, and model values. Any other token is a model value.
//! - **Errors** stop at the first problem, in TLC's order. Each carries a
//!   stable `DIVE-Cnnnn` code and TLC's own `EC` code, exit status and line
//!   ([`TlcError`]). A lexical error anywhere TLC reads it rejects the file
//!   (TLC `d17f1d35e`: before that fix TLC silently truncated the file).
//!
//! What TLC accepts here but rejects later (undefined names, arity, levels,
//! `SPECIFICATION` together with `INIT`) is semantic analysis, done against
//! the specification by later stages. [`ModelConfig`] keeps every name as
//! TLC's text with its span so those stages can report precisely.
//!
//! # Values and `dive-value`
//!
//! `dive-config` may depend on `dive-value`, but this crate does not use it
//! yet: cfg values are this crate's own small [`ConfigValue`], designed to be
//! lowered to `dive-value` later. The module docs of [`ConfigValue`]'s module
//! list what lowering must preserve.
//!
//! # Not yet supported
//!
//! TLC can read a configuration embedded in a `.tla` file
//! (`util.MonolithSpecExtractor`, used when the file name ends in `.tla`).
//! This crate parses `.cfg` text only; callers refuse `.tla` configurations.
//!
//! # Example
//!
//! ```
//! let parsed = dive_config::parse(
//!     "CONSTANTS N = 3  Procs = {p1, p2}\nINIT Init\nNEXT Next\nINVARIANT TypeOK Safe\nCHECK_DEADLOCK FALSE\n",
//! );
//! assert!(parsed.accepted());
//! let cfg = parsed.config().expect("accepted");
//! assert_eq!(cfg.init.as_ref().map(|n| n.text.as_str()), Some("Init"));
//! let invariants: Vec<&str> = cfg.invariants.iter().map(|n| n.text.as_str()).collect();
//! assert_eq!(invariants, ["TypeOK", "Safe"]);
//! assert!(!cfg.checks_deadlock());
//! assert_eq!(cfg.model_values, ["p1", "p2"]);
//!
//! // `N = -1` is not a negative integer to TLC: `-` is a model value, and
//! // `1` then starts a new entry that has no `=`.
//! let bad = dive_config::parse("CONSTANT N = -1\nINIT Init\n");
//! let err = bad.error().expect("rejected");
//! assert_eq!(err.code.as_str(), "DIVE-C0013");
//! let tlc = err.tlc.expect("TLC rejects it too");
//! assert_eq!((tlc.code, tlc.exit_code, tlc.line), (5006, 151, Some(2)));
//! ```
#![forbid(unsafe_code)]

mod diagnostics;
mod model;
mod parser;
mod value;

pub use diagnostics::{Code, Diagnostic, Severity, TlcError};
pub use dive_syntax::{TextRange, TextSize};
pub use model::{Assignment, ConstantEntry, ModelConfig, Name};
pub use parser::{KEYWORDS, MAX_SET_NESTING, is_keyword};
pub use value::{ConfigValue, Spanned};

/// The result of reading one cfg file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigParse {
    config: Option<ModelConfig>,
    diagnostics: Vec<Diagnostic>,
}

impl ConfigParse {
    /// The configuration, if TLC would accept the file.
    #[must_use]
    pub fn config(&self) -> Option<&ModelConfig> {
        self.config.as_ref()
    }

    /// Consumes the result, returning the configuration if accepted.
    #[must_use]
    pub fn into_config(self) -> Option<ModelConfig> {
        self.config
    }

    /// Warnings, then the error if the file was rejected, in the order
    /// they were found.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether TLC's cfg reader accepts the file.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.config.is_some()
    }

    /// The error that rejected the file. Like TLC, reading stops at the
    /// first one, so there is at most one.
    #[must_use]
    pub fn error(&self) -> Option<&Diagnostic> {
        self.diagnostics.iter().find(|d| d.is_error())
    }
}

/// Reads cfg text.
///
/// Never panics. Inputs of 4 GiB or more are refused with `DIVE-C0031`.
#[must_use]
pub fn parse(source: &str) -> ConfigParse {
    if u32::try_from(source.len()).is_err() {
        return ConfigParse {
            config: None,
            diagnostics: vec![Diagnostic::error(
                Code::InputTooLarge,
                TextRange::empty(TextSize::from(0)),
                "cfg input of 4 GiB or more is not supported",
                None,
            )],
        };
    }
    let (config, diagnostics) = parser::read(source);
    ConfigParse {
        config,
        diagnostics,
    }
}

/// Reads cfg bytes. As TLC's UTF-8 reader does, malformed bytes are read as
/// U+FFFD, which is a lexical error unless it sits inside a comment or
/// string; a `DIVE-C0004` warning records the decoding. Returns the text
/// that was parsed, which spans refer to.
#[must_use]
pub fn parse_bytes(bytes: &[u8]) -> (String, ConfigParse) {
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_owned(), parse(s)),
        Err(e) => {
            let text = String::from_utf8_lossy(bytes).into_owned();
            let mut parsed = parse(&text);
            let at = TextSize::try_from(e.valid_up_to()).unwrap_or_default();
            parsed.diagnostics.insert(
                0,
                Diagnostic::warning(
                    Code::InvalidUtf8,
                    TextRange::empty(at),
                    "the file is not valid UTF-8; invalid bytes were read as U+FFFD",
                )
                .with_help_text("save the file as UTF-8"),
            );
            (text, parsed)
        }
    }
}

impl Diagnostic {
    fn with_help_text(mut self, help: &str) -> Self {
        self.help = Some(help.to_owned());
        self
    }
}
