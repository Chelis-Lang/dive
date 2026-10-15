//! The translator's algorithm tree (semantics of `pcal.AST`).
//!
//! Unlike the source AST in `crate::ast`, this tree is rewritten in place by
//! the translation stages: macro expansion, label insertion, statement
//! grouping, renaming and explosion into labelled steps.

use super::expr::Expr;

/// A one-based source position, plus the macro call it came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Loc {
    pub line: usize,
    pub col: usize,
    /// The call site when the statement came from a macro expansion.
    pub macro_call: Option<(usize, usize)>,
}

impl Loc {
    /// `AST.location()`.
    pub(crate) fn describe(self) -> String {
        match self.macro_call {
            None => format!("line {}, column {}", self.line, self.col),
            Some((line, col)) => format!(
                "line {}, column {} of macro called at line {line}, column {col}",
                self.line, self.col
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SingleAssign {
    pub var: String,
    /// Subscripts and field selectors, empty for a whole-variable assignment.
    pub sub: Expr,
    pub rhs: Expr,
    pub loc: Loc,
}

#[derive(Clone, Debug)]
pub(crate) struct LabeledStmt {
    pub label: String,
    pub stmts: Vec<Stmt>,
    pub loc: Loc,
}

#[derive(Clone, Debug)]
pub(crate) struct Clause {
    pub unlab: Vec<Stmt>,
    pub lab: Vec<LabeledStmt>,
}

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Assign(Vec<SingleAssign>),
    /// An `if` without inner labels (after classification).
    If {
        test: Expr,
        then: Vec<Stmt>,
        els: Vec<Stmt>,
    },
    LabelIf {
        test: Expr,
        unlab_then: Vec<Stmt>,
        lab_then: Vec<LabeledStmt>,
        unlab_else: Vec<Stmt>,
        lab_else: Vec<LabeledStmt>,
    },
    /// An `either` without inner labels (after classification).
    Either(Vec<Vec<Stmt>>),
    LabelEither(Vec<Clause>),
    While {
        test: Expr,
        unlab: Vec<Stmt>,
        lab: Vec<LabeledStmt>,
    },
    With {
        var: String,
        is_eq: bool,
        exp: Expr,
        body: Vec<Stmt>,
    },
    When(Expr),
    Print(Expr),
    Assert(Expr),
    Skip,
    Call {
        to: String,
        args: Vec<Expr>,
    },
    Return {
        from: Option<String>,
    },
    CallReturn {
        to: String,
        from: Option<String>,
        args: Vec<Expr>,
    },
    CallGoto {
        to: String,
        after: String,
        args: Vec<Expr>,
    },
    Goto(String),
    MacroCall {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Stmt {
    /// Label, or empty.
    pub lbl: String,
    pub loc: Loc,
    /// Byte offset of the statement's first token (for diagnostics).
    pub at: usize,
    pub kind: Kind,
}

impl Stmt {
    pub(crate) fn new(kind: Kind, loc: Loc, at: usize) -> Self {
        Self {
            lbl: String::new(),
            loc,
            at,
            kind,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VarDecl {
    pub var: String,
    pub is_eq: bool,
    pub val: Expr,
    pub loc: Loc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fairness {
    Unfair,
    Weak,
    Strong,
}

#[derive(Clone, Debug)]
pub(crate) struct Procedure {
    pub name: String,
    pub params: Vec<VarDecl>,
    pub decls: Vec<VarDecl>,
    pub stmts: Vec<Stmt>,
    pub body: Vec<LabeledStmt>,
    pub plus_labels: Vec<String>,
    pub minus_labels: Vec<String>,
    pub procedures_called: Vec<String>,
    pub loc: Loc,
}

#[derive(Clone, Debug)]
pub(crate) struct Process {
    pub name: String,
    pub fairness: Fairness,
    pub is_eq: bool,
    pub id: Expr,
    pub decls: Vec<VarDecl>,
    pub stmts: Vec<Stmt>,
    pub body: Vec<LabeledStmt>,
    pub plus_labels: Vec<String>,
    pub minus_labels: Vec<String>,
    pub procedures_called: Vec<String>,
    pub loc: Loc,
}

#[derive(Clone, Debug)]
pub(crate) struct Macro {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub(crate) enum Body {
    /// A uniprocess algorithm: its statements, then its labelled steps.
    Uni {
        stmts: Vec<Stmt>,
        body: Vec<LabeledStmt>,
    },
    Multi(Vec<Process>),
}

/// Whole-algorithm facts that the parser collects (`ParseAlgorithm` statics).
#[derive(Clone, Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent translator facts, not a state machine"
)]
pub(crate) struct Facts {
    pub goto_used: bool,
    pub goto_done_used: bool,
    pub omit_pc: bool,
    pub omit_stuttering_when_done: bool,
    pub has_default_initialization: bool,
    pub has_label: bool,
    /// Every label typed in the source.
    pub all_labels: Vec<String>,
    /// Labels inserted by the translator with their statement locations.
    pub added_labels: Vec<(String, Loc, usize)>,
    pub next_label_num: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Algorithm {
    pub fair: bool,
    pub decls: Vec<VarDecl>,
    pub defs: Expr,
    pub macros: Vec<Macro>,
    pub prcds: Vec<Procedure>,
    pub body: Body,
    pub facts: Facts,
}
