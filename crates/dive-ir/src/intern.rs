//! The names SANY's lexer interns ([`crate::Ir::lexical_names`]).
//!
//! TLC keeps every name in one table, `util.UniqueString`, and orders
//! strings, model values and record fields by their position in it (ADR
//! 0012). SANY's lexer (`TLAplusParserTokenManager`) interns the image of
//! every token it reads, file by file in the order the files are parsed; the
//! contents of string literals are interned later, when TLC processes the
//! specification (`SpecProcessor.processConstants`, `new StringValue`).
//! This module records the lexer's part: each token image at its first
//! occurrence. Measured against the pinned TLC's table (`dive-model`'s
//! acceptance tooling).

use dive_sema::Analysis;
use dive_syntax::SyntaxKind;
use std::collections::HashSet;

/// The token images of every loaded file, in load order, each at its first
/// occurrence.
pub(crate) fn lexical_names(analysis: &Analysis) -> Vec<Box<str>> {
    let mut seen: HashSet<Box<str>> = HashSet::new();
    let mut out = Vec::new();
    for file in analysis.files() {
        let root = file.parse.syntax();
        for el in root.descendants_with_tokens() {
            let Some(tok) = el.into_token() else { continue };
            let kind = tok.kind();
            if kind.is_trivia() || kind == SyntaxKind::LEX_ERROR {
                continue;
            }
            let text = tok.text();
            if text.is_empty() {
                continue;
            }
            if seen.insert(text.into()) {
                out.push(text.into());
            }
        }
    }
    out
}
