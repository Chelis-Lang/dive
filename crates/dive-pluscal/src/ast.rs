//! Source grammar nodes. Spelling and byte ranges are never normalized away.
use serde::{Deserialize, Serialize};

/// Half-open UTF-8 byte range in the original source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// First byte after the node.
    pub end: usize,
}
/// One-based source display position, counting Unicode scalars and 8-column tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// One-based line, with CRLF treated as one line ending.
    pub line: usize,
    /// One-based display column (not a byte offset).
    pub column: usize,
}
/// Significant lexical token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    /// Original spelling.
    pub text: String,
    /// Original bytes.
    pub span: Span,
}
/// Identifier or target spelling with its location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    /// Exact spelling, including source quotes where present.
    pub text: String,
    /// Original bytes.
    pub span: Span,
}
/// TLA+ token slice with counted nesting; syntax/semantics are later stages.
/// The reference scanner counts opener/closer depth without matching kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expr {
    /// First through last significant expression token.
    pub span: Span,
}
/// PlusCal surface syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Syntax {
    /// `begin ... end algorithm`.
    P,
    /// Nested curly braces.
    C,
}
/// Explicit process or algorithm fairness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fairness {
    /// No declaration.
    #[default]
    Unfair,
    /// `fair` or `--fair algorithm`.
    Weak,
    /// `fair + process`.
    Strong,
}
/// Initializer or binder kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingKind {
    /// Equality.
    Equal,
    /// Membership.
    Member,
}
/// Explicit initializer or local binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    /// Equality or membership.
    pub kind: BindingKind,
    /// Expression slice.
    pub expression: Expr,
}
/// Variable declaration or procedure parameter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decl {
    /// Declared spelling.
    pub name: Name,
    /// Absent preserves implicit default initialization.
    pub initial: Option<Binding>,
    /// Complete declaration, excluding separator.
    pub span: Span,
}
/// Statement id in [`crate::Parsed::statements`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StmtId(pub usize);
/// Ordered statement list; child statements are arena references.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Statement extent; explicit C block statements include their braces.
    pub span: Span,
    /// Source order.
    pub statements: Vec<StmtId>,
}
/// Explicit label, before synthetic label insertion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    /// Label spelling.
    pub name: Name,
    /// `+`, `-`, or absent.
    pub modifier: Option<char>,
    /// Includes colon/modifier.
    pub span: Span,
}
/// One part of a parallel assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    /// Assigned variable spelling.
    pub variable: Name,
    /// Balanced field/subscript slice between variable and assignment token.
    pub selectors: Option<Expr>,
    /// Right-hand side.
    pub value: Expr,
    /// Entire single assignment.
    pub span: Span,
}
/// Conditional branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// Original condition.
    pub test: Expr,
    /// Source body.
    pub body: Block,
}
/// One locally bound `with` variable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithBinding {
    /// Bound spelling.
    pub name: Name,
    /// Equality/membership expression.
    pub binding: Binding,
}
/// Source statements before expansion or control-flow normalization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatementKind {
    /// Parallel assignments, preserving order.
    Assign(Vec<Assignment>),
    /// C-syntax curly statement list.
    Block(Block),
    /// Loop.
    While {
        /// Original condition.
        test: Expr,
        /// Original body.
        body: Block,
    },
    /// Conditional.
    If {
        /// `if` and every `elsif` branch.
        branches: Vec<Branch>,
        /// Explicit else body.
        otherwise: Option<Block>,
    },
    /// Nondeterministic alternative bodies.
    Either(Vec<Block>),
    /// Local binders, preserved before normalization to nested With nodes.
    With {
        /// Source order.
        bindings: Vec<WithBinding>,
        /// Original body.
        body: Block,
    },
    /// Waiting predicate.
    Await {
        /// Source used `when` instead of `await`.
        when: bool,
        /// Original condition.
        condition: Expr,
    },
    /// Assertion predicate.
    Assert(Expr),
    /// Print expression.
    Print(Expr),
    /// No-op.
    Skip,
    /// Procedure invocation; following return/goto remains a separate node.
    Call {
        /// Called spelling.
        name: Name,
        /// Source order.
        arguments: Vec<Expr>,
    },
    /// Return.
    Return,
    /// Label target, including quoted `"Done"` when written.
    Goto(Name),
    /// Macro invocation before expansion.
    MacroCall {
        /// Called spelling.
        name: Name,
        /// Source order.
        arguments: Vec<Expr>,
    },
}
/// Typed statement and optional source label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    /// Full original extent.
    pub span: Span,
    /// No synthetic label is invented by parsing.
    pub label: Option<Label>,
    /// Structure.
    pub kind: StatementKind,
}
/// Macro definition before expansion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    /// Macro spelling.
    pub name: Name,
    /// Parameter spellings.
    pub parameters: Vec<Name>,
    /// Body.
    pub body: Block,
    /// Complete definition.
    pub span: Span,
}
/// Procedure definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Procedure {
    /// Procedure spelling.
    pub name: Name,
    /// Parameters, preserving defaults.
    pub parameters: Vec<Decl>,
    /// Local declarations.
    pub variables: Vec<Decl>,
    /// Body.
    pub body: Block,
    /// Complete definition.
    pub span: Span,
}
/// Process definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Process {
    /// Process spelling.
    pub name: Name,
    /// Explicit fairness.
    pub fairness: Fairness,
    /// Single id or set of ids.
    pub id: Binding,
    /// Local declarations.
    pub variables: Vec<Decl>,
    /// Body.
    pub body: Block,
    /// Complete definition.
    pub span: Span,
}
/// Algorithm body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    /// Sequential body.
    Sequential(Block),
    /// Process declarations, in source order.
    Processes(Vec<Process>),
}
/// Complete source algorithm; does not claim translation acceptance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Algorithm {
    /// Algorithm spelling.
    pub name: Name,
    /// Surface syntax.
    pub syntax: Syntax,
    /// Header fairness.
    pub fairness: Fairness,
    /// Version 1.5 sequential `fair` before `begin`; unlike `--fair`, this
    /// selects weak fairness without selecting the special fair-header CRC.
    #[serde(default)]
    pub legacy_fair: bool,
    /// Globals.
    pub variables: Vec<Decl>,
    /// Original define block.
    pub definitions: Option<Expr>,
    /// Macros.
    pub macros: Vec<Macro>,
    /// Procedures.
    pub procedures: Vec<Procedure>,
    /// Sequential or multiprocess body.
    pub body: Body,
    /// Complete consumed extent, including header marker.
    pub span: Span,
}
