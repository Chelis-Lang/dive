//! Raw tokens of SANY's `SPEC` lexical state.
//!
//! Other TLA+ formats reuse SANY's token manager. TLC's model configuration
//! reader (`tlc2.tool.impl.ModelConfig`) creates a `TLAplusParserTokenManager`
//! directly in the `SPEC` state, so a `.cfg` file is tokenized exactly like
//! the body of a module. [`tokenize_spec`] exposes that lexer.
//!
//! # Example
//!
//! ```
//! use dive_syntax::SyntaxKind;
//! use dive_syntax::tokens::tokenize_spec;
//!
//! let src = "INIT Init \\* comment\nCONSTANT N <- 3";
//! let words: Vec<&str> = tokenize_spec(src)
//!     .iter()
//!     .filter(|t| !t.kind.is_trivia())
//!     .map(|t| &src[t.range])
//!     .collect();
//! assert_eq!(words, ["INIT", "Init", "CONSTANT", "N", "<-", "3"]);
//! assert_eq!(tokenize_spec("\"open").last().map(|t| t.kind), Some(SyntaxKind::LEX_ERROR));
//! ```

use crate::SyntaxKind;
use crate::lexer::{self, LexFlag};
use rowan::{TextRange, TextSize};

/// Why a token is a lexical error. SANY's token manager would throw a
/// `TokenMgrError` when it reaches such a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LexError {
    /// A character no token can start with.
    UnexpectedCharacter,
    /// A string literal with a bad escape, or no closing quote on its line.
    BadString,
    /// A block comment still open at end of input.
    UnterminatedComment,
}

/// One token, trivia included. Concatenating the ranges of all tokens
/// reproduces the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// The token kind. Lexical errors have kind [`SyntaxKind::LEX_ERROR`],
    /// except unterminated block comments, which keep
    /// [`SyntaxKind::BLOCK_COMMENT`] and carry [`Token::error`].
    pub kind: SyntaxKind,
    /// The token's bytes in the input.
    pub range: TextRange,
    /// Set when SANY's token manager would fail on this token.
    pub error: Option<LexError>,
}

/// Tokenizes `src` from the start in SANY's `SPEC` lexical state.
///
/// Inputs larger than 4 GiB cannot be represented by [`TextRange`]; callers
/// must refuse them first. Offsets past `u32::MAX` are clamped.
#[must_use]
pub fn tokenize_spec(src: &str) -> Vec<Token> {
    lexer::lex_spec(src)
        .into_iter()
        .map(|t| Token {
            kind: t.kind,
            range: TextRange::new(TextSize::from(t.start), TextSize::from(t.end)),
            error: match t.flag {
                LexFlag::Ok => None,
                LexFlag::BadChar => Some(LexError::UnexpectedCharacter),
                LexFlag::BadString => Some(LexError::BadString),
                LexFlag::UnterminatedComment => Some(LexError::UnterminatedComment),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_state_from_offset_zero() {
        // No module header is needed: `INIT` is lexed as a token at once.
        let src = "INIT x";
        let t: Vec<_> = tokenize_spec(src)
            .into_iter()
            .filter(|t| !t.kind.is_trivia())
            .collect();
        assert_eq!(t.len(), 2);
        assert_eq!(&src[t[0].range], "INIT");
    }

    #[test]
    fn errors_are_flagged() {
        let kinds = |s: &str| {
            tokenize_spec(s)
                .into_iter()
                .filter_map(|t| t.error)
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(";"), [LexError::UnexpectedCharacter]);
        assert_eq!(kinds("\"a\\q\""), [LexError::BadString]);
        assert_eq!(kinds("(* a (* b *)"), [LexError::UnterminatedComment]);
        assert_eq!(kinds("(* a (* b *) *) \\* c"), []);
    }

    #[test]
    fn lossless() {
        let src = "CONSTANT S = {a, \"b\"} (* c *)\r\n\\* d";
        let joined: String = tokenize_spec(src).iter().map(|t| &src[t.range]).collect();
        assert_eq!(joined, src);
    }
}
