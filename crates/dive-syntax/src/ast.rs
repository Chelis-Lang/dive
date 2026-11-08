//! Typed views over the concrete syntax tree.
//!
//! Each type wraps a [`SyntaxNode`] of one kind and offers accessors for its
//! parts. They are cheap to clone and never copy text. Accessors return
//! `Option` because a tree built after a syntax error may be incomplete.
//!
//! # Example
//!
//! ```
//! use dive_syntax::{parse_module, ast::{Expr, Unit}};
//!
//! let p = parse_module("---- MODULE M ----\nEXTENDS Naturals\nF(a, b) == a + b\n====");
//! let m = p.module().unwrap();
//! assert_eq!(m.extends(), ["Naturals"]);
//! let Some(Unit::OperatorDef(def)) = m.units().next() else { panic!() };
//! assert_eq!(def.name().as_deref(), Some("F"));
//! assert_eq!(def.parameters(), ["a", "b"]);
//! let Some(Expr::Infix(sum)) = def.body() else { panic!() };
//! assert_eq!(sum.operator().map(|t| t.text().to_string()).as_deref(), Some("+"));
//! ```

#[allow(
    clippy::enum_glob_use,
    reason = "the grammar names over a hundred token and node kinds"
)]
use crate::kind::SyntaxKind::{self, *};
use crate::kind::{SyntaxNode, SyntaxToken};

/// A typed wrapper around a [`SyntaxNode`] of a particular kind.
pub trait AstNode: Sized {
    /// Whether nodes of `kind` can be wrapped.
    fn can_cast(kind: SyntaxKind) -> bool;
    /// Wraps `node` if it has the right kind.
    fn cast(node: SyntaxNode) -> Option<Self>;
    /// The underlying node.
    fn syntax(&self) -> &SyntaxNode;
}

macro_rules! ast_node {
    ($(#[$m:meta])* $name:ident, $kind:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(SyntaxNode);

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }
            fn cast(node: SyntaxNode) -> Option<Self> {
                Self::can_cast(node.kind()).then(|| Self(node))
            }
            fn syntax(&self) -> &SyntaxNode {
                &self.0
            }
        }
    };
}

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

fn children<N: AstNode>(parent: &SyntaxNode) -> impl Iterator<Item = N> + use<N> {
    parent.children().filter_map(N::cast)
}

/// The non-trivia tokens directly under `parent`.
fn tokens(parent: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + use<> {
    parent
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|t| !t.kind().is_trivia())
}

fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(parent).find(|t| t.kind() == kind)
}

fn has_token(parent: &SyntaxNode, kind: SyntaxKind) -> bool {
    token(parent, kind).is_some()
}

fn ident_texts(parent: &SyntaxNode) -> Vec<String> {
    tokens(parent)
        .filter(|t| t.kind() == IDENT)
        .map(|t| t.text().to_owned())
        .collect()
}

fn first_expr(parent: &SyntaxNode) -> Option<Expr> {
    parent.children().find_map(Expr::cast)
}

fn last_expr(parent: &SyntaxNode) -> Option<Expr> {
    parent.children().filter_map(Expr::cast).last()
}

ast_node!(
    /// The whole file.
    SourceFile,
    SOURCE_FILE
);
ast_node!(
    /// A module (top-level or nested).
    Module,
    MODULE
);
ast_node!(
    /// `VARIABLE(S) x, y`.
    VariableDecl,
    VARIABLE_DECL
);
ast_node!(
    /// `CONSTANT(S) ...`.
    ConstantDecl,
    CONSTANT_DECL
);
ast_node!(
    /// `RECURSIVE ...`.
    RecursiveDecl,
    RECURSIVE_DECL
);
ast_node!(
    /// An operator definition.
    OperatorDef,
    OPERATOR_DEF
);
ast_node!(
    /// A function definition `f[x \in S] == e`.
    FunctionDef,
    FUNCTION_DEF
);
ast_node!(
    /// A module definition `I == INSTANCE M WITH ...`.
    ModuleDef,
    MODULE_DEF
);
ast_node!(
    /// `[LOCAL] INSTANCE M WITH ...` as a unit.
    Instance,
    INSTANCE
);
ast_node!(
    /// `INSTANCE M WITH ...`.
    Instantiation,
    INSTANTIATION
);
ast_node!(
    /// `x <- e`.
    Substitution,
    SUBSTITUTION
);
ast_node!(
    /// `ASSUME`/`ASSUMPTION`/`AXIOM`.
    Assumption,
    ASSUMPTION
);
ast_node!(
    /// `THEOREM`, `LEMMA`, `PROPOSITION`, `COROLLARY`.
    Theorem,
    THEOREM
);
ast_node!(
    /// `USE` or `HIDE`.
    UseOrHide,
    USE_OR_HIDE
);
ast_node!(
    /// A step-structured proof.
    Proof,
    PROOF
);
ast_node!(
    /// `BY ...`, `OBVIOUS` or `OMITTED`.
    TerminalProof,
    TERMINAL_PROOF
);
ast_node!(
    /// One proof step.
    ProofStep,
    PROOF_STEP
);
ast_node!(
    /// `ASSUME ... PROVE ...`.
    AssumeProve,
    ASSUME_PROVE
);
ast_node!(
    /// A declared constant or parameter `c` or `F(_, _)`.
    IdentDecl,
    IDENT_DECL
);
ast_node!(
    /// `x \in S` or `<<x, y>> \in S` in a quantifier or function.
    QuantBound,
    QUANT_BOUND
);
ast_node!(
    /// A possibly qualified name.
    GeneralId,
    GENERAL_ID
);
ast_node!(
    /// `Op(args)`.
    OpApplication,
    OP_APPLICATION
);
ast_node!(
    /// `a op b`.
    InfixExpr,
    INFIX_EXPR
);
ast_node!(
    /// `op e`.
    PrefixExpr,
    PREFIX_EXPR
);
ast_node!(
    /// `e op`.
    PostfixExpr,
    POSTFIX_EXPR
);

impl SourceFile {
    /// The top-level module.
    #[must_use]
    pub fn module(&self) -> Option<Module> {
        child(&self.0)
    }
}

/// A module-level unit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Unit {
    /// `VARIABLE ...`
    VariableDecl(VariableDecl),
    /// `CONSTANT ...`
    ConstantDecl(ConstantDecl),
    /// `RECURSIVE ...`
    RecursiveDecl(RecursiveDecl),
    /// An operator definition.
    OperatorDef(OperatorDef),
    /// A function definition.
    FunctionDef(FunctionDef),
    /// A module definition (`I == INSTANCE M`).
    ModuleDef(ModuleDef),
    /// `INSTANCE M`.
    Instance(Instance),
    /// An assumption.
    Assumption(Assumption),
    /// A theorem.
    Theorem(Theorem),
    /// `USE`/`HIDE`.
    UseOrHide(UseOrHide),
    /// A nested module.
    Module(Module),
    /// Tokens skipped after a syntax error.
    Error(SyntaxNode),
}

impl Unit {
    fn cast(node: SyntaxNode) -> Option<Self> {
        Some(match node.kind() {
            VARIABLE_DECL => Self::VariableDecl(VariableDecl(node)),
            CONSTANT_DECL => Self::ConstantDecl(ConstantDecl(node)),
            RECURSIVE_DECL => Self::RecursiveDecl(RecursiveDecl(node)),
            OPERATOR_DEF => Self::OperatorDef(OperatorDef(node)),
            FUNCTION_DEF => Self::FunctionDef(FunctionDef(node)),
            MODULE_DEF => Self::ModuleDef(ModuleDef(node)),
            INSTANCE => Self::Instance(Instance(node)),
            ASSUMPTION => Self::Assumption(Assumption(node)),
            THEOREM => Self::Theorem(Theorem(node)),
            USE_OR_HIDE => Self::UseOrHide(UseOrHide(node)),
            MODULE => Self::Module(Module(node)),
            ERROR => Self::Error(node),
            _ => return None,
        })
    }
}

impl Module {
    /// The module's name.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        let header = self.0.children().find(|n| n.kind() == MODULE_HEADER)?;
        token(&header, IDENT).map(|t| t.text().to_owned())
    }

    /// Names in the `EXTENDS` clause.
    #[must_use]
    pub fn extends(&self) -> Vec<String> {
        self.0
            .children()
            .find(|n| n.kind() == EXTENDS)
            .map_or_else(Vec::new, |e| ident_texts(&e))
    }

    /// The units of the module body, in order.
    pub fn units(&self) -> impl Iterator<Item = Unit> + use<> {
        self.0.children().filter_map(Unit::cast)
    }

    /// The operator definitions directly in this module.
    pub fn operator_definitions(&self) -> impl Iterator<Item = OperatorDef> + use<> {
        children(&self.0)
    }

    /// Whether the closing `====` is present.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        has_token(&self.0, END_MODULE)
    }
}

impl VariableDecl {
    /// The declared variable names.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        ident_texts(&self.0)
    }
}

impl ConstantDecl {
    /// The declared items.
    pub fn items(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
        self.0.children()
    }
}

impl RecursiveDecl {
    /// The declared items.
    pub fn items(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
        self.0.children()
    }
}

impl IdentDecl {
    /// The declared name.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        token(&self.0, IDENT).map(|t| t.text().to_owned())
    }

    /// The number of `_` arguments.
    #[must_use]
    pub fn arity(&self) -> usize {
        tokens(&self.0).filter(|t| t.kind() == UNDERSCORE).count()
    }
}

/// The text of the operator or name defined by a left-hand side node.
fn lhs_name(lhs: &SyntaxNode) -> Option<String> {
    match lhs.kind() {
        IDENT_LHS => token(lhs, IDENT).map(|t| t.text().to_owned()),
        PREFIX_LHS => tokens(lhs).next().map(|t| t.text().to_owned()),
        INFIX_LHS | POSTFIX_LHS => tokens(lhs).nth(1).map(|t| t.text().to_owned()),
        _ => None,
    }
}

fn find_lhs(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .find(|n| matches!(n.kind(), IDENT_LHS | PREFIX_LHS | INFIX_LHS | POSTFIX_LHS))
}

impl OperatorDef {
    /// The defined name or operator symbol.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        find_lhs(&self.0).and_then(|l| lhs_name(&l))
    }

    /// The left-hand side node (`IDENT_LHS`, `PREFIX_LHS`, `INFIX_LHS` or `POSTFIX_LHS`).
    #[must_use]
    pub fn lhs(&self) -> Option<SyntaxNode> {
        find_lhs(&self.0)
    }

    /// Parameter names (identifiers and operator symbols).
    #[must_use]
    pub fn parameters(&self) -> Vec<String> {
        let Some(lhs) = find_lhs(&self.0) else {
            return Vec::new();
        };
        match lhs.kind() {
            IDENT_LHS => lhs
                .children()
                .filter_map(|p| match p.kind() {
                    IDENT_DECL => token(&p, IDENT).map(|t| t.text().to_owned()),
                    PREFIX_DECL => tokens(&p).next().map(|t| t.text().to_owned()),
                    INFIX_DECL | POSTFIX_DECL => tokens(&p).nth(1).map(|t| t.text().to_owned()),
                    _ => None,
                })
                .collect(),
            PREFIX_LHS => tokens(&lhs)
                .nth(1)
                .map(|t| t.text().to_owned())
                .into_iter()
                .collect(),
            INFIX_LHS => {
                let t: Vec<SyntaxToken> = tokens(&lhs).collect();
                [t.first(), t.get(2)]
                    .into_iter()
                    .flatten()
                    .map(|t| t.text().to_owned())
                    .collect()
            }
            _ => tokens(&lhs)
                .next()
                .map(|t| t.text().to_owned())
                .into_iter()
                .collect(),
        }
    }

    /// Whether the definition is `LOCAL`.
    #[must_use]
    pub fn is_local(&self) -> bool {
        has_token(&self.0, LOCAL_KW)
    }

    /// The defining expression.
    #[must_use]
    pub fn body(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}

impl FunctionDef {
    /// The function name.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        token(&self.0, IDENT).map(|t| t.text().to_owned())
    }

    /// The bound variables.
    pub fn bounds(&self) -> impl Iterator<Item = QuantBound> + use<> {
        children(&self.0)
    }

    /// The defining expression.
    #[must_use]
    pub fn body(&self) -> Option<Expr> {
        last_expr(&self.0)
    }

    /// Whether the definition is `LOCAL`.
    #[must_use]
    pub fn is_local(&self) -> bool {
        has_token(&self.0, LOCAL_KW)
    }
}

impl ModuleDef {
    /// The defined name.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        find_lhs(&self.0).and_then(|l| lhs_name(&l))
    }

    /// The instantiation.
    #[must_use]
    pub fn instantiation(&self) -> Option<Instantiation> {
        child(&self.0)
    }
}

impl Instance {
    /// Whether the instance is `LOCAL`.
    #[must_use]
    pub fn is_local(&self) -> bool {
        has_token(&self.0, LOCAL_KW)
    }

    /// The instantiation.
    #[must_use]
    pub fn instantiation(&self) -> Option<Instantiation> {
        child(&self.0)
    }
}

impl Instantiation {
    /// The instantiated module's name.
    #[must_use]
    pub fn module_name(&self) -> Option<String> {
        token(&self.0, IDENT).map(|t| t.text().to_owned())
    }

    /// The `WITH` substitutions.
    pub fn substitutions(&self) -> impl Iterator<Item = Substitution> + use<> {
        children(&self.0)
    }
}

impl Substitution {
    /// The substituted name or operator symbol.
    #[must_use]
    pub fn target(&self) -> Option<SyntaxToken> {
        tokens(&self.0).next()
    }

    /// The replacement (an expression, `OP_REF` or `LAMBDA` node).
    #[must_use]
    pub fn replacement(&self) -> Option<SyntaxNode> {
        self.0.children().last()
    }
}

impl Assumption {
    /// The assumption's name, if any.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        has_token(&self.0, DEF_EQ)
            .then(|| token(&self.0, IDENT).map(|t| t.text().to_owned()))
            .flatten()
    }

    /// The assumed expression.
    #[must_use]
    pub fn expr(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}

impl Theorem {
    /// The theorem's name, if any.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        has_token(&self.0, DEF_EQ)
            .then(|| token(&self.0, IDENT).map(|t| t.text().to_owned()))
            .flatten()
    }

    /// The statement: an `ASSUME_PROVE` (or labelled one) or an expression.
    #[must_use]
    pub fn statement(&self) -> Option<SyntaxNode> {
        self.0
            .children()
            .find(|n| n.kind() != PROOF && n.kind() != TERMINAL_PROOF)
    }

    /// The proof, if any (`PROOF` or `TERMINAL_PROOF` node).
    #[must_use]
    pub fn proof(&self) -> Option<SyntaxNode> {
        self.0
            .children()
            .find(|n| matches!(n.kind(), PROOF | TERMINAL_PROOF))
    }
}

impl Proof {
    /// The steps, the last of which is the `QED` step.
    pub fn steps(&self) -> impl Iterator<Item = ProofStep> + use<> {
        children(&self.0)
    }
}

impl ProofStep {
    /// The step number token, e.g. `<1>2.`.
    #[must_use]
    pub fn step_token(&self) -> Option<SyntaxToken> {
        tokens(&self.0).next()
    }

    /// The step body node.
    #[must_use]
    pub fn body(&self) -> Option<SyntaxNode> {
        self.0
            .children()
            .find(|n| !matches!(n.kind(), PROOF | TERMINAL_PROOF))
    }

    /// The step's proof, if any.
    #[must_use]
    pub fn proof(&self) -> Option<SyntaxNode> {
        self.0
            .children()
            .find(|n| matches!(n.kind(), PROOF | TERMINAL_PROOF))
    }
}

impl QuantBound {
    /// The bound names (also those inside a `<<x, y>>` tuple).
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut v = ident_texts(&self.0);
        if let Some(t) = self.0.children().find(|n| n.kind() == IDENT_TUPLE) {
            v.extend(ident_texts(&t));
        }
        v
    }

    /// Whether the names form a tuple pattern.
    #[must_use]
    pub fn is_tuple(&self) -> bool {
        self.0.children().any(|n| n.kind() == IDENT_TUPLE)
    }

    /// The bounding set.
    #[must_use]
    pub fn set(&self) -> Option<Expr> {
        first_expr(&self.0)
    }
}

/// Defines a wrapper accepting several node kinds.
macro_rules! ast_node_multi {
    ($(#[$m:meta])* $name:ident, [$($kind:ident),+]) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(SyntaxNode);

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, $($kind)|+)
            }
            fn cast(node: SyntaxNode) -> Option<Self> {
                Self::can_cast(node.kind()).then(|| Self(node))
            }
            fn syntax(&self) -> &SyntaxNode {
                &self.0
            }
        }
    };
}

ast_node!(
    /// `LET defs IN e`.
    LetExpr,
    LET_EXPR
);
ast_node!(
    /// `IF c THEN a ELSE b`.
    IfExpr,
    IF_EXPR
);
ast_node!(
    /// `CASE p -> e [] ... [] OTHER -> e`.
    CaseExpr,
    CASE_EXPR
);
ast_node!(
    /// `CHOOSE x \in S : P`.
    ChooseExpr,
    CHOOSE
);
ast_node!(
    /// `f[a, b]`.
    FcnAppl,
    FCN_APPL
);
ast_node!(
    /// `r.field`.
    RecordField,
    RECORD_FIELD
);
ast_node!(
    /// `lab :: e`.
    LabelExpr,
    LABEL
);
ast_node!(
    /// `LAMBDA x, y : e` (an operator argument, not an expression).
    Lambda,
    LAMBDA
);
ast_node_multi!(
    /// `\A`, `\E` (bounded or not), `\AA` and `\EE`.
    QuantExpr,
    [BOUNDED_QUANT, UNBOUNDED_QUANT, TEMPORAL_QUANT]
);
ast_node_multi!(
    /// A bulleted `/\` or `\/` list.
    JunctionList,
    [CONJ_LIST, DISJ_LIST]
);

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    /// A name such as `x` or `M!Op`.
    Name(GeneralId),
    /// `Op(args)`.
    Apply(OpApplication),
    /// `a op b`.
    Infix(InfixExpr),
    /// `op e`.
    Prefix(PrefixExpr),
    /// `e op`.
    Postfix(PostfixExpr),
    /// `LET ... IN e`.
    Let(LetExpr),
    /// `IF ... THEN ... ELSE ...`.
    If(IfExpr),
    /// `CASE ...`.
    Case(CaseExpr),
    /// A quantifier.
    Quant(QuantExpr),
    /// `CHOOSE ...`.
    Choose(ChooseExpr),
    /// A bulleted conjunction or disjunction list.
    Junction(JunctionList),
    /// `f[args]`.
    FcnAppl(FcnAppl),
    /// `r.field`.
    Field(RecordField),
    /// `lab :: e`.
    Label(LabelExpr),
    /// Any other expression form; see [`Expr::syntax`] and its kind.
    Other(SyntaxNode),
}

impl Expr {
    /// Whether nodes of `kind` are expressions.
    #[must_use]
    pub fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            GENERAL_ID
                | OP_APPLICATION
                | NUMBER_LIT
                | DECIMAL_LIT
                | STRING_LIT
                | PREFIX_EXPR
                | INFIX_EXPR
                | POSTFIX_EXPR
                | CARTESIAN_PRODUCT
                | PAREN_EXPR
                | RECORD_FIELD
                | FCN_APPL
                | TUPLE
                | SET_ENUM
                | SET_FILTER
                | SET_MAP
                | FCN_CONSTRUCTOR
                | SET_OF_FCNS
                | RECORD
                | SET_OF_RECORDS
                | EXCEPT
                | ACTION_EXPR
                | FAIRNESS_EXPR
                | IF_EXPR
                | CASE_EXPR
                | LET_EXPR
                | CONJ_LIST
                | DISJ_LIST
                | BOUNDED_QUANT
                | UNBOUNDED_QUANT
                | TEMPORAL_QUANT
                | CHOOSE
                | LABEL
        )
    }

    /// Wraps an expression node.
    #[must_use]
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        Some(match node.kind() {
            GENERAL_ID => Self::Name(GeneralId(node)),
            OP_APPLICATION => Self::Apply(OpApplication(node)),
            INFIX_EXPR => Self::Infix(InfixExpr(node)),
            PREFIX_EXPR => Self::Prefix(PrefixExpr(node)),
            POSTFIX_EXPR => Self::Postfix(PostfixExpr(node)),
            LET_EXPR => Self::Let(LetExpr(node)),
            IF_EXPR => Self::If(IfExpr(node)),
            CASE_EXPR => Self::Case(CaseExpr(node)),
            BOUNDED_QUANT | UNBOUNDED_QUANT | TEMPORAL_QUANT => Self::Quant(QuantExpr(node)),
            CHOOSE => Self::Choose(ChooseExpr(node)),
            CONJ_LIST | DISJ_LIST => Self::Junction(JunctionList(node)),
            FCN_APPL => Self::FcnAppl(FcnAppl(node)),
            RECORD_FIELD => Self::Field(RecordField(node)),
            LABEL => Self::Label(LabelExpr(node)),
            k if Self::can_cast(k) => Self::Other(node),
            _ => return None,
        })
    }

    /// The underlying node.
    #[must_use]
    pub fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Name(n) => &n.0,
            Self::Apply(n) => &n.0,
            Self::Infix(n) => &n.0,
            Self::Prefix(n) => &n.0,
            Self::Postfix(n) => &n.0,
            Self::Let(n) => &n.0,
            Self::If(n) => &n.0,
            Self::Case(n) => &n.0,
            Self::Quant(n) => &n.0,
            Self::Choose(n) => &n.0,
            Self::Junction(n) => &n.0,
            Self::FcnAppl(n) => &n.0,
            Self::Field(n) => &n.0,
            Self::Label(n) => &n.0,
            Self::Other(n) => n,
        }
    }
}

fn exprs(n: &SyntaxNode) -> Vec<Expr> {
    n.children().filter_map(Expr::cast).collect()
}

impl LetExpr {
    /// The definitions (operator, function and module definitions, and
    /// `RECURSIVE` declarations).
    #[must_use]
    pub fn definitions(&self) -> Vec<Unit> {
        self.0
            .children()
            .find(|n| n.kind() == LET_DEFS)
            .map_or_else(Vec::new, |d| d.children().filter_map(Unit::cast).collect())
    }

    /// The expression after `IN`.
    #[must_use]
    pub fn body(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}

impl IfExpr {
    /// The condition.
    #[must_use]
    pub fn condition(&self) -> Option<Expr> {
        exprs(&self.0).into_iter().next()
    }

    /// The `THEN` branch.
    #[must_use]
    pub fn then_branch(&self) -> Option<Expr> {
        exprs(&self.0).into_iter().nth(1)
    }

    /// The `ELSE` branch.
    #[must_use]
    pub fn else_branch(&self) -> Option<Expr> {
        exprs(&self.0).into_iter().nth(2)
    }
}

impl CaseExpr {
    /// The `guard -> value` arms, without `OTHER`.
    #[must_use]
    pub fn arms(&self) -> Vec<(Option<Expr>, Option<Expr>)> {
        self.0
            .children()
            .filter(|n| n.kind() == CASE_ARM)
            .map(|a| {
                let mut e = exprs(&a).into_iter();
                (e.next(), e.next())
            })
            .collect()
    }

    /// The `OTHER` value, if present.
    #[must_use]
    pub fn other(&self) -> Option<Expr> {
        self.0
            .children()
            .find(|n| n.kind() == OTHER_ARM)
            .and_then(|o| first_expr(&o))
    }
}

impl QuantExpr {
    /// The quantifier token: `\A`, `\E`, `\AA`, `\EE` or a Unicode spelling.
    #[must_use]
    pub fn quantifier(&self) -> Option<SyntaxToken> {
        tokens(&self.0).next()
    }

    /// Whether this is `\A`/`\AA` (otherwise `\E`/`\EE`).
    #[must_use]
    pub fn is_forall(&self) -> bool {
        self.quantifier()
            .is_some_and(|t| matches!(t.kind(), FORALL_KW | TEMPORAL_FORALL_KW))
    }

    /// Whether this is a temporal quantifier `\AA`/`\EE`.
    #[must_use]
    pub fn is_temporal(&self) -> bool {
        self.0.kind() == TEMPORAL_QUANT
    }

    /// The bounds, for `\A x \in S : P`.
    pub fn bounds(&self) -> impl Iterator<Item = QuantBound> + use<> {
        children(&self.0)
    }

    /// The unbounded names, for `\A x, y : P`.
    #[must_use]
    pub fn unbounded_names(&self) -> Vec<String> {
        ident_texts(&self.0)
    }

    /// The quantified expression.
    #[must_use]
    pub fn body(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}

impl ChooseExpr {
    /// The bound name, or the names of a `<<x, y>>` pattern.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut v = ident_texts(&self.0);
        if let Some(t) = self.0.children().find(|n| n.kind() == IDENT_TUPLE) {
            v.extend(ident_texts(&t));
        }
        v
    }

    /// The bounding set, for `CHOOSE x \in S : P`.
    #[must_use]
    pub fn set(&self) -> Option<Expr> {
        has_token(&self.0, IN_OP)
            .then(|| exprs(&self.0).into_iter().next())
            .flatten()
    }

    /// The condition.
    #[must_use]
    pub fn body(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}

impl JunctionList {
    /// Whether this is a `/\` list (otherwise `\/`).
    #[must_use]
    pub fn is_conjunction(&self) -> bool {
        self.0.kind() == CONJ_LIST
    }

    /// The items' expressions, in order.
    #[must_use]
    pub fn items(&self) -> Vec<Expr> {
        self.0
            .children()
            .filter_map(|item| first_expr(&item))
            .collect()
    }
}

impl FcnAppl {
    /// The function. `None` for SANY's bare `[a, b]` form, which semantic
    /// analysis rejects.
    #[must_use]
    pub fn function(&self) -> Option<Expr> {
        let first = self
            .0
            .children_with_tokens()
            .find(|e| !e.kind().is_trivia())?;
        first.into_node().and_then(Expr::cast)
    }

    /// The arguments inside the brackets.
    #[must_use]
    pub fn arguments(&self) -> Vec<Expr> {
        let mut seen_bracket = false;
        let mut out = Vec::new();
        for e in self.0.children_with_tokens() {
            match e {
                rowan::NodeOrToken::Token(t) if t.kind() == L_BRACK => seen_bracket = true,
                rowan::NodeOrToken::Node(n) if seen_bracket => out.extend(Expr::cast(n)),
                _ => {}
            }
        }
        out
    }
}

impl RecordField {
    /// The record expression.
    #[must_use]
    pub fn record(&self) -> Option<Expr> {
        first_expr(&self.0)
    }

    /// The field name.
    #[must_use]
    pub fn field(&self) -> Option<String> {
        token(&self.0, IDENT).map(|t| t.text().to_owned())
    }
}

impl LabelExpr {
    /// The label's name.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        let first = self.0.children().next()?;
        let id = if first.kind() == OP_APPLICATION {
            first.children().find(|n| n.kind() == GENERAL_ID)?
        } else {
            first
        };
        GeneralId::cast(id).and_then(|g| g.name())
    }

    /// The labelled expression (or `ASSUME_PROVE`).
    #[must_use]
    pub fn body(&self) -> Option<SyntaxNode> {
        self.0.children().nth(1)
    }
}

impl Lambda {
    /// The parameter names.
    #[must_use]
    pub fn parameters(&self) -> Vec<String> {
        ident_texts(&self.0)
    }

    /// The body.
    #[must_use]
    pub fn body(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}

impl GeneralId {
    /// The final name part as text (an identifier, operator, or selector).
    #[must_use]
    pub fn name(&self) -> Option<String> {
        self.0
            .children_with_tokens()
            .filter(|e| e.kind() != ID_PREFIX && !e.kind().is_trivia())
            .last()
            .map(|e| e.to_string())
    }
}

impl OpApplication {
    /// The applied operator's name.
    #[must_use]
    pub fn operator(&self) -> Option<GeneralId> {
        child(&self.0)
    }

    /// The argument nodes (expressions, `OP_REF` or `LAMBDA`).
    #[must_use]
    pub fn arguments(&self) -> Vec<SyntaxNode> {
        self.0
            .children()
            .find(|n| n.kind() == OP_ARGS)
            .map_or_else(Vec::new, |a| a.children().collect())
    }
}

fn operator_token(n: &SyntaxNode) -> Option<SyntaxToken> {
    tokens(n).find(|t| t.kind().is_operator())
}

impl InfixExpr {
    /// The left operand.
    #[must_use]
    pub fn lhs(&self) -> Option<Expr> {
        first_expr(&self.0)
    }

    /// The operator token.
    #[must_use]
    pub fn operator(&self) -> Option<SyntaxToken> {
        operator_token(&self.0)
    }

    /// The right operand.
    #[must_use]
    pub fn rhs(&self) -> Option<Expr> {
        self.0.children().filter_map(Expr::cast).nth(1)
    }
}

impl PrefixExpr {
    /// The operator token.
    #[must_use]
    pub fn operator(&self) -> Option<SyntaxToken> {
        operator_token(&self.0)
    }

    /// The operand.
    #[must_use]
    pub fn operand(&self) -> Option<Expr> {
        first_expr(&self.0)
    }
}

impl PostfixExpr {
    /// The operand.
    #[must_use]
    pub fn operand(&self) -> Option<Expr> {
        first_expr(&self.0)
    }

    /// The operator token.
    #[must_use]
    pub fn operator(&self) -> Option<SyntaxToken> {
        operator_token(&self.0)
    }
}

impl AssumeProve {
    /// The `ASSUME` items (expressions, `NEW_DECL`s or nested `ASSUME_PROVE`s).
    pub fn assumptions(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
        let prove_seen = std::cell::Cell::new(false);
        let node = self.0.clone();
        let mut out = Vec::new();
        for e in node.children_with_tokens() {
            match e {
                rowan::NodeOrToken::Token(t) if matches!(t.kind(), PROVE_KW | BOX_PROVE_KW) => {
                    prove_seen.set(true);
                }
                rowan::NodeOrToken::Node(n) if !prove_seen.get() => out.push(n),
                _ => {}
            }
        }
        out.into_iter()
    }

    /// The `PROVE` expression.
    #[must_use]
    pub fn goal(&self) -> Option<Expr> {
        last_expr(&self.0)
    }
}
