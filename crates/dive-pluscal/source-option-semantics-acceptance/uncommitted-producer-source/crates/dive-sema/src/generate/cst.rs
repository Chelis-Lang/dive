//! Small helpers for reading the concrete syntax tree.

use dive_syntax::ast::Expr;
use dive_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// The child nodes of `n`.
pub(crate) fn nodes(n: &SyntaxNode) -> Vec<SyntaxNode> {
    n.children().collect()
}

/// The non-trivia tokens directly under `n`.
pub(crate) fn toks(n: &SyntaxNode) -> Vec<SyntaxToken> {
    n.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|t| !t.kind().is_trivia())
        .collect()
}

/// The non-trivia children (nodes and tokens) of `n`, in order.
pub(crate) fn elems(n: &SyntaxNode) -> Vec<SyntaxElement> {
    n.children_with_tokens()
        .filter(|e| !e.kind().is_trivia())
        .collect()
}

/// Whether `n` has a direct token of kind `k`.
pub(crate) fn has_tok(n: &SyntaxNode, k: SyntaxKind) -> bool {
    toks(n).iter().any(|t| t.kind() == k)
}

/// The expression children of `n`.
pub(crate) fn exprs(n: &SyntaxNode) -> Vec<SyntaxNode> {
    n.children().filter(|c| Expr::can_cast(c.kind())).collect()
}

/// The last expression child of `n`.
pub(crate) fn last_expr(n: &SyntaxNode) -> Option<SyntaxNode> {
    n.children().filter(|c| Expr::can_cast(c.kind())).last()
}

/// The first expression child of `n`.
pub(crate) fn first_expr(n: &SyntaxNode) -> Option<SyntaxNode> {
    n.children().find(|c| Expr::can_cast(c.kind()))
}

/// The child nodes of an `OP_ARGS` node: the arguments.
pub(crate) fn op_args(n: &SyntaxNode) -> Vec<SyntaxNode> {
    nodes(n)
}
