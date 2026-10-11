//! Shared TLA+ lexical state with the PlusCal-only delimiter spellings.
use crate::{
    Code, Diagnostic,
    ast::{Span, Token},
};

pub(crate) fn tokenize(
    source: &str,
    start: usize,
    end: usize,
    max_tokens: usize,
) -> Result<Vec<Token>, Diagnostic> {
    tokenize_owned(source, start, end, max_tokens, false).map(|(tokens, _)| tokens)
}

// Embedded source parsing may ignore ordinary tokens after the owned grammar
// end, but malformed quoted/comment lexical state and resource limits remain
// deciding errors. Deferred diagnostics are admitted only beyond that end.
pub(crate) fn tokenize_prefix(
    source: &str,
    start: usize,
    end: usize,
    max_tokens: usize,
) -> Result<(Vec<Token>, Vec<Diagnostic>), Diagnostic> {
    tokenize_owned(source, start, end, max_tokens, true)
}

fn tokenize_owned(
    source: &str,
    start: usize,
    end: usize,
    max_tokens: usize,
    prefix: bool,
) -> Result<(Vec<Token>, Vec<Diagnostic>), Diagnostic> {
    let mut tokens = Vec::new();
    let mut deferred = Vec::new();
    for raw in dive_syntax::tokens::tokenize_spec(&source[start..end]) {
        let span = Span {
            start: start + usize::from(raw.range.start()),
            end: start + usize::from(raw.range.end()),
        };
        let text = &source[span.start..span.end];
        // SANY does not use these PlusCal punctuation tokens. Its lexer flags
        // them individually; PlusCal's token manager accepts these spellings.
        if raw.error.is_some() && !matches!(text, ";" | "≔" | "‖") {
            let error = Diagnostic {
                code: Code::Lexical,
                span,
                message: format!("invalid PlusCal token: {text:?}"),
            };
            if prefix && raw.error == Some(dive_syntax::tokens::LexError::UnexpectedCharacter) {
                deferred.push(error);
            } else {
                return Err(error);
            }
        }
        if text.starts_with('"') && text.chars().any(|c| !(' '..='~').contains(&c)) {
            let error = Diagnostic {
                code: Code::Lexical,
                span,
                message: "illegal character in PlusCal string (pinned Tokenize.STRING)".into(),
            };
            if prefix {
                deferred.push(error);
            } else {
                return Err(error);
            }
        }
        if raw.kind.is_trivia() {
            continue;
        }
        if tokens.len() >= max_tokens {
            return Err(Diagnostic {
                code: Code::Resource,
                span,
                message: "PlusCal token limit reached".into(),
            });
        }
        tokens.push(Token {
            text: text.to_owned(),
            span,
        });
    }
    // SANY splits := and || (which are not TLA+ expression operators).
    // Fuse only contiguous tokens; whitespace changes the original grammar.
    let mut fused: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens {
        if let Some(previous) = fused.last_mut()
            && previous.span.end == token.span.start
            && matches!(
                (previous.text.as_str(), token.text.as_str()),
                (":", "=") | ("|", "|")
            )
        {
            previous.text.push_str(&token.text);
            previous.span.end = token.span.end;
            continue;
        }
        fused.push(token);
    }
    Ok((fused, deferred))
}
