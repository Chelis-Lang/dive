//! Expression nodes (ADR 0013 §2).
//!
//! Every construct SANY gives TLC has its own variant, named for what it
//! means rather than by SANY's `$`-names. Source order is kept everywhere:
//! junction items, `CASE` arms, quantifier bound groups, record fields,
//! `EXCEPT` clauses and arguments, because TLC's evaluation and successor
//! enumeration follow it.

use crate::ids::{ConstId, DeclId, DefId, ExprId, Slot, VarId};
use crate::std_op::StdOp;
use serde::{Deserialize, Serialize};

/// A reference to a slot of an enclosing frame.
///
/// `up` counts frames outwards: 0 is the frame of the definition being
/// evaluated, 1 the frame it was created in (for a `LET` operator or
/// `LAMBDA`), and so on. See [`crate::Def::depth`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalRef {
    /// How many frames to go out.
    pub up: u32,
    /// The slot in that frame.
    pub slot: Slot,
}

/// An operator used as a callee or passed as an argument.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpRef {
    /// A user definition: top level, instantiated (`I!Op`), `LET`-local with
    /// parameters, or `LAMBDA`.
    Def(DefId),
    /// An operator parameter (`F` in `Op(F(_)) == F(1)`), bound in a frame.
    Param(LocalRef),
    /// An operator TLC implements in Java.
    Std(StdOp),
    /// A built-in operator passed as an argument (`Op(\cup)`), or applied
    /// through a constant substituted by one.
    Builtin(BuiltinOp),
    /// A constant operator (`CONSTANT F(_)`) that no instance substitutes;
    /// the configuration binds it.
    Const(ConstId),
    /// A declaration that some instance substitutes: looked up in the dynamic
    /// substitution context first, as TLC does (see [`Expr::SubstRef`]).
    Subst(DeclId),
    /// A name SANY resolved but that TLC's context cannot reach at this
    /// point. Evaluating it raises TLC's `TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR`.
    Unbound(Box<str>),
}

/// How an argument, `LET` definition or substitution is passed: a laziness
/// point (ADR 0013 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pass {
    /// TLC creates a `LazyValue` here, and so must the evaluator: a thunk,
    /// forced at most once under TLC's caching rule (ADR 0014 §2).
    Lazy,
    /// TLC creates a `LazyValue`, but the effects analysis proves the
    /// expression pure and unable to fail (a literal, a bound variable, or a
    /// tuple or record of these), so evaluating it eagerly is
    /// indistinguishable.
    Strict,
    /// Not a laziness point: an argument of a built-in or a Java override,
    /// which TLC evaluates as the callee requires ([`crate::StdKind`]).
    Direct,
}

/// An argument of an operator application.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Arg {
    /// An expression argument.
    Expr {
        /// The argument.
        expr: ExprId,
        /// How it is passed.
        pass: Pass,
    },
    /// An operator argument.
    Op {
        /// The operator.
        op: OpRef,
        /// SANY's level of the argument (the operator's level), which TLC
        /// reads when it decides whether to inline an application while
        /// splitting the next-state relation (`Tool.getActionsAppl`).
        level: crate::Level,
    },
}

/// SANY's built-in operators that are not language constructs of their own,
/// and every built-in that can be passed as an operator argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "each variant is named after the operator")]
pub enum BuiltinOp {
    /// `=`
    Eq,
    /// `/=`
    Neq,
    /// `\in`
    In,
    /// `\notin`
    NotIn,
    /// `\subseteq`
    Subseteq,
    /// `\union`, `\cup`
    Cup,
    /// `\intersect`, `\cap`
    Cap,
    /// `\` (set difference)
    SetMinus,
    /// `SUBSET`
    PowerSet,
    /// `UNION`
    BigUnion,
    /// `DOMAIN`
    Domain,
    /// `\times` as an operator (`\X` in expressions is [`Expr::Cartesian`])
    Times,
    /// `BOOLEAN`
    Boolean,
    /// `STRING`
    String,
    /// `\lnot`
    Not,
    /// `\land`
    And,
    /// `\lor`
    Or,
    /// `=>`
    Implies,
    /// `\equiv`
    Equiv,
    /// `'`
    Prime,
    /// `ENABLED`
    Enabled,
    /// `UNCHANGED`
    Unchanged,
    /// `\cdot`
    Cdot,
    /// `[]`
    Always,
    /// `<>`
    Eventually,
    /// `~>`
    LeadsTo,
    /// `-+->`
    WhilePlus,
}

impl BuiltinOp {
    /// SANY's name for it.
    #[must_use]
    pub fn sany_name(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Neq => "/=",
            Self::In => "\\in",
            Self::NotIn => "\\notin",
            Self::Subseteq => "\\subseteq",
            Self::Cup => "\\union",
            Self::Cap => "\\intersect",
            Self::SetMinus => "\\",
            Self::PowerSet => "SUBSET",
            Self::BigUnion => "UNION",
            Self::Domain => "DOMAIN",
            Self::Times => "\\times",
            Self::Boolean => "BOOLEAN",
            Self::String => "STRING",
            Self::Not => "\\lnot",
            Self::And => "\\land",
            Self::Or => "\\lor",
            Self::Implies => "=>",
            Self::Equiv => "\\equiv",
            Self::Prime => "'",
            Self::Enabled => "ENABLED",
            Self::Unchanged => "UNCHANGED",
            Self::Cdot => "\\cdot",
            Self::Always => "[]",
            Self::Eventually => "<>",
            Self::LeadsTo => "~>",
            Self::WhilePlus => "-+->",
        }
    }

    /// The operator for SANY's name, if it is one of these.
    #[must_use]
    pub fn from_sany_name(name: &str) -> Option<Self> {
        Some(match name {
            "=" => Self::Eq,
            "/=" => Self::Neq,
            "\\in" => Self::In,
            "\\notin" => Self::NotIn,
            "\\subseteq" => Self::Subseteq,
            "\\union" => Self::Cup,
            "\\intersect" => Self::Cap,
            "\\" => Self::SetMinus,
            "SUBSET" => Self::PowerSet,
            "UNION" => Self::BigUnion,
            "DOMAIN" => Self::Domain,
            "\\times" => Self::Times,
            "BOOLEAN" => Self::Boolean,
            "STRING" => Self::String,
            "\\lnot" => Self::Not,
            "\\land" => Self::And,
            "\\lor" => Self::Or,
            "=>" => Self::Implies,
            "\\equiv" => Self::Equiv,
            "'" => Self::Prime,
            "ENABLED" => Self::Enabled,
            "UNCHANGED" => Self::Unchanged,
            "\\cdot" => Self::Cdot,
            "[]" => Self::Always,
            "<>" => Self::Eventually,
            "~>" => Self::LeadsTo,
            "-+->" => Self::WhilePlus,
            _ => return None,
        })
    }
}

/// `/\` or `\/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JunctionKind {
    /// Conjunction.
    And,
    /// Disjunction.
    Or,
}

/// `\E`, `\A` or `CHOOSE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuantKind {
    /// `\E`.
    Exists,
    /// `\A`.
    Forall,
    /// `CHOOSE`.
    Choose,
}

/// One bound group: `x, y \in S`, `<<x, y>> \in S`, or the unbounded `x` of
/// `\E x : P`. SANY's grouping is kept: TLC evaluates a group's domain once.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bound {
    /// The bound variables' slots, in order.
    pub vars: Box<[Slot]>,
    /// Whether the group is a tuple pattern `<<x, y>> \in S`.
    pub tuple: bool,
    /// The domain; `None` for an unbounded quantifier or `CHOOSE`, which TLC
    /// cannot evaluate and reports as an error.
    pub domain: Option<ExprId>,
}

/// A record field `a |-> e` or `a : S`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Field {
    /// The field name.
    pub name: Box<str>,
    /// Its value or set.
    pub value: ExprId,
}

/// One `EXCEPT` clause `![a][b].c = e`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExceptUpdate {
    /// The path. `.c` is the string `"c"`, as in TLC; `[a, b]` is the tuple
    /// `<<a, b>>`.
    pub path: Box<[ExprId]>,
    /// The new value; `@` inside it is [`Expr::ExceptAt`].
    pub value: ExprId,
}

/// One substitution `c <- e` of an instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubstBinding {
    /// The substituted declaration of the instantiated module.
    pub decl: DeclId,
    /// What replaces it, evaluated in the context outside the substitution.
    pub value: Arg,
}

/// A `CASE` arm `p -> e`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CaseArm {
    /// The guard.
    pub guard: ExprId,
    /// The value.
    pub value: ExprId,
}

/// What an [`Expr::Unevaluable`] node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unevaluable {
    /// An `ASSUME ... PROVE ...` used as the body of a theorem name.
    AssumeProve,
    /// A proof-language construct (`QED`, `CASE` steps, `TAKE`, `PICK`, ...).
    Proof,
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Expr {
    /// `TRUE` or `FALSE`.
    Bool(bool),
    /// A natural-number literal, as written. Whether it is too big is decided
    /// when the model is bound (`TLC_INTEGER_TOO_BIG` under `--int=tlc`).
    Num(Box<str>),
    /// A decimal literal `int.frac`. TLC refuses real numbers when it
    /// processes the specification (`TLC_CANT_HANDLE_REAL_NUMBERS`).
    Decimal {
        /// The digits before the point.
        int: Box<str>,
        /// The digits after it.
        frac: Box<str>,
    },
    /// A string literal, unescaped as SANY does.
    Str(Box<str>),

    /// A state variable of the root module, never substituted by an instance.
    Var {
        /// The variable.
        var: VarId,
        /// Primed: read in the successor state.
        primed: bool,
    },
    /// A constant that no instance substitutes.
    Const(ConstId),
    /// A slot of an enclosing frame: a parameter, a bound variable, a
    /// zero-arity `LET` definition (a thunk), or a recursive function's name.
    Local(LocalRef),
    /// A declaration (constant or variable, of any module) that some
    /// instance substitutes.
    ///
    /// TLC binds substitutions in its *dynamic* context (`Tool.java:1864-1873`,
    /// `SymbolNodeValueLookupProvider.lookup`): the body of `I!Op` calls the
    /// instantiated module's other definitions, whose references to the
    /// module's declarations resolve to the innermost active substitution.
    /// So the evaluator looks `decl` up in the substitution context and, if no
    /// substitution binds it, falls back to the declaration itself: the root
    /// variable or the configured constant ([`crate::Decl::var`],
    /// [`crate::Decl::constant`]). Under `ENABLED`, TLC cuts that lookup off
    /// for variables (`Context.branch`).
    SubstRef {
        /// The declaration.
        decl: DeclId,
        /// Primed: `e'` for the substituted `e`.
        primed: bool,
    },
    /// A use of a zero-arity definition.
    Def(DefId),
    /// An operator application.
    Apply {
        /// The operator.
        callee: OpRef,
        /// The arguments, in order.
        args: Box<[Arg]>,
    },
    /// A built-in set or equality operator ([`BuiltinOp::Eq`] through
    /// [`BuiltinOp::String`]).
    Builtin {
        /// The operator.
        op: BuiltinOp,
        /// Its operands.
        args: Box<[ExprId]>,
    },

    /// An n-ary junction: a bulleted list, or a binary `/\` or `\/`.
    /// Nested binary junctions are not flattened.
    Junction {
        /// `/\` or `\/`.
        kind: JunctionKind,
        /// A bulleted list (`$ConjList`/`$DisjList`) rather than `\land`/`\lor`.
        bulleted: bool,
        /// The items, in source order.
        items: Box<[ExprId]>,
    },
    /// `~e`.
    Not(ExprId),
    /// `a => b`.
    Implies(ExprId, ExprId),
    /// `a <=> b`.
    Equiv(ExprId, ExprId),
    /// `IF c THEN a ELSE b`.
    If {
        /// The condition.
        cond: ExprId,
        /// The `THEN` branch.
        then: ExprId,
        /// The `ELSE` branch.
        other: ExprId,
    },
    /// `CASE`.
    Case {
        /// The arms, in source order.
        arms: Box<[CaseArm]>,
        /// The `OTHER` arm.
        other: Option<ExprId>,
    },

    /// `\E`, `\A` or `CHOOSE`, bounded or not.
    Quant {
        /// Which quantifier.
        kind: QuantKind,
        /// The bound groups.
        bounds: Box<[Bound]>,
        /// The body.
        body: ExprId,
    },
    /// `{x \in S : P}`.
    SetFilter {
        /// The bound.
        bound: Bound,
        /// The predicate.
        pred: ExprId,
    },
    /// `{e : x \in S, ...}`.
    SetMap {
        /// The element expression.
        expr: ExprId,
        /// The bound groups.
        bounds: Box<[Bound]>,
    },
    /// `[x \in S |-> e]`, and the body of a non-recursive function definition
    /// `f[x \in S] == e`.
    FcnCons {
        /// The bound groups.
        bounds: Box<[Bound]>,
        /// The body.
        body: ExprId,
    },
    /// The body of a recursive function definition `f[x \in S] == e`.
    /// Inside `body`, `f` is the slot `name`, bound to the function itself.
    RecFcn {
        /// The slot holding the function.
        name: Slot,
        /// The bound groups.
        bounds: Box<[Bound]>,
        /// The body.
        body: ExprId,
    },

    /// `{a, b, c}`.
    SetEnum(Box<[ExprId]>),
    /// `<<a, b, c>>`.
    Tuple(Box<[ExprId]>),
    /// `[a |-> e, ...]`.
    Record(Box<[Field]>),
    /// `[a : S, ...]`.
    RecordSet(Box<[Field]>),
    /// `[S -> T]`.
    FcnSet {
        /// The domain.
        domain: ExprId,
        /// The range.
        range: ExprId,
    },
    /// `S \X T \X ...`.
    Cartesian(Box<[ExprId]>),
    /// `f[a]`; `f[a, b]` is `f[<<a, b>>]`.
    FcnApply {
        /// The function.
        fcn: ExprId,
        /// The argument.
        arg: ExprId,
    },
    /// `r.a`.
    RecordSelect {
        /// The record.
        record: ExprId,
        /// The field.
        field: Box<str>,
    },
    /// `[f EXCEPT !... = e, ...]`.
    Except {
        /// The function or record.
        base: ExprId,
        /// The clauses, in order.
        updates: Box<[ExceptUpdate]>,
    },
    /// `@`: the old value at the innermost enclosing `EXCEPT` clause's path.
    /// TLC binds it in the dynamic context (`Tool.java`, `EXCEPT_AT`).
    ExceptAt,

    /// `LET defs IN body`. Zero-arity definitions are thunks in slots of the
    /// current frame ([`crate::DefScope::LetValue`]); the others are local
    /// operators ([`crate::DefScope::Local`]).
    Let {
        /// The definitions, in order.
        defs: Box<[DefId]>,
        /// The body.
        body: ExprId,
    },
    /// The body of an instantiated definition under its instance's
    /// substitutions. The bindings are made in the dynamic substitution
    /// context; see [`Expr::SubstRef`].
    Subst {
        /// The substitutions, in SANY's order.
        bindings: Box<[SubstBinding]>,
        /// The body.
        body: ExprId,
    },
    /// A subexpression reached through a selector without parameters
    /// (`Op!1`, SANY's `$Nop`). Evaluation sees through it; TLC's split of the
    /// next-state relation does not (`Tool.getActionsAppl` treats it as a
    /// leaf).
    Selected(ExprId),
    /// `lab :: e`, transparent.
    Label {
        /// The label.
        name: Box<str>,
        /// The labelled expression.
        body: ExprId,
    },

    /// `e'` for an `e` that is not a variable.
    Prime(ExprId),
    /// `UNCHANGED e`.
    Unchanged(ExprId),
    /// `[A]_v`.
    StepBox {
        /// `A`.
        action: ExprId,
        /// `v`.
        sub: ExprId,
    },
    /// `<<A>>_v`.
    StepAngle {
        /// `A`.
        action: ExprId,
        /// `v`.
        sub: ExprId,
    },
    /// `ENABLED e`.
    Enabled(ExprId),
    /// `A \cdot B`.
    Cdot(ExprId, ExprId),

    /// `[]F`.
    Always(ExprId),
    /// `<>F`.
    Eventually(ExprId),
    /// `F ~> G`.
    LeadsTo(ExprId, ExprId),
    /// `F -+-> G`.
    WhilePlus(ExprId, ExprId),
    /// `WF_v(A)`.
    Wf {
        /// `v`.
        sub: ExprId,
        /// `A`.
        action: ExprId,
    },
    /// `SF_v(A)`.
    Sf {
        /// `v`.
        sub: ExprId,
        /// `A`.
        action: ExprId,
    },
    /// `\EE x : F` (`exists`) or `\AA x : F`.
    TemporalQuant {
        /// `\EE` rather than `\AA`.
        exists: bool,
        /// The bound variables' slots.
        vars: Box<[Slot]>,
        /// The body.
        body: ExprId,
    },

    /// A name SANY resolved but that TLC's context cannot reach here: a bound
    /// variable or `LET` definition of another definition, reached through a
    /// subexpression selector. Evaluating it raises TLC's
    /// `TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR`.
    Unbound(Box<str>),
    /// Something TLC cannot evaluate at all ("Attempted to evaluate an
    /// expression that cannot be evaluated").
    Unevaluable(Unevaluable),
}

impl Expr {
    /// The variant's name.
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Bool { .. } => "Bool",
            Self::Num { .. } => "Num",
            Self::Decimal { .. } => "Decimal",
            Self::Str { .. } => "Str",
            Self::Var { .. } => "Var",
            Self::Const { .. } => "Const",
            Self::Local { .. } => "Local",
            Self::SubstRef { .. } => "SubstRef",
            Self::Def { .. } => "Def",
            Self::Apply { .. } => "Apply",
            Self::Builtin { .. } => "Builtin",
            Self::Junction { .. } => "Junction",
            Self::Not { .. } => "Not",
            Self::Implies { .. } => "Implies",
            Self::Equiv { .. } => "Equiv",
            Self::If { .. } => "If",
            Self::Case { .. } => "Case",
            Self::Quant { .. } => "Quant",
            Self::SetFilter { .. } => "SetFilter",
            Self::SetMap { .. } => "SetMap",
            Self::FcnCons { .. } => "FcnCons",
            Self::RecFcn { .. } => "RecFcn",
            Self::SetEnum { .. } => "SetEnum",
            Self::Tuple { .. } => "Tuple",
            Self::Record { .. } => "Record",
            Self::RecordSet { .. } => "RecordSet",
            Self::FcnSet { .. } => "FcnSet",
            Self::Cartesian { .. } => "Cartesian",
            Self::FcnApply { .. } => "FcnApply",
            Self::RecordSelect { .. } => "RecordSelect",
            Self::Except { .. } => "Except",
            Self::ExceptAt { .. } => "ExceptAt",
            Self::Let { .. } => "Let",
            Self::Subst { .. } => "Subst",
            Self::Selected { .. } => "Selected",
            Self::Label { .. } => "Label",
            Self::Prime { .. } => "Prime",
            Self::Unchanged { .. } => "Unchanged",
            Self::StepBox { .. } => "StepBox",
            Self::StepAngle { .. } => "StepAngle",
            Self::Enabled { .. } => "Enabled",
            Self::Cdot { .. } => "Cdot",
            Self::Always { .. } => "Always",
            Self::Eventually { .. } => "Eventually",
            Self::LeadsTo { .. } => "LeadsTo",
            Self::WhilePlus { .. } => "WhilePlus",
            Self::Wf { .. } => "Wf",
            Self::Sf { .. } => "Sf",
            Self::TemporalQuant { .. } => "TemporalQuant",
            Self::Unbound { .. } => "Unbound",
            Self::Unevaluable { .. } => "Unevaluable",
        }
    }

    /// The expressions this node refers to directly, in order. Definitions
    /// referenced by [`Expr::Def`], [`Expr::Let`] and [`OpRef::Def`] are not
    /// included.
    #[must_use]
    pub fn children(&self) -> Vec<ExprId> {
        let mut out = Vec::new();
        let arg = |out: &mut Vec<ExprId>, a: &Arg| {
            if let Arg::Expr { expr, .. } = a {
                out.push(*expr);
            }
        };
        let bounds = |out: &mut Vec<ExprId>, bs: &[Bound]| {
            out.extend(bs.iter().filter_map(|b| b.domain));
        };
        match self {
            Self::Bool(_)
            | Self::Num(_)
            | Self::Decimal { .. }
            | Self::Str(_)
            | Self::Var { .. }
            | Self::Const(_)
            | Self::Local(_)
            | Self::SubstRef { .. }
            | Self::Def(_)
            | Self::ExceptAt
            | Self::Unbound(_)
            | Self::Unevaluable(_) => {}
            Self::Apply { args, .. } => {
                for a in args {
                    arg(&mut out, a);
                }
            }
            Self::Builtin { args: items, .. }
            | Self::Junction { items, .. }
            | Self::SetEnum(items)
            | Self::Tuple(items)
            | Self::Cartesian(items) => out.extend(items.iter().copied()),
            Self::Not(a)
            | Self::Selected(a)
            | Self::Prime(a)
            | Self::Unchanged(a)
            | Self::Enabled(a)
            | Self::Always(a)
            | Self::Eventually(a) => out.push(*a),
            Self::Implies(a, b)
            | Self::Equiv(a, b)
            | Self::Cdot(a, b)
            | Self::LeadsTo(a, b)
            | Self::WhilePlus(a, b) => out.extend([*a, *b]),
            Self::If { cond, then, other } => out.extend([*cond, *then, *other]),
            Self::Case { arms, other } => {
                for a in arms {
                    out.extend([a.guard, a.value]);
                }
                out.extend(*other);
            }
            Self::Quant {
                bounds: bs, body, ..
            }
            | Self::FcnCons { bounds: bs, body }
            | Self::RecFcn {
                bounds: bs, body, ..
            } => {
                bounds(&mut out, bs);
                out.push(*body);
            }
            Self::SetFilter { bound, pred } => {
                bounds(&mut out, std::slice::from_ref(bound));
                out.push(*pred);
            }
            Self::SetMap { expr, bounds: bs } => {
                bounds(&mut out, bs);
                out.push(*expr);
            }
            Self::Record(fs) | Self::RecordSet(fs) => out.extend(fs.iter().map(|f| f.value)),
            Self::FcnSet { domain, range } => out.extend([*domain, *range]),
            Self::FcnApply { fcn, arg } => out.extend([*fcn, *arg]),
            Self::RecordSelect { record, .. } => out.push(*record),
            Self::Except { base, updates } => {
                out.push(*base);
                for u in updates {
                    out.extend(u.path.iter().copied());
                    out.push(u.value);
                }
            }
            Self::Let { body, .. }
            | Self::Label { body, .. }
            | Self::TemporalQuant { body, .. } => {
                out.push(*body);
            }
            Self::Subst { bindings, body } => {
                for b in bindings {
                    arg(&mut out, &b.value);
                }
                out.push(*body);
            }
            Self::StepBox { action, sub } | Self::StepAngle { action, sub } => {
                out.extend([*action, *sub]);
            }
            Self::Wf { sub, action } | Self::Sf { sub, action } => out.extend([*sub, *action]),
        }
        out
    }

    /// The operator references this node makes directly (callees, operator
    /// arguments and operator substitutions).
    #[must_use]
    pub fn op_refs(&self) -> Vec<&OpRef> {
        let mut out = Vec::new();
        match self {
            Self::Apply { callee, args } => {
                out.push(callee);
                for a in args {
                    if let Arg::Op { op: o, .. } = a {
                        out.push(o);
                    }
                }
            }
            Self::Subst { bindings, .. } => {
                for b in bindings {
                    if let Arg::Op { op: o, .. } = &b.value {
                        out.push(o);
                    }
                }
            }
            _ => {}
        }
        out
    }
}
