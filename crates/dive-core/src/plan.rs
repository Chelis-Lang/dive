//! The compiled plan (`04-plan-compiler.md` §1, §6; `05-successor-generation.md`
//! §8.1): compact node arrays indexed by `u32`, not boxed trees.
//!
//! Value nodes ([`Node`]) evaluate to a handle. Generator nodes ([`GNode`])
//! enumerate successors with TLC's `getNextStates` rules. Every local is a
//! slot of one flat frame per role (or per recursive operator activation);
//! operator applications are inlined, with their arguments substituted,
//! cached in thunk slots, or re-evaluated by name.

use crate::handle::Handle;

/// A value node id.
pub type P = u32;
/// A generator node id.
pub type G = u32;

/// Integer arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arith {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `\div`
    Div,
    /// `\div` from `Naturals` without `Integers` (`Naturals.Divide`; floored
    /// in big mode, as `Div`).
    DivNaturals,
    /// `%`
    Mod,
    /// `^`
    Pow,
}

/// Integer comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// Binary set operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    /// `\cup`
    Cup,
    /// `\cap`
    Cap,
    /// `\`
    Minus,
}

/// Quantifiers in value position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quant {
    /// `\E`
    Exists,
    /// `\A`
    Forall,
    /// `CHOOSE`
    Choose,
}

/// Standard-module operators the core implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdFn {
    /// `Len(s)`
    Len,
    /// `Head(s)`
    Head,
    /// `Tail(s)`
    Tail,
    /// `Append(s, e)`
    Append,
    /// `s \o t`
    Concat,
    /// `SubSeq(s, m, n)`
    SubSeq,
    /// `Cardinality(S)`
    Cardinality,
    /// `Seq(S)`
    Seq,
    /// `IsFiniteSet(S)`
    IsFiniteSet,
    /// `EmptyBag` (Bags)
    EmptyBag,
    /// `SetToBag(S)` (Bags)
    SetToBag,
    /// `BagToSet(B)` (Bags)
    BagToSet,
    /// `BagIn(e, B)` (Bags)
    BagIn,
    /// `CopiesIn(e, B)` (Bags)
    CopiesIn,
}

/// One bound group: `x, y \in S` or `<<x, y>> \in S`.
#[derive(Clone, Debug)]
pub struct Bind {
    /// The bound variables' frame slots.
    pub slots: Box<[u32]>,
    /// A tuple pattern.
    pub tuple: bool,
    /// The domain.
    pub domain: P,
}

/// One `EXCEPT` clause.
#[derive(Clone, Debug)]
pub struct Update {
    /// The path.
    pub path: Box<[P]>,
    /// The slot `@` is bound in.
    pub at: u32,
    /// The new value.
    pub value: P,
}

/// An argument of a higher-order library operator ([`Node::LibHo`]).
#[derive(Clone, Debug)]
pub enum LibArg {
    /// A value, evaluated (normalized) before the call.
    Value(P),
    /// An operator: its parameters' slots and its body, which the library
    /// evaluates once per application.
    Op {
        /// The parameters' slots.
        params: Box<[u32]>,
        /// The body.
        body: P,
    },
}

/// A value node.
#[derive(Clone, Debug)]
pub enum Node {
    /// A constant (persistent or scalar).
    Const(Handle),
    /// A frame slot holding a value.
    Slot(u32),
    /// A thunk: evaluated on first use and cached in its slot until the
    /// enclosing scope is entered again.
    Lazy {
        /// The cache slot.
        slot: u32,
        /// The argument.
        body: P,
    },
    /// An unprimed state variable.
    Var(u16),
    /// A primed state variable (the successor buffer).
    Primed(u16),
    /// `~e`
    Not(P),
    /// Conjunction, short-circuit.
    And(Box<[P]>),
    /// Disjunction, short-circuit.
    Or(Box<[P]>),
    /// `a => b`
    Implies(P, P),
    /// `a <=> b`
    Equiv(P, P),
    /// Pure bounded `A \\cdot B`, tested against the complete successor.
    Composition {
        /// Enumerable first-stage action.
        first: G,
        /// Deterministic action assigning every final variable.
        then: G,
        /// Captured lexical slots.
        free: Box<[u32]>,
    },
    /// `IF c THEN a ELSE b`
    If(P, P, P),
    /// `CASE`
    Case(Box<[(P, P)]>, Option<P>),
    /// `a = b`
    Eq(P, P),
    /// `a /= b`
    Neq(P, P),
    /// `a \in S`
    In(P, P),
    /// `a \notin S`
    NotIn(P, P),
    /// `A \subseteq B`
    Subseteq(P, P),
    /// Integer arithmetic.
    Arith(Arith, P, P),
    /// Integer comparison.
    Cmp(Cmp, P, P),
    /// `-a`
    Neg(P),
    /// `a..b`
    Range(P, P),
    /// `{a, b, ...}`; finite sets use canonical order by default.
    SetEnum(Box<[P]>),
    /// `A \cup B` and friends.
    SetBin(SetOp, P, P),
    /// `SUBSET S`
    Powerset(P),
    /// `UNION S`
    BigUnion(P),
    /// `S1 \X ... \X Sn`
    Product(Box<[P]>),
    /// `[D -> R]`
    FcnSet(P, P),
    /// `[f : S, ...]`, field names sorted.
    RecSet(Box<[(Handle, P)]>),
    /// `{x \in S : P}`
    Filter {
        /// The bound.
        bind: Bind,
        /// The predicate.
        pred: P,
        /// A proven rewrite of `{E \in SUBSET T : \A e \in E : Q}` as
        /// `SUBSET F`: the node of `F = {e \in T : Q}`. The compiler sets it
        /// only when `Q` is pure, makes no call and does not read `E`, so
        /// the filter's elements are exactly the subsets of `F`. Only
        /// enumeration uses it; membership keeps the filter's own order of
        /// evaluation, and an evaluation error in `F` falls back to the
        /// filter itself.
        subset: Option<P>,
    },
    /// `{e : x \in S, ...}`
    Map {
        /// The bounds.
        binds: Box<[Bind]>,
        /// The element.
        body: P,
    },
    /// `[x \in S |-> e]`
    FcnCons {
        /// The bounds.
        binds: Box<[Bind]>,
        /// The body.
        body: P,
    },
    /// `\E`, `\A`, `CHOOSE`.
    Quant {
        /// Which.
        kind: Quant,
        /// The bounds.
        binds: Box<[Bind]>,
        /// The body.
        body: P,
    },
    /// `f[a]`
    Apply(P, P),
    /// `r.f`
    Select(P, Handle),
    /// `DOMAIN f`
    Domain(P),
    /// `[f EXCEPT ...]`
    Except {
        /// The function.
        base: P,
        /// The clauses.
        updates: Box<[Update]>,
    },
    /// `<<a, b>>`
    Tuple(Box<[P]>),
    /// `[a |-> e, ...]`, field names sorted.
    Record(Box<[(Handle, P)]>),
    /// Resets thunk slots, then evaluates the body (an inlined operator or
    /// a `LET`).
    Scope {
        /// Thunk slots to reset.
        reset: Box<[u32]>,
        /// The body.
        body: P,
    },
    /// A call of a recursive operator, in a new frame.
    Call {
        /// The function.
        fun: u32,
        /// The arguments, evaluated eagerly.
        args: Box<[P]>,
    },
    /// A standard operator.
    Std(StdFn, Box<[P]>),
    /// A standard or CommunityModules operator implemented in
    /// [`crate::stdlib`], on normalized arguments.
    Lib {
        /// The library operator.
        op: dive_ir::StdOp,
        /// Its arguments, evaluated in source order.
        args: Box<[P]>,
        /// Whether reevaluation is proven effect-free without calls or free slots.
        repeatable_args: bool,
    },
    /// A [`crate::stdlib`] operator with operator arguments.
    LibHo(dive_ir::StdOp, Box<[LibArg]>),
    /// `ENABLED A`: whether the action `A` has a successor from the current
    /// state, complete or not (TLC's `Tool.enabled`). `free` holds the
    /// frame slots `A` reads from outside it.
    Enabled {
        /// The action.
        g: G,
        /// The slots read from outside the action.
        free: Box<[u32]>,
    },
    /// An expression the core evaluates eagerly where TLC evaluates it
    /// lazily (`04-plan-compiler.md` §4.1): an evaluation error inside it
    /// is reported as `unsupported`, because TLC might never raise it.
    Spec(P),
    /// `a \in S` (or `a \notin S` with `neg`) with `S` a set TLC keeps
    /// lazy: a filter or a binary set operation, possibly behind thunks.
    /// Membership is decided on the structure, as TLC's `SetPredValue`,
    /// `SetCupValue`, `SetCapValue` and `SetDiffValue` do, without building
    /// the set.
    InLazy {
        /// The element.
        elem: P,
        /// The set's node.
        set: P,
        /// `\notin`.
        neg: bool,
    },
    /// `f[arg]` with `f` a function constructor TLC keeps lazy
    /// (`FcnLambdaValue.apply`): the body is evaluated for `arg` alone, after
    /// a domain check. `chain` leads to the constructor `fc` through thunks;
    /// a thunk already forced is applied as a value.
    Beta {
        /// The function expression.
        chain: P,
        /// The `FcnCons` node, with one bound variable.
        fc: P,
        /// The argument.
        arg: P,
    },
    /// The body of a recursive function `f[x \in S] == e`: checks that the
    /// argument in `slot` is in `domain`, then evaluates `body`.
    DomCheck {
        /// The argument's slot.
        slot: u32,
        /// The domain.
        domain: P,
        /// The body.
        body: P,
    },
    /// A memoized pure body (S6).
    Memo {
        /// The site.
        site: u32,
        /// The body.
        body: P,
    },
}

/// A generator node (`05-successor-generation.md` §4, §8.1).
#[derive(Clone, Debug)]
pub enum GNode {
    /// Pure bounded `A \\cdot B`, with complete intermediate rows and a
    /// deterministic tail assigning every final variable.
    Composition {
        /// Enumerable first-stage action.
        first: G,
        /// Fully assigning deterministic tail.
        then: G,
    },
    /// A conjunction: items processed left to right as continuations; each
    /// item has a boolean form for the all-assigned shortcut.
    Conj(Box<[(G, P)]>),
    /// A disjunction.
    Disj(Box<[G]>),
    /// Bounded `\E`.
    Exists {
        /// The bounds.
        binds: Box<[Bind]>,
        /// The body.
        body: G,
        /// The body's boolean form.
        body_p: P,
    },
    /// Bounded `\A`: the first binding, then the others last first.
    Forall {
        /// The bounds.
        binds: Box<[Bind]>,
        /// The body.
        body: G,
        /// The body's boolean form.
        body_p: P,
    },
    /// `IF`
    If {
        /// The guard.
        cond: P,
        /// `THEN`
        then: G,
        /// `ELSE`
        other: G,
    },
    /// `CASE`
    Case {
        /// The arms.
        arms: Box<[(P, G)]>,
        /// `OTHER`
        other: Option<G>,
    },
    /// `P => Q`
    Implies {
        /// `P`
        lhs: P,
        /// `Q`
        rhs: G,
    },
    /// `x' = e`
    Assign {
        /// The variable.
        var: u16,
        /// The right side.
        rhs: P,
    },
    /// `x' \in S`
    AssignIn {
        /// The variable.
        var: u16,
        /// The set.
        set: P,
    },
    /// `x' \in S` where TLC's `elements()` of `S` may list an element more
    /// than once (a lazy union, `05-successor-generation.md` §6): `raw`
    /// reproduces TLC's multiplicity, and the run refuses when it differs
    /// from the normalized set's.
    AssignInRaw {
        /// The variable.
        var: u16,
        /// The set.
        set: P,
        /// TLC's `elements()` structure.
        raw: Raw,
    },
    /// `UNCHANGED e`, decomposed by TLC's `processUnchanged` into items in
    /// order.
    Unchanged(Box<[Unch]>),
    /// `[A]_v`: `A`, then `UNCHANGED v`.
    StepBox {
        /// `A`
        action: G,
        /// `v`, decomposed.
        vars: Box<[Unch]>,
    },
    /// `<<A>>_v`: `A`, then a `CHANGED v` item.
    StepAngle {
        /// `A`
        action: G,
        /// `v = v'`, evaluated with `v` in `s0` and `v'` in `s1`.
        same: P,
    },
    /// `f[a]` used as an action, where `f` is a function constructor TLC
    /// keeps lazy (`FcnLambdaValue`, `Tool.getFcnContext`): the body is
    /// enumerated with the parameters bound to the argument.
    FcnAction {
        /// The parameters, one group per bound.
        binds: Box<[Bind]>,
        /// The argument.
        arg: P,
        /// The body.
        body: G,
    },
    /// Thunk slots reset around a body (inlined operator or `LET`).
    Scope {
        /// The thunk slots.
        reset: Box<[u32]>,
        /// The body.
        body: G,
    },
    /// Any other formula: a boolean guard.
    Guard(P),
}

/// One item of a decomposed `UNCHANGED` (TLC's `processUnchanged`).
#[derive(Clone, Copy, Debug)]
pub enum Unch {
    /// A state variable: bound to its old value, or compared with it. `at`
    /// is the expression TLC's warning 2143 names.
    Var {
        /// The variable.
        var: u16,
        /// Where it is written: the `ExprId` index.
        at: u32,
    },
    /// Any other expression: `e = e'`, with `e` in `s0` and `e'` in `s1`
    /// (`verifyUnchanged`).
    Verify(P),
}

/// TLC's `Enumerable.elements()` multiplicity of a set bound by `x' \in S`
/// (`05-successor-generation.md` §6).
#[derive(Clone, Debug)]
pub enum Raw {
    /// A set whose `elements()` lists every element once.
    Leaf(P),
    /// `SetCupValue`: the first set's elements, then the second's.
    Cup(Box<Raw>, Box<Raw>),
    /// `UnionValue` of an enumerated set of sets: each distinct member's
    /// elements in turn.
    Union(Box<[Raw]>),
    /// `UnionValue` of another set of sets: each member's elements in turn.
    UnionOf(P),
    /// `SetPredValue`: the bound set's elements, filtered.
    Filter {
        /// The bound variable (its domain is `inner`).
        bind: Bind,
        /// The predicate.
        pred: P,
        /// The bound set.
        inner: Box<Raw>,
    },
    /// `SetCapValue`: the enumerable operand's elements, filtered by
    /// membership in the other.
    Cap(Box<Raw>, Box<Raw>, P, P),
    /// `SetDiffValue`: the first operand's elements not in the second.
    Minus(Box<Raw>, P),
    /// An odometer over components (`[D -> R]`, records, products): an
    /// element repeats when a component's does.
    Odometer(Box<[Raw]>, P),
}

/// A recursive operator, compiled once with its own frame.
#[derive(Clone, Debug)]
pub struct Fun {
    /// The name.
    pub name: String,
    /// Number of parameters (slots `0..params`).
    pub params: u32,
    /// The frame size.
    pub frame: u32,
    /// The body.
    pub body: P,
    /// The slots of the values captured from enclosing definitions (a
    /// recursive `LET` operator's free locals, lambda-lifted): a call
    /// passes them after the parameters.
    pub caps: Box<[u32]>,
}

/// A memoization site (`06-memoization.md` §2.2).
#[derive(Clone, Debug)]
pub struct MemoSite {
    /// A description for statistics.
    pub name: String,
    /// The unprimed variables the body reads.
    pub vars: Box<[u16]>,
    /// The free slots the body reads.
    pub slots: Box<[u32]>,
    /// The primed variables the body reads: in a step predicate, whose
    /// pair is complete (`Compiler::step`), or in an action's primed
    /// definition, whose site is bypassed while one is unassigned
    /// (`Opts::primed_memo`).
    pub primed: Box<[u16]>,
    /// A step predicate's or primed definition's site (`Opts::step_memo`,
    /// `Opts::primed_memo`): on whatever `Opts::memo` says, and its results
    /// are interned to be kept.
    pub step: bool,
    /// A primed definition's site (`Opts::primed_memo`), evaluated once per
    /// enumeration candidate: its primed keys are candidate values, so the
    /// bytes its persistent keys and results add to the store are bounded
    /// (`Memo::charge_kept`).
    pub candidate: bool,
}

impl MemoSite {
    /// The length of the site's key, in handles.
    #[must_use]
    pub fn key_len(&self) -> usize {
        self.vars.len() + self.primed.len() + self.slots.len()
    }
}

/// The node arrays.
#[derive(Clone, Debug, Default)]
pub struct Code {
    /// Explicit compiled source maps; None allocates no tables.
    pub source_map: Option<crate::debug::SourceMap>,
    /// Producer observation capability, installed only in search machines.
    pub debugger: Option<std::sync::Arc<dyn crate::debug::Observer>>,
    /// Value nodes.
    pub nodes: Vec<Node>,
    /// Generator nodes.
    pub gnodes: Vec<GNode>,
    /// Recursive operators.
    pub funs: Vec<Fun>,
    /// Memo sites.
    pub sites: Vec<MemoSite>,
}

/// An initial predicate or next-state action.
#[derive(Clone, Debug)]
pub struct Action {
    /// TLC's name of the action.
    pub name: String,
    /// The generator.
    pub g: G,
    /// The frame size.
    pub frame: u32,
    /// The frame slots preset by splitting (`\E` and parameter values).
    pub preset: Vec<(u32, Handle)>,
}

/// One conjunct of an invariant or constraint.
#[derive(Clone, Debug)]
pub struct Conjunct {
    /// The boolean body.
    pub p: P,
    /// The unprimed variables it reads, as a mask.
    pub reads: u64,
    /// Whether the parent-delta skip applies (pure, no free slots, read set
    /// known).
    pub skippable: bool,
    /// The memo site, if any.
    pub site: Option<u32>,
}

/// An invariant or state constraint.
#[derive(Clone, Debug)]
pub struct Check {
    /// The configured name.
    pub name: String,
    /// The conjuncts, in order.
    pub conjuncts: Vec<Conjunct>,
    /// The frame size.
    pub frame: u32,
}

/// The compiled specification.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The node arrays.
    pub code: Code,
    /// State variable names, by index.
    pub vars: Vec<String>,
    /// The initial predicates.
    pub init: Vec<Action>,
    /// The next-state actions, in TLC's order.
    pub next: Vec<Action>,
    /// `INVARIANT`s.
    pub invariants: Vec<Check>,
    /// `CONSTRAINT`s.
    pub constraints: Vec<Check>,
    /// `ACTION_CONSTRAINT`s.
    pub action_constraints: Vec<Check>,
    /// Implied initial predicates (`PROPERTY` parts checked on the initial
    /// states).
    pub implied_inits: Vec<Check>,
    /// Implied actions (`[A]_v` of `[][A]_v` in `PROPERTY`s), step
    /// predicates.
    pub implied_actions: Vec<Check>,
    /// Whether deadlock is checked.
    pub check_deadlock: bool,
    /// `SYMMETRY`: states are keyed by their canonical image.
    pub symmetry: Option<crate::symmetry::Symmetry>,
    /// The plan's effects: the engine then runs one worker, in TLC's
    /// order.
    pub effects: Effects,
    /// `VIEW`: its value, the one conjunct, keys the state.
    pub view: Option<Check>,
    /// Compile statistics.
    pub stats: CompileStats,
    /// The frame slots a bound optional atom's role binds
    /// ([`crate::compile::compile_bound_test`]: `\E` variables and inlined
    /// parameters of its scope), with their values, which the caller
    /// installs after entering the check's frame; empty otherwise.
    pub bound_preset: Vec<(u32, Handle)>,
}

/// What effects a plan has (`Info::effect`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts about the plan"
)]
pub struct Effects {
    /// Any effect (`Print`, `PrintT`, `RandomElement`, `TLCSet`, `TLCGet`).
    pub any: bool,
    /// `Print` or `PrintT`.
    pub prints: bool,
    /// `RandomElement`: the engine starts each expansion's random stream
    /// (`crate::random`).
    pub draws: bool,
    /// `TLCSet` or `TLCGet` of a register (not of `"level"`).
    pub registers: bool,
    /// `TLCGet("level")`: the engine sets `Machine::level`.
    pub level: bool,
}

/// What the compiler did.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileStats {
    /// Value nodes.
    pub nodes: usize,
    /// Generator nodes.
    pub gnodes: usize,
    /// Operator applications inlined.
    pub inlined: usize,
    /// Arguments substituted directly.
    pub aliased: usize,
    /// Arguments bound to thunks.
    pub thunks: usize,
    /// Arguments re-evaluated by name.
    pub by_name: usize,
    /// Subexpressions folded to constants.
    pub folded: usize,
    /// State-level definitions cached once per frame activation.
    pub cached_defs: usize,
    /// Folds abandoned because they ran out of fuel (R9).
    pub fold_abandoned: usize,
    /// Folds not attempted because the compile-time budget was spent.
    pub fold_skipped: usize,
    /// Loop iterations spent folding.
    pub fold_fuel: u64,
    /// Wall time spent folding, in nanoseconds.
    pub fold_nanos: u64,
    /// Filters, function constructors and set operations the core builds
    /// eagerly where TLC is lazy (`Node::Spec`).
    pub speculative: u64,
    /// `INSTANCE` substitutions bound.
    pub substituted: usize,
    /// Applications of lazy function constructors evaluated for their
    /// argument alone (`Node::Beta`).
    pub beta: u64,
    /// Constant definitions the binder left to the core, evaluated here
    /// ([`crate::compile::Handoff::deferred`]).
    pub startup_values: usize,
    /// Wall time spent evaluating them, in nanoseconds.
    pub startup_nanos: u64,
    /// Calls their evaluation answered from its call results.
    pub startup_call_hits: u64,
    /// Those the core did not evaluate whose value the binder's evaluation
    /// supplied ([`crate::compile::Handoff::startup_evaluator`]).
    pub startup_fallbacks: usize,
}
