//! `dive-syntax`: the TLA+ lexer and parser.
//!
//! [`parse_module`] turns source text into a lossless concrete syntax tree
//! (CST): the tree's text is byte-identical to the input, including
//! whitespace, comments (and the PlusCal algorithms inside them), the text
//! before the module header and the text after the closing `====`.
//!
//! The parser accepts exactly what the pinned SANY parser accepts. That
//! includes SANY's syntactic restrictions, such as operator precedence
//! conflicts (`a * -b` is rejected) and junction-list alignment. It never
//! panics, and it always produces a tree; after an error it recovers at the
//! next module-level unit so editors keep working.
//!
//! Semantic analysis (name resolution, arity, levels) is not done here;
//! that is `dive-sema`.
//!
//! # Example
//!
//! ```
//! use dive_syntax::{parse_module, ast::AstNode};
//!
//! let src = "---- MODULE Counter ----\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
//! let parse = parse_module(src);
//! assert!(parse.is_ok());
//! assert_eq!(parse.syntax().to_string(), src); // lossless
//!
//! let module = parse.module().expect("a module");
//! assert_eq!(module.name().as_deref(), Some("Counter"));
//! let defs: Vec<String> = module
//!     .operator_definitions()
//!     .filter_map(|d| d.name())
//!     .collect();
//! assert_eq!(defs, ["Init", "Next"]);
//! ```
#![forbid(unsafe_code)]

pub mod ast;
pub mod diagnostics;
mod kind;
mod lexer;
pub mod ops;
mod parser;
mod tree;

pub use diagnostics::{Code, Diagnostic, Severity};
pub use kind::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TlaLanguage};
pub use rowan::{GreenNode, TextRange, TextSize};

/// The result of parsing one source file.
///
/// Cloning is cheap: the tree is reference counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// The root of the concrete syntax tree (a `SOURCE_FILE` node).
    #[must_use]
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// The immutable green tree, which is `Send + Sync` and cheap to clone.
    #[must_use]
    pub fn green(&self) -> &GreenNode {
        &self.green
    }

    /// All diagnostics, in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether SANY's parser would accept the input (no error diagnostics).
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// The top-level module, if a module header was found.
    #[must_use]
    pub fn module(&self) -> Option<ast::Module> {
        ast::SourceFile::cast(self.syntax()).and_then(|f| f.module())
    }

    /// SANY's file-name check: a file `Name.tla` must contain module `Name`
    /// (compared exactly). `file_name` is the file's base name; everything
    /// from its last `.` is ignored, as in SANY. Returns the error, if any.
    ///
    /// SANY only checks this after the module parsed, so this does too.
    ///
    /// ```
    /// let p = dive_syntax::parse_module("---- MODULE A ----\n====");
    /// assert!(p.check_file_name("A.tla").is_none());
    /// assert_eq!(p.check_file_name("B.tla").unwrap().code.as_str(), "DIVE-S0006");
    /// ```
    #[must_use]
    pub fn check_file_name(&self, file_name: &str) -> Option<Diagnostic> {
        if !self.is_ok() {
            return None;
        }
        let stem = file_name.rfind('.').map_or(file_name, |i| &file_name[..i]);
        let module = self.module()?;
        let name = module.name()?;
        if name == stem {
            return None;
        }
        let range = module
            .syntax()
            .children()
            .find(|n| n.kind() == SyntaxKind::MODULE_HEADER)
            .and_then(|h| {
                h.children_with_tokens()
                    .filter_map(rowan::NodeOrToken::into_token)
                    .find(|t| t.kind() == SyntaxKind::IDENT)
            })
            .map_or_else(|| module.syntax().text_range(), |t| t.text_range());
        Some(
            Diagnostic::error(
                Code::ModuleNameMismatch,
                range,
                format!("file name `{stem}` does not match the module name `{name}`"),
            )
            .with_help(format!(
                "rename the file to `{name}.tla` or the module to `{stem}`"
            )),
        )
    }
}

use ast::AstNode as _;

/// Parses a TLA+ source file containing one top-level module.
///
/// Text before the first `---- MODULE` line and after the module's closing
/// `====` is kept in the tree as `PRELUDE` and `TRAILER` trivia, as SANY
/// ignores it.
///
/// # Example
///
/// ```
/// let p = dive_syntax::parse_module("---- MODULE M ----\nx == 1 * -2\n====");
/// assert!(!p.is_ok());
/// assert_eq!(p.diagnostics()[0].code.as_str(), "DIVE-S0012"); // precedence conflict
/// ```
#[must_use]
pub fn parse_module(source: &str) -> Parse {
    if u32::try_from(source.len()).is_err() {
        // rowan text sizes are 32-bit; refuse rather than truncate.
        let mut b = rowan::GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SOURCE_FILE.into());
        b.finish_node();
        return Parse {
            green: b.finish(),
            diagnostics: vec![Diagnostic::error(
                Code::InputTooLarge,
                TextRange::empty(TextSize::from(0)),
                "input larger than 4 GiB is not supported",
            )],
        };
    }
    let lexed = lexer::lex(source);
    let out = parser::Parser::new(source, &lexed).parse();
    let green = tree::build_green(
        &out.root,
        &tree::GreenInput {
            src: source,
            lexed: &lexed,
            sig: &out.sig,
            kinds: &out.kinds,
            trailer_from: out.trailer_from,
        },
    );
    let mut diagnostics = parser::lexer_diagnostics(&lexed, out.trailer_from);
    diagnostics.extend(out.diags);
    diagnostics.sort_by_key(|d| (d.range.start(), d.range.end()));
    Parse { green, diagnostics }
}
