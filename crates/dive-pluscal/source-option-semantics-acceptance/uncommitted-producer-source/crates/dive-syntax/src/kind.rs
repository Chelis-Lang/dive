//! Token and node kinds of the concrete syntax tree.

/// Defines [`SyntaxKind`] and the table used to convert raw `u16` values back.
macro_rules! syntax_kinds {
    ($($(#[doc = $doc:literal])* $name:ident),* $(,)?) => {
        /// The kind of a token or node in the concrete syntax tree.
        ///
        /// Token kinds come first, then node kinds. The numeric values are an
        /// implementation detail and are not stable across versions.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        #[allow(non_camel_case_types, reason = "rust-analyzer style SCREAMING_CASE kinds")]
        pub enum SyntaxKind {
            $($(#[doc = $doc])* $name),*
        }

        impl SyntaxKind {
            /// Every kind, indexed by its raw value.
            const ALL: &'static [SyntaxKind] = &[$(SyntaxKind::$name),*];

            /// The kind's name, for tree dumps and JSON output.
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $(SyntaxKind::$name => stringify!($name)),*
                }
            }
        }
    };
}

syntax_kinds! {
    // ---- Trivia -------------------------------------------------------------
    /// Spaces, tabs, carriage returns and line feeds.
    WHITESPACE,
    /// A `\*` comment, up to but excluding the line terminator.
    LINE_COMMENT,
    /// A `(* ... *)` comment; these nest. PlusCal algorithms live here.
    BLOCK_COMMENT,
    /// Text before the first module header, which SANY ignores.
    PRELUDE,
    /// Text after the end of the top-level module, which SANY ignores.
    TRAILER,

    // ---- Module structure tokens -------------------------------------------
    /// `---- MODULE`, the start of a module (dashes, spaces and the keyword).
    BEGIN_MODULE,
    /// A line of four or more dashes.
    SEPARATOR,
    /// A line of four or more equals signs, ending a module.
    END_MODULE,
    /// `--->`, the start of a SANY pragma before the module.
    PRAGMA_START,

    // ---- Keywords -----------------------------------------------------------
    /// `ACTION` or `ACTIONS`.
    ACTION_KW,
    /// `ASSUME`.
    ASSUME_KW,
    /// `[]ASSUME`.
    BOX_ASSUME_KW,
    /// `ASSUMPTION` or `AXIOM`.
    ASSUMPTION_KW,
    /// `CASE`.
    CASE_KW,
    /// `CHOOSE`.
    CHOOSE_KW,
    /// `CONSTANT` or `CONSTANTS`.
    CONSTANT_KW,
    /// `ELSE`.
    ELSE_KW,
    /// `EXCEPT`.
    EXCEPT_KW,
    /// `\E`, `\exists` or `∃`.
    EXISTS_KW,
    /// `EXTENDS`.
    EXTENDS_KW,
    /// `\A`, `\forall` or `∀`.
    FORALL_KW,
    /// `IF`.
    IF_KW,
    /// `INSTANCE`.
    INSTANCE_KW,
    /// `LET`.
    LET_KW,
    /// `IN` (of `LET ... IN`).
    IN_KW,
    /// `LOCAL`.
    LOCAL_KW,
    /// `MODULE` (in `USE MODULE M` and similar).
    MODULE_KW,
    /// `NEW`.
    NEW_KW,
    /// `OTHER`.
    OTHER_KW,
    /// `PROPOSITION`, `LEMMA` or `COROLLARY`.
    PROPOSITION_KW,
    /// `SF_`.
    SF_KW,
    /// `\EE`.
    TEMPORAL_EXISTS_KW,
    /// `\AA`.
    TEMPORAL_FORALL_KW,
    /// `THEN`.
    THEN_KW,
    /// `BY`.
    BY_KW,
    /// `ONLY`.
    ONLY_KW,
    /// `DEFINE`.
    DEFINE_KW,
    /// `DEF` or `DEFS`.
    DEF_KW,
    /// `THEOREM`.
    THEOREM_KW,
    /// `USE`.
    USE_KW,
    /// `HIDE`.
    HIDE_KW,
    /// `HAVE`.
    HAVE_KW,
    /// `OBVIOUS`.
    OBVIOUS_KW,
    /// `OMITTED`.
    OMITTED_KW,
    /// `LAMBDA`.
    LAMBDA_KW,
    /// `TAKE`.
    TAKE_KW,
    /// `PROOF`.
    PROOF_KW,
    /// `PROVE`.
    PROVE_KW,
    /// `[]PROVE`.
    BOX_PROVE_KW,
    /// `QED`.
    QED_KW,
    /// `RECURSIVE`.
    RECURSIVE_KW,
    /// `STATE`.
    STATE_KW,
    /// `TEMPORAL` or `TEMPORALS`.
    TEMPORAL_KW,
    /// `PICK`.
    PICK_KW,
    /// `WITNESS`.
    WITNESS_KW,
    /// `SUFFICES`.
    SUFFICES_KW,
    /// `VARIABLE` or `VARIABLES`.
    VARIABLE_KW,
    /// `WF_`.
    WF_KW,
    /// `WITH`.
    WITH_KW,

    // ---- Punctuation --------------------------------------------------------
    /// `,`.
    COMMA,
    /// `:`.
    COLON,
    /// `::` or `∷`.
    COLON_COLON,
    /// `.`.
    DOT,
    /// `_`.
    UNDERSCORE,
    /// `==` or `≜`.
    DEF_EQ,
    /// `(`.
    L_PAREN,
    /// `)`.
    R_PAREN,
    /// The literal `-|-`, which SANY lexes as its internal definition marker.
    DEFBREAK_LIT,
    /// `[`.
    L_BRACK,
    /// `]_`.
    R_BRACK_UNDERSCORE,
    /// `]`.
    R_BRACK,
    /// `{`.
    L_BRACE,
    /// `}`.
    R_BRACE,
    /// `<<` or `⟨`.
    L_ANGLE,
    /// `>>_` or `⟩_`.
    R_ANGLE_UNDERSCORE,
    /// `>>` or `⟩`.
    R_ANGLE,
    /// `!`.
    BANG,
    /// `->` or `→`.
    ARROW,
    /// `<-` or `←`.
    LEFT_ARROW,
    /// `|->` or `↦`.
    MAPS_TO,

    // ---- Literals and names -------------------------------------------------
    /// A natural number: decimal digits, or `\b`, `\o`, `\h` forms.
    NUMBER,
    /// A string literal with its quotes.
    STRING,
    /// An identifier, `@`, or one of `ℕ ℤ ℝ`.
    IDENT,
    /// SANY's `x./\` and `x.\/` lexemes, which no production accepts.
    BAD_JUNCTION,
    /// A lexical error: a character or string no token matches.
    LEX_ERROR,

    // ---- Proof step numbers -------------------------------------------------
    /// `<1>2`: a numbered step name.
    STEP_NUMBER,
    /// `<*>a` or `<+>a`: a step name with an implicit level.
    STEP_IMPLICIT,
    /// `<1>2.`: a step start followed by dots.
    STEP_DOT,
    /// `<1>`, `<*>` or `<+>`: a bare level.
    STEP_BARE,
    /// `<1>.`, `<1>*` or `<1>-`: an unnumbered step start.
    STEP_UNNUMBERED,

    // ---- Operators ----------------------------------------------------------
    /// A prefix operator: `~ \lnot \neg ¬ ENABLED UNCHANGED SUBSET UNION DOMAIN`.
    PREFIX_OP,
    /// `[]` or `□`: the always operator and the `CASE` separator.
    BOX_OP,
    /// `<>` or `◇`.
    DIAMOND_OP,
    /// `-.`, the name of prefix minus.
    NEG_DOT_OP,
    /// A postfix operator: `' ^+ ⁺ ^* ^#`.
    POSTFIX_OP,
    /// An infix operator other than the specially handled ones below.
    INFIX_OP,
    /// `/\` or `∧` (also a conjunction-list bullet).
    AND_OP,
    /// `\/` or `∨` (also a disjunction-list bullet).
    OR_OP,
    /// `\in` or `∈`.
    IN_OP,
    /// `=`.
    EQ_OP,
    /// `-` (infix minus, and prefix minus in expressions).
    MINUS_OP,
    /// End of input (never appears in the tree).
    EOF,

    // ---- Nodes: module level -----------------------------------------------
    /// The root: prelude, top-level module and trailer.
    SOURCE_FILE,
    /// A module, top-level or nested.
    MODULE,
    /// `---- MODULE Name ----`.
    MODULE_HEADER,
    /// The `EXTENDS` clause.
    EXTENDS,
    /// SANY's pragma before a module.
    PRAGMA,
    /// `VARIABLE(S) x, y`.
    VARIABLE_DECL,
    /// `CONSTANT(S) ...`.
    CONSTANT_DECL,
    /// `RECURSIVE ...`.
    RECURSIVE_DECL,
    /// `Id` or `Id(_, _)` in a declaration.
    IDENT_DECL,
    /// `-. _` or `~ _` in a declaration.
    PREFIX_DECL,
    /// `_ + _` in a declaration.
    INFIX_DECL,
    /// `_ ^+` in a declaration.
    POSTFIX_DECL,
    /// `Op(params) == expr` and the other operator definition forms.
    OPERATOR_DEF,
    /// `f[x \in S] == expr`.
    FUNCTION_DEF,
    /// `I == INSTANCE M WITH ...`.
    MODULE_DEF,
    /// The left-hand side `Id` or `Id(params)`.
    IDENT_LHS,
    /// The left-hand side `op x` of a prefix operator definition.
    PREFIX_LHS,
    /// The left-hand side `x op y` of an infix operator definition.
    INFIX_LHS,
    /// The left-hand side `x op` of a postfix operator definition.
    POSTFIX_LHS,
    /// `[LOCAL] INSTANCE M WITH ...` as a module unit.
    INSTANCE,
    /// `INSTANCE M WITH ...`.
    INSTANTIATION,
    /// `x <- expr` in an `INSTANCE ... WITH`.
    SUBSTITUTION,
    /// `ASSUME` / `ASSUMPTION` / `AXIOM` unit.
    ASSUMPTION,
    /// `THEOREM` / `LEMMA` / `PROPOSITION` / `COROLLARY` unit, with its proof.
    THEOREM,
    /// `USE` or `HIDE` (also a proof step body).
    USE_OR_HIDE,
    /// `MODULE M` inside `USE`, `HIDE`, `BY` or `DEF` lists.
    MODULE_REF,

    // ---- Nodes: proofs ------------------------------------------------------
    /// A non-terminal proof: optional `PROOF` then steps ending in `QED`.
    PROOF,
    /// `BY ...`, `OBVIOUS` or `OMITTED`, optionally after `PROOF`.
    TERMINAL_PROOF,
    /// A proof step: step start token, body, optional proof.
    PROOF_STEP,
    /// `QED`.
    QED_STEP,
    /// `DEFINE` and/or definitions as a step.
    DEF_STEP,
    /// `HAVE expr`.
    HAVE_STEP,
    /// `TAKE ...`.
    TAKE_STEP,
    /// `WITNESS ...`.
    WITNESS_STEP,
    /// `PICK ... : expr`.
    PICK_STEP,
    /// `CASE expr` as a step.
    CASE_STEP,
    /// An assertion step: optional `SUFFICES`, then an expression or `ASSUME/PROVE`.
    ASSERT_STEP,
    /// `ASSUME ... PROVE ...`.
    ASSUME_PROVE,
    /// `NEW x \in S`, `CONSTANT c`, `VARIABLE v`, `STATE f` and similar.
    NEW_DECL,

    // ---- Nodes: expressions -------------------------------------------------
    /// A possibly qualified name such as `x`, `M!Op` or `Op!1!<<`.
    GENERAL_ID,
    /// The `M!N!` qualification of a [`SyntaxKind::GENERAL_ID`].
    ID_PREFIX,
    /// One `name[(args)]!` element of an [`SyntaxKind::ID_PREFIX`].
    ID_PREFIX_ELEMENT,
    /// A structural subexpression selector `<<`, `>>`, `:`, `@` or a number.
    STRUCT_OP,
    /// `Op(args)`.
    OP_APPLICATION,
    /// `(arg, ...)`.
    OP_ARGS,
    /// A bare operator symbol passed as an argument or substituted: `F(+)`.
    OP_REF,
    /// A natural number literal.
    NUMBER_LIT,
    /// A decimal literal `1.5`.
    DECIMAL_LIT,
    /// A string literal.
    STRING_LIT,
    /// `op e` for a prefix operator.
    PREFIX_EXPR,
    /// `a op b`.
    INFIX_EXPR,
    /// `e op` for a postfix operator.
    POSTFIX_EXPR,
    /// `A \X B \X C` (n-ary Cartesian product).
    CARTESIAN_PRODUCT,
    /// `(e)`.
    PAREN_EXPR,
    /// `r.field`.
    RECORD_FIELD,
    /// `f[a, b]`.
    FCN_APPL,
    /// `<<a, b>>`.
    TUPLE,
    /// `{a, b}`.
    SET_ENUM,
    /// `{x \in S : P}`.
    SET_FILTER,
    /// `{e : x \in S}`.
    SET_MAP,
    /// `[x \in S |-> e]`.
    FCN_CONSTRUCTOR,
    /// `[S -> T]`.
    SET_OF_FCNS,
    /// `[a |-> 1, b |-> 2]`.
    RECORD,
    /// `a |-> 1` inside a record.
    FIELD_VAL,
    /// `[a : S, b : T]`.
    SET_OF_RECORDS,
    /// `a : S` inside a set of records.
    FIELD_SET,
    /// `[f EXCEPT ![x] = y]`.
    EXCEPT,
    /// `![x].y = e` inside `EXCEPT`.
    EXCEPT_SPEC,
    /// `[x]` or `.y` inside an [`SyntaxKind::EXCEPT_SPEC`].
    EXCEPT_COMPONENT,
    /// `[A]_v` or `<<A>>_v`.
    ACTION_EXPR,
    /// `WF_v(A)` or `SF_v(A)`.
    FAIRNESS_EXPR,
    /// `IF c THEN a ELSE b`.
    IF_EXPR,
    /// `CASE ...`.
    CASE_EXPR,
    /// `p -> e` inside `CASE`.
    CASE_ARM,
    /// `OTHER -> e` inside `CASE`.
    OTHER_ARM,
    /// `LET ... IN e`.
    LET_EXPR,
    /// The definitions of a `LET`.
    LET_DEFS,
    /// A bulleted `/\` list.
    CONJ_LIST,
    /// One `/\ e` item.
    CONJ_ITEM,
    /// A bulleted `\/` list.
    DISJ_LIST,
    /// One `\/ e` item.
    DISJ_ITEM,
    /// `\A x \in S : P` and `\E x \in S : P`.
    BOUNDED_QUANT,
    /// `\A x : P` and `\E x : P`.
    UNBOUNDED_QUANT,
    /// `\AA x : P` and `\EE x : P`.
    TEMPORAL_QUANT,
    /// `x, y \in S` or `<<x, y>> \in S`.
    QUANT_BOUND,
    /// `<<x, y>>` as a bound variable tuple.
    IDENT_TUPLE,
    /// `CHOOSE x \in S : P`.
    CHOOSE,
    /// `LAMBDA x, y : e`.
    LAMBDA,
    /// `lab :: e` (also labels on `ASSUME/PROVE`).
    LABEL,
    /// Tokens skipped while recovering from a syntax error.
    ERROR,
}

impl SyntaxKind {
    /// Converts a raw value produced by [`SyntaxKind::to_raw`] back to a kind.
    #[must_use]
    pub fn from_raw(raw: u16) -> Option<Self> {
        Self::ALL.get(usize::from(raw)).copied()
    }

    /// The raw value of this kind.
    #[must_use]
    pub fn to_raw(self) -> u16 {
        self as u16
    }

    /// Whether the kind is trivia (whitespace, comments, ignored text).
    #[must_use]
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::WHITESPACE
                | Self::LINE_COMMENT
                | Self::BLOCK_COMMENT
                | Self::PRELUDE
                | Self::TRAILER
        )
    }

    /// Whether the kind is a keyword token.
    #[must_use]
    pub fn is_keyword(self) -> bool {
        (Self::ACTION_KW..=Self::WITH_KW).contains(&self)
    }

    /// Whether the kind is an operator token.
    #[must_use]
    pub fn is_operator(self) -> bool {
        (Self::PREFIX_OP..=Self::MINUS_OP).contains(&self)
    }

    /// Whether the kind is a node kind (as opposed to a token kind).
    #[must_use]
    pub fn is_node(self) -> bool {
        self >= Self::SOURCE_FILE
    }
}

/// The rowan language tag for TLA+.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TlaLanguage {}

impl rowan::Language for TlaLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        // Only kinds produced by this crate are ever stored in trees, so the
        // fallback is unreachable in practice; ERROR is the safe choice.
        SyntaxKind::from_raw(raw.0).unwrap_or(SyntaxKind::ERROR)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind.to_raw())
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        rowan::SyntaxKind(kind.to_raw())
    }
}

/// A node in the red (cursor) syntax tree.
pub type SyntaxNode = rowan::SyntaxNode<TlaLanguage>;
/// A token in the red (cursor) syntax tree.
pub type SyntaxToken = rowan::SyntaxToken<TlaLanguage>;
/// A node or a token.
pub type SyntaxElement = rowan::SyntaxElement<TlaLanguage>;

#[cfg(test)]
mod tests {
    use super::SyntaxKind;

    #[test]
    fn raw_round_trip() {
        for (i, k) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(usize::from(k.to_raw()), i);
            assert_eq!(SyntaxKind::from_raw(k.to_raw()), Some(*k));
        }
        assert_eq!(SyntaxKind::from_raw(u16::MAX), None);
    }

    #[test]
    fn classes() {
        assert!(SyntaxKind::WHITESPACE.is_trivia());
        assert!(SyntaxKind::THEOREM_KW.is_keyword());
        assert!(SyntaxKind::AND_OP.is_operator());
        assert!(SyntaxKind::MODULE.is_node());
        assert!(!SyntaxKind::EOF.is_node());
    }
}
