//! The parser's intermediate tree and its conversion to a rowan green tree.
//!
//! The parser builds owned [`Raw`] nodes whose leaves are indices of
//! significant tokens. Building a separate tree lets the operator-precedence
//! machinery rearrange subtrees freely, as SANY's operator stack does. Once
//! parsing ends, [`build_green`] walks the tree in token order and puts the
//! trivia (whitespace and comments) back between the tokens, so the green
//! tree's text is exactly the source.

use crate::kind::SyntaxKind;
use crate::lexer::LexToken;
use rowan::{GreenNode, GreenNodeBuilder};

/// A child of a [`Raw`] node.
#[derive(Debug)]
pub(crate) enum Child {
    /// A significant token, by index into the parser's token list.
    Tok(u32),
    /// A subtree.
    Node(Raw),
}

/// An owned syntax node under construction.
#[derive(Debug)]
pub(crate) struct Raw {
    pub(crate) kind: SyntaxKind,
    pub(crate) children: Vec<Child>,
}

impl Raw {
    pub(crate) fn new(kind: SyntaxKind) -> Self {
        Self {
            kind,
            children: Vec::new(),
        }
    }

    pub(crate) fn tok(&mut self, i: u32) {
        self.children.push(Child::Tok(i));
    }

    pub(crate) fn node(&mut self, n: Raw) {
        self.children.push(Child::Node(n));
    }

    /// Height of this subtree (a node with only tokens has height 1).
    pub(crate) fn height(&self) -> u32 {
        let mut max = 0;
        let mut stack: Vec<(&Raw, u32)> = vec![(self, 1)];
        while let Some((n, d)) = stack.pop() {
            max = max.max(d);
            for c in &n.children {
                if let Child::Node(m) = c {
                    stack.push((m, d + 1));
                }
            }
        }
        max
    }

    /// Index of the last token in this subtree, if it has any.
    pub(crate) fn last_token(&self) -> Option<u32> {
        let mut stack: Vec<&Child> = self.children.iter().collect();
        while let Some(c) = stack.pop() {
            match c {
                Child::Tok(i) => return Some(*i),
                Child::Node(n) => stack.extend(n.children.iter()),
            }
        }
        None
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        // Drop deep trees iteratively so long operator chains cannot
        // overflow the stack.
        let mut stack = std::mem::take(&mut self.children);
        while let Some(c) = stack.pop() {
            if let Child::Node(mut n) = c {
                stack.append(&mut n.children);
            }
        }
    }
}

/// Inputs needed to turn a [`Raw`] tree into a green tree.
pub(crate) struct GreenInput<'a> {
    pub(crate) src: &'a str,
    pub(crate) lexed: &'a [LexToken],
    /// Index into `lexed` of each significant token.
    pub(crate) sig: &'a [u32],
    /// Final kind of each significant token (keywords may become `IDENT`).
    pub(crate) kinds: &'a [SyntaxKind],
    /// Index into `lexed` where the trailer starts, if the module ended.
    pub(crate) trailer_from: Option<u32>,
}

struct Builder<'a> {
    input: &'a GreenInput<'a>,
    b: GreenNodeBuilder<'static>,
    /// Next index into `lexed` not yet emitted.
    cursor: u32,
    /// Next significant token expected in the tree.
    next_sig: u32,
}

fn text_of<'a>(input: &GreenInput<'a>, t: &LexToken) -> &'a str {
    input
        .src
        .get(t.start as usize..t.end as usize)
        .unwrap_or("")
}

impl Builder<'_> {
    /// Emits lexed tokens before `upto` (exclusive) as they are.
    fn flush(&mut self, upto: u32) {
        while self.cursor < upto {
            let Some(t) = self.input.lexed.get(self.cursor as usize) else {
                break;
            };
            self.b.token(t.kind.into(), text_of(self.input, t));
            self.cursor += 1;
        }
    }

    /// Emits trivia that precedes the next significant token, so that a node
    /// about to start does not swallow it.
    fn flush_leading(&mut self) {
        if let Some(&full) = self.input.sig.get(self.next_sig as usize) {
            self.flush(full);
        }
    }

    fn token(&mut self, sig_idx: u32) {
        let Some(&full) = self.input.sig.get(sig_idx as usize) else {
            return;
        };
        self.flush(full);
        if self.cursor != full {
            return;
        }
        let Some(t) = self.input.lexed.get(full as usize) else {
            return;
        };
        let kind = self
            .input
            .kinds
            .get(sig_idx as usize)
            .copied()
            .unwrap_or(t.kind);
        self.b.token(kind.into(), text_of(self.input, t));
        self.cursor += 1;
        self.next_sig = sig_idx + 1;
    }
}

/// Converts the parse result into a green tree whose text equals the source.
pub(crate) fn build_green(root: &Raw, input: &GreenInput<'_>) -> GreenNode {
    let mut bld = Builder {
        input,
        b: GreenNodeBuilder::new(),
        cursor: 0,
        next_sig: 0,
    };
    // Explicit stack of (node, next child index).
    let mut stack: Vec<(&Raw, usize)> = Vec::new();
    bld.b.start_node(root.kind.into());
    stack.push((root, 0));
    while let Some((node, i)) = stack.pop() {
        if let Some(child) = node.children.get(i) {
            stack.push((node, i + 1));
            match child {
                Child::Tok(t) => bld.token(*t),
                Child::Node(n) => {
                    // Tokens appear in the tree in source order, so a node's
                    // first token is the next one; leading trivia stays outside.
                    if !n.children.is_empty() {
                        bld.flush_leading();
                    }
                    bld.b.start_node(n.kind.into());
                    stack.push((n, 0));
                }
            }
        } else {
            if stack.is_empty() {
                // Closing the root: emit everything left, the trailer last.
                let end = u32::try_from(input.lexed.len()).unwrap_or(u32::MAX);
                match input.trailer_from {
                    Some(from) if from < end => {
                        bld.flush(from);
                        if let (Some(first), Some(last)) =
                            (input.lexed.get(from as usize), input.lexed.last())
                        {
                            let text = input
                                .src
                                .get(first.start as usize..last.end as usize)
                                .unwrap_or("");
                            bld.b.token(SyntaxKind::TRAILER.into(), text);
                        }
                        bld.cursor = end;
                    }
                    _ => bld.flush(end),
                }
            }
            bld.b.finish_node();
        }
    }
    bld.b.finish()
}
